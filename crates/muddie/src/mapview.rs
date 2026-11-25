//! Draws the map and lets the user browse it, pick a room to walk to, and
//! mark or move rooms.

use std::sync::Arc;

use eframe::egui;
use eframe::egui::{
    Color32, Galley, Painter, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
    vec2,
};
use muddie_mapper::{Map, Pos, Room, RoomId, Walk, direction_offset};

use crate::i18n::{tr, trf};
use crate::theme::{self, Theme, Weight};
use crate::widgets;

mod chips;
mod choosing;
mod controls;
mod draw;
mod explore;
mod near;
mod view;
mod words;

use chips::*;
use draw::*;
use explore::{Prompt, keys_hint};
pub use near::{NEAR, found_words, near};
use near::{all_keys, near_words};
pub use words::*;

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
pub enum MapAsk {
    /// Walk to this room.
    Walk(RoomId),
    /// Put this room at this grid position.
    Move(RoomId, Pos),
    /// Keep routes out of this room (true) or allow them again (false).
    Avoid(RoomId, bool),
    /// Draw this area again from scratch.
    Tidy(String),
    /// Run one of the client's own commands, such as `/map where`.
    Command(&'static str),
    /// Open the scripts, to set a mapper up.
    Scripts,
    /// This room was chosen to look at, from the one before it if the keys
    /// moved there: say it.
    Selected(RoomId, Option<RoomId>),
    /// Say the way to this room, without going.
    Way(RoomId),
    /// The player is in this room, as they say.
    Here(RoomId),
    /// The room chosen has no exit this way, as asked with the keys.
    NoExit(&'static str),
    /// Leave the map for the command line, with what was typed on it.
    Leave(String),
    /// Say this, while exploring the map.
    Say(String),
    /// A field over the map asks for this: said where no screen reader
    /// says where the keyboard went.
    Asking(String),
    /// Say all the map knows of this room.
    Info(RoomId),
    /// Mark this room with this name.
    Mark(RoomId, String),
    /// Write this note on this room; an empty one takes it away.
    Note(RoomId, String),
    /// Choose this room's appearance.
    Look(RoomId),
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

/// What the map's tools asked for.
#[derive(Debug, Default, Clone, Copy)]
struct Tools {
    locate: bool,
    tidy: bool,
    stop: bool,
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
    areas: std::cell::Cell<Option<(u64, usize)>>,
    /// The levels of an area, at a revision of the map.
    levels: Option<((u64, String), Vec<i32>)>,
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
    /// Exploring was started, and the keyboard is coming: not to be
    /// started again as it comes.
    explore_started: bool,
    /// It was drawn since the frame began.
    drawn: bool,
    /// The player is walking somewhere: Escape stops it.
    walking: bool,
    /// What a level button pressed is to say.
    level_said: Option<String>,
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
            areas: Default::default(),
            levels: None,
            selected: None,
            told: None,
            reveal: None,
            had_focus: false,
            explore_started: false,
            drawn: false,
            walking: false,
            level_said: None,
            focus_wanted: false,
            said_region: None,
            trail: Vec::new(),
            prompt: None,
            keypad: Vec::new(),
        }
    }
}

/// The rooms under `point`: the nearest, all of them if several are in
/// one place.
fn rooms_at<'a>(visible: &[&'a Room], grid: &Grid, point: Pos2) -> Vec<&'a Room> {
    let distance = |room: &&Room| grid.on_screen(room.pos).distance(point);
    let near = (visible.iter()).filter(|room| distance(room) <= grid.half() * 1.5);
    let Some(nearest) = near.clone().map(distance).reduce(f32::min) else {
        return Vec::new();
    };
    near.filter(|room| distance(room) <= nearest + 0.5)
        .copied()
        .collect()
}

/// Whether a room is on the level shown: of the area shown, or of any if
/// all of them are, and at the height shown.
fn level_test(all_areas: bool, area: &str, level: i32) -> impl Fn(&Room) -> bool {
    move |room| (all_areas || room.area == area) && room.pos.z == level
}

