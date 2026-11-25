//! Drawing the exits of the rooms: their lines, arrows and doors.

use super::*;
use muddie_mapper::Door;

/// The cell size from which exits are drawn with their arrows and dashes.
const DETAILED_FROM: f32 = 16.0;

/// The exits of the rooms shown: a line to a room shown, each half in the
/// colour of the way from the room it is next to, with an arrow at its end
/// if there is no way back, dashed for an exit off the compass, and a stub
/// the way of a compass exit to a room not shown; then the doors, over the
/// lines. Exits up, down and off the compass are marked on their room
/// besides.
pub(in crate::mapview) fn draw_exits(
    painter: &Painter,
    map: &Map,
    visible: &[&Room],
    grid: &Grid,
    colours: &Colours,
    on_level: &dyn Fn(&Room) -> bool,
) {
    let mut doors = Vec::new();
    for room in visible {
        let from = grid.on_screen(room.pos);
        for (direction, exit) in &room.exits {
            let step = direction_offset(direction);
            if is_vertical(step) {
                continue;
            }
            let target = (map.room(exit.to)).filter(|to| on_level(to) && to.id != room.id);
            let end = match (target, step) {
                (Some(target), _) => {
                    draw_link(painter, room, target, grid, colours);
                    grid.on_screen(target.pos)
                }
                (None, Some((dx, dy, _))) => {
                    let stub = vec2(dx as f32, -dy as f32) * grid.cell * 0.5;
                    let line = Stroke::new(grid.line_width(), colours.way(exit.blocked));
                    painter.line_segment([from, from + stub], line);
                    from + stub
                }
                (None, None) => continue,
            };
            doors.extend(exit.door.map(|door| (door, from, end)));
        }
    }
    if grid.is_detailed() {
        draw_doors(painter, &doors, grid, colours);
    }
}

/// Whether a step of the compass goes up or down.
fn is_vertical(step: Option<(i32, i32, i32)>) -> bool {
    step.is_some_and(|(_, _, dz)| dz != 0)
}

impl Grid {
    /// Zoomed far out, thousands of rooms on show, what cannot be told
    /// apart at that size is not drawn: arrow heads, dashes, doors, and the
    /// halves of a line.
    fn is_detailed(&self) -> bool {
        self.cell >= DETAILED_FROM
    }
}

impl Colours {
    /// The colour of a way between two rooms.
    fn way(&self, blocked: bool) -> Color32 {
        match blocked {
            true => self.blocked,
            false => self.exit,
        }
    }
}

/// The way from a room to another, by all the exits that lead there on the
/// level.
struct Way {
    /// None of them can be used.
    blocked: bool,
    /// None of them is of the compass.
    off_compass: bool,
}

impl Way {
    /// The way from `from` to `to`, if there is one.
    fn between(from: &Room, to: &Room) -> Option<Self> {
        (from.exits.iter())
            .filter(|(_, exit)| exit.to == to.id)
            .map(|(direction, exit)| (direction_offset(direction), exit.blocked))
            .filter(|(step, _)| !is_vertical(*step))
            .map(|(step, blocked)| Self {
                blocked,
                off_compass: step.is_none(),
            })
            .reduce(|one, other| Self {
                blocked: one.blocked && other.blocked,
                off_compass: one.off_compass && other.off_compass,
            })
    }
}

/// The line between two rooms, the same from whichever of them it is
/// drawn, with the head of an arrow if `room` has no way back from
/// `target`.
fn draw_link(painter: &Painter, room: &Room, target: &Room, grid: &Grid, colours: &Colours) {
    let (one, other) = match room.id <= target.id {
        true => (room, target),
        false => (target, room),
    };
    let ways = (Way::between(one, other), Way::between(other, one));
    let dashed = grid.is_detailed()
        && [&ways.0, &ways.1]
            .into_iter()
            .flatten()
            .all(|way| way.off_compass);
    let segment = |from: Pos2, to: Pos2, blocked: bool| {
        let line = Stroke::new(grid.line_width(), colours.way(blocked));
        if dashed {
            let dash = grid.half() * 0.5;
            painter.extend(Shape::dashed_line(&[from, to], line, dash, dash));
        } else {
            painter.line_segment([from, to], line);
        }
    };
    let (from, to) = (grid.on_screen(one.pos), grid.on_screen(other.pos));
    match &ways {
        (Some(there), Some(back)) if grid.is_detailed() && there.blocked != back.blocked => {
            let middle = from + (to - from) * 0.5;
            segment(from, middle, there.blocked);
            segment(middle, to, back.blocked);
        }
        (there, back) => {
            let blocked = [there, back].into_iter().flatten().any(|way| way.blocked);
            segment(from, to, blocked);
        }
    }
    let one_way = !target.exits.values().any(|back| back.to == room.id);
    if grid.is_detailed() && one_way {
        let blocked = Way::between(room, target).is_some_and(|way| way.blocked);
        let (from, to) = (grid.on_screen(room.pos), grid.on_screen(target.pos));
        arrow(painter, from, to, grid, colours.way(blocked));
    }
}

/// The doors, each from its room to where it leads: the more shut over the
/// less, for of two in a place it is what is in the way.
fn draw_doors(painter: &Painter, doors: &[(Door, Pos2, Pos2)], grid: &Grid, colours: &Colours) {
    for shut in [Door::Open, Door::Closed, Door::Locked] {
        for (door, from, to) in doors.iter().filter(|(door, ..)| *door == shut) {
            draw_door(painter, *door, *from, *to, grid, colours);
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

#[cfg(test)]
mod tests;
