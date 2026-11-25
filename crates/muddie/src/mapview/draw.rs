//! Drawing the map: where places go on screen, the rooms and their exits,
//! and the controls over it.

use super::*;
use eframe::egui::epaint::RectShape;
use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{Align2, Shape};
use muddie_mapper::{Appearance, Door, Shape as RoomShape};

/// The cell size from which the rooms' names are written under them.
const NAMES_FROM: f32 = 60.0;
/// The cell size from which exits are drawn with their arrows and dashes.
const DETAILED_FROM: f32 = 16.0;

/// The map's colours, taken from the theme.
pub(super) struct Colours {
    /// A room's colour, when it has none of its own.
    room: Color32,
    avoided: Color32,
    /// The ring round the player's room, and its glow.
    current: Color32,
    glow: Color32,
    exit: Color32,
    blocked: Color32,
    route: Color32,
    /// The ring round the room chosen to look at.
    selected: Color32,
    /// The outline of where a room being moved would go.
    pub(super) moving: Color32,
    name: Color32,
    name_behind: Color32,
    /// Behind the map, which the rooms of other areas fade into.
    behind: Color32,
}

/// Where the map is drawn: the point on screen of the room at `origin`,
/// how big a cell is, and the level shown.
pub(super) struct Grid {
    pub(super) centre: Pos2,
    pub(super) origin: (i32, i32),
    pub(super) cell: f32,
    pub(super) level: i32,
}

/// What sets some rooms apart: those the player is or may be in, the one
/// a walk ends in, and the area shown, whose rooms are drawn in full.
pub(super) struct Highlights<'a> {
    pub(super) here: &'a [RoomId],
    pub(super) destination: Option<RoomId>,
    pub(super) selected: Option<RoomId>,
    pub(super) area: &'a str,
}

/// The middle of the rooms given, to centre on.
pub(super) fn middle_of(places: impl Iterator<Item = Pos>) -> (i32, i32) {
    let spans = places.fold(None, |spans, pos| {
        let ((x0, x1), (y0, y1)) = spans.unwrap_or(((i32::MAX, i32::MIN), (i32::MAX, i32::MIN)));
        Some((
            (x0.min(pos.x), x1.max(pos.x)),
            (y0.min(pos.y), y1.max(pos.y)),
        ))
    });
    spans.map_or((0, 0), |((x0, x1), (y0, y1))| {
        ((x0 + x1) / 2, (y0 + y1) / 2)
    })
}