/// The line over the map: where the player is, or that it is not known.
fn show_head(ui: &mut Ui, map: &Map, current: &Room, unsure: bool) {
    if unsure {
        ui.label(unknown_whereabouts(map));
    } else {
        heading(ui, map, current);
    }
}

/// Shows under the map what is told of the room chosen, and its exits.
fn show_told(ui: &mut Ui, told_text: Option<Arc<Galley>>, told_exits: Option<Chips>) {
    if let Some(galley) = told_text {
        ui.add(egui::Label::new(galley));
    }
    if let Some(chips) = told_exits {
        show_chips(ui, chips);
    }
}

/// What is told of the room chosen, under the map, and its exits under
/// that, and the height they take together.
fn told_footer(
    ui: &Ui,
    map: &Map,
    told: Option<&String>,
    chosen_room: Option<&Room>,
) -> (Option<Arc<Galley>>, Option<Chips>, f32) {
    // The room chosen is told under the map, its exits under it: in as
    // many lines as it takes at this width, room kept for them.
    let width = ui.available_width();
    let told_text = told.map(|told| {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let colour = theme::current().text_dim;
        ui.painter().layout(told.clone(), font, colour, width)
    });
    let told_exits = chosen_room.map(|room| exit_chips(ui, map, room, width));
    let gap = ui.spacing().item_spacing.y;
    let footer = told_text
        .as_ref()
        .map_or(0.0, |galley| galley.size().y + gap)
        + told_exits.as_ref().map_or(0.0, |chips| chips.size.y + gap);
    (told_text, told_exits, footer)
}

