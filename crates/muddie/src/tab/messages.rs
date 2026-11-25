//! What the client says in a session: over the corner of the window, kept
//! under the bell to be read again, and handed to the screen reader; never
//! among the lines of the game, which are the game's. What a command
//! answers is shown where it was asked for (see `commands::answers`).

use super::*;
use muddie_script::{Aloud, How, Saying};

/// Something the client said: when, what, and whether it was that
/// something could not be done.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
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
        let problem = problem || (self.palette.answering && self.palette.answer_failed);
        // A problem is said at once, not after the game queued before it.
        let how = match problem && !self.palette.answering {
            true => How::Cut,
            false => How::Wait,
        };
        self.tell(text, problem, how);
    }

    /// Tells the user something, heard as told.
    pub(super) fn tell(&mut self, text: &str, problem: bool, how: How) {
        crate::debug::show(&self.title(), "NOTE", text);
        let answering = self.palette.answering;
        self.keep_message(text, problem, !answering);
        if answering {
            self.palette.answer.push(commands::Said {
                text: text.to_owned(),
                about: String::new(),
                run: None,
            });
        }
        match how {
            How::Never => {}
            How::Cut => self.voice.say_now(text),
            _ => self.voice.say(text),
        }
    }

    /// A line a script shows among the lines of the game, and says: what
    /// it asks for. Run from the palette (Run Lua…), it is what it
    /// answers.
    pub(super) fn echo(&mut self, text: &str, aloud: Aloud) {
        if self.palette.answering {
            return self.tell(text, false, aloud.how);
        }
        self.show_local(text);
        self.voice.say_of_game(&Saying {
            text: text.to_owned(),
            aloud,
        });
    }

    /// Keeps something the client said, to be read again, and to show over
    /// the corner if `corner`.
    pub(super) fn keep_message(&mut self, text: &str, problem: bool, corner: bool) {
        if text.trim().is_empty() {
            return;
        }
        if self.messages.list.len() >= MOST_NOTICES {
            self.messages.list.remove(0);
        }
        self.messages.said += 1;
        self.messages.list.push(Message {
            time: crate::output::clock(),
            text: text.to_owned(),
            problem,
            serial: self.messages.said,
            at: Instant::now(),
            corner,
        });
        self.messages.unread += 1;
    }

    /// What the client said in this session, newest last, and how many
    /// of them have not been looked at.
    pub fn messages(&self) -> (&[Message], usize) {
        (&self.messages.list, self.messages.unread)
    }

    /// They have been looked at.
    /// Says `text` at once, without keeping it among the messages: what
    /// answers something just done, such as a message copied.
    pub fn say_aside(&mut self, text: &str) {
        self.voice.say_now(text);
    }

    pub fn messages_read(&mut self) {
        self.messages.unread = 0;
    }

    pub fn clear_notices(&mut self) {
        self.messages.list.clear();
        self.messages.unread = 0;
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
pub(super) struct Messages {
    /// What the client said, to be read again, and how many of those
    /// have not been looked at.
    pub(super) list: Vec<Message>,
    pub(super) unread: usize,
    /// How many have been said in the session, kept or not.
    pub(super) said: u64,
}
