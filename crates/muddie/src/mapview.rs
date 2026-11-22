//! Draws the map and lets the user browse it, pick a room to walk to, and
//! mark or move rooms.

use eframe::egui;
use eframe::egui::{
    Color32, FontId, Painter, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
    vec2,
};
use muddie_mapper::{Map, Pos, Room, RoomId, Walk, direction_offset};

use crate::i18n::tr;
use crate::theme::{self, Theme, Weight};
use crate::widgets;

mod draw;
mod explore;

use draw::*;
use explore::Prompt;
pub use explore::{found_words, place_words};

/// Distance between neighbouring rooms, in points, at the default zoom.
const DEFAULT_CELL: f32 = 36.0;
const CELL_RANGE: std::ops::RangeInclusive<f32> = 10.0..=96.0;
/// How long, in seconds, the view takes to glide to the room the player
/// moved to.
const GLIDE: f64 = 0.3;
/// The farthest move, in cells, the view glides over; past it, it jumps.
const GLIDE_REACH: i32 = 12;

/// Something the user did on the map that the caller must carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapAction {
    /// Walk to this room.
    Walk(RoomId),
    /// Put this room at this grid position.
    Move(RoomId, Pos),
    /// Keep routes out of this room (true) or allow them again (false).
    Avoid(RoomId, bool),
    /// Draw this area again from scratch.
    Tidy(String),
    /// Run one of the client's own commands, such as `/where`.
    Command(&'static str),
    /// Open the scripts, to set a mapper up.
    Scripts,
    /// This room was chosen to look at, from the one before it if the keys
    /// moved there: say it.
    Selected(RoomId, Option<RoomId>),
    /// Say the way to this room, without going.
    Way(RoomId),
    /// The room chosen has no exit this way, as asked with the keys.
    NoExit(&'static str),
    /// Leave the map for the command line, with what was typed on it.
    Leave(String),
    /// Say this, while exploring the map.
    Say(String),
    /// Say all the map knows of this room.
    Info(RoomId),
    /// Give this room this name.
    Mark(RoomId, String),
    /// Write this note on this room; an empty one takes it away.
    Note(RoomId, String),
}

/// The keys that move the room chosen along its exits while the map has
/// the keyboard: the arrows, the digits as on a keypad, and the pages up
/// and down.
const MOVES: [(egui::Key, &str); 14] = [
    (egui::Key::ArrowUp, "n"),
    (egui::Key::ArrowDown, "s"),
    (egui::Key::ArrowLeft, "w"),
    (egui::Key::ArrowRight, "e"),
    (egui::Key::Num8, "n"),
    (egui::Key::Num2, "s"),
    (egui::Key::Num4, "w"),
    (egui::Key::Num6, "e"),
    (egui::Key::Num7, "nw"),
    (egui::Key::Num9, "ne"),
    (egui::Key::Num1, "sw"),
    (egui::Key::Num3, "se"),
    (egui::Key::PageUp, "u"),
    (egui::Key::PageDown, "d"),
];

/// A room as it is told when chosen: its name, its region if the name
/// does not say it, its number, how many moves away it is from `from`,
/// and its exits.
pub fn selection_line(map: &Map, from: Option<RoomId>, room: &Room) -> String {
    let name = map.bare_name(&room.name);
    let mut parts = vec![name.to_owned()];
    if let Some(region) = map.region(room).filter(|region| !name.starts_with(region)) {
        parts.push(region.to_owned());
    }
    parts.push(format!("#{}", room.id));
    parts.push(how_far(map, from, room));
    parts.push(exits_words(map, room));
    if !room.note.is_empty() {
        parts.push(room.note.clone());
    }
    parts.join(" · ")
}

/// A direction in words: the MUD's, if the map was taught them, else as
/// people say it.
pub fn direction_words(map: &Map, direction: &str) -> String {
    let taught = map.command_for(direction);
    if taught != direction {
        return taught.to_owned();
    }
    match direction {
        "n" => tr("north"),
        "ne" => tr("northeast"),
        "e" => tr("east"),
        "se" => tr("southeast"),
        "s" => tr("south"),
        "sw" => tr("southwest"),
        "w" => tr("west"),
        "nw" => tr("northwest"),
        "u" => tr("up"),
        "d" => tr("down"),
        "in" => tr("in"),
        "out" => tr("out"),
        other => other,
    }
    .to_owned()
}

/// Which way `to` lies from `from`, said: "to the north-east". None if
/// the map cannot tell, being in another area or level.
pub fn toward(map: &Map, from: RoomId, to: RoomId) -> Option<String> {
    let direction = map.bearing(from, to)?;
    Some(crate::i18n::trf(
        "to the {0}",
        &[&direction_words(map, direction)],
    ))
}

/// How far a room is from `from`, said: here, or how many moves and which
/// way, or that there is no known way.
pub fn how_far(map: &Map, from: Option<RoomId>, room: &Room) -> String {
    let Some(from) = from else {
        return tr("the map does not know where you are").to_owned();
    };
    if from == room.id {
        return tr("you are here").to_owned();
    }
    let Some(steps) = map.route(from, room.id) else {
        return tr("no known way").to_owned();
    };
    let moves = crate::i18n::trn("{0} move", "{0} moves", steps.len() as u64);
    match toward(map, from, room.id) {
        Some(way) => format!("{moves} {way}"),
        None => moves,
    }
}

/// A room's exits, said in the MUD's words, in the order of the compass,
/// then up and down, in and out, and the rest.
fn exits_words(map: &Map, room: &Room) -> String {
    const ORDER: [&str; 12] = [
        "n", "ne", "e", "se", "s", "sw", "w", "nw", "u", "d", "in", "out",
    ];
    let mut ways: Vec<&String> = room.exits.keys().collect();
    ways.sort_by_key(|way| {
        ORDER
            .iter()
            .position(|known| known == way)
            .unwrap_or(ORDER.len())
    });
    let exits: Vec<String> = ways
        .into_iter()
        .map(|way| direction_words(map, way))
        .collect();
    match exits.is_empty() {
        true => tr("no exits").to_owned(),
        false => crate::i18n::trf("exits: {0}", &[&exits.join(", ")]),
    }
}

/// What the map's tools asked for.
#[derive(Debug, Default, Clone, Copy)]
struct Tools {
    locate: bool,
    tidy: bool,
    stop: bool,
    avoid: bool,
    hide: bool,
}

pub struct MapView {
    cell: f32,
    /// How far the user dragged the map away from its centre.
    pan: Vec2,
    /// Room the view was last centred on; a move re-centres it.
    centred_on: Option<RoomId>,
    /// While the view glides to the room the player moved to: how far
    /// from it the view set out, in cells, and when.
    glide: Option<(Vec2, f64)>,
    /// Area and level on show while the user looks away from where the
    /// player is; `None` follows the player.
    browsing: Option<(String, i32)>,
    /// Draw the rooms of every area on the level, not just the chosen one.
    pub all_areas: bool,
    /// Room being repositioned with Shift+drag.
    moving: Option<RoomId>,
    /// Room the context menu was opened on.
    menu_room: Option<RoomId>,
    /// What the tools on the panel's header asked for, to be done when
    /// the map is drawn.
    asked: Tools,
    /// The rooms on the level shown, worked out again only when the map,
    /// the area, the level or whether every area is shown changes.
    on_level: Option<(Shown, Vec<(Pos, RoomId)>)>,
    /// How many areas the map has, at a revision of it.
    areas: Option<(u64, usize)>,
    /// The room chosen to look at, with a click or the keys.
    selected: Option<RoomId>,
    /// What is told of it, for the room chosen, where the player is and
    /// the map's revision it was worked out at.
    told: Option<(Told, String)>,
    /// A room to bring into sight, as the keys move the one chosen.
    reveal: Option<RoomId>,
    /// Whether the map had the keyboard the frame before: Escape takes it
    /// before it is seen.
    had_focus: bool,
    /// Whether the map is to take the keyboard when drawn, to explore it.
    focus_wanted: bool,
    /// The region of the room said last, to say a region on coming to it.
    said_region: Option<String>,
    /// The rooms the keys chose before, to go back to.
    trail: Vec<RoomId>,
    /// The words asked for over the map while exploring it.
    prompt: Option<Prompt>,
    /// Keys of the keypad pressed while the map has the keyboard, by name.
    keypad: Vec<&'static str>,
}

/// What is told of the room chosen depends on: it, where the player is,
/// and the map's revision.
type Told = (RoomId, Option<RoomId>, u64);

/// What the rooms on show depend on: the map's revision, the area, the
/// level, and whether every area is shown.
type Shown = (u64, String, i32, bool);

impl Default for MapView {
    fn default() -> Self {
        Self {
            cell: DEFAULT_CELL,
            pan: Vec2::ZERO,
            centred_on: None,
            glide: None,
            browsing: None,
            all_areas: false,
            moving: None,
            menu_room: None,
            asked: Tools::default(),
            on_level: None,
            areas: None,
            selected: None,
            told: None,
            reveal: None,
            had_focus: false,
            focus_wanted: false,
            said_region: None,
            trail: Vec::new(),
            prompt: None,
            keypad: Vec::new(),
        }
    }
}

/// Why the map cannot say where the player is, and what to do.
pub fn unknown_whereabouts(map: &Map) -> String {
    match map.candidates().len() {
        0 => tr("The map does not know where you are. /map here <number> tells it.").to_owned(),
        several => crate::i18n::trn(
            "The map is not sure where you are: in one of {0} rooms alike. Moving tells them apart; /map here lists them.",
            "The map is not sure where you are: in one of {0} rooms alike. Moving tells them apart; /map here lists them.",
            several as u64,
        ),
    }
}

impl MapView {
    /// The map's tools, on its panel's header: where am I, tidy, and stop
    /// while walking. What they ask for is done when the map is drawn.
    pub fn tools(&mut self, ui: &mut Ui, walking: bool) {
        ui.spacing_mut().item_spacing.x = 2.0;
        self.asked.locate |=
            widgets::icon_button(ui, widgets::Icon::Locate, tr("Show my position"), false)
                .clicked();
        self.asked.tidy |=
            widgets::icon_button(ui, widgets::Icon::Tidy, tr("Tidy this area"), false).clicked();
        if walking {
            self.asked.stop |=
                widgets::icon_button(ui, widgets::Icon::Stop, tr("Stop walking"), true).clicked();
        }
    }

    /// What the map adds to its panel's menu.
    pub fn menu_items(&mut self, ui: &mut Ui) {
        widgets::menu_check(ui, &mut self.all_areas, tr("Show all areas"));
        if widgets::menu_item(ui, tr("Avoid this room in routes"), "").clicked() {
            self.asked.avoid = true;
            ui.close();
        }
    }

    /// Shows one level of one area: the player's, unless the user chose
    /// another with the controls on top, with the rest of `walk`, if one
    /// is under way. Clicking a room chooses it, to be told under the map;
    /// the keys move it along the exits. Double-clicking a room asks to walk there, right-clicking
    /// opens its menu, dragging pans, the scroll wheel zooms, and
    /// Shift+drag moves a room to another cell.
    pub fn show(&mut self, ui: &mut Ui, map: &Map, walk: Option<&Walk>) -> Option<MapAction> {
        // Where the player is, or, while the map is unsure, the first of
        // the rooms they may be in: the map is drawn there all the same.
        let unsure = map.current().is_none();
        let first_candidate = || map.candidates().first().and_then(|id| map.room(*id));
        let Some(current) = map.current().or_else(first_candidate) else {
            ui.label(match map.rooms().next() {
                None => tr("No map yet: the server has not reported a room.").to_owned(),
                Some(_) => unknown_whereabouts(map),
            });
            return None;
        };
        let here: Vec<RoomId> = match unsure {
            true => map.candidates().to_vec(),
            false => vec![current.id],
        };
        let tools = std::mem::take(&mut self.asked);
        if unsure {
            ui.label(unknown_whereabouts(map));
        } else {
            heading(ui, map, current);
        }
        let (area, level, home) = self.place(ui, map, current, tools.locate);
        let (area, level) = self.place_controls(ui, map, (area, level), home);
        let here_id = (!unsure).then_some(current.id);
        let prompted = self.prompt_line(ui, map, here_id);
        // A room no longer there, or left over, is no longer chosen.
        self.selected = self
            .selected
            .filter(|id| map.room(*id).is_some() && !map.is_ghost(*id));
        let told = self.told_of_selected(map);
        // The room chosen is told under the map.
        // As many lines as it takes at this width, room kept for them.
        let told_text = told.as_ref().map(|told| {
            let font = egui::TextStyle::Body.resolve(ui.style());
            let colour = theme::current().text_dim;
            ui.painter()
                .layout(told.clone(), font, colour, ui.available_width())
        });
        let footer = told_text
            .as_ref()
            .map_or(0.0, |galley| galley.size().y + ui.spacing().item_spacing.y);
        let size = ui.available_size() - vec2(0.0, footer);
        // An id of its own, not one counted from what is drawn before it,
        // which a line over the map changes: the keyboard stays with it.
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let response = ui.interact(
            rect,
            ui.make_persistent_id("map view"),
            Sense::click_and_drag(),
        );
        let painter = ui.painter_at(rect);
        let label = match &told {
            Some(told) => format!("{}. {}", tr("Map view"), told),
            None => tr("Map view").to_owned(),
        };
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(label));
        if std::mem::take(&mut self.focus_wanted) {
            response.request_focus();
        }
        self.zoom_and_pan(ui, &response);
        let gliding = self.glide_offset(ui);
        let all_areas = self.all_areas;
        let on_level = |room: &Room| (all_areas || room.area == area) && room.pos.z == level;
        let shown: Shown = (map.revision(), area.clone(), level, all_areas);
        if self
            .on_level
            .as_ref()
            .is_none_or(|(known, _)| *known != shown)
        {
            // Rooms left over are not drawn: their place is another's.
            let places = map
                .rooms()
                .filter(|room| on_level(room) && !map.is_ghost(room.id))
                .map(|room| (room.pos, room.id));
            self.on_level = Some((shown, places.collect()));
        }
        // Where the rooms on the level are, kept: of a map of thousands,
        // only those on the screen are looked up.
        let places: &[(Pos, RoomId)] = self.on_level.as_ref().map_or(&[], |(_, places)| places);
        // The player's own area is centred on the player whatever the
        // level, so going up and down keeps the place; any other area is
        // centred on the middle of what is drawn.
        let origin = if area == current.area || all_areas {
            (current.pos.x, current.pos.y)
        } else {
            middle_of(places.iter().map(|(pos, _)| *pos))
        };
        let grid = Grid {
            centre: response.rect.center() + self.pan + gliding,
            origin,
            cell: self.cell,
            level,
        };
        let around = response.rect.expand(grid.cell);
        let visible: Vec<&Room> = (places.iter())
            .filter(|(pos, _)| around.contains(grid.on_screen(*pos)))
            .filter_map(|(_, id)| map.room(*id))
            .collect();
        let colours = Colours::of(theme::current());
        draw_exits(&painter, map, &visible, &grid, &colours, &on_level);
        if let Some(walk) = walk {
            let rooms = std::iter::once(current.id).chain(walk.rooms_left());
            draw_route(&painter, map, rooms, &grid, &colours, &on_level);
        }
        let marks = Marks {
            here: &here,
            destination: walk.and_then(|walk| walk.left().1),
            selected: self.selected,
            area: &area,
        };
        draw_rooms(&painter, &visible, &grid, &colours, &marks);
        draw_names(&painter, &visible, &grid, &colours);
        // The rooms under the pointer: the nearest, all of them if several
        // are in one place.
        let rooms_at = |point: Pos2| -> Vec<&Room> {
            let distance = |room: &&Room| grid.on_screen(room.pos).distance(point);
            let near = (visible.iter()).filter(|room| distance(room) <= grid.half() * 1.5);
            let Some(nearest) = near.clone().map(distance).reduce(f32::min) else {
                return Vec::new();
            };
            near.filter(|room| distance(room) <= nearest + 0.5)
                .copied()
                .collect()
        };
        let room_at = |point: Pos2| rooms_at(point).first().copied();
        if let Some(moved) = self.move_room(ui, &response, &painter, &grid, &room_at, &colours) {
            return Some(moved);
        }
        let pointed = response.hover_pos().and_then(room_at);
        if crate::widgets::menu_pressed(&response) {
            self.menu_room = pointed.map(|room| room.id);
        }
        let mut chosen = tools.action(&area).or(prompted);
        if let Some(asked) = self.room_menu(&response, map, &area) {
            chosen = Some(asked);
        }
        // Twice, so that a click to look is not a walk.
        // Of several in one place, one there is a way to.
        if chosen.is_none() && response.double_clicked() {
            let there = response.hover_pos().map(rooms_at).unwrap_or_default();
            let reached =
                |room: &&&Room| room.id == current.id || map.route(current.id, room.id).is_some();
            let walked = there.iter().find(reached).or(there.first());
            chosen = walked.map(|room| MapAction::Walk(room.id));
        }
        // A click chooses the room to look at, or none; not Enter, which
        // egui takes for a click of what has the keyboard.
        if response.clicked_by(egui::PointerButton::Primary) {
            response.request_focus();
            let before = self.selected;
            self.selected = pointed.map(|room| room.id);
            if let Some(id) = self.selected.filter(|id| before != Some(*id)) {
                chosen = chosen.or(Some(MapAction::Selected(id, None)));
            }
        }
        if let Some(keyed) = self.keys(ui, &response, map, current) {
            chosen = chosen.or(Some(keyed));
        }
        self.bring_into_sight(ui, &response, map, &grid, (&area, level));
        room_tip(response, pointed);
        if let Some(galley) = told_text {
            ui.add(egui::Label::new(galley));
        }
        chosen
    }

    /// What is told of the room chosen, worked out again only when it,
    /// where the player is or the map changes.
    fn told_of_selected(&mut self, map: &Map) -> Option<String> {
        let room = map.room(self.selected?)?;
        let key = (room.id, map.current().map(|here| here.id), map.revision());
        if self.told.as_ref().is_none_or(|(known, _)| *known != key) {
            self.told = Some((key, selection_line(map, key.1, room)));
        }
        self.told.as_ref().map(|(_, told)| told.clone())
    }

    /// Brings the room the keys chose into sight: to its area and level,
    /// and onto the screen.
    fn bring_into_sight(
        &mut self,
        ui: &Ui,
        response: &Response,
        map: &Map,
        grid: &Grid,
        (area, level): (&str, i32),
    ) {
        let Some(room) = self.reveal.and_then(|id| map.room(id)) else {
            return;
        };
        if room.area != area || room.pos.z != level {
            // Another area or level: shown first, brought onto the screen
            // when drawn.
            self.browsing = Some((room.area.clone(), room.pos.z));
            ui.ctx().request_repaint();
            return;
        }
        self.reveal = None;
        let at = grid.on_screen(room.pos);
        if !response.rect.shrink(grid.cell * 1.5).contains(at) {
            self.pan += response.rect.center() - at;
            ui.ctx().request_repaint();
        }
    }

    /// Which area and level to show: the player's, unless the view was
    /// taken to another; and the player's.
    fn place(
        &mut self,
        ui: &Ui,
        map: &Map,
        current: &Room,
        locate: bool,
    ) -> (String, i32, (String, i32)) {
        let home = (current.area.clone(), current.pos.z);
        if self.centred_on != Some(current.id) {
            // The player moved: go back to following them, gliding there
            // from where the view was if that was near.
            let before = self.centred_on.and_then(|id| map.room(id));
            let following = self.browsing.is_none();
            let set_out = self.pan + self.glide_offset(ui);
            self.glide = before
                .filter(|before| following && before.area == current.area)
                .map(|before| (before.pos.x - current.pos.x, before.pos.y - current.pos.y))
                .filter(|(dx, dy)| dx.abs().max(dy.abs()) <= GLIDE_REACH)
                .filter(|_| ui.style().animation_time > 0.0)
                .map(|(dx, dy)| {
                    let from = set_out / self.cell - vec2(dx as f32, -dy as f32);
                    (from, ui.input(|input| input.time))
                });
            self.centred_on = Some(current.id);
            self.pan = Vec2::ZERO;
            self.browsing = None;
        }
        if locate {
            // Back to the player, wherever the view had wandered.
            self.browsing = None;
            self.pan = Vec2::ZERO;
            self.glide = None;
        }
        let (area, level) = self.browsing.clone().unwrap_or_else(|| home.clone());
        (area, level, home)
    }

    /// How far the view still is from where it glides to, on screen; it
    /// is drawn again until it gets there.
    fn glide_offset(&mut self, ui: &Ui) -> Vec2 {
        let Some((from, start)) = self.glide else {
            return Vec2::ZERO;
        };
        let done = ((ui.input(|input| input.time) - start) / GLIDE) as f32;
        if done >= 1.0 {
            self.glide = None;
            return Vec2::ZERO;
        }
        ui.ctx().request_repaint();
        // Fast at first, slowing down as it arrives.
        let left = (1.0 - done.max(0.0)).powi(3);
        from * left * self.cell
    }

    /// Zooms to cells of this size, as the scroll wheel does.
    #[cfg(test)]
    pub fn zoom_to(&mut self, cell: f32) {
        self.cell = cell.clamp(*CELL_RANGE.start(), *CELL_RANGE.end());
    }

    /// How many areas the map has, counted again only when it changes.
    fn area_count(&mut self, map: &Map) -> usize {
        match self.areas {
            Some((revision, count)) if revision == map.revision() => count,
            _ => {
                let count = map.areas().len();
                self.areas = Some((map.revision(), count));
                count
            }
        }
    }

    /// The controls over the map, in one compact bar: which area, if the
    /// map has several, which level, and the way back to where the player
    /// is. Returns the area and level to show.
    fn place_controls(
        &mut self,
        ui: &mut Ui,
        map: &Map,
        (mut area, mut level): (String, i32),
        home: (String, i32),
    ) -> (String, i32) {
        let theme = theme::current();
        let browsing = self.browsing.is_some();
        let several_areas = self.area_count(map) > 1;
        ui.horizontal(|ui| {
            eframe::egui::Frame::new()
                .fill(theme.raised)
                .corner_radius(eframe::egui::CornerRadius::same(8))
                .inner_margin(eframe::egui::Margin::symmetric(4, 2))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if several_areas {
                            area_choice(ui, map, &mut area);
                        }
                        level_steps(ui, &mut level);
                    });
                });
            if browsing && crate::widgets::button(ui, tr("Back to my position")).clicked() {
                (area, level) = home.clone();
            }
        });
        ui.add_space(2.0);
        let shown = (area, level);
        if self.browsing.as_ref().unwrap_or(&home) != &shown {
            self.pan = Vec2::ZERO;
        }
        self.browsing = (shown != home).then(|| shown.clone());
        shown
    }

    /// The scroll wheel zooms over the map, and dragging pans it, but for a
    /// room being moved.
    fn zoom_and_pan(&mut self, ui: &Ui, response: &Response) {
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            let zoomed = self.cell * (1.0 + scroll * 0.002);
            self.cell = zoomed.clamp(*CELL_RANGE.start(), *CELL_RANGE.end());
        }
        if self.moving.is_none() {
            self.pan += response.drag_delta();
        }
    }

    /// Shift+drag picks a room up and shows where it would go; releasing
    /// drops it on the nearest cell. Returns the move, once dropped.
    fn move_room<'a>(
        &mut self,
        ui: &Ui,
        response: &Response,
        painter: &Painter,
        grid: &Grid,
        room_at: &dyn Fn(Pos2) -> Option<&'a Room>,
        colours: &Colours,
    ) -> Option<MapAction> {
        let pointer = response.interact_pointer_pos();
        if response.drag_started() && ui.input(|input| input.modifiers.shift) {
            let pressed_at = ui.input(|input| input.pointer.press_origin());
            self.moving = pressed_at.and_then(room_at).map(|room| room.id);
        }
        if let (Some(_), Some(point)) = (self.moving, pointer) {
            let ghost = Rect::from_center_size(
                grid.on_screen(grid.cell_under(point)),
                Vec2::splat(grid.half() * 2.0),
            );
            let stroke = Stroke::new(1.5, colours.mark);
            painter.rect_stroke(ghost, grid.corner(), stroke, StrokeKind::Outside);
        }
        if response.drag_stopped()
            && let Some(room) = self.moving.take()
        {
            return pointer.map(|point| MapAction::Move(room, grid.cell_under(point)));
        }
        None
    }

    /// The menu of the room right-clicked: walk there, avoid it, tidy its
    /// area, show all areas. Returns what was chosen.
    fn room_menu(&mut self, response: &Response, map: &Map, area: &str) -> Option<MapAction> {
        let mut chosen = None;
        crate::widgets::context_menu_of(response, |ui| {
            widgets::menu_width(ui);
            let room = self.menu_room.and_then(|id| map.room(id));
            if let Some(room) = room {
                widgets::menu_heading(ui, &format!("{} (#{})", room.name, room.id));
                if widgets::menu_item(ui, tr("Walk here"), "").clicked() {
                    chosen = Some(MapAction::Walk(room.id));
                    ui.close();
                }
                let toggle = if room.avoid {
                    tr("Allow in routes")
                } else {
                    tr("Avoid in routes")
                };
                if widgets::menu_item(ui, toggle, "").clicked() {
                    chosen = Some(MapAction::Avoid(room.id, !room.avoid));
                    ui.close();
                }
                widgets::menu_separator(ui);
            }
            let tidied = room.map_or_else(|| area.to_owned(), |room| room.area.clone());
            if widgets::menu_item(ui, tr("Tidy this area"), "").clicked() {
                chosen = Some(MapAction::Tidy(tidied));
                ui.close();
            }
            widgets::menu_check(ui, &mut self.all_areas, tr("Show all areas"));
        });
        chosen
    }
}

impl Tools {
    /// What the tools on the panel's header asked of the map.
    fn action(self, area: &str) -> Option<MapAction> {
        if self.hide {
            Some(MapAction::Command("/map hide"))
        } else if self.avoid {
            Some(MapAction::Command("/map avoid"))
        } else if self.stop {
            Some(MapAction::Command("/map stop"))
        } else if self.tidy {
            Some(MapAction::Tidy(area.to_owned()))
        } else if self.locate {
            Some(MapAction::Command("/map where"))
        } else {
            None
        }
    }
}