impl MapView {
    /// The grid the rooms are drawn on, and the rooms of the level that
    /// are in sight on it.
    fn grid_and_visible<'a>(
        &mut self,
        ui: &Ui,
        response: &Response,
        map: &'a Map,
        current: &Room,
        area: &str,
        level: i32,
    ) -> (Grid, Vec<&'a Room>) {
        let gliding = self.glide_offset(ui);
        let all_areas = self.all_areas;
        let on_level = level_test(all_areas, area, level);
        let shown: Shown = (map.revision(), area.to_owned(), level, all_areas);
        self.keep_places_on_level(map, shown, &on_level);
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
        (grid, visible)
    }

    /// The map's own place in the panel, under what a line over it
    /// takes: its room, its corner controls and its interaction. Gives
    /// the interaction, and the area and level the controls left.
    fn allocate_map(
        &mut self,
        ui: &mut Ui,
        map: &Map,
        (area, level, home): (String, i32, (String, i32)),
        footer: f32,
    ) -> (Response, String, i32) {
        let size = ui.available_size() - vec2(0.0, footer);
        // An id of its own, not one counted from what is drawn before it,
        // which a line over the map changes: the keyboard stays with it.
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        // Over its corners, before it, to be reached with Tab before it.
        let (area, level) = self.corner_controls(ui, map, (area, level), home, rect);
        let response = ui.interact(
            rect,
            ui.make_persistent_id("map view"),
            Sense::click_and_drag(),
        );
        (response, area, level)
    }

    /// Sets the room whose menu is to open: the one pointed at, or, when
    /// the keys open it, the one being explored.
    fn choose_menu_room(
        &mut self,
        ui: &Ui,
        response: &Response,
        current: &Room,
        pointed: Option<&Room>,
    ) {
        if crate::widgets::menu_pressed(response) {
            self.menu_room = pointed.map(|room| room.id);
        }
        // Opened with the keys, the menu is of the room being explored.
        let by_keys = response.has_focus()
            && ui.input(|input| {
                (input.modifiers.shift && input.key_pressed(egui::Key::F10))
                    || input.key_pressed(egui::Key::Menu)
            });
        if by_keys {
            self.menu_room = Some(self.selected.unwrap_or(current.id));
        }
    }

    /// Carries out a double click or a click on the map: a double click
    /// asks to walk, if `chosen` is nothing yet; a click chooses the room
    /// pointed at. Gives `chosen` with what they ask for.
    fn clicked<'a>(
        &mut self,
        response: &Response,
        map: &Map,
        current: &Room,
        rooms_at: &dyn Fn(Pos2) -> Vec<&'a Room>,
        pointed: Option<&Room>,
        mut chosen: Option<MapAsk>,
    ) -> Option<MapAsk> {
        // Twice, so that a click to look is not a walk.
        // Of several in one place, one there is a way to.
        if chosen.is_none() && response.double_clicked() {
            let there = response.hover_pos().map(rooms_at).unwrap_or_default();
            let reached = |room: &&&Room| map.moves_between(current.id, room.id).is_some();
            let walked = there.iter().find(reached).or(there.first());
            chosen = walked.map(|room| MapAsk::Walk(room.id));
        }
        // A click chooses the room to look at, or none; not Enter, which
        // egui takes for a click of what has the keyboard.
        if response.clicked_by(egui::PointerButton::Primary) {
            response.request_focus();
            let before = self.selected;
            self.selected = pointed.map(|room| room.id);
            if let Some(id) = self.selected.filter(|id| before != Some(*id)) {
                chosen = chosen.or(Some(MapAsk::Selected(id, None)));
            }
        }
        chosen
    }

    /// Gives `chosen` with what the keys, and then the level buttons, ask
    /// for, if it is nothing yet.
    fn keyed_or_said(
        &mut self,
        ui: &Ui,
        response: &Response,
        map: &Map,
        current: &Room,
        mut chosen: Option<MapAsk>,
    ) -> Option<MapAsk> {
        if let Some(keyed) = self.keys(ui, response, map, current) {
            chosen = chosen.or(Some(keyed));
        }
        if let Some(said) = self.level_said.take() {
            chosen = chosen.or(Some(MapAsk::Say(said)));
        }
        chosen
    }

    /// Names the map for a screen reader, and gives it the keyboard if it
    /// is wanted to have it.
    fn name_and_focus(&mut self, ui: &Ui, response: &Response) {
        // Named once, the same always: the room chosen is said by the
        // keys as they choose it, not again as the map's name changes.
        let hint = explore::keys_hint();
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(egui::accesskit::Role::Group);
            node.set_label(tr("Map"));
            node.set_description(hint);
        });
        if std::mem::take(&mut self.focus_wanted) {
            response.request_focus();
        }
    }

    /// Keeps where the rooms on the level are, anew when the map, the area
    /// or the level changed.
    fn keep_places_on_level(&mut self, map: &Map, shown: Shown, on_level: &dyn Fn(&Room) -> bool) {
        if self
            .on_level
            .as_ref()
            .is_none_or(|(known, _)| *known != shown)
        {
            // Rooms left over are not drawn: their place is another's.
            let mut places: Vec<(Pos, RoomId)> = (map.rooms())
                .filter(|room| on_level(room) && !map.is_ghost(room.id))
                .map(|room| (room.pos, room.id))
                .collect();
            // The map gives them in no order: in one, what is over what is
            // always the same.
            places.sort_unstable_by_key(|(_, id)| *id);
            self.on_level = Some((shown, places));
        }
    }

    /// The map's tools, on its panel's header: where am I, tidy, and stop
    /// while walking. What they ask for is done when the map is drawn.
    pub fn tools(&mut self, ui: &mut Ui, walking: bool) {
        ui.spacing_mut().item_spacing.x = 2.0;
        self.asked.locate |=
            widgets::icon_button(ui, widgets::Icon::Locate, tr("Where am I"), false).clicked();
        self.asked.tidy |=
            widgets::icon_button(ui, widgets::Icon::Tidy, tr("Tidy this area"), false).clicked();
        if walking {
            self.asked.stop |=
                widgets::icon_button(ui, widgets::Icon::Stop, tr("Stop going"), true).clicked();
        }
    }

    /// What the map adds to its panel's menu: how much of it is shown.
    /// What is done with a room is in the room's own menu.
    pub fn menu_items(&mut self, ui: &mut Ui) {
        widgets::menu_check(ui, &mut self.all_areas, tr("Show all areas"));
    }

    /// Shows one level of one area: the player's, unless the user chose
    /// another with the controls on top, with the rest of `walk`, if one
    /// is under way. Clicking a room chooses it, to be told under the map;
    /// the keys move it along the exits. Double-clicking a room asks to walk there, right-clicking
    /// opens its menu, dragging pans, the scroll wheel zooms, and
    /// Shift+drag moves a room to another cell.
    pub fn show(&mut self, ui: &mut Ui, map: &Map, walk: Option<&Walk>) -> Option<MapAsk> {
        // Where the player is, or, while the map is unsure, the first of
        // the rooms they may be in: the map is drawn there all the same.
        // Its panel shows it only once there is one or the other.
        let unsure = map.current().is_none();
        let first_candidate = || map.candidates().first().and_then(|id| map.room(*id));
        let current = map.current().or_else(first_candidate)?;
        let here: Vec<RoomId> = match unsure {
            true => map.candidates().to_vec(),
            false => vec![current.id],
        };
        self.drawn = true;
        self.walking = walk.is_some();
        let tools = std::mem::take(&mut self.asked);
        show_head(ui, map, current, unsure);
        let (area, level, home) = self.place(ui, map, current, tools.locate);
        let here_id = (!unsure).then_some(current.id);
        let prompted = self.prompt_line(ui, map, here_id);
        // A room no longer there, or left over, is no longer chosen.
        self.selected = self
            .selected
            .filter(|id| map.room(*id).is_some() && !map.is_ghost(*id));
        let told = self.told_of_selected(map);
        let chosen_room = self.selected.and_then(|id| map.room(id));
        let (told_text, told_exits, footer) = told_footer(ui, map, told.as_ref(), chosen_room);
        let (response, area, level) = self.allocate_map(ui, map, (area, level, home), footer);
        let painter = ui.painter_at(response.rect);
        self.name_and_focus(ui, &response);
        self.zoom_and_pan(ui, &response);
        let (grid, visible) = self.grid_and_visible(ui, &response, map, current, &area, level);
        let on_level = level_test(self.all_areas, &area, level);
        let colours = Colours::of(theme::current());
        draw_exits(&painter, map, &visible, &grid, &colours, &on_level);
        if let Some(walk) = walk {
            let rooms = std::iter::once(current.id).chain(walk.rooms_left());
            draw_route(&painter, map, rooms, &grid, &colours, &on_level);
        }
        let shown = Highlights {
            here: &here,
            destination: walk.and_then(|walk| walk.left().1),
            selected: self.selected,
            area: &area,
        };
        draw_rooms(
            &painter,
            &visible,
            &grid,
            &colours,
            &shown,
            map.appearance(),
        );
        draw_names(&painter, &visible, &grid, &colours);
        let rooms_at = |point: Pos2| rooms_at(&visible, &grid, point);
        let room_at = |point: Pos2| rooms_at(point).first().copied();
        if let Some(moved) = self.move_room(ui, &response, &painter, &grid, &room_at, &colours) {
            return Some(moved);
        }
        let pointed = response.hover_pos().and_then(room_at);
        self.choose_menu_room(ui, &response, current, pointed);
        let mut chosen = tools.action(&area).or(prompted);
        if let Some(asked) = self.room_menu(&response, map, &area) {
            chosen = Some(asked);
        }
        let chosen = self.clicked(&response, map, current, &rooms_at, pointed, chosen);
        let chosen = self.keyed_or_said(ui, &response, map, current, chosen);
        self.bring_into_sight(ui, &response, map, &grid, (&area, level));
        room_tip(response, map, pointed);
        show_told(ui, told_text, told_exits);
        chosen
    }
}

#[cfg(test)]
mod tests;
