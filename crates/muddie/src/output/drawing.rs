//! Drawing the output: its rows measured a share a frame, those on screen
//! drawn, and what their links do.

use super::*;

/// How many rows are laid out per frame only to learn their height: new
/// ones, and after the width changed, the ones measured for the old one.
pub const REMEASURED_PER_FRAME: usize = 300;

/// Where the rows are: the first inside the viewport and its top, the top
/// and height of the row asked to be scrolled to, and how tall they all
/// are.
pub(super) struct Placed {
    pub(super) first_visible: Option<(usize, f32)>,
    pub(super) target: Option<(f32, f32)>,
    pub(super) height: f32,
}

/// Lays text out to the width given.
pub(super) fn lay_out(ui: &Ui, mut job: LayoutJob, width: f32) -> Arc<Galley> {
    job.wrap.max_width = width;
    ui.painter().layout_job(job)
}

/// What the links of a row on screen do: a click opens an address, sends
/// a command, puts one in the command line, or sends the first a menu
/// offers; the other button shows a menu's commands, to choose one; and
/// pointed at, a link says what it is for. Returns what was asked for.
pub(super) fn row_links(
    ui: &Ui,
    response: &eframe::egui::Response,
    row: &Row,
    galley: &Galley,
    timestamps: bool,
    serial: u64,
    menu: &mut Option<(u64, Vec<(String, String)>)>,
) -> Option<Clicked> {
    let link_under = |pointer: eframe::egui::Pos2| {
        let cursor = galley.cursor_from_pos(pointer - response.rect.min);
        row.link(cursor.index.0, timestamps)
    };
    let mut command = None;
    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        match link_under(pointer).map(|link| &link.target) {
            Some(LinkTarget::Url(address)) => ui.ctx().open_url(OpenUrl::new_tab(address)),
            Some(LinkTarget::Send(text)) => command = Some(Clicked::Send(text.clone())),
            Some(LinkTarget::Prompt(text)) => command = Some(Clicked::Prompt(text.clone())),
            // The first of several is the one a click sends.
            Some(LinkTarget::Menu(items)) => {
                command = items.first().map(|(_, text)| Clicked::Send(text.clone()))
            }
            None => {}
        }
    }
    if response.secondary_clicked()
        && let Some(pointer) = response.interact_pointer_pos()
        && let Some(LinkTarget::Menu(items)) = link_under(pointer).map(|link| &link.target)
    {
        *menu = Some((serial, items.clone()));
    }
    if let Some((of, items)) = menu.as_ref()
        && *of == serial
    {
        response.context_menu(|ui| {
            crate::widgets::menu_width(ui);
            for (called, text) in items {
                if crate::widgets::menu_item(ui, called, "").clicked() {
                    command = Some(Clicked::Send(text.clone()));
                    ui.close();
                }
            }
        });
    }
    if response.hovered()
        && let Some(pointer) = response.hover_pos()
        && let Some(link) = link_under(pointer)
        && !link.hint.is_empty()
    {
        response.clone().on_hover_text(&link.hint);
    }
    command
}

impl Output {
    /// Draws the rows that are on screen, and what is not yet a whole line,
    /// in a space as tall as all of them. Returns what a click on a link
    /// asked for.
    pub(super) fn show_rows(&mut self, ui: &mut Ui, viewport: Rect) -> Option<Clicked> {
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width();
        self.follow_width(ui, width);
        self.measure_some(ui, width);
        let guess = self.guesser(ui, width);
        let scroll_to = self.scroll_to.take();
        let Placed {
            first_visible,
            target,
            height: rows_height,
        } = self.place_rows(viewport, scroll_to, &guess);
        // Rows on screen are never left with a height from another width.
        if let Some((first, _)) = first_visible {
            self.forget_heights_from(first);
        }
        let partial = self
            .partial
            .as_ref()
            .map(|line| lay_out(ui, server_job(line), width));
        let total = rows_height + partial.as_ref().map_or(0.0, |galley| galley.size().y);
        ui.set_height(total);
        // The viewport was placed against last frame's content: lines that
        // arrived since do not mean the reader scrolled back.
        self.at_end = viewport.max.y >= self.shown_height.min(total) - 2.0;
        self.shown_height = total;
        self.scroll_as_asked(ui, total, target);
        let (first, top) = first_visible.unwrap_or((self.rows.len(), rows_height));
        let max_rect = ui.max_rect();
        let rect = Rect::from_min_size(
            max_rect.min + vec2(0.0, top),
            vec2(max_rect.width(), total - top),
        );
        let (command, learned) = self.draw_rows(ui, rect, first, top, viewport, partial);
        if !learned.is_empty() {
            ui.ctx().request_repaint();
        }
        for (index, height) in learned {
            self.rows[index].height = Some(height);
        }
        command
    }

    /// At a new width every height is for another: measuring them all
    /// again at once would stall each frame of a window being resized, so
    /// it is done a share at a time, newest first.
    pub(super) fn follow_width(&mut self, ui: &Ui, width: f32) {
        if width != self.width {
            let first = self.width == 0.0;
            self.width = width;
            self.stale = self.rows.len();
            if first {
                self.rows.iter_mut().for_each(|row| row.height = None);
                self.stale = 0;
            }
        }
        self.stale = self.stale.min(self.rows.len());
        let from = self.stale.saturating_sub(REMEASURED_PER_FRAME);
        for row in self.rows.range_mut(from..self.stale) {
            row.height = None;
        }
        self.stale = from;
        if self.stale > 0 {
            ui.ctx().request_repaint();
        }
    }

