//! Drawing the map: where places go on screen, the rooms and their exits,
//! and the controls over it.

use super::*;

/// The map's colours, taken from the theme.
pub(super) struct Colours {
    pub(super) room: Color32,
    pub(super) avoided: Color32,
    pub(super) current: Color32,
    pub(super) exit: Color32,
    pub(super) locked: Color32,
    pub(super) mark: Color32,
}

/// Where the map is drawn: the point on screen of the room at `origin`,
/// how big a cell is, and the level shown.
pub(super) struct Grid {
    pub(super) centre: Pos2,
    pub(super) origin: (i32, i32),
    pub(super) cell: f32,
    pub(super) level: i32,
}

/// Where the player is, as text: for screen readers, and at a glance.
pub(super) fn heading(ui: &mut Ui, current: &Room) {
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
pub(super) fn area_choice(ui: &mut Ui, map: &Map, area: &mut String) {
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
pub(super) fn level_steps(ui: &mut Ui, level: &mut i32) {
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
pub(super) fn middle_of<'a>(rooms: impl Iterator<Item = &'a Room>) -> (i32, i32) {
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
pub(super) fn draw_exits(
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
pub(super) fn draw_rooms(
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
pub(super) fn room_tip(response: Response, pointed: Option<&Room>) {
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

impl Colours {
    pub(super) fn of(theme: Theme) -> Self {
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

impl Grid {
    /// Where a place of the map is on screen: screen y grows downwards,
    /// map y northwards.
    pub(super) fn on_screen(&self, pos: Pos) -> Pos2 {
        self.centre
            + vec2(
                (pos.x - self.origin.0) as f32,
                -(pos.y - self.origin.1) as f32,
            ) * self.cell
    }

    /// The cell of the map under a point on screen.
    pub(super) fn cell_under(&self, point: Pos2) -> Pos {
        let offset = (point - self.centre) / self.cell;
        Pos {
            x: self.origin.0 + offset.x.round() as i32,
            y: self.origin.1 - offset.y.round() as i32,
            z: self.level,
        }
    }

    /// Half the side of a room.
    pub(super) fn half(&self) -> f32 {
        self.cell * 0.28
    }
}
