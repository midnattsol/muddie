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

#[derive(Default)]
pub(super) struct Reader {
    /// It is on screen, in place of the output, while it has the keyboard.
    pub open: bool,
    /// Its text, and the serial (see [`Output::total_pushed`]) of the line
    /// after its last.
    text: String,
    end: u64,
    /// The keyboard is to go to it, on the newest line, once laid out.
    place: bool,
}

impl Reader {
    /// Opens it on the latest lines of `output`.
    fn open(&mut self, output: &Output) {
        let first = output.total_pushed() - output.len() as u64;
        let rows = output.len().saturating_sub(MOST_LINES)..output.len();
        self.text = rows
            .map(|row| output.text(row))
            .collect::<Vec<_>>()
            .join("\n");
        self.end = first + output.len() as u64;
        self.open = true;
        self.place = true;
    }

    /// Adds what came since: what is read stays where it is.
    fn grow(&mut self, output: &Output) {
        let first = output.total_pushed() - output.len() as u64;
        let from = self.end.max(first);
        for serial in from..output.total_pushed() {
            if !self.text.is_empty() {
                self.text.push('\n');
            }
            self.text.push_str(output.text((serial - first) as usize));
        }
        self.end = output.total_pushed();
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
        ui.ctx()
            .accesskit_node_builder(way.id, |node| node.set_label(label));
        self.panels.zone_output = Some(way.id);
        if way.has_focus() {
            self.open_reader();
        }
    }

    /// Puts the keyboard in the output as text.
    pub(super) fn open_reader(&mut self) {
        self.reader.open(&self.output);
    }

    /// The output as a read-only text, filling `ui`, while it has the
    /// keyboard; when it lets it go, the output is drawn again.
    pub(super) fn show_reader(&mut self, ui: &mut egui::Ui) {
        self.reader.grow(&self.output);
        let id = ui.make_persistent_id("output reader");
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
        if response.has_focus() {
            self.reader_keys(ui);
        } else {
            // Gone elsewhere: the output is drawn again, and with nowhere
            // else, the command line takes the keyboard.
            self.reader.open = false;
        }
    }

    /// In the text: Tab, Shift+Tab, Escape and Enter go back to the
    /// command line; typing goes there too, with what was typed.
    fn reader_keys(&mut self, ui: &mut egui::Ui) {
        let (back, typed) = ui.input_mut(|input| {
            let back = [
                (Modifiers::NONE, Key::Tab),
                (Modifiers::SHIFT, Key::Tab),
                (Modifiers::NONE, Key::Escape),
                (Modifiers::NONE, Key::Enter),
            ]
            .into_iter()
            .fold(false, |back, (held, key)| {
                input.consume_key(held, key) || back
            });
            let typed: String = (input.events.iter())
                .filter_map(|event| match event {
                    egui::Event::Text(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            (back, typed)
        });
        if back || !typed.is_empty() {
            self.input.push_str(&typed);
            self.cursor_to_end = true;
            self.reader.open = false;
            self.focus_line = true;
        }
    }
}
