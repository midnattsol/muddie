//! Speech for the main window: narration of incoming text and the keys for
//! reviewing earlier output. What is said goes through the screen reader
//! in use, and so to a braille display too, or through the system's voice,
//! as the settings say; it is the same on every system.

use std::time::Instant;

use eframe::egui::{Key, Modifiers, Ui};
use muddie_speech::{Narrator, Speech, Through};

use crate::i18n::tr;
use crate::output::Output;
use crate::settings::{Settings, SpeechOutput};

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
    /// What speaks, as the settings say, unless this session asked for
    /// the system's voice.
    output: SpeechOutput,
    forced: bool,
    /// Silenced for this session.
    muted: bool,
    /// The line to speech, while there is one to keep, and where it goes.
    speech: Option<Speech>,
    opened: Option<Through>,
    narrator: Narrator,
    /// A new line of the game cuts short what is being said.
    interrupts: bool,
    /// Rows last read with the review keys: in the output, and in the chat.
    review: [Option<usize>; 2],
    ctrl_was_down: bool,
    /// Speech rate from -100 to 100, and the language code to speak in.
    rate: i32,
    language: String,
    /// What was said, in tests, which speak to nothing.
    #[cfg(test)]
    pub said: Vec<String>,
}

impl Voice {
    /// Takes what the settings say of speech.
    pub fn configure(&mut self, settings: &Settings) {
        let tuned = self.rate != settings.speech_rate || self.language != settings.speech_language;
        self.rate = settings.speech_rate;
        self.language = settings.speech_language.clone();
        self.interrupts = settings.speech_interrupts;
        self.output = settings.speech;
        let opened = self.opened;
        self.open();
        // A line just opened was told them already.
        if let Some(speech) = self
            .speech
            .as_ref()
            .filter(|_| tuned && opened == self.opened)
        {
            speech.set_rate(self.rate);
            speech.set_language(&self.language);
        }
    }

    /// This session speaks with the system's voice, whatever the settings
    /// say: as `--speech` asks.
    pub fn force_voice(&mut self) {
        self.forced = true;
        self.open();
    }

    /// Where speech goes, or nothing.
    fn wanted(&self) -> Option<Through> {
        match (self.forced, self.output, self.muted) {
            (_, _, true) => None,
            (true, ..) | (false, SpeechOutput::Voice, _) => Some(Through::Voice),
            (false, SpeechOutput::Reader, _) => Some(Through::ScreenReader),
            (false, SpeechOutput::Off, _) => None,
        }
    }

    /// Starts or stops the line to speech when where it is wanted to go
    /// changes. In tests there is none: they speak to nothing, and note
    /// what is said.
    fn open(&mut self) {
        let wanted = self.wanted();
        if wanted == self.opened {
            return;
        }
        self.opened = wanted;
        #[cfg(not(test))]
        {
            self.speech = wanted.map(Speech::start);
            if let Some(speech) = &self.speech {
                speech.set_rate(self.rate);
                speech.set_language(&self.language);
                speech.announce_changes(tr("Speech through {0}."));
            }
        }
        self.narrator = Narrator::default();
    }

    /// Whether anything is being said: speech is on, and there is a screen
    /// reader or a voice to say it.
    pub fn enabled(&self) -> bool {
        #[cfg(test)]
        return self.wanted().is_some();
        #[cfg(not(test))]
        self.speech
            .as_ref()
            .is_some_and(|speech| speech.status().name.is_some())
    }

    /// What speaks now, by its name: the screen reader or the voice.
    pub fn through(&self) -> Option<String> {
        self.speech.as_ref().and_then(|speech| speech.status().name)
    }

    /// Whether a screen reader is what speaks: the MUD is told of one.
    pub fn screen_reader(&self) -> bool {
        (self.speech.as_ref()).is_some_and(|speech| speech.status().screen_reader)
    }

    /// Silences speech for this session, or lets it speak again.
    pub fn set_muted(&mut self, muted: bool) {
        if muted != self.muted {
            self.muted = muted;
            self.open();
            if !muted {
                self.say(tr("Speech on."));
            }
        }
    }

    /// Says `text` after whatever is being said.
    pub fn say(&mut self, text: &str) {
        self.speak(text, false);
    }

    /// Says `text` at once, cutting short whatever is being said.
    pub fn say_now(&mut self, text: &str) {
        self.speak(text, true);
    }

    fn speak(&mut self, text: &str, interrupt: bool) {
        #[cfg(test)]
        if self.wanted().is_some() {
            self.said.push(text.to_owned());
        }
        if let Some(speech) = &self.speech {
            speech.say(text, interrupt);
        }
    }

    /// Silences what is being said and all that is waiting.
    pub fn stop(&mut self) {
        if let Some(speech) = &self.speech {
            speech.stop();
        }
    }

    /// A line or prompt from the server was completed.
    pub fn line(&mut self, text: &str) {
        if let Some(text) = self.narrator.line(text) {
            self.speak(&text, self.interrupts);
        }
    }

    /// The unterminated text was hidden by a script, so it is not spoken.
    pub fn skip_partial(&mut self, text: &str) {
        self.narrator.partial(text, Instant::now());
        self.narrator.line(text);
    }

    /// A line was completed but hidden by a script, so it is not spoken.
    pub fn skip_line(&mut self, text: &str) {
        self.narrator.line(text);
    }

    /// The unterminated text at the end of the output changed.
    pub fn partial(&mut self, text: &str) {
        self.narrator.partial(text, Instant::now());
    }

    /// Runs once per frame: speaks settled text and handles the speech
    /// keys.
    pub fn update(&mut self, ui: &mut Ui, output: &Output, chat: &Output) {
        // A screen reader can start or stop at any time: the window looks
        // again now and then, so the status bar and the MUD learn of it.
        if self.speech.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }
        if !self.enabled() {
            return;
        }
        let now = Instant::now();
        if let Some(text) = self.narrator.poll(now) {
            self.speak(&text, self.interrupts);
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

        self.review_keys(ui, output, 0, Modifiers::ALT);
        self.review_keys(ui, chat, 1, Modifiers::ALT | Modifiers::SHIFT);
    }

    /// With `modifiers` held, Up/Down/Home/End move through `output`
    /// reading each line, and 1..9 read the most recent lines without
    /// moving. Alt reviews the main output and Alt+Shift the chat.
    fn review_keys(&mut self, ui: &mut Ui, output: &Output, which: usize, modifiers: Modifiers) {
        let Some(last) = output.len().checked_sub(1) else {
            return;
        };
        let current = self.review[which].map(|row| row.min(last));
        let (moved, recent) = ui.input_mut(|input| {
            let mut pressed = |key| input.consume_key(modifiers, key);
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
            self.review[which] = moved;
        }
        let row = moved.or_else(|| recent.and_then(|back| last.checked_sub(back)));
        if let Some(row) = row {
            let text = output.text(row).trim();
            self.say_now(if text.is_empty() { tr("Blank.") } else { text });
        }
    }
}
