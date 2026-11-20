//! What is asked before something is done: the name for a new profile, a
//! copy or another name, whether to delete one, and where to install a
//! plugin from.

use super::*;
use crate::profile::{Profile, SessionPrefs};

/// A question asked before something is done, with what was typed so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Ask {
    /// The name of a new, empty profile.
    New(String),
    /// The name to keep the session on screen under.
    SaveSession(String),
    Rename {
        from: String,
        to: String,
    },
    Duplicate {
        from: String,
        to: String,
    },
    Delete(String),
    /// The address of the git repository of a plugin to install.
    Install(String),
}

impl Ask {
    fn title(&self) -> &'static str {
        match self {
            Ask::New(_) => tr("New profile"),
            Ask::SaveSession(_) => tr("Save as profile"),
            Ask::Rename { .. } => tr("Rename profile"),
            Ask::Duplicate { .. } => tr("Duplicate profile"),
            Ask::Delete(_) => tr("Delete profile"),
            Ask::Install(_) => tr("Install a plugin"),
        }
    }

    /// What doing it is called, on its button.
    fn verb(&self) -> &'static str {
        match self {
            Ask::New(_) => tr("Create"),
            Ask::SaveSession(_) => tr("Save"),
            Ask::Rename { .. } => tr("Rename"),
            Ask::Duplicate { .. } => tr("Duplicate"),
            Ask::Delete(_) => tr("Delete"),
            Ask::Install(_) => tr("Install"),
        }
    }

    /// What is being typed, for those that ask for something, with what
    /// it is called and an example of it.
    fn typed_mut(&mut self) -> Option<(&mut String, &'static str, &'static str)> {
        match self {
            Ask::New(name) | Ask::SaveSession(name) => Some((name, tr("Name"), "")),
            Ask::Rename { to, .. } | Ask::Duplicate { to, .. } => Some((to, tr("Name"), "")),
            Ask::Install(address) => Some((
                address,
                tr("Address of its git repository"),
                "https://github.com/…/….git",
            )),
            Ask::Delete(_) => None,
        }
    }
}

impl App {
    /// Does what was chosen in a profile's menu, asking first what has
    /// to be asked.
    pub(super) fn profile_action(&mut self, action: ProfileAction, name: &str) {
        let Some(profiles) = &self.profiles else {
            return;
        };
        let (from, ask_error) = (name.to_owned(), None);
        self.asking = match action {
            ProfileAction::Edit => return self.edit_profile(name),
            ProfileAction::Duplicate => Some(Ask::Duplicate {
                to: profiles.unused_name(name),
                from,
            }),
            ProfileAction::Rename => Some(Ask::Rename {
                to: from.clone(),
                from,
            }),
            ProfileAction::Delete => Some(Ask::Delete(from)),
        };
        self.ask_error = ask_error;
    }

    /// Asks for a name to keep the session on screen under.
    pub(super) fn ask_save_session(&mut self) {
        let Some(profiles) = &self.profiles else {
            return;
        };
        let suggested = self.tabs[self.active].suggested_profile_name();
        self.asking = Some(Ask::SaveSession(profiles.unused_name(&suggested)));
        self.ask_error = None;
    }

    /// The question being asked, if any, over the window.
    pub(super) fn ask_window(&mut self, ctx: &egui::Context) {
        let Some(mut ask) = self.asking.clone() else {
            return;
        };
        let in_use = match &ask {
            Ask::Delete(name) => self.tabs.iter().any(|tab| tab.uses_profile(name)),
            _ => false,
        };
        let (mut open, mut answer) = (true, None);
        widgets::dialog(
            ctx,
            "profile question",
            ask.title(),
            &mut open,
            size::DIALOG_S,
            |ui| {
                ui.spacing_mut().item_spacing.y = size::GAP_S;
                answer = ask_body(ui, &mut ask, in_use, self.ask_error.as_deref());
            },
        );
        self.asking = (open && answer != Some(false)).then_some(ask.clone());
        if answer == Some(true) {
            self.answer(ask);
        }
    }

    /// Does what was asked, once answered; a name that cannot be taken
    /// is said in the question, which stays.
    fn answer(&mut self, ask: Ask) {
        let Some(profiles) = self.profiles.clone() else {
            return;
        };
        let done = match &ask {
            Ask::New(name) => self.create_profile(&profiles, name),
            Ask::SaveSession(name) => self.save_session_as(&profiles, name),
            Ask::Rename { from, to } => self.rename_profile(&profiles, from, to),
            Ask::Duplicate { from, to } => profiles
                .duplicate(from, to.trim())
                .map(|()| Some(to.trim().to_owned())),
            Ask::Delete(name) => self.delete_profile(&profiles, name),
            Ask::Install(address) => {
                let command = format!("/plugin install {}", address.trim());
                self.tabs[self.active].plugin_command(&command);
                Ok(None)
            }
        };
        match done {
            Ok(edit) => {
                self.asking = None;
                self.ask_error = None;
                self.tabs.iter_mut().for_each(Tab::profiles_changed);
                if let Some(name) = edit {
                    self.edit_profile(&name);
                }
            }
            Err(error) => self.ask_error = Some(error),
        }
    }

