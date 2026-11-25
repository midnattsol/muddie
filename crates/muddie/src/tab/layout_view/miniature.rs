//! The picture of the window in the layout view: where each place of panels is,
//! choosing and dragging panels in it, and where a dragged one would go.

use super::*;

impl Tab {
    /// A picture of the window as the panels are, the one chosen marked.
    /// Choosing a panel in it chooses it in the list, and dragging it puts
    /// it elsewhere: it is lifted, follows the pointer over the window
    /// without it, where it would go marked, and goes there when let go;
    /// Escape puts it back. A screen reader is told what it is, and has the
    /// list for what it shows and does.
    pub(super) fn miniature(&mut self, ui: &mut egui::Ui) {
        let screen = ui.ctx().content_rect();
        let width = ui.available_width();
        let height = (width * screen.height() / screen.width().max(1.0)).clamp(120.0, 360.0);
        let sense = egui::Sense::CLICK | egui::Sense::DRAG;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), sense);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Image,
                true,
                tr("Picture of the window's panels"),
            )
        });
        let scale = rect.width() / screen.width().max(1.0);
        let now = blocks(&self.layout, rect, scale);
        let pointer = response
            .interact_pointer_pos()
            .or(ui.ctx().pointer_latest_pos());
        if response.drag_started() {
            self.take_panel(ui, pointer, &now);
        }
        let escape =
            ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if escape && self.layout_view.dragged.take().is_some() {
            self.layout_view.aim = None;
            self.voice.say_now(tr("Not moved."));
        }
        let mut drawn = now.clone();
        let mut lifted = None;
        if let (Some(id), Some(pointer)) = (self.layout_view.dragged, pointer) {
            let (base, onto, nearest) = self.aim_panel(id, pointer, rect, scale);
            if response.drag_stopped() || !ui.input(|input| input.pointer.any_down()) {
                self.put_panel(id, onto, nearest);
                drawn = blocks(&self.layout, rect, scale);
            } else {
                lifted = Some(self.lifted_panel(id, pointer, (onto, nearest), rect, &now));
                drawn = base;
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
        } else if response.hovered() && pointer.and_then(|point| place_at(&now, point)).is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        self.draw_miniature(ui, rect, &drawn, lifted);
        if response.clicked()
            && let Some((_, ids, _)) = pointer.and_then(|point| place_at(&now, point))
        {
            self.layout_view.chosen = ids.first().copied();
        }
    }

    /// Lifts the panel taken where the press was, not where the pointer is
    /// once it has moved enough to drag: the one chosen, if it is in the
    /// place taken, else the first there.
    fn take_panel(&mut self, ui: &egui::Ui, pointer: Option<egui::Pos2>, now: &Blocks) {
        let origin = ui.input(|input| input.pointer.press_origin()).or(pointer);
        if let Some((block, ids, _)) = origin.and_then(|point| place_at(now, point)) {
            let taken = (self.layout_view.chosen)
                .filter(|id| ids.contains(id))
                .unwrap_or(ids[0]);
            self.layout_view.chosen = Some(taken);
            self.layout_view.dragged = Some(taken);
            self.layout_view.grab = origin.map_or(egui::Vec2::ZERO, |origin| origin - block.min);
            self.layout_view.aim = None;
        }
    }

    /// Settles where the dragged panel is aimed at, over the window
    /// without it. Returns that picture of the window without it, the
    /// panel it is aimed at, if any, and the nearest side.
    fn aim_panel(
        &mut self,
        id: u32,
        pointer: egui::Pos2,
        rect: egui::Rect,
        scale: f32,
    ) -> (Blocks, Option<DropTarget>, Side) {
        // The window without it: what it is aimed at is what is seen.
        let mut without = self.layout.clone();
        if let Some(panel) = without.panels.iter_mut().find(|panel| panel.id == id) {
            panel.visible = false;
        }
        let base = blocks(&without, rect, scale);
        let rects: Vec<(u32, egui::Rect)> = (base.iter())
            .flat_map(|(block, ids, _)| ids.iter().map(|id| (*id, *block)))
            .collect();
        let aimed = (
            without.drop_target(id, pointer, &rects),
            layout::nearest_side(rect, pointer),
        );
        self.layout_view.aim = Some(settled(self.layout_view.aim, aimed, pointer));
        let (onto, nearest) = self.layout_view.aim.unwrap_or(aimed);
        (base, onto, nearest)
    }

    /// Lets the dragged panel go where it is aimed at, and says where.
    fn put_panel(&mut self, id: u32, onto: Option<DropTarget>, nearest: Side) {
        self.layout_view.dragged = None;
        self.layout_view.aim = None;
        self.drop_panel(id, onto, nearest);
        let side = self.layout.panel(id).map_or(nearest, |panel| panel.side);
        self.voice.say_now(&trf("Moved: {0}.", &[side_name(side)]));
    }

    /// The panel being dragged, as it is drawn lifted: where its corner
    /// is, its size, the band where it would go, and its name.
    fn lifted_panel(
        &self,
        id: u32,
        pointer: egui::Pos2,
        (onto, nearest): (Option<DropTarget>, Side),
        rect: egui::Rect,
        now: &Blocks,
    ) -> Lifted {
        let size = (now.iter())
            .find(|(_, ids, _)| ids.contains(&id))
            .map_or(egui::vec2(60.0, 40.0), |(block, _, _)| block.size());
        let size = size.min(rect.size() * 0.45).max(egui::vec2(48.0, 28.0));
        let band = onto.map_or_else(|| layout::side_band(rect, nearest), |onto| onto.band());
        let name = self
            .layout
            .panel(id)
            .map(PanelSpec::shown_name)
            .unwrap_or_default();
        (pointer - self.layout_view.grab, size, band, name)
    }

    /// Draws the picture: each place gliding to where it is now, the one
    /// chosen marked, and a panel lifted, if one is, over where it would
    /// go.
    fn draw_miniature(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        drawn: &Blocks,
        lifted: Option<Lifted>,
    ) {
        let theme = theme::current();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, egui::CornerRadius::same(8), theme.field);
        self.draw_places(ui, &painter, &theme, rect, drawn, lifted.is_some());
        let Some(lifted) = lifted else {
            return;
        };
        draw_lifted(ui, &painter, &theme, rect, lifted);
    }

    /// Draws each place, gliding to where it is now, the one chosen marked
    /// unless a panel is lifted.
    fn draw_places(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        theme: &theme::Theme,
        rect: egui::Rect,
        drawn: &Blocks,
        lifting: bool,
    ) {
        // Each place glides from where it was, measured from the picture's
        // corner, so that scrolling does not drag it behind.
        let glide = |key: u32, part: usize, value: f32| {
            let id = ui.make_persistent_id(("miniature", key, part));
            ui.ctx().animate_value_with_time(id, value, 0.12)
        };
        let round = egui::CornerRadius::same(5);
        for (block, ids, name) in drawn {
            let key = ids.first().copied().unwrap_or(u32::MAX);
            let (from, to) = (block.min - rect.min, block.max - rect.min);
            let block = egui::Rect::from_min_max(
                rect.min + egui::vec2(glide(key, 0, from.x), glide(key, 1, from.y)),
                rect.min + egui::vec2(glide(key, 2, to.x), glide(key, 3, to.y)),
            );
            let chosen = !lifting && self.layout_view.chosen.is_some_and(|id| ids.contains(&id));
            let game = ids.is_empty();
            let fill = match (game, chosen) {
                (true, _) => theme.surface,
                (false, true) => theme.accent.gamma_multiply(0.35),
                (false, false) => theme.raised,
            };
            let inner = block.shrink(2.0);
            painter.rect_filled(inner, round, fill);
            if chosen {
                let edge = egui::Stroke::new(1.5, theme.accent);
                painter.rect_stroke(inner, round, edge, egui::StrokeKind::Inside);
            }
            let colour = if game {
                theme.text_faint
            } else {
                theme.text_dim
            };
            painter.with_clip_rect(inner.shrink(4.0)).text(
                inner.center(),
                egui::Align2::CENTER_CENTER,
                name,
                theme::font(Weight::Regular, 11.5),
                colour,
            );
        }
    }
}

