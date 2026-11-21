//! Drawing the output: its rows measured a share a frame, those on screen
//! drawn, and what their links do.

use super::*;

/// How many rows are laid out per frame only to learn their height: new
/// ones, and after the width changed, the ones measured for the old one.
pub const REMEASURED_PER_FRAME: usize = 300;

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
        crate::widgets::context_menu_of(response, |ui| {
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
        self.add_up(ui, width);
        let scroll_to = self.scroll_to.take();
        let first = self.ends.first_below(viewport.min.y).min(self.rows.len());
        // Rows on screen are never left with a height from another width.
        if first < self.rows.len() && first < self.stale {
            self.forget_heights_from(first);
            self.add_up(ui, width);
        }
        let rows_height = self.ends.total();
        let partial = self
            .partial
            .as_ref()
            .map(|line| lay_out(ui, server_job(line), width));
        let total = rows_height + partial.as_ref().map_or(0.0, |galley| galley.size().y);
        ui.set_height(total);
        self.follow_reading(viewport, total);
        self.shown_height = total;
        let target = scroll_to.filter(|row| *row < self.rows.len()).map(|row| {
            (
                self.ends.top(row),
                self.ends.top(row + 1) - self.ends.top(row),
            )
        });
        self.scroll_as_asked(ui, total, target);
        let top = self.ends.top(first);
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
            self.set_height(index, Some(height));
        }
        command
    }

    /// Whether the reader is reading back: from when they scroll a third
    /// of the view away from the end, until they are back at it. The
    /// viewport was placed against last frame's content: lines that
    /// arrived since do not mean the reader scrolled.
    fn follow_reading(&mut self, viewport: Rect, total: f32) {
        let back = self.shown_height.min(total) - viewport.max.y;
        let far = (viewport.height() / 3.0).max(3.0 * self.unit);
        if back <= 2.0 {
            self.reading = false;
        } else if back > far {
            self.reading = true;
        }
    }

    /// Gives a row its height, or takes it away to be measured again,
    /// keeping count of those to measure and of where the rows end.
    pub(super) fn set_height(&mut self, index: usize, height: Option<f32>) {
        let row = &mut self.rows[index];
        match (row.height.is_some(), height.is_some()) {
            (true, false) => self.unmeasured += 1,
            (false, true) => self.unmeasured -= 1,
            _ => {}
        }
        if row.height != height {
            row.height = height;
            self.ends.changed_from(index);
        }
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
                for index in 0..self.rows.len() {
                    self.set_height(index, None);
                }
                self.stale = 0;
            }
        }
        self.stale = self.stale.min(self.rows.len());
        let from = self.stale.saturating_sub(REMEASURED_PER_FRAME);
        for index in from..self.stale {
            self.set_height(index, None);
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
        if self.unmeasured == 0 {
            return;
        }
        let timestamps = self.timestamps;
        let wanted = self.unmeasured.min(REMEASURED_PER_FRAME);
        let mut learned = Vec::with_capacity(wanted);
        for (index, row) in self.rows.iter().enumerate().rev() {
            if learned.len() == wanted {
                break;
            }
            if row.height.is_none() {
                let galley = lay_out(ui, row.job(timestamps), width);
                if galley.rows.len() == 1 {
                    self.unit = galley.size().y;
                }
                learned.push((index, galley.size().y));
            }
        }
        for (index, height) in learned {
            self.set_height(index, Some(height));
        }
        if self.unmeasured > 0 {
            ui.ctx().request_repaint();
        }
    }

    /// Adds up where the rows end again, from the first that changed. Rows
    /// not yet measured are taken to be as many rows of text as their
    /// characters fill at this width; when what that guess is made with
    /// changes, they are all added up again.
    fn add_up(&mut self, ui: &Ui, width: f32) {
        let font = palette::font();
        let (glyph, plain) =
            ui.fonts_mut(|fonts| (fonts.glyph_width(&font, 'm'), fonts.row_height(&font)));
        let unit = if self.unit > 0.0 { self.unit } else { plain };
        if self.guessed_with != (unit, width) {
            self.guessed_with = (unit, width);
            self.ends.changed_from(0);
        }
        let timestamps = self.timestamps;
        let guess = move |row: &Row| {
            let characters = row.text.chars().count() + if timestamps { 9 } else { 0 };
            unit * (characters as f32 * glyph / width.max(1.0)).ceil().max(1.0)
        };
        self.ends.update(&self.rows, &guess);
    }

    /// Forgets the heights from another width of the rows about to be
    /// shown, from `first`.
    pub(super) fn forget_heights_from(&mut self, first: usize) {
        let end = (first + 200).min(self.stale);
        for index in first..end {
            self.set_height(index, None);
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
