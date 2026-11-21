//! Drawing the map: where places go on screen, the rooms and their exits,
//! and the controls over it.

use super::*;
use eframe::egui::Shape;
use eframe::egui::epaint::RectShape;
use eframe::egui::text::{LayoutJob, TextWrapping};

/// The cell size from which the rooms' names are written under them.
const NAMES_FROM: f32 = 60.0;

/// The map's colours, taken from the theme.
pub(super) struct Colours {
    /// A room's colour, when it has none of its own.
    room: Color32,
    avoided: Color32,
    /// The ring round the player's room, and its glow.
    current: Color32,
    glow: Color32,
    exit: Color32,
    locked: Color32,
    route: Color32,
    pub(super) mark: Color32,
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

/// What marks some rooms: those the player is or may be in, the one a
/// walk ends in, and the area shown, whose rooms are drawn in full.
pub(super) struct Marks<'a> {
    pub(super) here: &'a [RoomId],
    pub(super) destination: Option<RoomId>,
    pub(super) area: &'a str,
}

/// Where the player is, as text: for screen readers, and at a glance.
pub(super) fn heading(ui: &mut Ui, map: &Map, current: &Room) {
    let palette = theme::current();
    ui.label(
        RichText::new(&current.name)
            .font(theme::font(Weight::Medium, 14.0))
            .color(palette.text),
    );
    // The exits, each a chip: read as the word and the exits after it.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        let word = RichText::new(tr("exits"))
            .font(theme::font(Weight::Regular, 12.0))
            .color(palette.text_faint);
        ui.add(eframe::egui::Label::new(word).selectable(false));
        for exit in current.exits.keys() {
            eframe::egui::Frame::new()
                .fill(palette.raised)
                .corner_radius(eframe::egui::CornerRadius::same(5))
                .inner_margin(eframe::egui::Margin::symmetric(6, 1))
                .show(ui, |ui| {
                    let text = RichText::new(map.command_for(exit))
                        .font(FontId::monospace(11.5))
                        .color(palette.text_dim);
                    ui.add(eframe::egui::Label::new(text).selectable(false));
                });
        }
    });
    ui.add_space(4.0);
}

/// The area to show, among those the map has.
pub(super) fn area_choice(ui: &mut Ui, map: &Map, area: &mut String) {
    let chooser = crate::widgets::dropdown("map area")
        .selected_text(area.as_str())
        .width(120.0)
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
    if crate::widgets::sign_button(ui, "−", tr("Down"), false).clicked() {
        *level -= 1;
    }
    let shown = format!("{} {level}", tr("level"));
    ui.label(
        RichText::new(shown)
            .font(theme::font(Weight::Regular, 12.5))
            .color(theme::current().text_dim),
    );
    if crate::widgets::sign_button(ui, "+", tr("Up"), false).clicked() {
        *level += 1;
    }
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
    for room in visible {
        let from = grid.on_screen(room.pos);
        for (direction, exit) in &room.exits {
            let step = direction_offset(direction);
            if step.is_some_and(|(_, _, dz)| dz != 0) {
                continue;
            }
            let colour = if exit.locked {
                colours.locked
            } else {
                colours.exit
            };
            let line = Stroke::new(width, colour);
            let target = (map.room(exit.to)).filter(|to| on_level(to) && to.id != room.id);
            match (target, step) {
                (Some(target), _) => {
                    let to = grid.on_screen(target.pos);
                    if step.is_some() {
                        painter.line_segment([from, to], line);
                    } else {
                        let dash = grid.half() * 0.5;
                        painter.extend(Shape::dashed_line(&[from, to], line, dash, dash));
                    }
                    if !target.exits.values().any(|back| back.to == room.id) {
                        arrow(painter, from, to, grid, colour);
                    }
                }
                (None, Some((dx, dy, _))) => {
                    let stub = vec2(dx as f32, -dy as f32) * grid.cell * 0.5;
                    painter.line_segment([from, from + stub], line);
                }
                (None, None) => {}
            }
        }
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
/// of a walk ringed, avoided ones struck through, their exits up, down
/// and off the compass marked inside, and those of other areas, with
/// every area on show, faint. The player's are drawn last, over any room
/// in the same place.
pub(super) fn draw_rooms(
    painter: &Painter,
    visible: &[&Room],
    grid: &Grid,
    colours: &Colours,
    marks: &Marks<'_>,
) {
    let (half, corner) = (grid.half(), grid.corner());
    let is_here = |room: &&&Room| marks.here.contains(&room.id);
    let elsewhere = visible.iter().filter(|room| !is_here(room));
    for room in elsewhere.chain(visible.iter().filter(is_here)) {
        let rect = Rect::from_center_size(grid.on_screen(room.pos), Vec2::splat(half * 2.0));
        let is_here = marks.here.contains(&room.id);
        if is_here && marks.here.len() == 1 {
            let glow = RectShape::filled(rect.expand(half * 0.3), corner * 2.0, colours.glow);
            painter.add(glow.with_blur_width(half * 1.4));
        }
        let mut fill = (room.colour).map_or(colours.room, |[red, green, blue]| {
            Color32::from_rgb(red, green, blue)
        });
        if room.area != marks.area {
            fill = theme::mix(fill, colours.behind, 0.55);
        }
        let border = match room.avoid {
            true => Stroke::new(1.5, colours.avoided),
            false => Stroke::new(1.0, theme::mix(fill, Color32::BLACK, 0.45)),
        };
        painter.rect(rect, corner, fill, border, StrokeKind::Inside);
        if room.avoid {
            let inset = rect.shrink(half * 0.3);
            let strike = Stroke::new(grid.line_width() + 0.5, colours.avoided);
            painter.line_segment([inset.left_bottom(), inset.right_top()], strike);
        }
        draw_exit_marks(painter, room, rect, fill);
        let ring = match (is_here, marks.destination == Some(room.id)) {
            (true, _) => Some(colours.current),
            (false, true) => Some(colours.route),
            (false, false) => None,
        };
        if let Some(ring) = ring {
            let stroke = Stroke::new(2.0, ring);
            painter.rect_stroke(rect.expand(2.5), corner + 2.5, stroke, StrokeKind::Outside);
        }
    }
}

/// Marks inside a room for its exits that no line shows: a triangle up,
/// one down, and a dot for those off the compass, in black or white,
/// whichever stands out from the room. Too small a room has none.
fn draw_exit_marks(painter: &Painter, room: &Room, rect: Rect, fill: Color32) {
    let half = rect.width() / 2.0;
    if half < 4.0 {
        return;
    }
    let contrast = |ink| crate::palette::contrast(fill, ink);
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
    if other {
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
            room: theme::mix(theme.surface, theme.text_dim, 0.5),
            avoided: theme.danger,
            current: theme.text,
            glow: theme.accent.gamma_multiply(0.75),
            exit: theme::mix(theme.surface, theme.text_dim, 0.7),
            locked: theme.danger,
            route: theme.accent.gamma_multiply(0.8),
            mark: theme.text_dim,
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