    /// Rows whose height is not known are laid out to learn it a share a
    /// frame, newest first. All at once, a burst of text or a whole
    /// scrollback would hold a laid out copy of every row for a frame:
    /// hundreds of megabytes that the system does not get back.
    pub(super) fn measure_some(&mut self, ui: &Ui, width: f32) {
        let timestamps = self.timestamps;
        let mut budget = REMEASURED_PER_FRAME;
        for row in self.rows.iter_mut().rev() {
            if budget == 0 {
                ui.ctx().request_repaint();
                break;
            }
            if row.height.is_none() {
                let galley = lay_out(ui, row.job(timestamps), width);
                if galley.rows.len() == 1 {
                    self.unit = galley.size().y;
                }
                row.height = Some(galley.size().y);
                budget -= 1;
            }
        }
    }

    /// How tall a row not yet measured is taken to be: as many rows of text
    /// as its characters fill at this width.
    pub(super) fn guesser(&self, ui: &Ui, width: f32) -> impl Fn(&Row) -> f32 + use<> {
        let font = palette::font();
        let (glyph, plain) =
            ui.fonts_mut(|fonts| (fonts.glyph_width(&font, 'm'), fonts.row_height(&font)));
        let unit = if self.unit > 0.0 { self.unit } else { plain };
        let timestamps = self.timestamps;
        move |row: &Row| {
            let characters = row.text.chars().count() + if timestamps { 9 } else { 0 };
            unit * (characters as f32 * glyph / width.max(1.0)).ceil().max(1.0)
        }
    }

    /// Where the rows are, those not yet measured by a guess.
    pub(super) fn place_rows(
        &self,
        viewport: Rect,
        scroll_to: Option<usize>,
        guess: &dyn Fn(&Row) -> f32,
    ) -> Placed {
        let (mut total, mut first_visible, mut target) = (0.0, None, None);
        for (index, row) in self.rows.iter().enumerate() {
            let height = row.height.unwrap_or_else(|| guess(row));
            if first_visible.is_none() && total + height > viewport.min.y {
                first_visible = Some((index, total));
            }
            if scroll_to == Some(index) {
                target = Some((total, height));
            }
            total += height;
        }
        Placed {
            first_visible,
            target,
            height: total,
        }
    }

    /// Forgets the heights from another width of the rows about to be
    /// shown, from `first`.
    pub(super) fn forget_heights_from(&mut self, first: usize) {
        if first < self.stale {
            let end = (first + 200).min(self.stale);
            for row in self.rows.range_mut(first..end) {
                row.height = None;
            }
        }
    }

    /// Asking the scroll area to show a place is what lets it leave the
    /// end it otherwise sticks to, and go back to it.
    pub(super) fn scroll_as_asked(&mut self, ui: &mut Ui, total: f32, target: Option<(f32, f32)>) {
        let origin = ui.max_rect().min;
        if std::mem::take(&mut self.jump_to_end) {
            let end = Rect::from_min_size(origin + vec2(0.0, total - 1.0), vec2(1.0, 1.0));
            ui.scroll_to_rect(end, Some(Align::BOTTOM));
        } else if let Some((top, height)) = target {
            let row = Rect::from_min_size(origin + vec2(0.0, top), vec2(1.0, height));
            ui.scroll_to_rect(row, Some(Align::Center));
        }
    }

    /// Draws the rows from `first`, at `top`, until past the viewport,
    /// then what is not yet a whole line. Returns what a click asked for,
    /// and the heights learned of rows drawn before their turn to be
    /// measured.
    pub(super) fn draw_rows(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        first: usize,
        top: f32,
        viewport: Rect,
        partial: Option<Arc<Galley>>,
    ) -> (Option<Clicked>, Vec<(usize, f32)>) {
        let (timestamps, found, width) = (self.timestamps, self.found, self.width);
        let first_serial = self.pushed - self.rows.len() as u64;
        // The commands a link offers, shown by the other button, and the
        // row they are of: it is the row's menu while it is open.
        let mut menu = self.menu.clone();
        let (mut command, mut learned) = (None, Vec::new());
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            // Keep widget ids stable while scrolling.
            ui.skip_ahead_auto_ids(first);
            let mut y = top;
            for (index, row) in self.rows.iter().enumerate().skip(first) {
                if y > viewport.max.y {
                    return;
                }
                let galley = lay_out(ui, row.job(timestamps), width);
                y += galley.size().y;
                if row.height.is_none() {
                    learned.push((index, galley.size().y));
                }
                let mut label = Label::new(Arc::clone(&galley));
                if !row.links.is_empty() {
                    label = label.sense(Sense::click());
                }
                let response = ui.add(label);
                if found == Some(index) {
                    let mark = theme::current().accent.gamma_multiply(0.22);
                    ui.painter()
                        .rect_filled(response.rect.expand2(vec2(2.0, 0.0)), 2.0, mark);
                }
                let serial = first_serial + index as u64;
                if let Some(asked) =
                    row_links(ui, &response, row, &galley, timestamps, serial, &mut menu)
                {
                    command = Some(asked);
                }
            }
            if let Some(galley) = partial {
                ui.add(Label::new(galley));
            }
        });
        self.menu = menu;
        (command, learned)
    }
}
