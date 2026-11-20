//! What the client says in a session: in the output, kept to be read again,
//! and handed to the screen reader.

use super::*;

/// Something the client said: when, what, and whether it was that
/// something could not be done.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub time: String,
    pub text: String,
    pub problem: bool,
}

/// How many of the client's messages are kept to be read again.
pub(super) const MOST_NOTICES: usize = 200;

impl Tab {
    /// Shows and speaks a message from the client itself.
    pub fn note(&mut self, text: &str) {
        self.remember_notice(text, self.palette.answering && self.palette.answer_failed);
        // What a command typed with `/` says is its answer, shown at the
        // command line; anything else is a line of the client's.
        if self.palette.answering {
            self.palette.answer.push(commands::Said {
                text: text.to_owned(),
                about: String::new(),
                run: None,
            });
        } else {
            self.show_local(text);
        }
        self.voice.say(text);
    }

    /// Keeps something the client said, wherever it was shown, to be read
    /// again: what was shown for a moment is not lost.
    pub(super) fn remember_notice(&mut self, text: &str, problem: bool) {
        if text.trim().is_empty() {
            return;
        }
        if self.notices.list.len() >= MOST_NOTICES {
            self.notices.list.remove(0);
        }
        self.notices.list.push(Notice {
            time: crate::output::clock(),
            text: text.to_owned(),
            problem,
        });
        self.notices.unread += 1;
    }

    /// What the client said in this session, newest last, and how many
    /// of them have not been looked at.
    pub fn notices(&self) -> (&[Notice], usize) {
        (&self.notices.list, self.notices.unread)
    }

    /// They have been looked at.
    pub fn notices_read(&mut self) {
        self.notices.unread = 0;
    }

    pub fn clear_notices(&mut self) {
        self.notices.list.clear();
        self.notices.unread = 0;
    }

    /// Shows a line of the client's own, without speaking it.
    pub(super) fn show_local(&mut self, text: &str) {
        self.output.push_local(text);
        let time = self.output.time(self.output.len() - 1).to_owned();
        self.logged(|log| log.local(&time, text));
    }

    /// Shows a line from the server.
    /// How many lines the scrollback holds.
    pub fn lines(&self) -> usize {
        self.output.len()
    }

    /// Hands new text to the screen reader: a label too small to see, marked
    /// as a live region, which screen readers announce when it changes, by
    /// voice and in braille.
    pub(super) fn announce(&mut self, ui: &mut egui::Ui) {
        if !self.voice.announcing() {
            return;
        }
        // A screen reader is only told of changes to a region it already
        // knows, so text waits until the region has been in the tree.
        let lines = if self.announce_ready {
            self.voice.take_announcements()
        } else {
            Vec::new()
        };
        if !lines.is_empty() {
            let mut text = lines.join("\n");
            // The same text twice running would not count as a change.
            if text == self.announcement {
                text.push('\u{200b}');
            }
            self.announcement = text;
        }
        let label = RichText::new(&self.announcement)
            .size(1.0)
            .color(egui::Color32::TRANSPARENT);
        let response = ui.add(Label::new(label).selectable(false));
        let in_tree = ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_live(egui::accesskit::Live::Polite);
        });
        self.announce_ready = in_tree.is_some();
        if !self.announce_ready {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
    }
}

/// What the client said, to be read again, and how many of those have not
/// been looked at.
#[derive(Default)]
pub(super) struct Notices {
    /// What the client said, to be read again, and how many of those
    /// have not been looked at.
    pub(super) list: Vec<Notice>,
    pub(super) unread: usize,
}
