//! What the client says in a session: over the corner of the window, kept
//! under the bell to be read again, and handed to the screen reader; never
//! among the lines of the game, which are the game's. What a command
//! answers is shown where it was asked for (see `commands::answers`).

use super::*;

/// Something the client said: when, what, and whether it was that
/// something could not be done.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub time: String,
    pub text: String,
    pub problem: bool,
    /// How many were said before it in the session, and when it was.
    pub serial: u64,
    pub at: Instant,
    /// Shown over the corner of the window: all are but what a command
    /// run from the palette answers, which the palette shows.
    pub corner: bool,
}

/// How many of the client's messages are kept to be read again.
pub(super) const MOST_NOTICES: usize = 200;

impl Tab {
    /// Tells the user something.
    pub fn note(&mut self, text: &str) {
        self.say(text, false);
    }

    /// Tells the user something, kept as one of something that could not
    /// be done if it is a `problem`: over the corner and aloud, or, while
    /// a command is answering, as part of what it answers.
    pub(super) fn say(&mut self, text: &str, problem: bool) {
        crate::debug::show(&self.title(), "NOTE", text);
        let problem = problem || (self.palette.answering && self.palette.answer_failed);
        let answering = self.palette.answering;
        self.keep_notice(text, problem, !answering);
        if answering {
            self.palette.answer.push(commands::Said {
                text: text.to_owned(),
                about: String::new(),
                run: None,
            });
        }
        self.voice.say(text);
    }

    /// A line a script shows among the lines of the game, and says: what
    /// it asks for. Run from the palette (Run Lua…), it is what it
    /// answers.
    pub(super) fn echo(&mut self, text: &str) {
        if self.palette.answering {
            return self.note(text);
        }
        self.show_local(text);
        self.voice.say(text);
    }

    /// Keeps something the client said, to be read again, and to show over
    /// the corner if `corner`.
    pub(super) fn keep_notice(&mut self, text: &str, problem: bool, corner: bool) {
        if text.trim().is_empty() {
            return;
        }
        if self.notices.list.len() >= MOST_NOTICES {
            self.notices.list.remove(0);
        }
        self.notices.said += 1;
        self.notices.list.push(Notice {
            time: crate::output::clock(),
            text: text.to_owned(),
            problem,
            serial: self.notices.said,
            at: Instant::now(),
            corner,
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

    /// How many lines the scrollback holds.
    pub fn lines(&self) -> usize {
        self.output.len()
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
    /// How many have been said in the session, kept or not.
    pub(super) said: u64,
}
