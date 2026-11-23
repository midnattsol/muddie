//! Speech for the main window: narration of incoming text and the keys for
//! reviewing earlier output. What is said goes through the screen reader
//! in use, and so to a braille display too, or through the system's voice,
//! as the settings say; it is the same on every system.

use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use eframe::egui::{Key, Modifiers, Ui};
use muddie_speech::{Narrator, Speech, Through};

use crate::i18n::tr;
use crate::output::Output;
use crate::settings::{Settings, SpeechOutput};

/// Lines of the game closer than this to the one before are one burst: a
/// new line cuts short what is said only at the first of them.
const BURST: Duration = Duration::from_millis(300);

/// What was read of the game this recently is not read again as said on a
/// channel or put in a panel, nor the other way round: many MUDs send a
/// tell as a line and over GMCP both.
const ALREADY_READ: Duration = Duration::from_secs(3);

/// What the review keys ask for besides reading.
#[derive(Debug, Clone, PartialEq)]
pub enum Reviewed {
    /// The line read last, to copy.
    Copy(String),
    /// The first link of the line read last, in the main output, by row.
    Follow(usize),
}

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
    /// The game is not read aloud in this session; what Muddie says of
    /// itself still is.
    muted: bool,
    /// Nothing is ever said: the session is kept out of sight.
    silent: bool,
    /// Prompts are not read.
    quiet_prompts: bool,
    /// When a line of the game was last read: the start of a burst
    /// interrupts, the rest of it waits.
    last_read: Option<Instant>,
    /// What was read lately, and whether it was a line of the game.
    recent: VecDeque<(String, Instant, bool)>,
    /// The line to speech, while there is one to keep, and where it goes.
    speech: Option<Speech>,
    opened: Option<Through>,
    narrator: Narrator,
    /// A new line of the game cuts short what is being said.
    interrupts: bool,
    /// The lines last read with the review keys, by their serial (see
    /// [`Output::total_pushed`]), so that dropping old ones does not move
    /// them: in the output, and in the panel reviewed.
    review: [Option<u64>; 2],
    /// The text panel reviewed with Alt+Shift, by its name.
    panel: Option<String>,
    ctrl_was_down: bool,
    /// Speech rate from -100 to 100, and the language code to speak in.
    rate: i32,
    language: String,
    /// The window, to be woken when what speaks changes; known from its
    /// first frame.
    window: Arc<OnceLock<eframe::egui::Context>>,
    /// Speech was opened before: opened again, it says what it speaks
    /// with, as it was the player who changed it.
    reopened: bool,
    /// What was said, in tests, which speak to nothing.
    #[cfg(test)]
    pub said: Vec<String>,
    /// What of that cut short what was being said, in tests.
    #[cfg(test)]
    pub cut: Vec<String>,
    /// How many times what was being said was silenced, in tests.
    #[cfg(test)]
    pub stopped: usize,
}

impl Voice {
    /// Takes what the settings say of speech.
    pub fn configure(&mut self, settings: &Settings) {
        let tuned = self.rate != settings.speech_rate || self.language != settings.speech_language;
        self.rate = settings.speech_rate;
        self.language = settings.speech_language.clone();
        self.interrupts = settings.speech_interrupts;
        self.quiet_prompts = !settings.speech_prompts;
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
        match (self.forced, self.output, self.silent) {
            (_, _, true) => None,
            (true, ..) | (false, SpeechOutput::Voice, _) => Some(Through::Voice),
            (false, SpeechOutput::Reader, _) => Some(Through::ScreenReader),
            (false, SpeechOutput::Off, _) => None,
        }
    }

