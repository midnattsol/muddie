//! Draws the map around the player and lets them pick a room to walk to.

use eframe::egui::{
    Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, vec2,
};
use muddie_mapper::{Map, Pos, Room, RoomId, direction_offset};

const ROOM_FILL: Color32 = Color32::from_rgb(0x4a, 0x52, 0x63);
const CURRENT_FILL: Color32 = Color32::from_rgb(0xe5, 0xc0, 0x7b);
const EXIT_LINE: Color32 = Color32::from_rgb(0x6b, 0x72, 0x80);
const MARK: Color32 = Color32::from_rgb(0xc8, 0xcc, 0xd4);

/// Distance between neighbouring rooms, in points, at the default zoom.
const DEFAULT_CELL: f32 = 28.0;
const CELL_RANGE: std::ops::RangeInclusive<f32> = 10.0..=72.0;

/// Something the user did on the map that the caller must carry out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapAction {
    /// Walk to this room.
    Walk(RoomId),
    /// Put this room at this grid position.
    Move(RoomId, Pos),
}

pub struct MapView {
    cell: f32,
    /// How far the user dragged the map away from the player's room.
    pan: Vec2,
    /// Room the view was last centred on; a move re-centres it.
    centred_on: Option<RoomId>,
    /// Room being repositioned with Shift+drag.
    moving: Option<RoomId>,
}

impl Default for MapView {
    fn default() -> Self {
        Self {
            cell: DEFAULT_CELL,
            pan: Vec2::ZERO,
            centred_on: None,
            moving: None,
        }
    }
}

impl MapView {
    /// Shows the player's level of their area. Clicking a room asks to walk
    /// there, dragging pans, the scroll wheel zooms, and Shift+drag moves a
    /// room to another cell.
    pub fn show(&mut self, ui: &mut Ui, map: &Map) -> Option<MapAction> {
        let Some(current) = map.current() else {
            ui.label("No map yet: the server has not reported a room.");
            return None;
        };
        // The same facts as text, for screen readers and at a glance.
        let exits: Vec<&str> = current.exits.keys().map(String::as_str).collect();
        ui.label(format!("{} | exits: {}", current.name, exits.join(", ")));
        ui.label(RichText::new(format!("{}, level {}", current.area, current.pos.z)).weak());

        if self.centred_on != Some(current.id) {
            self.centred_on = Some(current.id);
            self.pan = Vec2::ZERO;
        }
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label("Map");
        });
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            self.cell =
                (self.cell * (1.0 + scroll * 0.002)).clamp(*CELL_RANGE.start(), *CELL_RANGE.end());
        }
        let cell = self.cell;
        let half = cell * 0.28;
        if self.moving.is_none() {
            self.pan += response.drag_delta();
        }
        let centre = response.rect.center() + self.pan;
        let on_screen = |pos: Pos| -> Pos2 {
            let dx = (pos.x - current.pos.x) as f32;
            let dy = (pos.y - current.pos.y) as f32;
            // Screen y grows downwards, map y northwards.
            centre + vec2(dx, -dy) * cell
        };
        let on_level = |room: &Room| room.area == current.area && room.pos.z == current.pos.z;
        let visible: Vec<&Room> = (map.rooms())
            .filter(|room| {
                on_level(room) && response.rect.expand(cell).contains(on_screen(room.pos))
            })
            .collect();
        let room_at = |point: Pos2| {
            (visible.iter()).find(|room| on_screen(room.pos).distance(point) <= half * 1.5)
        };

        let line = Stroke::new(1.5, EXIT_LINE);
        for room in &visible {
            let from = on_screen(room.pos);
            let mut other_exits = false;
            for (direction, to) in &room.exits {
                let target = map.room(*to).filter(|target| on_level(target));
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
                        painter.text(from + corner * 1.6, Align2::CENTER_CENTER, text, font, MARK);
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
                painter.circle_filled(from + vec2(-half, -half) * 1.5, cell * 0.07, MARK);
            }
        }
        for room in &visible {
            let rect = Rect::from_center_size(on_screen(room.pos), Vec2::splat(half * 2.0));
            let is_current = room.id == current.id;
            painter.rect_filled(rect, 3.0, if is_current { CURRENT_FILL } else { ROOM_FILL });
            if is_current {
                let ring = Stroke::new(1.5, CURRENT_FILL);
                painter.rect_stroke(rect.expand(3.0), 4.0, ring, StrokeKind::Outside);
            }
        }

        // Shift+drag picks a room up; releasing drops it on the nearest cell.
        let pointer = response.interact_pointer_pos();
        let cell_under = |point: Pos2| {
            let offset = (point - centre) / cell;
            Pos {
                x: current.pos.x + offset.x.round() as i32,
                y: current.pos.y - offset.y.round() as i32,
                z: current.pos.z,
            }
        };
        if response.drag_started() && ui.input(|input| input.modifiers.shift) {
            let origin = ui.input(|input| input.pointer.press_origin());
            self.moving = origin.and_then(room_at).map(|room| room.id);
        }
        if let (Some(_), Some(point)) = (self.moving, pointer) {
            let ghost =
                Rect::from_center_size(on_screen(cell_under(point)), Vec2::splat(half * 2.0));
            painter.rect_stroke(ghost, 3.0, Stroke::new(1.5, MARK), StrokeKind::Outside);
        }
        if response.drag_stopped()
            && let Some(room) = self.moving.take()
        {
            return pointer.map(|point| MapAction::Move(room, cell_under(point)));
        }

        let pointed = response.hover_pos().and_then(room_at);
        let clicked = response
            .clicked()
            .then(|| pointed.map(|room| MapAction::Walk(room.id)));
        if let Some(room) = pointed {
            response.on_hover_text_at_pointer(format!("{} (#{})", room.name, room.id));
        }
        clicked.flatten()
    }
}
