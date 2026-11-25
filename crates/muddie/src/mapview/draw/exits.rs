//! Drawing the exits of the rooms: their lines, arrows and doors.

use super::*;
use muddie_mapper::Door;

/// The cell size from which exits are drawn with their arrows and dashes.
const DETAILED_FROM: f32 = 16.0;

/// The exits of the rooms shown: a line to a room shown, with an arrow
/// at its end if there is no way back, dashed for an exit off the
/// compass, and a stub the way of a compass exit to a room not shown.
/// Exits up, down and off the compass are marked on their room besides.
pub(in crate::mapview) fn draw_exits(
    painter: &Painter,
    map: &Map,
    visible: &[&Room],
    grid: &Grid,
    colours: &Colours,
    on_level: &dyn Fn(&Room) -> bool,
) {
    let width = grid.line_width();
    // Zoomed far out, thousands of rooms on show, what cannot be told
    // apart at that size is not drawn: arrow heads, and dashes.
    let detailed = grid.cell >= DETAILED_FROM;
    for room in visible {
        let from = grid.on_screen(room.pos);
        for (direction, exit) in &room.exits {
            let step = direction_offset(direction);
            if step.is_some_and(|(_, _, dz)| dz != 0) {
                continue;
            }
            let colour = if exit.blocked {
                colours.blocked
            } else {
                colours.exit
            };
            let line = Stroke::new(width, colour);
            let target = (map.room(exit.to)).filter(|to| on_level(to) && to.id != room.id);
            let end = match (target, step) {
                (Some(target), _) => {
                    let to = grid.on_screen(target.pos);
                    if step.is_some() || !detailed {
                        painter.line_segment([from, to], line);
                    } else {
                        let dash = grid.half() * 0.5;
                        painter.extend(Shape::dashed_line(&[from, to], line, dash, dash));
                    }
                    if detailed && !target.exits.values().any(|back| back.to == room.id) {
                        arrow(painter, from, to, grid, colour);
                    }
                    Some(to)
                }
                (None, Some((dx, dy, _))) => {
                    let stub = vec2(dx as f32, -dy as f32) * grid.cell * 0.5;
                    painter.line_segment([from, from + stub], line);
                    Some(from + stub)
                }
                (None, None) => None,
            };
            if let (Some(door), Some(end), true) = (exit.door, end, detailed) {
                draw_door(painter, door, from, end, grid, colours);
            }
        }
    }
}

/// A door on an exit, between its room and where it leads, as a small
/// square on the line: hollow while open, filled while shut, in the colour
/// of exits blocked to routes.
fn draw_door(painter: &Painter, door: Door, from: Pos2, to: Pos2, grid: &Grid, colours: &Colours) {
    let along = (to - from).normalized();
    let at = from + along * (grid.half() + (grid.cell - grid.half() * 2.0) * 0.5);
    let square = Rect::from_center_size(at, Vec2::splat((grid.half() * 0.55).max(4.0)));
    match door {
        Door::Open => {
            painter.rect_filled(square, 1.0, colours.behind);
            painter.rect_stroke(
                square,
                1.0,
                Stroke::new(1.5, colours.exit),
                StrokeKind::Inside,
            );
        }
        Door::Closed => drop(painter.rect_filled(square, 1.0, colours.exit)),
        Door::Locked => drop(painter.rect_filled(square, 1.0, colours.blocked)),
    }
}

/// The head of an arrow from `from` to `to`, at the edge of the room it
/// points to.
fn arrow(painter: &Painter, from: Pos2, to: Pos2, grid: &Grid, colour: Color32) {
    let along = (to - from).normalized();
    let across = along.rot90();
    let size = (grid.half() * 0.6).max(3.5);
    let tip = to - along * (grid.half() + 2.0);
    let back = tip - along * size;
    let points = vec![
        tip,
        back + across * size * 0.55,
        back - across * size * 0.55,
    ];
    painter.add(Shape::convex_polygon(points, colour, Stroke::NONE));
}
