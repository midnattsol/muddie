//! Running a command, and what it answers at the command line.

use super::*;

impl Tab {
    /// Says, as part of what a command answers, a thing that can be
    /// chosen: `command` is run by choosing it. Said outside a command, it
    /// is a line like any other.
    pub(in crate::tab) fn offer(&mut self, text: &str, about: &str, command: &str) {
        if self.palette.answering {
            self.voice.say(&format!("{text}: {about}"));
            self.palette.answer.push(Said {
                text: text.to_owned(),
                about: about.to_owned(),
                run: Some(command.to_owned()),
            });
        } else {
            self.note(&format!("{text}: {about}"));
        }
    }

    /// Says that what was asked could not be done.
    pub(in crate::tab) fn refuse(&mut self, text: &str) {
        self.palette.answer_failed |= self.palette.answering;
        self.note(text);
    }

    /// Says something inside the command line, if it is empty to say it
    /// in, and among the lines of the game if not: for what a command
    /// answers some time after it was given.
    pub(in crate::tab) fn bar(&mut self, text: &str, problem: bool) {
        if self.input.is_empty() {
            self.remember_notice(text, problem);
            self.voice.say(text);
            self.palette.bar_note = Some(BarNote {
                text: text.to_owned(),
                problem,
                since: None,
            });
        } else {
            self.note(text);
        }
    }

    /// Carries out a command of the client and puts what it answers where
    /// it is shown. Returns whether it was one.
    pub(in crate::tab) fn run_command(&mut self, line: &str) -> bool {
        self.palette.results = None;
        self.palette.bar_note = None;
        self.palette.cache = None;
        self.palette.chosen = 0;
        self.palette.answer.clear();
        (self.palette.answering, self.palette.answer_failed) = (true, false);
        let mut ours = self.do_client_command(line);
        // A line of Lua is a command of the client too, carried out by
        // the scripts: what it prints is its answer, and if it prints
        // nothing, that it was run is.
        if !ours && (line == "/lua" || line.starts_with("/lua ")) {
            self.scripts.on_input(line);
            self.run_actions();
            if self.palette.answer.is_empty() {
                self.note(tr("Done."));
            }
            ours = true;
        }
        self.palette.answering = false;
        let mut said = std::mem::take(&mut self.palette.answer);
        match said.len() {
            0 => {}
            // A line of text is said in the command line itself.
            1 if said[0].run.is_none() => {
                self.palette.bar_note = Some(BarNote {
                    text: said.remove(0).text,
                    problem: self.palette.answer_failed,
                    since: None,
                });
            }
            // More than that is a list, to read and to choose from.
            _ => {
                let items = said.into_iter().map(|said| Item {
                    insert: said.run.unwrap_or_default(),
                    usage: said.text,
                    about: said.about,
                    more: false,
                    act: None,
                });
                self.palette.results = Some(items.collect());
                self.palette.query.clear();
                self.palette.open = true;
            }
        }
        ours
    }

    /// What the last command said in a line, while it is to be shown,
    /// and the colour to show it in: for a moment, fading at the end.
    pub(in crate::tab) fn bar_said(&mut self, ui: &egui::Ui) -> Option<(String, egui::Color32)> {
        let theme = theme::current();
        let now = ui.input(|input| input.time);
        let note = self.palette.bar_note.as_mut()?;
        let since = *note.since.get_or_insert(now);
        // A problem stays to be read; the rest goes by itself, sooner the
        // less it says.
        let mut fading = 1.0;
        if !note.problem {
            let lasts = (2.0 + 0.05 * note.text.chars().count() as f64).min(7.0);
            let left = since + lasts - now;
            if left <= 0.0 {
                self.palette.bar_note = None;
                return None;
            }
            if left < 0.4 {
                fading = (left / 0.4) as f32;
                ui.ctx().request_repaint();
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(left - 0.4));
            }
        }
        let colour = if note.problem {
            theme.danger
        } else {
            theme.text_dim
        };
        Some((note.text.clone(), colour.gamma_multiply(fading)))
    }
}
