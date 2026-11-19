//! The scrollback and its view: a virtualised list of wrapped lines, with
//! search, links, optional timestamps, and a view of the latest lines
//! while scrolled back.

use std::collections::VecDeque;
use std::ops::Range;
use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    Frame, Galley, Label, OpenUrl, Panel, Rect, RichText, ScrollArea, Sense, Ui, UiBuilder, vec2,
};
use muddie_core::{Color, Line, LinkTarget};

use crate::i18n::tr;
use crate::palette;
use crate::theme;

/// Lines shown under the scrollback while it is scrolled away from the
/// end, so new output is not missed.
const TAIL_LINES: usize = 5;

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
    /// When it arrived, as `HH:MM:SS`.
    time: String,
    /// Parts of `text` that can be clicked: web addresses found in it, and
    /// links the server marked.
    links: Vec<(Range<usize>, LinkTarget)>,
    /// Height when wrapped to `Output::width`; `None` until measured.
    height: Option<f32>,
}

impl Row {
    fn job(&self, timestamps: bool) -> LayoutJob {
        let mut job = LayoutJob::default();
        if timestamps {
            job.append(&format!("{} ", self.time), 0.0, palette::local_format());
        }
        match &self.content {
            Content::Server(line) if self.links.is_empty() => append_line(&mut job, line),
            Content::Server(line) => {
                let accent = theme::current().accent;
                let mut line = line.clone();
                for (link, _) in &self.links {
                    line.restyle(link.clone(), |style| {
                        style.underline = true;
                        style.fg = Color::Rgb(accent.r(), accent.g(), accent.b());
                    });
                }
                append_line(&mut job, &line);
            }
            Content::Local(text) => job.append(text, 0.0, palette::local_format()),
        }
        if job.text.is_empty() {
            // An empty job has no height; a blank line must still take a row.
            job.append(" ", 0.0, palette::local_format());
        }
        job
    }

    /// The link at a character offset into the row as drawn, if any.
    fn link_at(&self, char_index: usize, timestamps: bool) -> Option<&LinkTarget> {
        let prefix = if timestamps {
            self.time.chars().count() + 1
        } else {
            0
        };
        let char_index = char_index.checked_sub(prefix)?;
        let (byte, _) = self.text.char_indices().nth(char_index)?;
        let (_, target) = self.links.iter().find(|(link, _)| link.contains(&byte))?;
        Some(target)
    }
}

/// The byte ranges of `text` that are `http://` or `https://` links.
pub fn links(text: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find("http") {
        let start = from + at;
        let rest = &text[start..];
        let is_link = rest.starts_with("http://") || rest.starts_with("https://");
        let len = rest
            .find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '<' | '>' | '\''))
            .unwrap_or(rest.len());
        // Punctuation that ends the sentence is not part of the link.
        let link = rest[..len].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']']);
        if is_link && link.len() > "https://".len() {
            found.push(start..start + link.len());
        }
        from = start + len.max(4);
    }
    found
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
    timestamps: bool,
    /// Whether the view was at the end of the scrollback last frame.
    at_end: bool,
    /// Row to bring into view on the next frame.
    scroll_to: Option<usize>,
    /// Go back to the end on the next frame.
    jump_to_end: bool,
    /// Row marked as the current search result.
    found: Option<usize>,
}

impl Default for Output {
    fn default() -> Self {
        Self {
            rows: VecDeque::new(),
            partial: None,
            width: 0.0,
            capacity: 20_000,
            pushed: 0,
            timestamps: false,
            at_end: true,
            scroll_to: None,
            jump_to_end: false,
            found: None,
        }
    }
}

impl Output {
    /// Appends a completed server line, replacing any partial text.
    pub fn push(&mut self, line: &Line) {
        self.partial = None;
        let marked = line
            .links
            .iter()
            .map(|link| (link.range.clone(), link.target.clone()));
        self.push_row(Content::Server(line.clone()), line.text(), marked.collect());
    }

    /// Appends a line produced by the client itself.
    pub fn push_local(&mut self, text: &str) {
        self.push_row(Content::Local(text.to_owned()), text.to_owned(), Vec::new());
    }

    /// Drops the partial text without showing a line in its place.
    pub fn clear_partial(&mut self) {
        self.partial = None;
    }

    pub fn set_partial(&mut self, line: &Line) {
        self.partial = Some(line.clone());
    }

