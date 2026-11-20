//! Draws the map and lets the user browse it, pick a room to walk to, and
//! mark or move rooms.

use eframe::egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind,
    Ui, Vec2, vec2,
};
use muddie_mapper::{Map, Pos, Room, RoomId, direction_offset};

use crate::i18n::tr;
use crate::theme::{self, Theme, Weight};
use crate::widgets;

/// Distance between neighbouring rooms, in points, at the default zoom.
const DEFAULT_CELL: f32 = 36.0;
const CELL_RANGE: std::ops::RangeInclusive<f32> = 10.0..=72.0;

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

/// The map's colours, taken from the theme.
struct Colours {
    room: Color32,
    avoided: Color32,
    current: Color32,
    exit: Color32,
    locked: Color32,
    mark: Color32,
}

impl Colours {
    fn of(theme: Theme) -> Self {
        Self {
            // Opaque, so the exit lines do not show through the rooms.
            room: theme::mix(theme.surface, theme.text_dim, 0.6),
            avoided: theme::mix(theme.surface, theme.terminal[1], 0.65),
            current: theme.terminal[3],
            exit: theme::mix(theme.surface, theme.text_dim, 0.75),
            locked: theme.terminal[1],
            mark: theme.text_dim,
        }
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
}

impl Default for MapView {
    fn default() -> Self {
        Self {
            cell: DEFAULT_CELL,
            pan: Vec2::ZERO,
            centred_on: None,
            browsing: None,
            all_areas: false,
            moving: None,
            menu_room: None,
            asked: Tools::default(),
        }
    }
}

impl MapView {
    /// The map's tools, on its panel's header: where am I, tidy, and stop
    /// while walking. What they ask for is done when the map is drawn.
    pub fn tools(&mut self, ui: &mut Ui, walking: bool) {
        ui.spacing_mut().item_spacing.x = 2.0;
        if walking {
            self.asked.stop |=
                widgets::icon_button(ui, widgets::Icon::Stop, tr("Stop walking"), true).clicked();
        }
        self.asked.tidy |=
            widgets::icon_button(ui, widgets::Icon::Tidy, tr("Tidy this area"), false).clicked();
        self.asked.locate |=
            widgets::icon_button(ui, widgets::Icon::Locate, tr("Where am I?"), false).clicked();
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
    /// another with the controls on top. Clicking a room asks to walk there,
    /// right-clicking opens its menu, dragging pans, the scroll wheel zooms,
    /// and Shift+drag moves a room to another cell.
    pub fn show(&mut self, ui: &mut Ui, map: &Map) -> Option<MapAction> {
        let Some(current) = map.current() else {
            ui.label(tr("No map yet: the server has not reported a room."));
            return None;
        };
        let tools = std::mem::take(&mut self.asked);
        heading(ui, current);
        let (area, level) = self.place_row(ui, map, current, tools.locate);
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(tr("Map view")));
        self.zoom_and_pan(ui, &response);
        let all_areas = self.all_areas;
        let on_level = |room: &Room| (all_areas || room.area == area) && room.pos.z == level;
        // The player's own area is centred on the player whatever the
        // level, so going up and down keeps the place; any other area is
        // centred on the middle of what is drawn.
        let origin = if area == current.area || all_areas {
            (current.pos.x, current.pos.y)
        } else {
            middle_of(map.rooms().filter(|room| on_level(room)))
        };
        let grid = Grid {
            centre: response.rect.center() + self.pan,
            origin,
            cell: self.cell,
            level,
        };
        let visible: Vec<&Room> = (map.rooms())
            .filter(|room| {
                on_level(room)
                    && response
                        .rect
                        .expand(grid.cell)
                        .contains(grid.on_screen(room.pos))
            })
            .collect();
        let colours = Colours::of(theme::current());
        draw_exits(&painter, map, &visible, &grid, &colours, &on_level);
        draw_rooms(&painter, &visible, &grid, current, &area, &colours);
        let room_at = |point: Pos2| {
            (visible.iter())
                .find(|room| grid.on_screen(room.pos).distance(point) <= grid.half() * 1.5)
                .copied()
        };
        if let Some(moved) = self.move_room(ui, &response, &painter, &grid, &room_at, &colours) {
            return Some(moved);
        }
        let pointed = response.hover_pos().and_then(room_at);
        if response.secondary_clicked() {
            self.menu_room = pointed.map(|room| room.id);
        }
        let mut chosen = tools.action(&area);
        if let Some(asked) = self.room_menu(&response, map, &area) {
            chosen = Some(asked);
        }
        if chosen.is_none() && response.clicked() {
            chosen = pointed.map(|room| MapAction::Walk(room.id));
        }
        room_tip(response, pointed);
        chosen
    }

