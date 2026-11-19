//! Draws the map and lets the user browse it, pick a room to walk to, and
//! mark or move rooms.

use eframe::egui::{
    Align2, Color32, ComboBox, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2,
    vec2,
};
use muddie_mapper::{Map, Pos, Room, RoomId, direction_offset};

use crate::i18n::tr;
use crate::theme::{self, Theme};

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
    all_areas: bool,
    /// Room being repositioned with Shift+drag.
    moving: Option<RoomId>,
    /// Room the context menu was opened on.
    menu_room: Option<RoomId>,
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
        }
    }
}

impl MapView {
    /// Shows one level of one area: the player's, unless the user chose
    /// another with the controls on top. Clicking a room asks to walk there,
    /// right-clicking opens its menu, dragging pans, the scroll wheel zooms,
    /// and Shift+drag moves a room to another cell.
    pub fn show(&mut self, ui: &mut Ui, map: &Map) -> Option<MapAction> {
        let Some(current) = map.current() else {
            ui.label(tr("No map yet: the server has not reported a room."));
            return None;
        };
        // The same facts as text, for screen readers and at a glance.
        let exits: Vec<&str> = current.exits.keys().map(String::as_str).collect();
        ui.label(format!(
            "{} | {}: {}",
            current.name,
            tr("exits"),
            exits.join(", ")
        ));

        let home = (current.area.clone(), current.pos.z);
        if self.centred_on != Some(current.id) {
            // The player moved: go back to following them.
            self.centred_on = Some(current.id);
            self.pan = Vec2::ZERO;
            self.browsing = None;
        }
        let (mut area, mut level) = self.browsing.clone().unwrap_or_else(|| home.clone());
        ui.horizontal_wrapped(|ui| {
            let label = ui.label(tr("Area"));
            ComboBox::from_id_salt("map area")
                .selected_text(area.as_str())
                .show_ui(ui, |ui| {
                    for name in map.areas() {
                        ui.selectable_value(&mut area, name.to_owned(), name);
                    }
                })
                .response
                .labelled_by(label.id);
            if ui.button(tr("Down")).clicked() {
                level -= 1;
            }
            ui.label(format!("{} {level}", tr("level")));
            if ui.button(tr("Up")).clicked() {
                level += 1;
            }
            if self.browsing.is_some() && ui.button(tr("Back to me")).clicked() {
                (area, level) = home.clone();
            }
            ui.checkbox(&mut self.all_areas, tr("All areas"));
        });
        let shown = (area, level);
        if self.browsing.as_ref().unwrap_or(&home) != &shown {
            self.pan = Vec2::ZERO;
        }
        self.browsing = (shown != home).then(|| shown.clone());
        let (area, level) = shown;

        let hint = tr("Click a room to walk there, right-click for more, Shift+drag to move it.");
        ui.label(RichText::new(hint).weak().small());
        let colours = Colours::of(theme::current());
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label(tr("Map view"));
        });
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            let zoomed = self.cell * (1.0 + scroll * 0.002);
            self.cell = zoomed.clamp(*CELL_RANGE.start(), *CELL_RANGE.end());
        }
        let cell = self.cell;
        let half = cell * 0.28;
        if self.moving.is_none() {
            self.pan += response.drag_delta();
        }

        let all_areas = self.all_areas;
        let on_level = |room: &Room| (all_areas || room.area == area) && room.pos.z == level;
        // The player's own area is centred on the player whatever the
        // level, so going up and down keeps the place; any other area is
        // centred on the middle of what is drawn.
        let origin = if area == current.area || all_areas {
            (current.pos.x, current.pos.y)
        } else {
            let spans = map
                .rooms()
                .filter(|room| on_level(room))
                .fold(None, |spans, room| {
                    let ((x0, x1), (y0, y1)) =
                        spans.unwrap_or(((i32::MAX, i32::MIN), (i32::MAX, i32::MIN)));
                    let pos = room.pos;
                    Some((
                        (x0.min(pos.x), x1.max(pos.x)),
                        (y0.min(pos.y), y1.max(pos.y)),
                    ))
                });
            spans.map_or((0, 0), |((x0, x1), (y0, y1))| {
                ((x0 + x1) / 2, (y0 + y1) / 2)
            })
        };
        let centre = response.rect.center() + self.pan;
        let on_screen = |pos: Pos| -> Pos2 {
            // Screen y grows downwards, map y northwards.
            centre + vec2((pos.x - origin.0) as f32, -(pos.y - origin.1) as f32) * cell
        };
        let visible: Vec<&Room> = (map.rooms())
            .filter(|room| {
                on_level(room) && response.rect.expand(cell).contains(on_screen(room.pos))
            })
            .collect();
        let room_at = |point: Pos2| {
            (visible.iter()).find(|room| on_screen(room.pos).distance(point) <= half * 1.5)
        };

        for room in &visible {
            let from = on_screen(room.pos);
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
                        painter.line_segment([from, on_screen(target.pos)], line);
                    }
                    // Another level: mark which way the exit goes.
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
                    // Unexplored, or in another area: a stub pointing the way.
                    (None, Some((dx, dy, _))) => {
                        let stub = vec2(dx as f32, -dy as f32) * cell * 0.5;
                        painter.line_segment([from, from + stub], line);
                    }
                    // An exit with no direction on the grid (a portal, "enter").
                    (None, None) => other_exits = true,
                }
            }
            if other_exits {
                painter.circle_filled(from + vec2(-half, -half) * 1.5, cell * 0.07, colours.mark);
            }
        }
        for room in &visible {
            let rect = Rect::from_center_size(on_screen(room.pos), Vec2::splat(half * 2.0));
            let is_current = room.id == current.id;
            let fill = match (is_current, room.avoid) {
                (true, _) => colours.current,
                (false, true) => colours.avoided,
                (false, false) => colours.room,
            };
            // With every area on show, the chosen one stands out.
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

        // Shift+drag picks a room up; releasing drops it on the nearest cell.
        let pointer = response.interact_pointer_pos();
        let cell_under = |point: Pos2| {
            let offset = (point - centre) / cell;
            Pos {
                x: origin.0 + offset.x.round() as i32,
                y: origin.1 - offset.y.round() as i32,
                z: level,
            }
        };
        if response.drag_started() && ui.input(|input| input.modifiers.shift) {
            let pressed_at = ui.input(|input| input.pointer.press_origin());
            self.moving = pressed_at.and_then(room_at).map(|room| room.id);
        }
        if let (Some(_), Some(point)) = (self.moving, pointer) {
            let ghost =
                Rect::from_center_size(on_screen(cell_under(point)), Vec2::splat(half * 2.0));
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
            return pointer.map(|point| MapAction::Move(room, cell_under(point)));
        }

        let pointed = response.hover_pos().and_then(room_at);
        if response.secondary_clicked() {
            self.menu_room = pointed.map(|room| room.id);
        }
        let mut chosen = None;
        response.context_menu(|ui| {
            let Some(room) = self.menu_room.and_then(|id| map.room(id)) else {
                if ui.button(tr("Tidy this area")).clicked() {
                    chosen = Some(MapAction::Tidy(area.clone()));
                    ui.close();
                }
                return;
            };
            ui.label(format!("{} (#{})", room.name, room.id));
            if ui.button(tr("Walk here")).clicked() {
                chosen = Some(MapAction::Walk(room.id));
                ui.close();
            }
            let toggle = if room.avoid {
                tr("Allow in routes")
            } else {
                tr("Avoid in routes")
            };
            if ui.button(toggle).clicked() {
                chosen = Some(MapAction::Avoid(room.id, !room.avoid));
                ui.close();
            }
            if ui.button(tr("Tidy this area")).clicked() {
                chosen = Some(MapAction::Tidy(room.area.clone()));
                ui.close();
            }
        });
        if chosen.is_none() && response.clicked() {
            chosen = pointed.map(|room| MapAction::Walk(room.id));
        }
        if let Some(room) = pointed {
            let mut text = format!("{} (#{})", room.name, room.id);
            if room.avoid {
                text.push_str(&format!(", {}", tr("avoided")));
            }
            if room.weight > 1 {
                text.push_str(&format!(", {} {}", tr("weight"), room.weight));
            }
            response.on_hover_text_at_pointer(text);
        }
        chosen
    }
}
