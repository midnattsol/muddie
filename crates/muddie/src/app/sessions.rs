//! Opening, going through and closing sessions: each said aloud, for
//! whoever cannot see the bar of sessions.

use super::*;

impl App {
    pub(super) fn new_tab(&mut self, ctx: &egui::Context) {
        let startup = Startup {
            profiles: self.profiles.clone(),
            profile: None,
            target: None,
            speech: false,
            scripts: self.extra_scripts.clone(),
            download: self.updates.download.clone(),
        };
        self.tabs.push(Tab::new(ctx, startup, &self.settings));
        self.active = self.tabs.len() - 1;
    }

    /// Goes `ahead` sessions on, round from the last to the first, to its
    /// command line, saying which it is: whoever cannot see the bar of
    /// sessions hears where they are.
    pub(super) fn go_to_session(&mut self, ahead: usize) {
        let count = self.tabs.len();
        if count < 2 {
            return;
        }
        self.active = (self.active + ahead) % count;
        let tab = &mut self.tabs[self.active];
        tab.keys_to_the_line();
        let said = trf(
            "Session {0} of {1}: {2}",
            &[
                &(self.active + 1).to_string(),
                &count.to_string(),
                &tab.title(),
            ],
        );
        tab.say_aside(&said);
    }

    /// Closes a session, saying which and which is on screen now: whoever
    /// cannot see the bar of sessions hears where they are.
    pub(super) fn close_tab(&mut self, index: usize, ctx: &egui::Context) {
        let mut gone = self.tabs.remove(index);
        // What was just typed in the editor is saved, not lost with it.
        gone.close_automation();
        if self.tabs.is_empty() {
            self.new_tab(ctx);
        }
        if self.active > index || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        let count = self.tabs.len();
        let tab = &mut self.tabs[self.active];
        tab.keys_to_the_line();
        let said = trf(
            "Closed {0}. Session {1} of {2}: {3}",
            &[
                &gone.title(),
                &(self.active + 1).to_string(),
                &count.to_string(),
                &tab.title(),
            ],
        );
        tab.say_aside(&said);
    }
}