    /// One compact row: which area, which level, and the way back to where
    /// the player is. Returns the area and level to show.
    fn place_row(&mut self, ui: &mut Ui, map: &Map, current: &Room, locate: bool) -> (String, i32) {
        let home = (current.area.clone(), current.pos.z);
        if self.centred_on != Some(current.id) {
            // The player moved: go back to following them.
            self.centred_on = Some(current.id);
            self.pan = Vec2::ZERO;
            self.browsing = None;
        }
        let (mut area, mut level) = self.browsing.clone().unwrap_or_else(|| home.clone());
        if locate {
            // Back to the player, wherever the view had wandered.
            (area, level) = home.clone();
            self.pan = Vec2::ZERO;
        }
        let browsing = self.browsing.is_some();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            area_choice(ui, map, &mut area);
            level_steps(ui, &mut level);
            if browsing && crate::widgets::button(ui, tr("Back to me")).clicked() {
                (area, level) = home.clone();
            }
        });
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
            painter.rect_stroke(
                ghost,
                3.0,
                Stroke::new(1.5, colours.mark),
                StrokeKind::Outside,
            );
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
        response.context_menu(|ui| {
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

/// Where the map is drawn: the point on screen of the room at `origin`,
/// how big a cell is, and the level shown.
struct Grid {
    centre: Pos2,
    origin: (i32, i32),
    cell: f32,
    level: i32,
}

impl Grid {
    /// Where a place of the map is on screen: screen y grows downwards,
    /// map y northwards.
    fn on_screen(&self, pos: Pos) -> Pos2 {
        self.centre
            + vec2(
                (pos.x - self.origin.0) as f32,
                -(pos.y - self.origin.1) as f32,
            ) * self.cell
    }

    /// The cell of the map under a point on screen.
    fn cell_under(&self, point: Pos2) -> Pos {
        let offset = (point - self.centre) / self.cell;
        Pos {
            x: self.origin.0 + offset.x.round() as i32,
            y: self.origin.1 - offset.y.round() as i32,
            z: self.level,
        }
    }

    /// Half the side of a room.
    fn half(&self) -> f32 {
        self.cell * 0.28
    }
}

/// Where the player is, as text: for screen readers, and at a glance.
fn heading(ui: &mut Ui, current: &Room) {
    let palette = theme::current();
    ui.label(
        RichText::new(&current.name)
            .font(theme::font(Weight::Medium, 14.0))
            .color(palette.text),
    );
    let exits: Vec<&str> = current.exits.keys().map(String::as_str).collect();
    let leading = format!("{}: {}", tr("exits"), exits.join(", "));
    ui.label(
        RichText::new(leading)
            .font(theme::font(Weight::Regular, 12.0))
            .color(palette.text_dim),
    );
    ui.add_space(4.0);
}

/// The area to show, among those the map has.
fn area_choice(ui: &mut Ui, map: &Map, area: &mut String) {
    let chooser = crate::widgets::dropdown("map area")
        .selected_text(area.as_str())
        .width(130.0)
        .show_ui(ui, |ui| {
            for name in map.areas() {
                crate::widgets::choice_value(ui, area, name.to_owned(), name);
            }
        })
        .response;
    ui.ctx()
        .accesskit_node_builder(chooser.id, |node| node.set_label(tr("Area")));
}

/// The level to show, a step down or up.
fn level_steps(ui: &mut Ui, level: &mut i32) {
    if crate::widgets::sign_button(ui, "−", tr("Down"), true).clicked() {
        *level -= 1;
    }
    let shown = format!("{} {level}", tr("level"));
    ui.label(
        RichText::new(shown)
            .font(theme::font(Weight::Regular, 12.5))
            .color(theme::current().text_dim),
    );
    if crate::widgets::sign_button(ui, "+", tr("Up"), true).clicked() {
        *level += 1;
    }
}

/// The middle of the rooms given, to centre on.
fn middle_of<'a>(rooms: impl Iterator<Item = &'a Room>) -> (i32, i32) {
    let spans = rooms.fold(None, |spans, room| {
        let ((x0, x1), (y0, y1)) = spans.unwrap_or(((i32::MAX, i32::MIN), (i32::MAX, i32::MIN)));
        let pos = room.pos;
        Some((
            (x0.min(pos.x), x1.max(pos.x)),
            (y0.min(pos.y), y1.max(pos.y)),
        ))
    });
    spans.map_or((0, 0), |((x0, x1), (y0, y1))| {
        ((x0 + x1) / 2, (y0 + y1) / 2)
    })
}

/// The exits of the rooms shown: a line to a room shown, a mark for one
/// up or down, a stub the way of one not shown, and a dot for one with
/// no direction on the grid.
fn draw_exits(
    painter: &Painter,
    map: &Map,
    visible: &[&Room],
    grid: &Grid,
    colours: &Colours,
    on_level: &dyn Fn(&Room) -> bool,
) {
    let (cell, half) = (grid.cell, grid.half());
    for room in visible {
        let from = grid.on_screen(room.pos);
        let mut other_exits = false;
        for (direction, exit) in &room.exits {
            let colour = if exit.locked {
                colours.locked
            } else {
                colours.exit
            };
            let line = Stroke::new(1.5, colour);
            let target = map.room(exit.to).filter(|target| on_level(target));
            match (target, direction_offset(direction)) {
                (Some(target), _) => {
                    painter.line_segment([from, grid.on_screen(target.pos)], line);
                }
                (None, Some((_, _, dz))) if dz != 0 => {
                    let (text, corner) = if dz > 0 {
                        ("u", vec2(half, -half))
                    } else {
                        ("d", vec2(half, half))
                    };
                    let font = FontId::monospace(cell * 0.36);
                    painter.text(
                        from + corner * 1.6,
                        Align2::CENTER_CENTER,
                        text,
                        font,
                        colours.mark,
                    );
                }
                (None, Some((dx, dy, _))) => {
                    let stub = vec2(dx as f32, -dy as f32) * cell * 0.5;
                    painter.line_segment([from, from + stub], line);
                }
                (None, None) => other_exits = true,
            }
        }
        if other_exits {
            painter.circle_filled(from + vec2(-half, -half) * 1.5, cell * 0.07, colours.mark);
        }
    }
}

/// The rooms shown: the player's ringed, avoided ones marked, and those of
/// other areas, with every area on show, faint.
fn draw_rooms(
    painter: &Painter,
    visible: &[&Room],
    grid: &Grid,
    current: &Room,
    area: &str,
    colours: &Colours,
) {
    for room in visible {
        let rect = Rect::from_center_size(grid.on_screen(room.pos), Vec2::splat(grid.half() * 2.0));
        let is_current = room.id == current.id;
        let fill = match (is_current, room.avoid) {
            (true, _) => colours.current,
            (false, true) => colours.avoided,
            (false, false) => colours.room,
        };
        let fill = if room.area == area {
            fill
        } else {
            fill.gamma_multiply(0.45)
        };
        painter.rect_filled(rect, 3.0, fill);
        if is_current {
            let ring = Stroke::new(1.5, colours.current);
            painter.rect_stroke(rect.expand(3.0), 4.0, ring, StrokeKind::Outside);
        }
    }
}

/// Over empty space the map says how it is used; over a room, which.
fn room_tip(response: Response, pointed: Option<&Room>) {
    let Some(room) = pointed else {
        if response.hovered() && !response.dragged() {
            let hint =
                tr("Click a room to walk there, right-click for more, Shift+drag to move it.");
            response.on_hover_text_at_pointer(hint);
        }
        return;
    };
    let mut text = format!("{} (#{})", room.name, room.id);
    if room.avoid {
        text.push_str(&format!(", {}", tr("avoided")));
    }
    if room.weight > 1 {
        text.push_str(&format!(", {} {}", tr("weight"), room.weight));
    }
    response.on_hover_text_at_pointer(text);
}