    /// This session says nothing, ever: one kept out of sight, whose
    /// messages are said by the session on screen.
    pub fn silence(&mut self) {
        self.silent = true;
        self.open();
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
        let now = std::mem::replace(&mut self.reopened, true);
        #[cfg(not(test))]
        {
            self.speech = wanted.map(|through| {
                let window = Arc::clone(&self.window);
                let wake = Box::new(move || {
                    if let Some(window) = window.get() {
                        window.request_repaint();
                    }
                });
                Speech::start(through, wake)
            });
            if let Some(speech) = &self.speech {
                speech.set_rate(self.rate);
                speech.set_language(&self.language);
                speech.announce_changes(tr("Speech through {0}."), now);
            }
        }
        #[cfg(test)]
        let _ = now;
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

    /// Stops reading the game in this session, or reads it again. What
    /// Muddie says of itself, the review keys and the map are still said.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if muted {
            self.stop();
        }
    }

    /// Whether the game is read aloud in this session.
    pub fn reads_game(&self) -> bool {
        !self.muted
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
            if interrupt {
                self.cut.push(text.to_owned());
            }
        }
        if let Some(speech) = &self.speech {
            speech.say(text, interrupt);
        }
    }

    /// Pressing Ctrl silences speech, as it does in screen readers: of
    /// every session, the ones out of sight too, each with its own voice.
    pub fn hush_on_ctrl(&mut self, ctx: &eframe::egui::Context) {
        let ctrl_down = ctx.input(|input| input.modifiers.ctrl);
        if ctrl_down && !self.ctrl_was_down {
            self.stop();
        }
        self.ctrl_was_down = ctrl_down;
    }

    /// Silences what is being said and all that is waiting.
    pub fn stop(&mut self) {
        #[cfg(test)]
        {
            self.stopped += 1;
        }
        if let Some(speech) = &self.speech {
            speech.stop();
        }
    }

    /// A line from the server was completed.
    pub fn line(&mut self, text: &str) {
        if let Some(text) = self.narrator.line(text) {
            self.read_game(&text);
        }
    }

    /// A prompt from the server was completed: read as a line, unless
    /// prompts are not read.
    pub fn prompt(&mut self, text: &str) {
        match self.quiet_prompts {
            false => self.line(text),
            true => self.skip_line(text),
        }
    }

    /// Reads what the game said, unless the game is not read: cutting
    /// short what is said only at the start of a burst, so that a room
    /// that comes in many lines is read whole. What was just read as said
    /// on a channel is not read again.
    fn read_game(&mut self, text: &str) {
        if self.muted || self.read_lately(text, false) {
            return;
        }
        let now = Instant::now();
        let first = self
            .last_read
            .is_none_or(|at| now.duration_since(at) > BURST);
        self.last_read = Some(now);
        self.keep_read(text, true);
        self.speak(text, self.interrupts && first);
    }

    /// Reads what was said on a channel or put in a panel, after what is
    /// being said, unless the game is not read or it was just read as a
    /// line.
    pub fn read(&mut self, text: &str, line: &str) {
        if self.muted || self.read_lately(line, true) {
            return;
        }
        self.keep_read(line, false);
        self.say(text);
    }

    /// Whether `text` was read lately the other way: as a line, if it
    /// comes from a channel, or from a channel if it is a line.
    fn read_lately(&mut self, text: &str, from_channel: bool) -> bool {
        let now = Instant::now();
        (self.recent).retain(|(_, at, _)| now.duration_since(*at) < ALREADY_READ);
        let text = text.trim();
        !text.is_empty()
            && (self.recent.iter()).any(|(read, _, line)| {
                *line == from_channel && (read.contains(text) || text.contains(read.as_str()))
            })
    }

    fn keep_read(&mut self, text: &str, line: bool) {
        let text = text.trim();
        if !text.is_empty() {
            self.recent
                .push_back((text.to_owned(), Instant::now(), line));
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
    /// keys, over the output and the text panels, by their names. Returns
    /// what the keys asked for besides reading.
    pub fn update(
        &mut self,
        ui: &mut Ui,
        output: &Output,
        panels: &[(&str, &Output)],
    ) -> Option<Reviewed> {
        // What speaks can change at any time: the speech thread wakes the
        // window then, so the status bar and the MUD learn of it.
        let _ = self.window.set(ui.ctx().clone());
        if !self.enabled() {
            return None;
        }
        let now = Instant::now();
        // Text that stays unended is a prompt.
        if let Some(text) = self.narrator.poll(now)
            && !self.quiet_prompts
        {
            self.read_game(&text);
        }
        if let Some(deadline) = self.narrator.deadline() {
            ui.ctx()
                .request_repaint_after(deadline.saturating_duration_since(now));
        }

        // Shift first: Alt alone would take Alt+Shift too.
        let mut asked = self.choose_panel(ui, panels);
        let panel = (self.panel.as_deref())
            .and_then(|name| {
                panels
                    .iter()
                    .find(|(known, _)| known.eq_ignore_ascii_case(name))
            })
            .or(panels.first());
        if let Some((_, log)) = panel {
            asked = asked.or(self.review_keys(ui, log, 1, Modifiers::SHIFT));
        }
        asked.or(self.review_keys(ui, output, 0, Modifiers::NONE))
    }

    /// Alt+Shift+Left and Right choose the text panel Alt+Shift reviews,
    /// saying its name and how many lines it has.
    fn choose_panel(&mut self, ui: &mut Ui, panels: &[(&str, &Output)]) -> Option<Reviewed> {
        let keys = crate::shortcuts::WITH_ARROWS | Modifiers::SHIFT;
        let (back, on) = ui.input_mut(|input| {
            (
                input.consume_key(keys, Key::ArrowLeft),
                input.consume_key(keys, Key::ArrowRight),
            )
        });
        if !(back || on) {
            return None;
        }
        if panels.is_empty() {
            self.say_now(tr("No text panels."));
            return None;
        }
        let at = (self.panel.as_deref())
            .and_then(|name| {
                panels
                    .iter()
                    .position(|(known, _)| known.eq_ignore_ascii_case(name))
            })
            .unwrap_or(0);
        let next = match back {
            true => (at + panels.len() - 1) % panels.len(),
            false => (at + 1) % panels.len(),
        };
        let (name, log) = panels[next];
        self.panel = Some(name.to_owned());
        self.review[1] = None;
        let lines = crate::i18n::trn("{0} line", "{0} lines", log.len() as u64);
        self.say_now(&crate::i18n::trf("{0}, {1}.", &[name, &lines]));
        None
    }

    /// With `modifiers` held, Up/Down/Home/End move through `output`
    /// reading each line, saying so at either end, and 1..9 read the most
    /// recent lines without moving. Alt reviews the main output and
    /// Alt+Shift a text panel. C copies the line read, and O, in the main
    /// output, follows its link.
    fn review_keys(
        &mut self,
        ui: &mut Ui,
        output: &Output,
        which: usize,
        modifiers: Modifiers,
    ) -> Option<Reviewed> {
        let last = output.len().checked_sub(1)?;
        // The first row there is, by serial.
        let first = output.total_pushed() - output.len() as u64;
        let current =
            self.review[which].map(|serial| (serial.saturating_sub(first) as usize).min(last));
        let (moved, recent, copy, follow) = ui.input_mut(|input| {
            // The arrows with Alt (Option on a Mac), the figures and the
            // letters with Alt (Ctrl on a Mac), and Shift for a panel.
            let arrows = crate::shortcuts::WITH_ARROWS | modifiers;
            let figures = crate::shortcuts::WITH_KEY | modifiers;
            let mut pressed = |key| input.consume_key(arrows, key);
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
            let recent = (RECENT_KEYS.iter()).position(|&key| input.consume_key(figures, key));
            let copy = input.consume_key(figures, Key::C);
            let follow = which == 0 && input.consume_key(figures, Key::O);
            (moved, recent, copy, follow)
        });
        if let Some(row) = moved {
            self.review[which] = Some(first + row as u64);
        }
        let read = current.unwrap_or(last);
        if copy {
            return Some(Reviewed::Copy(output.text(read).trim().to_owned()));
        }
        if follow {
            return Some(Reviewed::Follow(read));
        }
        let row = moved.or_else(|| recent.and_then(|back| last.checked_sub(back)))?;
        let text = output.text(row).trim();
        let text = if text.is_empty() { tr("Blank.") } else { text };
        // Pressed again at either end, it says it is there.
        let said = match (moved.is_some() && moved == current, row) {
            (true, 0) => crate::i18n::trf("First line: {0}", &[text]),
            (true, row) if row == last => crate::i18n::trf("Last line: {0}", &[text]),
            _ => text.to_owned(),
        };
        self.say_now(&said);
        None
    }
}
