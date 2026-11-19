//! The scrollback and its view: a virtualised list of wrapped lines.

use std::collections::VecDeque;
use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{Galley, Label, Rect, ScrollArea, Ui, UiBuilder, vec2};
use muddie_core::Line;

use crate::palette;

/// Oldest lines are dropped beyond this.
const MAX_LINES: usize = 20_000;

struct Row {
    job: LayoutJob,
    /// Height when wrapped to `Output::width`; `None` until measured.
    height: Option<f32>,
}

#[derive(Default)]
pub struct Output {
    rows: VecDeque<Row>,
    /// Unterminated text shown after the last row until it is replaced.
    partial: Option<LayoutJob>,
    /// Wrap width the cached heights were measured for.
    width: f32,
}

impl Output {
    /// Appends a completed server line, replacing any partial text.
    pub fn push(&mut self, line: &Line) {
        self.partial = None;
        self.push_job(server_job(line));
    }

    /// Appends a line produced by the client itself.
    pub fn push_local(&mut self, text: &str) {
        let mut job = LayoutJob::default();
        job.append(text, 0.0, palette::local_format());
        self.push_job(job);
    }

    /// Drops the partial text without showing a line in its place.
    pub fn clear_partial(&mut self) {
        self.partial = None;
    }

    pub fn set_partial(&mut self, line: &Line) {
        self.partial = Some(server_job(line));
    }

    fn push_job(&mut self, job: LayoutJob) {
        if self.rows.len() == MAX_LINES {
            self.rows.pop_front();
        }
        self.rows.push_back(Row { job, height: None });
    }

    /// Number of completed rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Plain text of a completed row.
    pub fn text(&self, index: usize) -> &str {
        &self.rows[index].job.text
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
        let layout = |ui: &Ui, job: &LayoutJob| -> Arc<Galley> {
            let mut job = job.clone();
            job.wrap.max_width = width;
            ui.painter().layout_job(job)
        };

        // Measure new rows, and find the first one inside the viewport.
        let mut total = 0.0;
        let mut first_visible = None;
        for (index, row) in self.rows.iter_mut().enumerate() {
            let height = *row
                .height
                .get_or_insert_with(|| layout(ui, &row.job).size().y);
            if first_visible.is_none() && total + height > viewport.min.y {
                first_visible = Some((index, total));
            }
            total += height;
        }
        let partial = self.partial.as_ref().map(|job| layout(ui, job));
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
                let galley = layout(ui, &row.job);
                y += galley.size().y;
                ui.add(Label::new(galley));
            }
            if let Some(galley) = partial {
                ui.add(Label::new(galley));
            }
        });
    }
}

fn server_job(line: &Line) -> LayoutJob {
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