    fn push_row(
        &mut self,
        content: Content,
        text: String,
        marked: Vec<(Range<usize>, LinkTarget)>,
    ) {
        while self.rows.len() >= self.capacity.max(1) {
            self.drop_oldest();
        }
        self.pushed += 1;
        let found = links(&text).into_iter().map(|range| {
            let address = LinkTarget::Url(text[range.clone()].to_owned());
            (range, address)
        });
        self.rows.push_back(Row {
            links: found.chain(marked).collect(),
            content,
            text,
            time: chrono::Local::now().format("%H:%M:%S").to_string(),
            height: None,
        });
    }

    /// Removes the oldest row, keeping row numbers held elsewhere right.
    fn drop_oldest(&mut self) {
        self.rows.pop_front();
        let shift = |row: &mut Option<usize>| *row = row.and_then(|row| row.checked_sub(1));
        shift(&mut self.found);
        shift(&mut self.scroll_to);
    }

    /// Lays everything out again, after a change of theme or text size,
    /// and applies the settings that affect the scrollback.
    pub fn restyle(&mut self, capacity: usize, timestamps: bool) {
        self.capacity = capacity.max(100);
        self.timestamps = timestamps;
        while self.rows.len() > self.capacity {
            self.drop_oldest();
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

    /// When a completed row arrived, as `HH:MM:SS`.
    pub fn time(&self, index: usize) -> &str {
        &self.rows[index].time
    }

    /// Moves to the nearest row containing `query`, ignoring case, before
    /// (or after, if `forward`) the current result; from the end at first,
    /// and around the ends. Returns the row found, which is brought into
    /// view and marked.
    pub fn find(&mut self, query: &str, forward: bool) -> Option<usize> {
        let query = query.to_lowercase();
        let count = self.rows.len();
        if query.is_empty() || count == 0 {
            self.found = None;
            return None;
        }
        let start = self.found.unwrap_or(if forward { count - 1 } else { 0 });
        let row = (1..=count)
            .map(|step| {
                if forward {
                    (start + step) % count
                } else {
                    (start + count - step) % count
                }
            })
            .find(|&row| self.rows[row].text.to_lowercase().contains(&query));
        self.found = row;
        self.scroll_to = row;
        row
    }

    /// Forgets the search result and goes back to the end.
    pub fn end_find(&mut self) {
        self.found = None;
        self.jump_to_end = true;
    }

    /// Draws the scrollback in `ui`. `name` tells this output's panels
    /// apart from another's. Returns the command of a link the user
    /// clicked, for the caller to send.
    pub fn show(&mut self, ui: &mut Ui, name: &str) -> Option<String> {
        let theme = theme::current();
        // While scrolled back, the latest lines stay in view below.
        if !self.at_end && !self.rows.is_empty() {
            let frame = Frame::new()
                .fill(theme.background)
                .inner_margin(vec2(0.0, 4.0));
            Panel::bottom(format!("{name} tail"))
                .frame(frame)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(tr("Latest lines"))
                                .small()
                                .color(theme.text_dim),
                        );
                        if ui.small_button(tr("Back to the end")).clicked() {
                            self.jump_to_end = true;
                        }
                    });
                    let first = self.rows.len().saturating_sub(TAIL_LINES);
                    for row in self.rows.iter().skip(first) {
                        ui.add(Label::new(row.job(self.timestamps)).truncate());
                    }
                    if let Some(partial) = &self.partial {
                        ui.add(Label::new(server_job(partial)).truncate());
                    }
                });
        }

        let mut area = ScrollArea::vertical()
            .id_salt(name)
            .auto_shrink(false)
            .stick_to_bottom(true);
        if std::mem::take(&mut self.jump_to_end) {
            area = area.vertical_scroll_offset(f32::MAX);
        } else if let Some(row) = self.scroll_to.take() {
            // Unmeasured rows are taken as one line high; close enough to
            // land on the row, which a later frame then corrects.
            let line_height = ui.fonts_mut(|fonts| fonts.row_height(&palette::font()));
            let above: f32 = (self.rows.iter().take(row))
                .map(|row| row.height.unwrap_or(line_height))
                .sum();
            area = area.vertical_scroll_offset((above - ui.available_height() / 3.0).max(0.0));
        }
        area.show_viewport(ui, |ui, viewport| self.show_rows(ui, viewport))
            .inner
    }

    fn show_rows(&mut self, ui: &mut Ui, viewport: Rect) -> Option<String> {
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width();
        if width != self.width {
            self.width = width;
            for row in &mut self.rows {
                row.height = None;
            }
        }
        let timestamps = self.timestamps;
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
                None => *row.height.insert(layout(ui, row.job(timestamps)).size().y),
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
        self.at_end = viewport.max.y >= total - 2.0;

        let (first, top) = first_visible.unwrap_or((self.rows.len(), partial_top));
        let max_rect = ui.max_rect();
        let rect = Rect::from_min_size(
            max_rect.min + vec2(0.0, top),
            vec2(max_rect.width(), total - top),
        );
        let found = self.found;
        let mut command = None;
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            // Keep widget ids stable while scrolling.
            ui.skip_ahead_auto_ids(first);
            let mut y = top;
            for (index, row) in self.rows.iter().enumerate().skip(first) {
                if y > viewport.max.y {
                    return;
                }
                let galley = layout(ui, row.job(timestamps));
                y += galley.size().y;
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
                if response.clicked()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let cursor = galley.cursor_from_pos(pointer - response.rect.min);
                    match row.link_at(cursor.index.0, timestamps) {
                        Some(LinkTarget::Url(address)) => {
                            ui.ctx().open_url(OpenUrl::new_tab(address))
                        }
                        Some(LinkTarget::Send(text)) => command = Some(text.clone()),
                        None => {}
                    }
                }
            }
            if let Some(galley) = partial {
                ui.add(Label::new(galley));
            }
        });
        command
    }
}

