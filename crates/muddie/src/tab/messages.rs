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
    /// How many were said before it in the session, and when it was.
    pub serial: u64,
    pub at: Instant,
    /// Shown over the corner wherever the user is, rather than among the
    /// lines of the game: about work done aside, as taking a map.
    pub aside: bool,
}

/// How many of the client's messages are kept to be read again.
pub(super) const MOST_NOTICES: usize = 200;

impl Tab {
    /// Shows and speaks a message from the client itself.
    pub fn note(&mut self, text: &str) {
        self.say(text, false);
    }

    /// Says something about work done aside, over the corner of the
    /// window: neither among the lines of the game nor in the command line.
    pub(super) fn toast(&mut self, text: &str, problem: bool) {
        crate::capture::debug(&self.title(), "NOTE", text);
        self.keep_notice(text, problem, true);
        self.voice.say(text);
    }

    /// Shows and speaks a message, kept as one of something that could not
    /// be done if it is a `problem`.
    pub(super) fn say(&mut self, text: &str, problem: bool) {
        crate::capture::debug(&self.title(), "NOTE", text);
        let problem = problem || (self.palette.answering && self.palette.answer_failed);
        self.remember_notice(text, problem);
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
        self.keep_notice(text, problem, false);
    }

    fn keep_notice(&mut self, text: &str, problem: bool, aside: bool) {
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
            aside,
        });
        self.notices.unread += 1;
    }

    /// What the client said in this session, newest last, and how many
    /// of them have not been looked at.
    pub fn notices(&self) -> (&[Notice], usize) {
        (&self.notices.list, self.notices.unread)
    }

    /// How many the client has said in the session.
    pub fn notices_said(&self) -> u64 {
        self.notices.said
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
