//! Speech for the main window: narration of incoming text and the keys for
//! reviewing earlier output. What is said goes through the screen reader
//! in use, and so to a braille display too, or through the system's voice,
//! as the settings say; it is the same on every system.

use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use eframe::egui::Ui;
use muddie_script::{Aloud, How, Saying};
use muddie_speech::{Narrator, Speech, Through};

use crate::i18n::tr;
use crate::output::Output;
use crate::settings::{Settings, SpeechOutput};

/// Lines of the game closer than this to the one before are one burst: a
/// new line cuts short what is said only at the first of them.
const BURST: Duration = Duration::from_millis(300);
/// The longest a burst goes on: lines that never stop coming, as in a
/// fight, start another after this, which cuts in, so that what is heard
/// does not fall ever further behind the game.
const BURST_LONGEST: Duration = Duration::from_secs(1);

/// What was read of the game this recently is not read again as said on a
/// channel or put in a panel, nor the other way round: many MUDs send a
/// tell as a line and over GMCP both.
const ALREADY_READ: Duration = Duration::from_secs(3);

/// The fewest characters a text read lately has for another that holds it
/// to be taken as the same words: "Ok." or a prompt inside a tell is not
/// the tell.
const SAME_WORDS: usize = 12;

/// What the review keys ask for besides reading.
#[derive(Debug, Clone, PartialEq)]
pub enum Reviewed {
    /// The line read last, to copy.
    Copy(String),
    /// The first link of the line read last, in the main output, by row.
    Follow(usize),
}

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
    /// The session is not the one on screen: only that one is heard.
    pub away: bool,
    /// When a line of the game was last read, and when the burst it is
    /// of began: the start of a burst interrupts, the rest of it waits.
    last_read: Option<(Instant, Instant)>,
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
    /// A screen reader is taken to be running, in tests.
    #[cfg(test)]
    pub pretend_reader: bool,
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
        #[cfg(test)]
        if self.pretend_reader {
            return true;
        }
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

    /// Says what is of the game and not a line of it, as a script or a
    /// gauge, as it is to be heard. The very words just read are not said
    /// again, unless they are to cut in.
    pub fn say_of_game(&mut self, said: &Saying) {
        let cut = said.aloud.how == How::Cut;
        if self.hears(said.aloud) && (cut || !self.just_read(&said.text)) {
            self.speak(&said.text, cut);
        }
    }

    /// Whether something of the game is heard at all: what all of it goes
    /// through.
    fn hears(&self, aloud: Aloud) -> bool {
        !self.away && aloud.how != How::Never && (!self.muted || aloud.always)
    }

    /// Whether those very words were read lately, of a line or a channel.
    fn just_read(&self, text: &str) -> bool {
        (self.recent.iter()).any(|(read, at, _)| read == text.trim() && at.elapsed() < ALREADY_READ)
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
        use crate::shortcuts::{HUSH, Held, Pressed};
        let keys = HUSH.keys();
        // Held alone, as Ctrl: as it is pressed, not while it is held.
        let held = ctx.input(|input| Held::of(input.modifiers));
        let alone = (keys.iter()).any(|keys| {
            let wanted = keys.held;
            keys.key == Pressed::Alone
                && (!wanted.ctrl || held.ctrl)
                && (!wanted.alt || held.alt)
                && (!wanted.shift || held.shift)
                && (!wanted.cmd || held.cmd)
        });
        let pressed = ctx.input_mut(|input| HUSH.pressed(input));
        if (alone && !self.ctrl_was_down) || pressed {
            self.stop();
        }
        self.ctrl_was_down = alone;
    }

    /// Silences it as a script asks: in the session on screen only, for it
    /// is what that one says.
    pub fn hush(&mut self) {
        if !self.away {
            self.stop();
        }
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
            self.read_game(&text, Aloud::default());
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

    /// Reads `text` in place of a line, or of a prompt, as it would be.
    pub fn instead(&mut self, said: &Saying, prompt: bool) {
        let text = said.text.trim();
        if !text.is_empty() && !(prompt && self.quiet_prompts) {
            self.read_game(text, said.aloud);
        }
    }

    /// Reads what the game said, unless the game is not read: cutting
    /// short what is said only at the start of a burst, so that a room
    /// that comes in many lines is read whole. What was just read as said
    /// on a channel is not read again.
    fn read_game(&mut self, text: &str, aloud: Aloud) {
        if !self.hears(aloud) || self.read_lately(text, false) {
            return;
        }
        let now = Instant::now();
        let first = self.last_read.is_none_or(|(last, began)| {
            now.duration_since(last) > BURST || now.duration_since(began) > BURST_LONGEST
        });
        let began = match (first, self.last_read) {
            (false, Some((_, began))) => began,
            _ => now,
        };
        self.last_read = Some((now, began));
        self.keep_read(text, true);
        let cut = match aloud.how {
            How::Cut => true,
            How::Wait => false,
            _ => self.interrupts && first,
        };
        self.speak(text, cut);
    }

    /// Reads what was said on a channel or put in a panel, after what is
    /// being said, unless the game is not read or it was just read as a
    /// line.
    pub fn read(&mut self, text: &str, line: &str, aloud: Aloud) {
        if !self.hears(aloud) || self.read_lately(line, true) {
            return;
        }
        self.keep_read(line, false);
        self.speak(text, aloud.how == How::Cut);
    }

    /// Whether `text` was read lately the other way: as a line, if it
    /// comes from a channel, or from a channel if it is a line.
    fn read_lately(&mut self, text: &str, from_channel: bool) -> bool {
        let now = Instant::now();
        (self.recent).retain(|(_, at, _)| now.duration_since(*at) < ALREADY_READ);
        let text = text.trim();
        !text.is_empty()
            && (self.recent.iter())
                .any(|(read, _, line)| *line == from_channel && same_words(read, text))
    }

    fn keep_read(&mut self, text: &str, line: bool) {
        let text = text.trim();
        if !text.is_empty() {
            self.recent
                .push_back((text.to_owned(), Instant::now(), line));
        }
    }

    /// The unterminated text was hidden by a script, or kept from being
    /// read, so it is not spoken, however many times it comes.
    pub fn skip_partial(&mut self, text: &str) {
        self.narrator.taken(text);
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
            self.read_game(&text, Aloud::default());
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
            asked = asked.or(self.review_keys(ui, log, 1));
        }
        asked.or(self.review_keys(ui, output, 0))
    }

    /// Its keys, Alt+Shift+Left and Right, choose the text panel reviewed,
    /// saying its name and how many lines it has.
    fn choose_panel(&mut self, ui: &mut Ui, panels: &[(&str, &Output)]) -> Option<Reviewed> {
        let (back, on) = ui.input_mut(|input| {
            (
                crate::shortcuts::PANEL_PREVIOUS.pressed(input),
                crate::shortcuts::PANEL_NEXT.pressed(input),
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

    /// The review keys of `output`, the main one (`which` 0) or a text
    /// panel's: those that move through it reading each line, saying so at
    /// either end, and those that read the most recent lines without
    /// moving; one copies the line read, and, in the main output, one
    /// follows its link.
    fn review_keys(&mut self, ui: &mut Ui, output: &Output, which: usize) -> Option<Reviewed> {
        let last = output.len().checked_sub(1)?;
        // The first row there is, by serial.
        let first = output.total_pushed() - output.len() as u64;
        let current =
            self.review[which].map(|serial| (serial.saturating_sub(first) as usize).min(last));
        let (moved, recent, copy, follow) = ui.input_mut(|input| {
            use crate::shortcuts as keys;
            let [previous, next, first, end, copy] = match which {
                0 => [
                    keys::READ_PREVIOUS,
                    keys::READ_NEXT,
                    keys::READ_FIRST,
                    keys::READ_LAST,
                    keys::COPY_READ,
                ],
                _ => [
                    keys::PANEL_READ_PREVIOUS,
                    keys::PANEL_READ_NEXT,
                    keys::PANEL_READ_FIRST,
                    keys::PANEL_READ_LAST,
                    keys::PANEL_COPY_READ,
                ],
            };
            let moved = if previous.pressed(input) {
                Some(current.map_or(last, |row| row.saturating_sub(1)))
            } else if next.pressed(input) {
                Some(current.map_or(last, |row| (row + 1).min(last)))
            } else if first.pressed(input) {
                Some(0)
            } else if end.pressed(input) {
                Some(last)
            } else {
                None
            };
            let recent_keys = match which {
                0 => keys::READ_RECENT,
                _ => keys::PANEL_RECENT,
            };
            let recent = (recent_keys.iter()).position(|shortcut| shortcut.pressed(input));
            let copy = copy.pressed(input);
            let follow = which == 0 && keys::FOLLOW_LINK.pressed(input);
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

/// Whether `read` and `text` are the same words: the same, or one holding
/// the other when that other is long enough to be more than a word or a
/// prompt that comes up in many.
fn same_words(read: &str, text: &str) -> bool {
    let (short, long) = match read.len() <= text.len() {
        true => (read, text),
        false => (text, read),
    };
    short == long || (short.chars().count() >= SAME_WORDS && long.contains(short))
}

#[cfg(test)]
mod tests {
    use super::same_words;

    #[test]
    fn a_tell_is_the_same_words_as_its_line_and_not_as_a_word_in_it() {
        assert!(same_words("Ana te dice: hola", "Ana te dice: hola"));
        assert!(same_words(
            "[Chat] Ana: nos vemos en la plaza",
            "Ana: nos vemos en la plaza"
        ));
        assert!(same_words(
            "Ana: nos vemos en la plaza",
            "[Chat] Ana: nos vemos en la plaza"
        ));
        assert!(!same_words("Ok.", "[Chat] Ana: Ok."));
        assert!(!same_words(">", "[Chat] Ana: a > b"));
        assert!(!same_words("¿Qué tal?", "[Chat] Ana: ¿Qué tal?"));
    }
}