fn append_line(job: &mut LayoutJob, line: &Line) {
    for span in &line.spans {
        job.append(&span.text, 0.0, palette::format(span.style));
    }
}

/// A line of server text laid out with its colours.
pub fn server_job(line: &Line) -> LayoutJob {
    let mut job = LayoutJob::default();
    append_line(&mut job, line);
    if job.text.is_empty() {
        job.append(" ", 0.0, palette::format(Default::default()));
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_found_without_trailing_punctuation() {
        let text = "See https://example.org/a?b=1, or (http://x.io/wiki). Not httpd or http://";
        let found: Vec<&str> = links(text).into_iter().map(|range| &text[range]).collect();
        assert_eq!(found, ["https://example.org/a?b=1", "http://x.io/wiki"]);
        assert!(links("nothing here").is_empty());
    }

    #[test]
    fn link_under_a_character_accounts_for_the_timestamp() {
        let mut output = Output::default();
        output.push_local("ñ go to https://example.org now");
        let row = &output.rows[0];
        let address = LinkTarget::Url("https://example.org".into());
        assert_eq!(row.link_at(10, false), Some(&address));
        assert_eq!(row.link_at(2, false), None);
        assert_eq!(row.link_at(10 + 9, true), Some(&address));
        assert_eq!(row.link_at(3, true), None);
    }

    #[test]
    fn links_marked_by_the_server_are_kept_with_the_row() {
        let mut output = Output::default();
        let mut line = Line::default();
        "Go north"
            .chars()
            .for_each(|ch| line.push(ch, Default::default()));
        line.links.push(muddie_core::LineLink {
            range: 3..8,
            target: LinkTarget::Send("north".into()),
        });
        output.push(&line);
        assert_eq!(
            output.rows[0].link_at(4, false),
            Some(&LinkTarget::Send("north".into()))
        );
        assert_eq!(output.rows[0].link_at(0, false), None);
    }

    #[test]
    fn find_walks_backwards_from_the_end_and_wraps() {
        let mut output = Output::default();
        for text in ["An Orc arrives", "nothing", "the orc leaves", "silence"] {
            output.push_local(text);
        }
        assert_eq!(output.find("ORC", false), Some(2));
        assert_eq!(output.find("orc", false), Some(0));
        assert_eq!(output.find("orc", false), Some(2));
        assert_eq!(output.find("orc", true), Some(0));
        assert_eq!(output.find("dragon", false), None);
        assert_eq!(output.find("", false), None);
    }

    #[test]
    fn dropping_old_rows_keeps_the_found_row_right() {
        let mut output = Output::default();
        output.restyle(100, false);
        (0..100).for_each(|n| output.push_local(&format!("line {n}")));
        assert_eq!(output.find("line 50", false), Some(50));
        output.push_local("one more");
        assert_eq!(output.found, Some(49));
        assert_eq!(output.text(49), "line 50");
        assert_eq!((output.len(), output.total_pushed()), (100, 101));
    }
}
