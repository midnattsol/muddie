//! Running a command, and where what it answers is shown.

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
    pub fn refuse(&mut self, text: &str) {
        self.palette.answer_failed |= self.palette.answering;
        self.say(text, true);
    }

    /// Carries out a command of the client asked from the map, and shows
    /// what it answers. Returns whether it was one.
    pub(in crate::tab) fn run_command(&mut self, line: &str) -> bool {
        self.run_command_from(line, false)
    }

    /// Carries out a command of the client and shows what it answers where
    /// it was asked: a line, in the palette if it was run from there, else
    /// over the corner as the client says anything; a list, in the
    /// palette, under the command, to read and to choose from. Returns
    /// whether it was one.
    pub(in crate::tab) fn run_command_from(&mut self, line: &str, from_palette: bool) -> bool {
        self.palette.results = None;
        self.palette.title = None;
        self.palette.message = None;
        self.palette.cache = None;
        self.palette.chosen = 0;
        self.palette.answer.clear();
        (self.palette.answering, self.palette.answer_failed) = (true, false);
        let ours = self.do_client_command(line);
        self.palette.answering = false;
        let said = std::mem::take(&mut self.palette.answer);
        let failed = self.palette.answer_failed;
        let list = said.len() > 1 || said.iter().any(|said| said.run.is_some());
        match (said.first(), list, from_palette) {
            (None, ..) => {}
            // A line asked from elsewhere, as the map: over the corner.
            (Some(_), false, false) => {
                if let Some(notice) = self.notices.list.last_mut() {
                    notice.corner = true;
                }
            }
            // A line asked of the palette: in it.
            (Some(said), false, true) => {
                self.palette.message = Some((said.text.clone(), failed));
                self.palette.open = true;
            }
            // A list, to read and to choose from, under what was asked.
            (Some(_), true, _) => {
                let items = said.into_iter().map(|said| Item {
                    insert: said.run.unwrap_or_default(),
                    usage: said.text,
                    group: String::new(),
                    about: said.about,
                    act: None,
                });
                self.palette.results = Some(items.collect());
                // Under what was chosen, by its name; never the line.
                self.palette.title = self.palette.doing.take();
                self.palette.query.clear();
                self.palette.open = true;
            }
        }
        ours
    }
}
