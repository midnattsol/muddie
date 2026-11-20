//! The scrollback and its view: a virtualised list of wrapped lines, with
//! search, links, optional timestamps, and a view of the latest lines
//! while scrolled back.

use std::collections::VecDeque;
use std::ops::Range;
use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    Align, Frame, Galley, Label, OpenUrl, Panel, Rect, RichText, ScrollArea, Sense, Ui, UiBuilder,
    vec2,
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

/// A part of a row that can be clicked: where, what it does, what it is
/// for, and the name the server can take it away by.
#[derive(Debug, Clone, PartialEq)]
struct RowLink {
    range: Range<usize>,
    target: LinkTarget,
    hint: String,
    name: String,
}

/// What clicking a link of the server's asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Clicked {
    /// A command to send.
    Send(String),
    /// A command to put in the command line, to be finished there.
    Prompt(String),
}

struct Row {
    content: Content,
    /// The row without styling, for search, review and logs.
    text: String,
    /// When it arrived, as `HH:MM:SS`.
    time: String,
    /// Parts of `text` that can be clicked: web addresses found in it, and
    /// links the server marked.
    links: Vec<RowLink>,
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
                for link in &self.links {
                    line.restyle(link.range.clone(), |style| {
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
    /// The link at a character of the row as it is shown.
    fn link(&self, char_index: usize, timestamps: bool) -> Option<&RowLink> {
        let prefix = if timestamps {
            self.time.chars().count() + 1
        } else {
            0
        };
        let char_index = char_index.checked_sub(prefix)?;
        let (byte, _) = self.text.char_indices().nth(char_index)?;
        self.links.iter().find(|link| link.range.contains(&byte))
    }

    #[cfg(test)]
    fn link_at(&self, char_index: usize, timestamps: bool) -> Option<&LinkTarget> {
        self.link(char_index, timestamps).map(|link| &link.target)
    }
}

/// The time of day as it is shown, `HH:MM:SS`. Worked out once a second
/// and not for every line: a burst of text is thousands of lines in one.
pub fn clock() -> String {
    use std::cell::RefCell;
    thread_local! {
        static SHOWN: RefCell<(i64, String)> = const { RefCell::new((i64::MIN, String::new())) };
    }
    let now = chrono::Local::now();
    SHOWN.with(|shown| {
        let mut shown = shown.borrow_mut();
        if shown.0 != now.timestamp() {
            *shown = (now.timestamp(), now.format("%H:%M:%S").to_string());
        }
        shown.1.clone()
    })
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
    /// How tall the content was when it was last drawn.
    shown_height: f32,
    /// Rows before this one have a height measured for another width.
    stale: usize,
    /// How tall a row of one line of text is, as last measured.
    unit: f32,
    /// Row to bring into view on the next frame.
    scroll_to: Option<usize>,
    /// Go back to the end on the next frame.
    jump_to_end: bool,
    /// Row marked as the current search result.
    found: Option<usize>,
    /// The commands a link offers, and the row they are of, while its
    /// menu is asked for.
    menu: Option<(u64, Vec<(String, String)>)>,
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
            shown_height: 0.0,
            stale: 0,
            unit: 0.0,
            scroll_to: None,
            jump_to_end: false,
            found: None,
            menu: None,
        }
    }
}

impl Output {
    /// Appends a completed server line, replacing any partial text.
    pub fn push(&mut self, line: &Line) {
        self.partial = None;
        let marked = line.links.iter().map(|link| RowLink {
            range: link.range.clone(),
            target: link.target.clone(),
            hint: link.hint.clone(),
            name: link.name.clone(),
        });
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

    fn push_row(&mut self, content: Content, text: String, marked: Vec<RowLink>) {
        while self.rows.len() >= self.capacity.max(1) {
            self.drop_oldest();
        }
        self.pushed += 1;
        let found = links(&text).into_iter().map(|range| RowLink {
            target: LinkTarget::Url(text[range.clone()].to_owned()),
            range,
            hint: String::new(),
            name: String::new(),
        });
        self.rows.push_back(Row {
            links: found.chain(marked).collect(),
            content,
            text,
            time: clock(),
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
    /// Forgets every height at once, as was done before they were
    /// measured a share per frame: what that costs is what a test
    /// compares against.
    /// How many rows have had their height measured.
    #[cfg(test)]
    pub fn measured(&self) -> usize {
        self.rows.iter().filter(|row| row.height.is_some()).count()
    }

    pub fn restyle(&mut self, capacity: usize, timestamps: bool) {
        self.capacity = capacity.max(100);
        self.timestamps = timestamps;
        while self.rows.len() > self.capacity {
            self.drop_oldest();
        }
        // Every row may now be another height. They are measured again a
        // share per frame, those on screen first, as after a resize.
        self.stale = self.rows.len();
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
    pub fn show(&mut self, ui: &mut Ui, name: &str) -> Option<Clicked> {
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
                        if crate::widgets::button(ui, tr("Jump to end")).clicked() {
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

        let area = ScrollArea::vertical()
            .id_salt(name)
            .auto_shrink(false)
            .stick_to_bottom(true);
        area.show_viewport(ui, |ui, viewport| self.show_rows(ui, viewport))
            .inner
    }

    /// Takes away the links the server named `name`, which are no
    /// longer to be followed: with no name, all those it named.
    pub fn expire(&mut self, name: &str) {
        for row in &mut self.rows {
            row.links.retain(|link| {
                link.name.is_empty() || !(name.is_empty() || link.name.eq_ignore_ascii_case(name))
            });
        }
    }

    /// Empties it.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.partial = None;
        (self.found, self.scroll_to, self.stale) = (None, None, 0);
    }

    /// Draws the rows that are on screen, and what is not yet a whole line,
    /// in a space as tall as all of them. Returns what a click on a link
    /// asked for.
    fn show_rows(&mut self, ui: &mut Ui, viewport: Rect) -> Option<Clicked> {
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
    fn follow_width(&mut self, ui: &Ui, width: f32) {
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
    fn measure_some(&mut self, ui: &Ui, width: f32) {
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
    fn guesser(&self, ui: &Ui, width: f32) -> impl Fn(&Row) -> f32 + use<> {
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
    fn place_rows(
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
    fn forget_heights_from(&mut self, first: usize) {
        if first < self.stale {
            let end = (first + 200).min(self.stale);
            for row in self.rows.range_mut(first..end) {
                row.height = None;
            }
        }
    }

    /// Asking the scroll area to show a place is what lets it leave the
    /// end it otherwise sticks to, and go back to it.
    fn scroll_as_asked(&mut self, ui: &mut Ui, total: f32, target: Option<(f32, f32)>) {
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
    fn draw_rows(
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

/// Where the rows are: the first inside the viewport and its top, the top
/// and height of the row asked to be scrolled to, and how tall they all
/// are.
struct Placed {
    first_visible: Option<(usize, f32)>,
    target: Option<(f32, f32)>,
    height: f32,
}

/// Lays text out to the width given.
fn lay_out(ui: &Ui, mut job: LayoutJob, width: f32) -> Arc<Galley> {
    job.wrap.max_width = width;
    ui.painter().layout_job(job)
}

/// What the links of a row on screen do: a click opens an address, sends
/// a command, puts one in the command line, or sends the first a menu
/// offers; the other button shows a menu's commands, to choose one; and
/// pointed at, a link says what it is for. Returns what was asked for.
fn row_links(
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

fn append_line(job: &mut LayoutJob, line: &Line) {
    for span in &line.spans {
        job.append(&span.text, 0.0, palette::format(span.style));
    }
}

/// How many rows are laid out per frame only to learn their height: new
/// ones, and after the width changed, the ones measured for the old one.
pub const REMEASURED_PER_FRAME: usize = 300;

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
            hint: String::new(),
            name: "exits".into(),
        });
        output.push(&line);
        assert_eq!(
            output.rows[0].link_at(4, false),
            Some(&LinkTarget::Send("north".into()))
        );
        // Named by the server, it can be taken away; the others stay.
        output.push_local("see https://example.org");
        output.expire("exits");
        assert_eq!(output.rows[0].link_at(4, false), None);
        assert!(output.rows[1].link_at(6, false).is_some());
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