/// Draws a panel lifted: where it would go, then the panel itself, over
/// all.
fn draw_lifted(
    ui: &egui::Ui,
    painter: &egui::Painter,
    theme: &theme::Theme,
    rect: egui::Rect,
    (corner, size, band, name): Lifted,
) {
    let round = egui::CornerRadius::same(5);
    let band = band.intersect(rect).shrink(2.0);
    painter.rect_filled(band, round, theme.accent.gamma_multiply(0.18));
    let edge = egui::Stroke::new(1.5, theme.accent.gamma_multiply(0.8));
    painter.rect_stroke(band, round, edge, egui::StrokeKind::Inside);
    let card = egui::Rect::from_min_size(corner, size);
    let card = card.translate(egui::vec2(
        (rect.left() - card.left()).max(0.0) + (rect.right() - card.right()).min(0.0),
        (rect.top() - card.top()).max(0.0) + (rect.bottom() - card.bottom()).min(0.0),
    ));
    let floating = ui.painter();
    floating.add(
        egui::epaint::Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: egui::Color32::from_black_alpha(90),
        }
        .as_shape(card, round),
    );
    floating.rect_filled(card, round, theme.accent.gamma_multiply(0.85));
    floating.text(
        card.center(),
        egui::Align2::CENTER_CENTER,
        name,
        theme::font(Weight::Medium, 11.5),
        theme.background,
    );
}

