//! Draws the map and lets the user browse it, pick a room to walk to, and
//! mark or move rooms.

use eframe::egui::{
    Color32, FontId, Painter, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
    vec2,
};
use muddie_mapper::{Map, Pos, Room, RoomId, Walk, direction_offset};

use crate::i18n::tr;
use crate::theme::{self, Theme, Weight};
use crate::widgets;

mod draw;

use draw::*;

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
}

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
    /// is under way. Double-clicking a room asks to walk there, right-clicking
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
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(tr("Map view")));
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
            let places = map
                .rooms()
                .filter(|room| on_level(room))
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
            area: &area,
        };
        draw_rooms(&painter, &visible, &grid, &colours, &marks);
        draw_names(&painter, &visible, &grid, &colours);
        let room_at = |point: Pos2| {
            (visible.iter())
                .find(|room| grid.on_screen(room.pos).distance(point) <= grid.half() * 1.5)
                .copied()
        };
        if let Some(moved) = self.move_room(ui, &response, &painter, &grid, &room_at, &colours) {
            return Some(moved);
        }
        let pointed = response.hover_pos().and_then(room_at);
        if crate::widgets::menu_pressed(&response) {
            self.menu_room = pointed.map(|room| room.id);
        }
        let mut chosen = tools.action(&area);
        if let Some(asked) = self.room_menu(&response, map, &area) {
            chosen = Some(asked);
        }
        // Twice, so that a click to look is not a walk.
        if chosen.is_none() && response.double_clicked() {
            chosen = pointed.map(|room| MapAction::Walk(room.id));
        }
        room_tip(response, pointed);
        chosen
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
