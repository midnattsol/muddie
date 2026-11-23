//! The Profiles window: the saved profiles in a list at the left, and the
//! one chosen at the right, its parts in tabs. What is changed is saved
//! as it is changed, and the sessions playing with that profile take it.

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
/// Editing it is choosing it, so that is not in it.
pub fn profile_menu(ui: &mut Ui) -> Option<ProfileAction> {
    widgets::menu_width(ui);
    let items = [
        (ProfileAction::Duplicate, tr("Duplicate…")),
        (ProfileAction::Rename, tr("Rename…")),
        (ProfileAction::Delete, tr("Delete…")),
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

/// The parts of a profile, each in a tab of its page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ProfileTab {
    /// Where it connects, and who logs in.
    #[default]
    Connection,
    /// How the connection is kept, logged, and the sounds asked for.
    Session,
    Plugins,
}

impl ProfileTab {
    pub const ALL: [ProfileTab; 3] = [
        ProfileTab::Connection,
        ProfileTab::Session,
        ProfileTab::Plugins,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ProfileTab::Connection => tr("Connection"),
            ProfileTab::Session => tr("Session"),
            ProfileTab::Plugins => tr("Plugins"),
        }
    }
}

/// The profile chosen in the Profiles window: as saved, and as being
/// changed.
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
    /// Opens the Profiles window on the profile of this name; with none,
    /// on the one chosen before, or the session's, or the first.
    pub fn open_profiles(&mut self, name: Option<&str>) {
        self.show_profiles = true;
        let chosen = self.editing.as_ref().map(|edit| edit.name.clone());
        let session = self.tabs[self.active].profile_name().map(str::to_owned);
        let first =
            (self.profiles.as_ref()).and_then(|profiles| profiles.names().into_iter().next());
        let wanted = name.map(str::to_owned).or(chosen).or(session).or(first);
        let exists = |name: &String| {
            self.profiles
                .as_ref()
                .is_some_and(|profiles| profiles.exists(name))
        };
        match wanted.filter(exists) {
            Some(name) => self.edit_profile(&name),
            None => self.editing = None,
        }
    }

    /// Chooses a saved profile in the Profiles window, opening it; the one
    /// chosen before is kept first.
    pub fn edit_profile(&mut self, name: &str) {
        self.show_profiles = true;
        if self.editing.as_ref().is_some_and(|edit| edit.name == name) {
            return;
        }
        self.keep_chosen_profile();
        let Some(profiles) = &self.profiles else {
            return;
        };
        match profiles.load(name) {
            Ok(profile) => {
                let mut draft = profile.clone();
                // One saved before it kept these has the settings'.
                draft.session.get_or_insert_with(SessionPrefs::default);
                self.editing = Some(ProfileEdit {
                    name: name.to_owned(),
                    seen: draft.clone(),
                    draft,
                    saved: profile,
                    settle: Settle::default(),
                    password: String::new(),
                });
            }
            Err(error) => self.profile_error = Some(error),
        }
    }

    /// The Profiles window: the list at the left, the profile chosen at
    /// the right, both as tall as the window allows.
    pub(super) fn profiles_window(&mut self, ctx: &egui::Context) {
        if !self.show_profiles {
            return self.keep_profile(ctx, false);
        }
        let mut open = true;
        let height = (ctx.content_rect().height() - 160.0).clamp(320.0, 640.0);
        widgets::dialog(
            ctx,
            "profiles",
            tr("Profiles"),
            &mut open,
            size::DIALOG_XL,
            |ui| {
                ui.horizontal_top(|ui| {
                    let list = egui::vec2(LIST_WIDTH, height);
                    ui.allocate_ui_with_layout(
                        list,
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_min_size(list);
                            self.profiles_list(ui);
                        },
                    );
                    ui.add_space(size::GAP_L - ui.spacing().item_spacing.x);
                    let page = egui::vec2(ui.available_width(), height);
                    ui.allocate_ui_with_layout(
                        page,
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_min_size(page);
                            self.profile_page(ui, ctx);
                        },
                    );
                });
            },
        );
        self.show_profiles &= open;
        self.keep_profile(ctx, self.show_profiles);
    }

    /// Saves what was changed of the profile chosen, and its password,
    /// before another is chosen or the window closes.
    fn keep_chosen_profile(&mut self) {
        if self
            .editing
            .as_ref()
            .is_some_and(|edit| edit.draft != edit.saved)
        {
            self.save_profile_now();
        }
        self.keep_password();
    }
}

#[cfg(test)]
impl App {
    /// Shows a tab of the profile chosen, as clicking it would.
    pub fn show_profile_tab(&mut self, tab: ProfileTab) {
        self.profile_tab = tab;
    }
}

/// How wide the list of profiles is.
const LIST_WIDTH: f32 = 240.0;
