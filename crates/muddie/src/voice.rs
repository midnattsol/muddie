//! Speech for the main window: narration of incoming text and the keys for
//! reviewing earlier output.

use std::time::Instant;

use eframe::egui::{Key, Modifiers, Ui};
use muddie_speech::{Narrator, Speech};

use crate::output::Output;

/// Alt plus one of these reads the first to ninth most recent line.
const RECENT_KEYS: [Key; 9] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
    Key::Num9,
];

#[derive(Default)]
pub struct Voice {
    /// `Some` while speech is turned on.
    speech: Option<Speech>,
    narrator: Narrator,
    /// Output row last read with the review keys.
    review: Option<usize>,
    ctrl_was_down: bool,
}

impl Voice {
    pub fn enabled(&self) -> bool {
        self.speech.is_some()
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled == self.enabled() {
            return;
        }
        self.speech = enabled.then(Speech::start);
        self.narrator = Narrator::default();
        self.say("Speech on.");
    }

    /// Speaks `text` after whatever is already queued.
    pub fn say(&self, text: &str) {
        if let Some(speech) = &self.speech {
            speech.say(text);
        }
    }

    pub fn stop(&self) {
        if let Some(speech) = &self.speech {
            speech.stop();
        }
    }

    /// A line or prompt from the server was completed.
    pub fn line(&mut self, text: &str) {
        if let Some(text) = self.narrator.line(text) {
            self.say(&text);
        }
    }

    /// A line was completed but hidden by a script, so it is not spoken.
    pub fn skip_line(&mut self, text: &str) {
        self.narrator.line(text);
    }

    /// The unterminated text at the end of the output changed.
    pub fn partial(&mut self, text: &str) {
        self.narrator.partial(text, Instant::now());
    }

    /// Runs once per frame: speaks settled text and handles the speech keys.
    pub fn update(&mut self, ui: &mut Ui, output: &Output) {
        if !self.enabled() {
            return;
        }
        let now = Instant::now();
        if let Some(text) = self.narrator.poll(now) {
            self.say(&text);
        }
        if let Some(deadline) = self.narrator.deadline() {
            ui.ctx()
                .request_repaint_after(deadline.saturating_duration_since(now));
        }

        // Pressing Ctrl silences speech, as it does in screen readers.
        let ctrl_down = ui.input(|input| input.modifiers.ctrl);
        if ctrl_down && !self.ctrl_was_down {
            self.stop();
        }
        self.ctrl_was_down = ctrl_down;

        self.review_keys(ui, output);
    }

    /// Alt+Up/Down/Home/End move through the output reading each line;
    /// Alt+1..9 read the most recent lines without moving.
    fn review_keys(&mut self, ui: &mut Ui, output: &Output) {
        let Some(last) = output.len().checked_sub(1) else {
            return;
        };
        let current = self.review.map(|row| row.min(last));
        let (moved, recent) = ui.input_mut(|input| {
            let mut pressed = |key| input.consume_key(Modifiers::ALT, key);
            let moved = if pressed(Key::ArrowUp) {
                Some(current.map_or(last, |row| row.saturating_sub(1)))
            } else if pressed(Key::ArrowDown) {
                Some(current.map_or(last, |row| (row + 1).min(last)))
            } else if pressed(Key::Home) {
                Some(0)
            } else if pressed(Key::End) {
                Some(last)
            } else {
                None
            };
            let recent = RECENT_KEYS.iter().position(|&key| pressed(key));
            (moved, recent)
        });
        if moved.is_some() {
            self.review = moved;
        }
        let row = moved.or_else(|| recent.and_then(|back| last.checked_sub(back)));
        if let Some(row) = row {
            let text = output.text(row).trim();
            self.stop();
            self.say(if text.is_empty() { "Blank." } else { text });
        }
    }
}