    /// Makes an empty profile, to be filled in. Returns its name.
    fn create_profile(
        &mut self,
        profiles: &Profiles,
        name: &str,
    ) -> Result<Option<String>, String> {
        let name = name.trim();
        if profiles.exists(name) {
            return Err(trf("There is a profile called {0} already.", &[name]));
        }
        let profile = Profile {
            session: Some(SessionPrefs::from(&self.settings)),
            ..Profile::default()
        };
        profiles.save(name, &profile)?;
        Ok(Some(name.to_owned()))
    }

    /// Keeps the session on screen as a profile. Returns its name.
    fn save_session_as(
        &mut self,
        profiles: &Profiles,
        name: &str,
    ) -> Result<Option<String>, String> {
        let name = name.trim();
        if !crate::profile::valid_name(name) {
            return Err(
                tr("A profile name may only have letters, digits, spaces, - and _.").to_owned(),
            );
        }
        if profiles.exists(name) {
            return Err(trf("There is a profile called {0} already.", &[name]));
        }
        self.tabs[self.active].save_as_profile(name);
        Ok(Some(name.to_owned()))
    }

    /// Gives a profile another name; the sessions playing with it go on
    /// with it under that one.
    fn rename_profile(
        &mut self,
        profiles: &Profiles,
        from: &str,
        to: &str,
    ) -> Result<Option<String>, String> {
        let to = to.trim();
        if to == from {
            return Ok(None);
        }
        profiles.rename(from, to)?;
        for tab in self.tabs.iter_mut().filter(|tab| tab.uses_profile(from)) {
            tab.profile_renamed(to);
        }
        let editing = self.editing.as_ref().is_some_and(|edit| edit.name == from);
        Ok(editing.then(|| to.to_owned()))
    }

    /// Deletes a profile no session plays with.
    fn delete_profile(
        &mut self,
        profiles: &Profiles,
        name: &str,
    ) -> Result<Option<String>, String> {
        profiles.delete(name)?;
        // The window goes on with another, if there is one.
        if self.editing.as_ref().is_some_and(|edit| edit.name == name) {
            self.editing = None;
            if self.show_profiles {
                self.open_profiles(None);
            }
        }
        Ok(None)
    }
}

/// What is asked: what to type, under what it is called, or what is
/// about to happen; then Cancel and the button that does it, which stays
/// greyed out until there is something to do it with. Returns true when
/// it is to be done, false when cancelled.
fn ask_body(ui: &mut Ui, ask: &mut Ask, in_use: bool, error: Option<&str>) -> Option<bool> {
    let verb = ask.verb();
    let mut ready = true;
    let mut entered = false;
    match ask {
        Ask::Delete(name) if in_use => {
            ui.label(trf(
                "{0} is in use by an open session: close it first.",
                &[name],
            ));
            ready = false;
        }
        Ask::Delete(name) => {
            ui.label(trf(
                "Delete {0}? Its scripts, rules, map and layout go with it.",
                &[name],
            ));
        }
        _ => {
            if let Some((typed, label, example)) = ask.typed_mut() {
                let theme = theme::current();
                let name = ui.label(
                    RichText::new(label)
                        .font(theme::font(Weight::Medium, 12.5))
                        .color(theme.text_dim),
                );
                let field = widgets::text_box(ui, typed, example).labelled_by(name.id);
                field.request_focus();
                ready = !typed.trim().is_empty();
                entered =
                    field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            }
        }
    }
    if matches!(ask, Ask::Install(_)) {
        widgets::page_note(
            ui,
            tr("Only install what you trust: a plugin can send commands as you."),
        );
    }
    if let Some(error) = error {
        ui.colored_label(theme::current().danger, error);
    }
    ui.add_space(size::GAP_M);
    let mut answer = None;
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        if ui
            .add_enabled_ui(ready, |ui| widgets::primary_button(ui, verb))
            .inner
            .clicked()
        {
            answer = Some(true);
        }
        if widgets::button(ui, tr("Cancel")).clicked() {
            answer = Some(false);
        }
    });
    if ready && entered {
        answer = Some(true);
    }
    answer
}
