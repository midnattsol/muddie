//! The scrollback and its view: a virtualised list of wrapped lines, with
//! search, links, optional timestamps, and a view of the latest lines
//! while scrolled back.

use std::collections::VecDeque;
use std::ops::Range;
use std::sync::Arc;

use eframe::egui::text::LayoutJob;
use eframe::egui::{
    Align, Galley, Label, OpenUrl, Rect, RichText, ScrollArea, Sense, Ui, UiBuilder, vec2,
};
use muddie_core::{Color, Line, LinkTarget};

use crate::i18n::tr;
use crate::output_style;
use crate::theme;

mod drawing;
mod heights;
mod rows;

#[cfg(test)]
pub use drawing::REMEASURED_PER_FRAME;
use heights::Ends;
use rows::*;
pub use rows::{clock, links, server_job};

/// Lines shown over the foot of the scrollback while it is read back, so
/// new output is not missed.
const TAIL_LINES: usize = 5;

/// How many rows a step of the mouse wheel moves the scrollback.
const WHEEL_ROWS: f32 = 5.0;

/// What clicking a link of the server's asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Clicked {
    /// A command to send.
    Send(String),
    /// A command to put in the command line, to be finished there.
    Prompt(String),
    /// To look for something in it, from its menu.
    Find,
    /// Something of it was copied, from its menu, to be said.
    Copied,
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
    /// Whether the reader has scrolled back from the end, far enough to be
    /// reading what came before rather than following what comes.
    reading: bool,
    /// How tall the content was when it was last drawn.
    shown_height: f32,
    /// Where each row ends, added up.
    ends: Ends,
    /// How many rows have no height measured yet.
    unmeasured: usize,
    /// The row height and width the guesses of rows not yet measured were
    /// made with.
    guessed_with: (f32, f32),
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
            reading: false,
            shown_height: 0.0,
            ends: Ends::default(),
            unmeasured: 0,
            guessed_with: (0.0, 0.0),
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
            found: false,
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
        self.unmeasured += 1;
        let found = links(&text).into_iter().map(|range| RowLink {
            target: LinkTarget::Url(text[range.clone()].to_owned()),
            range,
            hint: String::new(),
            name: String::new(),
            found: true,
        });
        self.rows.push_back(Row {
            links: found.chain(marked).collect(),
            content,
            characters: text.chars().count(),
            text,
            time: clock(),
            height: None,
        });
    }

    /// Removes the oldest row, keeping row numbers held elsewhere right.
    fn drop_oldest(&mut self) {
        if self
            .rows
            .pop_front()
            .is_some_and(|row| row.height.is_none())
        {
            self.unmeasured -= 1;
        }
        self.ends.dropped_first();
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
        self.ends.changed_from(0);
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

    /// Where the line found is among those that have `query`, counted
    /// from the first, and how many there are.
    pub fn found_place(&self, query: &str) -> Option<(usize, usize)> {
        let found = self.found?;
        let query = query.to_lowercase();
        let matching = |row: &usize| self.rows[*row].text.to_lowercase().contains(&query);
        let before = (0..found).filter(matching).count();
        let after = (found..self.rows.len()).filter(matching).count();
        Some((before + 1, before + after))
    }

    /// Forgets the search result and goes back to the end.
    pub fn end_find(&mut self) {
        self.found = None;
        self.jump_to_end = true;
    }

    /// Draws the scrollback in `ui`. `name` tells this output's panels
    /// apart from another's. While it is read back, the latest lines are
    /// shown over its foot, which leaves the space it takes as it is: what
    /// is shown never changes the place of what is scrolled. Returns the
    /// command of a link the user clicked, for the caller to send.
    pub fn show(&mut self, ui: &mut Ui, name: &str) -> Option<Clicked> {
        let wheel_line = ui
            .ctx()
            .options(|options| options.input_options.line_scroll_speed);
        let step = WHEEL_ROWS * self.row_height(ui) / wheel_line.max(1.0);
        // While text is being chosen in it with the pointer, it holds
        // still: what is chosen does not run away from the pointer as lines
        // come.
        let here = ui.available_rect_before_wrap();
        let choosing = ui.input(|input| {
            input.pointer.primary_down()
                && (input.pointer.press_origin()).is_some_and(|at| here.contains(at))
        });
        let area = ScrollArea::vertical()
            .id_salt(name)
            .auto_shrink(false)
            // Only while following the end: reading back, nothing pulls.
            .stick_to_bottom(!self.reading && !choosing)
            .wheel_scroll_multiplier(vec2(1.0, step));
        let shown = area.show_viewport(ui, |ui, viewport| self.show_rows(ui, viewport));
        if self.reading && !self.rows.is_empty() {
            self.show_tail(ui, shown.inner_rect, name);
        }
        shown.inner
    }

    /// How tall a row of one line is.
    fn row_height(&self, ui: &Ui) -> f32 {
        match self.unit {
            unit if unit > 0.0 => unit,
            _ => ui.fonts_mut(|fonts| fonts.row_height(&theme::output_font())),
        }
    }

    /// The latest lines, over the foot of the scrollback `area`, with the
    /// way back to the end; the scroll bar is left clear.
    fn show_tail(&mut self, ui: &mut Ui, area: Rect, name: &str) {
        let theme = theme::current();
        let line = self.row_height(ui);
        let header = crate::theme::size::CONTROL;
        let shown = TAIL_LINES.min(self.rows.len()) + usize::from(self.partial.is_some());
        let height = 8.0 + header + 4.0 + line * shown as f32 + 8.0;
        let scroll = &ui.spacing().scroll;
        let bar = scroll.bar_width + scroll.bar_inner_margin + scroll.bar_outer_margin;
        let rect = Rect::from_min_max(
            eframe::egui::pos2(area.left(), (area.bottom() - height).max(area.top())),
            eframe::egui::pos2(area.right() - bar, area.bottom()),
        );
        let builder = UiBuilder::new()
            .id_salt((name, "tail"))
            .max_rect(rect)
            .layout(eframe::egui::Layout::top_down(Align::Min));
        let mut tail = ui.new_child(builder);
        // What is under it is not to be clicked through it.
        tail.interact(rect, tail.make_persistent_id("cover"), Sense::click());
        tail.painter().rect_filled(rect, 0.0, theme.background);
        tail.painter().hline(
            rect.x_range(),
            rect.top(),
            eframe::egui::Stroke::new(1.0, theme.border),
        );
        let inner = rect.shrink2(vec2(0.0, 8.0));
        tail.scope_builder(UiBuilder::new().max_rect(inner), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.horizontal(|ui| {
                ui.set_min_height(header);
                ui.label(
                    RichText::new(tr("Latest lines"))
                        .small()
                        .color(theme.text_dim),
                );
                if crate::widgets::button(ui, tr("Jump to end")).clicked() {
                    self.jump_to_end = true;
                }
            });
            ui.add_space(4.0);
            let first = self.rows.len().saturating_sub(TAIL_LINES);
            for row in self.rows.iter().skip(first) {
                ui.add(Label::new(row.job(self.timestamps)).truncate());
            }
            if let Some(partial) = &self.partial {
                ui.add(Label::new(server_job(partial)).truncate());
            }
        });
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

    /// Does what the first link of a row does when clicked: opens a web
    /// address, or gives back the command to send or to finish. `None` if
    /// it has none; `Some(None)` if a web address was opened.
    pub fn follow_link(
        &self,
        ctx: &eframe::egui::Context,
        index: usize,
    ) -> Option<Option<Clicked>> {
        let link = self.rows.get(index)?.links.first()?;
        Some(match &link.target {
            LinkTarget::Url(address) => {
                ctx.open_url(eframe::egui::OpenUrl::new_tab(address));
                None
            }
            LinkTarget::Send(text) => Some(Clicked::Send(text.clone())),
            LinkTarget::Prompt(text) => Some(Clicked::Prompt(text.clone())),
            LinkTarget::Menu(items) => items.first().map(|(_, text)| Clicked::Send(text.clone())),
        })
    }

    /// Copies all of it, as text: from its menu and from the palette alike.
    pub fn copy_all(&self, ctx: &eframe::egui::Context) {
        ctx.copy_text(self.all_text());
    }

    /// All of it as text, a line a row, as it would be copied.
    pub fn all_text(&self) -> String {
        let mut text = String::new();
        for row in &self.rows {
            text.push_str(&row.text);
            text.push('\n');
        }
        if let Some(partial) = &self.partial {
            text.push_str(&partial.text());
        }
        text
    }

    /// Empties it.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.partial = None;
        self.ends.clear();
        (self.found, self.scroll_to, self.stale, self.unmeasured) = (None, None, 0, 0);
    }
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
        let red = muddie_core::Style {
            fg: muddie_core::Color::Rgb(200, 30, 30),
            ..Default::default()
        };
        "Go north".chars().for_each(|ch| line.push(ch, red));
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
        // Underlined, in the colours the server gave it, not the client's.
        // The link ends the line: the last of what it is drawn in.
        let format_at =
            |job: &LayoutJob, _: usize| job.sections.last().expect("a section").format.clone();
        let mut plain = Output::default();
        let mut unlinked = line.clone();
        unlinked.links.clear();
        plain.push(&unlinked);
        let (linked, as_sent) = (output.rows[0].job(false), plain.rows[0].job(false));
        let (linked, as_sent) = (format_at(&linked, 4), format_at(&as_sent, 4));
        assert_eq!(linked.color, as_sent.color);
        assert!(linked.underline.width > 0.0 && as_sent.underline.width == 0.0);
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
