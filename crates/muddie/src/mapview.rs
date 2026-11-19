//! Draws the map around the player and lets them pick a room to walk to.

use eframe::egui::{Align2, Color32, FontId, Pos2, Sense, Stroke, StrokeKind, Ui, Vec2, vec2};
use muddie_mapper::{Map, Room, RoomId, direction_offset};

const ROOM_FILL: Color32 = Color32::from_rgb(0x4a, 0x52, 0x63);
const CURRENT_FILL: Color32 = Color32::from_rgb(0xe5, 0xc0, 0x7b);
const EXIT_LINE: Color32 = Color32::from_rgb(0x6b, 0x72, 0x80);
const MARK: Color32 = Color32::from_rgb(0xc8, 0xcc, 0xd4);

/// Distance between neighbouring rooms, in points, at the default zoom.
const DEFAULT_CELL: f32 = 28.0;
const CELL_RANGE: std::ops::RangeInclusive<f32> = 10.0..=72.0;

pub struct MapView {
    cell: f32,
    /// How far the user dragged the map away from the player's room.
    pan: Vec2,
    /// Room the view was last centred on; a move re-centres it.
    centred_on: Option<RoomId>,
}

impl Default for MapView {
    fn default() -> Self {
        Self {
            cell: DEFAULT_CELL,
            pan: Vec2::ZERO,
            centred_on: None,
        }
    }
}

impl MapView {
    /// Shows the player's level of their area. Returns a room the user
    /// clicked, if any. Drag pans and the scroll wheel zooms.
    pub fn show(&mut self, ui: &mut Ui, map: &Map) -> Option<RoomId> {
        let Some(current) = map.current() else {
            ui.label("No map yet: the server has not reported a room.");
            return None;
        };
        // The same facts as text, for screen readers and at a glance.
        let exits: Vec<&str> = current.exits.keys().map(String::as_str).collect();
        ui.label(format!("{} | exits: {}", current.name, exits.join(", ")));

        if self.centred_on != Some(current.id) {
            self.centred_on = Some(current.id);
            self.pan = Vec2::ZERO;
        }
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label("Map");
        });
        self.pan += response.drag_delta();
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            self.cell =
                (self.cell * (1.0 + scroll * 0.002)).clamp(*CELL_RANGE.start(), *CELL_RANGE.end());
        }

        let cell = self.cell;
        let centre = response.rect.center() + self.pan;
        let on_screen = |room: &Room| -> Pos2 {
            let dx = (room.pos.x - current.pos.x) as f32;
            let dy = (room.pos.y - current.pos.y) as f32;
            // Screen y grows downwards, map y northwards.
            centre + vec2(dx, -dy) * cell
        };
        let on_level = |room: &Room| room.area == current.area && room.pos.z == current.pos.z;
        let visible: Vec<&Room> = (map.rooms())
            .filter(|room| on_level(room) && response.rect.expand(cell).contains(on_screen(room)))
            .collect();

        let line = Stroke::new(1.5, EXIT_LINE);
        let half = cell * 0.28;
        for room in &visible {
            let from = on_screen(room);
            for (direction, to) in &room.exits {
                match (map.room(*to), direction_offset(direction)) {
                    (Some(target), _) if on_level(target) => {
                        painter.line_segment([from, on_screen(target)], line);
                    }
                    // Another level: mark which way the exit goes.
                    (_, Some((_, _, dz))) if dz != 0 => {
                        let (text, corner) = if dz > 0 {
                            ("u", vec2(half, -half))
                        } else {
                            ("d", vec2(half, half))
                        };
                        let at = from + corner * 1.6;
                        painter.text(
                            at,
                            Align2::CENTER_CENTER,
                            text,
                            FontId::monospace(cell * 0.36),
                            MARK,
                        );
                    }
                    // Unexplored: a stub pointing the way.
                    (None, Some((dx, dy, _))) => {
                        let stub = vec2(dx as f32, -dy as f32) * cell * 0.5;
                        painter.line_segment([from, from + stub], line);
                    }
                    _ => {}
                }
            }
        }
        for room in &visible {
            let rect =
                eframe::egui::Rect::from_center_size(on_screen(room), Vec2::splat(half * 2.0));
            let is_current = room.id == current.id;
            painter.rect_filled(rect, 3.0, if is_current { CURRENT_FILL } else { ROOM_FILL });
            if is_current {
                painter.rect_stroke(
                    rect.expand(3.0),
                    4.0,
                    Stroke::new(1.5, CURRENT_FILL),
                    StrokeKind::Outside,
                );
            }
        }

        let pointed = response.hover_pos().and_then(|pointer| {
            (visible.iter()).find(|room| on_screen(room).distance(pointer) <= half * 1.5)
        });
        let clicked = response
            .clicked()
            .then(|| pointed.map(|room| room.id))
            .flatten();
        if let Some(room) = pointed {
            response.on_hover_text_at_pointer(format!("{} (#{})", room.name, room.id));
        }
        clicked
    }
}