/// Where a dragged panel would go, settled: it changes to another panel's
/// part only once the pointer is well inside it, so that a hand that
/// shakes on a border does not make it come and go. Over no panel, the
/// nearest side is taken at once.
fn settled(
    before: Option<(Option<DropTarget>, Side)>,
    now: (Option<DropTarget>, Side),
    pointer: egui::Pos2,
) -> (Option<DropTarget>, Side) {
    const WELL_INSIDE: f32 = 6.0;
    match (before, now.0) {
        (Some((Some(was), side)), Some(aimed))
            if !was.same_place(&aimed) && !aimed.band().shrink(WELL_INSIDE).contains(pointer) =>
        {
            (Some(was), side)
        }
        _ => now,
    }
}

/// A panel being dragged, as it is drawn lifted: where its corner is, its
/// size, the band where it would go, and its name.
type Lifted = (egui::Pos2, egui::Vec2, egui::Rect, String);

/// The place of a picture of the window with panels in it that a point is
/// over, if any.
fn place_at(blocks: &Blocks, point: egui::Pos2) -> Option<&(egui::Rect, Vec<u32>, String)> {
    (blocks.iter()).find(|(block, ids, _)| !ids.is_empty() && block.contains(point))
}

/// The places of a picture of the window: each with its panels, none for
/// the game's, and its words.
type Blocks = Vec<(egui::Rect, Vec<u32>, String)>;

/// Where the panels on screen, and the game, are in a picture of the
/// window at `rect`, `scale` times its size: each place with its panels
/// and its words. The game's place has no panels.
fn blocks(layout: &layout::Layout, rect: egui::Rect, scale: f32) -> Blocks {
    let on: Vec<&PanelSpec> = (layout.panels.iter())
        .filter(|panel| layout.on_screen(panel))
        .collect();
    let depths = Depths::of(&on, rect, scale);
    let mut blocks = Vec::new();
    for side in SIDES {
        let places = side_places(layout, &on, side);
        let total: f32 = places.iter().map(|(_, _, weight)| weight).sum();
        let area = region(side, rect, &depths, layout.columns_full);
        let mut start = 0.0;
        for (ids, names, weight) in places {
            let (from, to) = (start / total, (start + weight) / total);
            start += weight;
            blocks.push((place_rect(side, area, from, to), ids, names.join(" | ")));
        }
    }
    let game = egui::Rect::from_min_max(
        egui::pos2(rect.left() + depths.left, rect.top() + depths.top),
        egui::pos2(rect.right() - depths.right, rect.bottom() - depths.bottom),
    );
    blocks.push((game, Vec::new(), tr("Game").to_owned()));
    blocks
}