/// The exits of the rooms shown: a line to a room shown, with an arrow
/// at its end if there is no way back, dashed for an exit off the
/// compass, and a stub the way of a compass exit to a room not shown.
/// Exits up, down and off the compass are marked on their room besides.
pub(super) fn draw_exits(
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

/// The rest of a walk: a line through the rooms it is yet to go through,
/// broken where it leaves the level shown.
pub(super) fn draw_route(
    painter: &Painter,
    map: &Map,
    rooms: impl Iterator<Item = RoomId>,
    grid: &Grid,
    colours: &Colours,
    on_level: &dyn Fn(&Room) -> bool,
) {
    let stroke = Stroke::new((grid.half() * 0.4).clamp(2.0, 6.0), colours.route);
    let mut stretch = Vec::new();
    for room in rooms.map(|id| map.room(id).filter(|room| on_level(room))) {
        match room {
            Some(room) => stretch.push(grid.on_screen(room.pos)),
            None => draw_stretch(painter, &mut stretch, stroke),
        }
    }
    draw_stretch(painter, &mut stretch, stroke);
}

/// Draws a stretch of a route, if it joins two rooms or more, and starts
/// the next.
fn draw_stretch(painter: &Painter, stretch: &mut Vec<Pos2>, stroke: Stroke) {
    let points = std::mem::take(stretch);
    if points.len() > 1 {
        painter.line(points, stroke);
    }
}

/// The rooms shown, each in its colour: the player's glowing and ringed
/// (each they may be in, only ringed, while the map is unsure), the end
/// of a walk ringed, avoided ones struck through, those the map added
/// of itself dashed round, their exits up, down
/// and off the compass marked inside, and those of other areas, with
/// every area on show, faint. The player's are drawn last, over any room
/// in the same place.
pub(super) fn draw_rooms(
    painter: &Painter,
    visible: &[&Room],
    grid: &Grid,
    colours: &Colours,
    shown: &Highlights<'_>,
    appearance: Appearance,
) {
    let (half, corner) = (grid.half(), grid.corner());
    let rgb = |[red, green, blue]: muddie_mapper::Colour| Color32::from_rgb(red, green, blue);
    let usual = appearance.colour.map_or(colours.room, rgb);
    let is_here = |room: &&&Room| shown.here.contains(&room.id);
    let elsewhere = visible.iter().filter(|room| !is_here(room));
    for room in elsewhere.chain(visible.iter().filter(is_here)) {
        let rect = Rect::from_center_size(grid.on_screen(room.pos), Vec2::splat(half * 2.0));
        let is_here = shown.here.contains(&room.id);
        if is_here && shown.here.len() == 1 {
            let glow = RectShape::filled(rect.expand(half * 0.3), corner * 2.0, colours.glow);
            painter.add(glow.with_blur_width(half * 1.4));
        }
        let mut fill = room.colour.map_or(usual, rgb);
        let shape = room.shape.unwrap_or(appearance.shape);
        if room.area != shown.area {
            fill = theme::mix(fill, colours.behind, 0.55);
        }
        let border = match room.avoid {
            true => Stroke::new(1.5, colours.avoided),
            false => Stroke::new(1.0, theme::mix(fill, Color32::BLACK, 0.45)),
        };
        if room.added && !room.avoid {
            // A room the map added of itself, which no map taken has:
            // dashed round, in the colour of exits, to tell it apart.
            fill_shape(painter, shape, rect, corner, fill);
            let line = Stroke::new(1.5, colours.exit);
            let dash = (half * 0.35).max(2.0);
            let mut points = outline(shape, rect.expand(1.5));
            points.push(points[0]);
            painter.extend(Shape::dashed_line(&points, line, dash, dash));
        } else {
            fill_shape(painter, shape, rect, corner, fill);
            ring_shape(painter, shape, rect, corner, border);
        }
        if room.avoid {
            let inset = rect.shrink(half * 0.3);
            let strike = Stroke::new(grid.line_width() + 0.5, colours.avoided);
            painter.line_segment([inset.left_bottom(), inset.right_top()], strike);
        }
        draw_exit_marks(painter, room, rect, fill);
        let ring = match (is_here, shown.destination == Some(room.id)) {
            (true, _) => Some(colours.current),
            (false, true) => Some(colours.route),
            (false, false) => None,
        };
        if let Some(ring) = ring {
            let stroke = Stroke::new(2.0, ring);
            ring_shape(painter, shape, rect.expand(3.5), corner + 3.5, stroke);
        }
        // The room chosen to look at, round whatever ring it has.
        if shown.selected == Some(room.id) {
            let stroke = Stroke::new(2.5, colours.selected);
            ring_shape(painter, shape, rect.expand(6.75), corner + 6.75, stroke);
        }
    }
}

/// The points round a room of a shape, in `rect`, clockwise from the top;
/// a circle as a polygon fine enough to look round.
fn outline(shape: RoomShape, rect: Rect) -> Vec<Pos2> {
    let (centre, half) = (rect.center(), rect.width() / 2.0);
    let round = |count: usize, from: f32| -> Vec<Pos2> {
        (0..count)
            .map(|at| {
                let angle = from + at as f32 * std::f32::consts::TAU / count as f32;
                centre + vec2(angle.cos(), angle.sin()) * half
            })
            .collect()
    };
    match shape {
        RoomShape::Square => vec![
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ],
        RoomShape::Circle => round(32, 0.0),
        RoomShape::Diamond => round(4, -std::f32::consts::FRAC_PI_2),
        RoomShape::Hexagon => round(6, 0.0),
    }
}

/// A room's shape filled.
fn fill_shape(painter: &Painter, shape: RoomShape, rect: Rect, corner: f32, fill: Color32) {
    match shape {
        RoomShape::Square => drop(painter.rect_filled(rect, corner, fill)),
        RoomShape::Circle => drop(painter.circle_filled(rect.center(), rect.width() / 2.0, fill)),
        _ => drop(painter.add(Shape::convex_polygon(
            outline(shape, rect),
            fill,
            Stroke::NONE,
        ))),
    }
}

/// A line round a room's shape.
fn ring_shape(painter: &Painter, shape: RoomShape, rect: Rect, corner: f32, stroke: Stroke) {
    match shape {
        RoomShape::Square => drop(painter.rect_stroke(rect, corner, stroke, StrokeKind::Inside)),
        RoomShape::Circle => drop(painter.circle_stroke(rect.center(), rect.width() / 2.0, stroke)),
        _ => drop(painter.add(Shape::closed_line(outline(shape, rect), stroke))),
    }
}

/// What is drawn inside a room: what is written in it, if anything, and for its
/// exits that no line shows a triangle up, one down, and a dot for those
/// off the compass (none where something is written), in black or white,
/// whichever stands out from the room. Too small a room has none.
fn draw_exit_marks(painter: &Painter, room: &Room, rect: Rect, fill: Color32) {
    let half = rect.width() / 2.0;
    if half < 4.0 {
        return;
    }
    let contrast = |ink| crate::output_style::contrast(fill, ink);
    let ink = if contrast(Color32::BLACK) > contrast(Color32::WHITE) {
        Color32::from_black_alpha(180)
    } else {
        Color32::from_white_alpha(220)
    };
    let (mut up, mut down, mut other) = (false, false, false);
    for direction in room.exits.keys() {
        match direction_offset(direction) {
            Some((_, _, 1..)) => up = true,
            Some((_, _, ..0)) => down = true,
            Some((_, _, 0)) => {}
            None => other = true,
        }
    }
    let (size, centre) = (half * 0.42, rect.center());
    for (shown, sign) in [(up, -1.0), (down, 1.0)] {
        if shown {
            let tip = centre + vec2(0.0, sign * half * 0.78);
            let base = tip - vec2(0.0, sign * size);
            let points = vec![tip, base + vec2(size, 0.0), base - vec2(size, 0.0)];
            painter.add(Shape::convex_polygon(points, ink, Stroke::NONE));
        }
    }
    if !room.symbol.is_empty() && half >= 6.0 {
        let font = theme::font(Weight::Semibold, half * 1.05);
        painter.text(centre, Align2::CENTER_CENTER, &room.symbol, font, ink);
    } else if other {
        painter.circle_filled(centre, half * 0.18, ink);
    }
}

/// The names of the rooms shown, each under its room, once rooms are drawn
/// large enough for them to be read; cut short to the room's cell.
pub(super) fn draw_names(painter: &Painter, visible: &[&Room], grid: &Grid, colours: &Colours) {
    if grid.cell < NAMES_FROM {
        return;
    }
    let font = theme::font(Weight::Regular, (grid.cell * 0.14).clamp(9.5, 12.5));
    for room in visible.iter().filter(|room| !room.name.is_empty()) {
        let mut job = LayoutJob::simple_singleline(room.name.clone(), font.clone(), colours.name);
        job.wrap = TextWrapping {
            max_width: grid.cell * 0.96,
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
        let galley = painter.layout_job(job);
        let top = grid.on_screen(room.pos) + vec2(0.0, grid.half() + 3.0);
        let at = top - vec2(galley.size().x / 2.0, 0.0);
        let backing = Rect::from_min_size(at, galley.size()).expand2(vec2(3.0, 1.0));
        painter.rect_filled(backing, 3.0, colours.name_behind);
        painter.galley(at, galley, colours.name);
    }
}

/// Over empty space the map says how it is used; over a room, which.
pub(super) fn room_tip(response: Response, map: &Map, pointed: Option<&Room>) {
    let Some(room) = pointed else {
        if response.hovered() && !response.dragged() {
            let hint = tr(
                "Click a room to see it, double-click to walk there, right-click for more, Shift+drag to move it.",
            );
            response.on_hover_text_at_pointer(hint);
        }
        return;
    };
    let mut parts = vec![numbered(map, room)];
    if room.added {
        parts.push(tr("added by the map").to_owned());
    }
    if room.avoid {
        parts.push(tr("kept off routes").to_owned());
    }
    if room.cost > 0 {
        parts.push(crate::i18n::trf(
            "costs {0} more to enter",
            &[&room.cost.to_string()],
        ));
    }
    let text = parts.join(tr(", "));
    response.on_hover_text_at_pointer(text);
}

impl Colours {
    pub(super) fn of(theme: Theme) -> Self {
        Self {
            // Opaque, so the exit lines do not show through the rooms.
            room: theme::mix(theme.surface, theme.text_dim, 0.5),
            avoided: theme.danger,
            current: theme.text,
            glow: theme.accent.gamma_multiply(0.75),
            exit: theme::mix(theme.surface, theme.text_dim, 0.7),
            blocked: theme.danger,
            route: theme.accent.gamma_multiply(0.8),
            selected: theme.accent,
            moving: theme.text_dim,
            name: theme.text_dim,
            name_behind: theme.surface.gamma_multiply(0.8),
            behind: theme.surface,
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

    /// How round a room's corners are.
    pub(super) fn corner(&self) -> f32 {
        self.half() * 0.35
    }

    /// How thick an exit's line is.
    fn line_width(&self) -> f32 {
        (self.cell / 24.0).clamp(1.0, 2.5)
    }
}
