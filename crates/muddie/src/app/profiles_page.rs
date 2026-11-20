//! The Profiles page of the settings: the saved profiles in a list, and
//! one of them, chosen there, in a page of its own. What is changed is
//! saved as it is changed, and the sessions playing with that profile
//! take it.

use super::*;
use crate::autosave::Settle;
use crate::profile::{Profile, SessionPrefs};

mod groups;
mod keeping;
mod list;
mod page;
mod plugins;

use groups::*;

/// What can be done with a saved profile, from wherever it is listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileAction {
    Edit,
    Duplicate,
    Rename,
    Delete,
}

/// The menu of a saved profile: what it offers, and the one chosen.
pub fn profile_menu(ui: &mut Ui) -> Option<ProfileAction> {
    widgets::menu_width(ui);
    let items = [
        (ProfileAction::Edit, tr("Edit")),
        (ProfileAction::Duplicate, tr("Duplicate")),
        (ProfileAction::Rename, tr("Rename")),
        (ProfileAction::Delete, tr("Delete")),
    ];
    let mut chosen = None;
    for (action, label) in items {
        if action == ProfileAction::Delete {
            widgets::menu_separator(ui);
        }
        if widgets::menu_item(ui, label, "").clicked() {
            chosen = Some(action);
            ui.close();
        }
    }
    chosen
}

/// A profile open in the settings: as saved, and as being changed.
pub(super) struct ProfileEdit {
    pub name: String,
    pub(in crate::app) draft: Profile,
    pub(in crate::app) saved: Profile,
    pub(in crate::app) seen: Profile,
    pub(in crate::app) settle: Settle,
    /// A password typed, to be put in the keyring.
    pub(in crate::app) password: String,
}

impl App {
    /// Opens a saved profile in the settings.
    pub fn edit_profile(&mut self, name: &str) {
        let Some(profiles) = &self.profiles else {
            return;
        };
        match profiles.load(name) {
            Ok(profile) => {
                let mut draft = profile.clone();
                // One saved before it kept these has the settings'.
                draft
                    .session
                    .get_or_insert_with(|| SessionPrefs::from(&self.settings));
                self.editing = Some(ProfileEdit {
                    name: name.to_owned(),
                    seen: draft.clone(),
                    draft,
                    saved: profile,
                    settle: Settle::default(),
                    password: String::new(),
                });
                self.open_settings(Section::Profiles);
            }
            Err(error) => self.settings_error = Some(error),
        }
    }
}
