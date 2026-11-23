//! Keeping a profile as it is changed, and handing it to the sessions that
//! play with it.

use super::*;

/// Whether two profiles differ only in what is typed letter by letter,
/// which is saved once typing stops rather than at every key.
pub(in crate::app) fn only_typed(now: &Profile, before: &Profile) -> bool {
    let mut same = now.clone();
    same.host.clone_from(&before.host);
    same.character.clone_from(&before.character);
    same == *before
}

impl App {
    /// Keeps the profile being edited: what is switched at once, what is
    /// typed once typing stops, and all of it when the settings close.
    pub(in crate::app) fn keep_profile(&mut self, ctx: &egui::Context, open: bool) {
        self.reread_profile();
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        if edit.draft != edit.seen {
            let typed_only = only_typed(&edit.draft, &edit.seen);
            edit.seen = edit.draft.clone();
            edit.settle.changed();
            if !typed_only {
                edit.settle.clear();
                return self.save_profile_now();
            }
        }
        let due = edit.settle.due(ctx);
        if due || (!open && edit.draft != edit.saved) {
            self.save_profile_now();
        }
        if !open {
            self.keep_password();
        }
    }

    /// Writes the profile being edited, and hands it to the sessions that
    /// play with it.
    pub(in crate::app) fn save_profile_now(&mut self) {
        let (Some(profiles), Some(edit)) = (&self.profiles, self.editing.as_mut()) else {
            return;
        };
        edit.settle.clear();
        self.profile_error = None;
        if let Err(error) = profiles.save(&edit.name, &edit.draft) {
            self.profile_error = Some(trf("Could not save the profile: {0}", &[&error]));
            return;
        }
        edit.saved = edit.draft.clone();
        edit.seen = edit.draft.clone();
        for tab in self
            .tabs
            .iter_mut()
            .filter(|tab| tab.uses_profile(&edit.name))
        {
            tab.adopt_profile(&edit.draft);
        }
    }

    /// When a session wrote the profile being edited (a plugin switched
    /// from a command, a channel kept quiet, say), takes all it wrote,
    /// keeping only what is being typed in the window and not saved yet:
    /// what the window saves next is not what was there before.
    pub(in crate::app) fn reread_profile(&mut self) {
        let sessions = self.tabs.iter_mut().chain(self.profile_session.as_mut());
        let written = sessions
            .map(Tab::take_profile_written)
            .fold(false, |any, one| any | one);
        let (Some(profiles), Some(edit), true) = (&self.profiles, self.editing.as_mut(), written)
        else {
            return;
        };
        let Ok(stored) = profiles.load(&edit.name) else {
            return;
        };
        let mut draft = stored.clone();
        if edit.draft.host != edit.saved.host {
            draft.host.clone_from(&edit.draft.host);
        }
        if edit.draft.character != edit.saved.character {
            draft.character.clone_from(&edit.draft.character);
        }
        edit.saved = stored;
        edit.seen = draft.clone();
        edit.draft = draft;
    }

    /// Puts the password typed in the keyring, under the profile.
    pub(in crate::app) fn keep_password(&mut self) {
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        if edit.password.is_empty() {
            return;
        }
        let password = std::mem::take(&mut edit.password);
        if let Err(error) = crate::secrets::set_password(&edit.name, Some(&password)) {
            self.profile_error = Some(trf(
                "The password could not be saved in the keyring: {0}",
                &[&error],
            ));
        }
    }
}
