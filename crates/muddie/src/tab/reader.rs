//! The output as a read-only text, as screen reader users expect of a MUD
//! client: Tab from the command line goes to it, and in it the screen
//! reader's own keys read it by line, word or character, and choose text
//! to copy. It is the output's latest lines, the newest last, and it grows
//! as lines come. Tab, Escape, Enter or typing goes back to the command
//! line, with what was typed.

use super::*;

/// The most lines it holds: what a screen reader goes through with ease.
/// Older ones are read back with the review keys.
const MOST_LINES: usize = 1000;
/// How many lines past [`MOST_LINES`] it grows before the oldest go: not
/// one at a time, as each going moves where the screen reader is.
const SLACK: usize = 200;

#[derive(Default)]
pub(super) struct Reader {
    /// It is on screen, in place of the output, while it has the keyboard.
    pub open: bool,
    /// Its text, and the serial (see [`Output::total_pushed`]) of the line
    /// after its last.
    text: String,
    end: u64,
    /// How many lines its text has.
    lines: usize,
    /// The keyboard is to go to it, on the newest line, once laid out.
    place: bool,
}

impl Reader {
    /// Opens it on the latest lines of `output`.
    fn open(&mut self, output: &Output) {
        let first = output.total_pushed() - output.len() as u64;
        let rows = output.len().saturating_sub(MOST_LINES)..output.len();
        self.lines = rows.len();
        self.text = rows
            .map(|row| output.text(row))
            .collect::<Vec<_>>()
            .join("\n");
        self.end = first + output.len() as u64;
        self.open = true;
        self.place = true;
    }

    /// Adds what came since: what is read stays where it is. Grown well
    /// past [`MOST_LINES`], the oldest go, or each line that came would
    /// lay out ever more text. Returns how many characters went.
    fn grow(&mut self, output: &Output) -> usize {
        let first = output.total_pushed() - output.len() as u64;
        let from = self.end.max(first);
        for serial in from..output.total_pushed() {
            if !self.text.is_empty() {
                self.text.push('\n');
            }
            self.text.push_str(output.text((serial - first) as usize));
            self.lines += 1;
        }
        self.end = output.total_pushed();
        if self.lines <= MOST_LINES + SLACK {
            return 0;
        }
        let gone = self.lines - MOST_LINES;
        let cut = (self.text.match_indices('\n'))
            .nth(gone - 1)
            .map_or(self.text.len(), |(at, _)| at + 1);
        let chars = self.text[..cut].chars().count();
        self.text.drain(..cut);
        self.lines = MOST_LINES;
        chars
    }

    #[cfg(test)]
    pub(super) fn text(&self) -> &str {
        &self.text
    }

    /// Where its newest line starts, in characters.
    fn newest_line(&self) -> usize {
        let start = self.text.rfind('\n').map_or(0, |at| at + 1);
        self.text[..start].chars().count()
    }
}

impl Tab {
    /// The way to it from the keyboard, where the output is: a point a
    /// screen reader is told of as the output, which opens it as it is
    /// reached.
    pub(super) fn reader_way_in(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let at = egui::Rect::from_min_size(rect.min, egui::vec2(1.0, 1.0));
        let way = ui.interact(
            at,
            ui.make_persistent_id("output way in"),
            egui::Sense::FOCUSABLE,
        );
        let label = tr("Output");
        ui.ctx().accesskit_node_builder(way.id, |node| {
            node.set_role(egui::accesskit::Role::Group);
            node.set_label(label);
        });
        self.panels.zone_output = Some(way.id);
        if way.has_focus() {
            self.open_reader();
            // Drawn again as text at once: the way in is not said as well.
            ui.ctx().request_discard("the output is read as text");
        }
    }

    /// Puts the keyboard in the output as text.
    pub(super) fn open_reader(&mut self) {
        self.reader.open(&self.output);
    }

    /// The output as a read-only text, filling `ui`, while it has the
    /// keyboard; when it lets it go, the output is drawn again.
    pub(super) fn show_reader(&mut self, ui: &mut egui::Ui) {
        let gone = self.reader.grow(&self.output);
        let id = ui.make_persistent_id("output reader");
        // Where the screen reader is stays on the same text, the oldest
        // gone before it.
        if gone > 0
            && let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id)
            && let Some(range) = state.cursor.char_range()
        {
            let back = |at: egui::text::CCursor| egui::text::CCursor {
                index: egui::text::CharIndex(at.index.0.saturating_sub(gone)),
                ..at
            };
            state.cursor.set_char_range(Some(egui::text::CCursorRange {
                primary: back(range.primary),
                secondary: back(range.secondary),
                ..range
            }));
            state.store(ui.ctx(), id);
        }
        // Its keys, before the text takes them: Tab would go on to what
        // comes after it otherwise.
        if !self.reader.place && ui.memory(|memory| memory.has_focus(id)) && self.reader_keys(ui) {
            return;
        }
        let theme = theme::current();
        let mut text: &str = &self.reader.text;
        let shown = egui::ScrollArea::vertical()
            .id_salt("output reader")
            .auto_shrink(false)
            .show(ui, |ui| {
                egui::TextEdit::multiline(&mut text)
                    .id(id)
                    .font(theme::output_font())
                    .text_color(theme.text)
                    .frame(egui::Frame::NONE)
                    .desired_width(f32::INFINITY)
                    .lock_focus(false)
                    .show(ui)
            })
            .inner;
        let response = shown.response;
        self.panels.zone_output = Some(response.id);
        let label = tr("Output");
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(label));
        if std::mem::take(&mut self.reader.place) {
            let newest = egui::text::CCursor::new(self.reader.newest_line());
            let mut state = shown.state;
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(newest)));
            state.store(ui.ctx(), id);
            response.request_focus();
            response.scroll_to_me(Some(egui::Align::BOTTOM));
            return;
        }
        if !response.has_focus() {
            // Gone elsewhere: the output is drawn again, and with nowhere
            // else, the command line takes the keyboard.
            self.reader.open = false;
        }
    }

    /// In the text: Tab, Escape and Enter go back to the command line;
    /// typing goes there too, with what was typed. Shift+Tab goes on back,
    /// to what is before the output. Returns whether it left.
    fn reader_keys(&mut self, ui: &mut egui::Ui) -> bool {
        let (back, typed) = ui.input_mut(|input| {
            let tab = crate::shortcuts::take_tab(input);
            let escape = input.consume_key(Modifiers::NONE, Key::Escape);
            let enter = input.consume_key(Modifiers::NONE, Key::Enter);
            let typed: String = (input.events.iter())
                .filter_map(|event| match event {
                    egui::Event::Text(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            (tab || escape || enter, typed)
        });
        if !back && typed.is_empty() {
            return false;
        }
        self.input.push_str(&typed);
        self.cursor_to_end = true;
        self.reader.open = false;
        self.focus_line = true;
        // Not on to what is next, as Tab was going to.
        ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        true
    }
}