/// How deep each side is in a picture of the window.
struct Depths {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl Depths {
    /// How deep each side is: as much as its largest panel, to a part of
    /// the picture.
    fn of(on: &[&PanelSpec], rect: egui::Rect, scale: f32) -> Self {
        let depth = |side: Side| {
            let most = (on.iter())
                .filter(|panel| panel.side == side)
                .map(|panel| panel.size)
                .fold(0.0_f32, f32::max);
            let limit = match side {
                Side::Left | Side::Right => rect.width() * 0.4,
                Side::Top | Side::Bottom => rect.height() * 0.4,
            };
            (most * scale).min(limit)
        };
        let (left, right, top, bottom) = (
            depth(Side::Left),
            depth(Side::Right),
            depth(Side::Top),
            depth(Side::Bottom),
        );
        Self {
            left,
            right,
            top,
            bottom,
        }
    }
}

/// The room of a side in a picture of the window at `rect`, those at the
/// left and right reaching its top and bottom when `full`.
fn region(side: Side, rect: egui::Rect, depths: &Depths, full: bool) -> egui::Rect {
    let Depths {
        left,
        right,
        top,
        bottom,
    } = *depths;
    match side {
        Side::Left => egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + if full { 0.0 } else { top }),
            egui::pos2(
                rect.left() + left,
                rect.bottom() - if full { 0.0 } else { bottom },
            ),
        ),
        Side::Right => egui::Rect::from_min_max(
            egui::pos2(
                rect.right() - right,
                rect.top() + if full { 0.0 } else { top },
            ),
            egui::pos2(
                rect.right(),
                rect.bottom() - if full { 0.0 } else { bottom },
            ),
        ),
        Side::Top => egui::Rect::from_min_max(
            egui::pos2(rect.left() + if full { left } else { 0.0 }, rect.top()),
            egui::pos2(
                rect.right() - if full { right } else { 0.0 },
                rect.top() + top,
            ),
        ),
        Side::Bottom => egui::Rect::from_min_max(
            egui::pos2(
                rect.left() + if full { left } else { 0.0 },
                rect.bottom() - bottom,
            ),
            egui::pos2(rect.right() - if full { right } else { 0.0 }, rect.bottom()),
        ),
    }
}

/// The places of a side, in order: a panel, or those sharing one as tabs,
/// each with its panels, their names and its weight.
fn side_places(
    layout: &layout::Layout,
    on: &[&PanelSpec],
    side: Side,
) -> Vec<(Vec<u32>, Vec<String>, f32)> {
    let mut places: Vec<(Vec<u32>, Vec<String>, f32)> = Vec::new();
    for panel in on.iter().filter(|panel| panel.side == side) {
        match places.last_mut() {
            Some((ids, names, _))
                if panel.stack != 0
                    && layout
                        .panel(ids[0])
                        .is_some_and(|first| first.stack == panel.stack) =>
            {
                ids.push(panel.id);
                names.push(panel.shown_name());
            }
            _ => places.push((
                vec![panel.id],
                vec![panel.shown_name()],
                panel.weight.max(0.1),
            )),
        }
    }
    places
}

/// The part of the room of a side that a place takes, from `from` to `to`
/// of its length.
fn place_rect(side: Side, area: egui::Rect, from: f32, to: f32) -> egui::Rect {
    match side {
        Side::Left | Side::Right => egui::Rect::from_x_y_ranges(
            area.x_range(),
            area.top() + area.height() * from..=area.top() + area.height() * to,
        ),
        Side::Top | Side::Bottom => egui::Rect::from_x_y_ranges(
            area.left() + area.width() * from..=area.left() + area.width() * to,
            area.y_range(),
        ),
    }
}
