//! The scrollback and its view: a virtualised list of wrapped lines.

use std::collections::VecDeque;
use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{Galley, Label, Rect, ScrollArea, Ui, UiBuilder, vec2};
use muddie_core::Line;

use crate::palette;

/// What a row shows. Rows are laid out when drawn rather than when added,
/// so a change of theme or text size reaches the whole scrollback.
enum Content {
    /// A line from the server, with its colours.
    Server(Line),
    /// A line the client added itself.
    Local(String),
}

struct Row {
    content: Content,
    /// The row without styling, for search, review and logs.
    text: String,
    /// Height when wrapped to `Output::width`; `None` until measured.
    height: Option<f32>,
}

impl Row {
    fn job(&self) -> LayoutJob {
        match &self.content {
            Content::Server(line) => server_job(line),
            Content::Local(text) => {
                let mut job = LayoutJob::default();
                job.append(text, 0.0, palette::local_format());
                job
            }
        }
    }
}

pub struct Output {
    rows: VecDeque<Row>,
    /// Unterminated text shown after the last row until it is replaced.
    partial: Option<Line>,
    /// Wrap width the cached heights were measured for.
    width: f32,
    /// Oldest rows are dropped beyond this many.
    capacity: usize,
    /// Rows ever added, which keeps counting when old ones are dropped.
    pushed: u64,
}

impl Default for Output {
    fn default() -> Self {
        Self {
            rows: VecDeque::new(),
            partial: None,
            width: 0.0,
            capacity: 20_000,
            pushed: 0,
        }
    }
}

impl Output {
    /// Appends a completed server line, replacing any partial text.
    pub fn push(&mut self, line: &Line) {
        self.partial = None;
        self.push_row(Content::Server(line.clone()), line.text());
    }

    /// Appends a line produced by the client itself.
    pub fn push_local(&mut self, text: &str) {
        self.push_row(Content::Local(text.to_owned()), text.to_owned());
    }

    /// Drops the partial text without showing a line in its place.
    pub fn clear_partial(&mut self) {
        self.partial = None;
    }

    pub fn set_partial(&mut self, line: &Line) {
        self.partial = Some(line.clone());
    }

    fn push_row(&mut self, content: Content, text: String) {
        while self.rows.len() >= self.capacity.max(1) {
            self.rows.pop_front();
        }
        self.pushed += 1;
        self.rows.push_back(Row {
            content,
            text,
            height: None,
        });
    }

    /// Lays everything out again, after a change of theme or text size,
    /// and applies a new limit on the rows kept.
    pub fn restyle(&mut self, capacity: usize) {
        self.capacity = capacity.max(100);
        while self.rows.len() > self.capacity {
            self.rows.pop_front();
        }
        for row in &mut self.rows {
            row.height = None;
        }
    }

    /// Number of completed rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Rows added since the session started, including dropped ones.
    pub fn total_pushed(&self) -> u64 {
        self.pushed
    }

    /// Plain text of a completed row.
    pub fn text(&self, index: usize) -> &str {
        &self.rows[index].text
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show_viewport(ui, |ui, viewport| self.show_rows(ui, viewport));
    }

    fn show_rows(&mut self, ui: &mut Ui, viewport: Rect) {
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width();
        if width != self.width {
            self.width = width;
            for row in &mut self.rows {
                row.height = None;
            }
        }
        let layout = |ui: &Ui, mut job: LayoutJob| -> Arc<Galley> {
            job.wrap.max_width = width;
            ui.painter().layout_job(job)
        };

        // Measure new rows, and find the first one inside the viewport.
        let mut total = 0.0;
        let mut first_visible = None;
        for (index, row) in self.rows.iter_mut().enumerate() {
            let height = match row.height {
                Some(height) => height,
                None => *row.height.insert(layout(ui, row.job()).size().y),
            };
            if first_visible.is_none() && total + height > viewport.min.y {
                first_visible = Some((index, total));
            }
            total += height;
        }
        let partial = self
            .partial
            .as_ref()
            .map(|line| layout(ui, server_job(line)));
        let partial_top = total;
        total += partial.as_ref().map_or(0.0, |galley| galley.size().y);
        ui.set_height(total);

        let (first, top) = first_visible.unwrap_or((self.rows.len(), partial_top));
        let max_rect = ui.max_rect();
        let rect = Rect::from_min_size(
            max_rect.min + vec2(0.0, top),
            vec2(max_rect.width(), total - top),
        );
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            // Keep widget ids stable while scrolling.
            ui.skip_ahead_auto_ids(first);
            let mut y = top;
            for row in self.rows.iter().skip(first) {
                if y > viewport.max.y {
                    return;
                }
                let galley = layout(ui, row.job());
                y += galley.size().y;
                ui.add(Label::new(galley));
            }
            if let Some(galley) = partial {
                ui.add(Label::new(galley));
            }
        });
    }
}

/// A line of server text laid out with its colours.
pub fn server_job(line: &Line) -> LayoutJob {
    let mut job = LayoutJob::default();
    for span in &line.spans {
        job.append(&span.text, 0.0, palette::format(span.style));
    }
    if job.text.is_empty() {
        // An empty job has no height; a blank line must still take a row.
        job.append(" ", 0.0, palette::format(Default::default()));
    }
    job
}
