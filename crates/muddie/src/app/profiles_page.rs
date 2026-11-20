//! The Profiles page of the settings: the saved profiles in a list, and
//! one of them, chosen there, in a page of its own. What is changed is
//! saved as it is changed, and the sessions playing with that profile
//! take it.

use super::*;
use crate::autosave::Settle;
use crate::profile::{Profile, SessionPrefs};

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
    draft: Profile,
    saved: Profile,
    seen: Profile,
    settle: Settle,
    /// A password typed, to be put in the keyring.
    password: String,
}

/// The text encodings offered, by the label the decoder knows them by.
const ENCODINGS: [(&str, &str); 9] = [
    ("auto", ""),
    ("utf-8", "UTF-8"),
    ("windows-1252", "Latin-1 (Windows-1252)"),
    ("iso-8859-15", "Latin-9 (ISO-8859-15)"),
    ("koi8-r", "KOI8-R"),
    ("big5", "Big5"),
    ("gbk", "GBK"),
    ("shift_jis", "Shift JIS"),
    ("euc-kr", "EUC-KR"),
];

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

    /// The saved profiles, each with where it leads, whether a session
    /// plays with it, and its menu; and the way to make another.
    pub(super) fn profiles_list(&mut self, ui: &mut Ui) {
        let Some(profiles) = self.profiles.clone() else {
            widgets::row_note(
                ui,
                tr("Profiles need a configuration folder, and none was found."),
            );
            return;
        };
        let names = profiles.names();
        if names.is_empty() {
            widgets::row_note(
                ui,
                tr("No profiles yet. A profile keeps a MUD's connection, scripts, map and layout."),
            );
        }
        let mut asked = None;
        widgets::group(ui, |ui| {
            for name in &names {
                let place = (profiles.load(name).ok())
                    .map(|profile| format!("{}:{}", profile.host, profile.port))
                    .unwrap_or_default();
                let in_use = self.tabs.iter().any(|tab| tab.uses_profile(name));
                let badge = in_use.then(|| tr("In use"));
                let (open, more) = widgets::link_row(ui, name, &place, badge);
                if open.clicked() {
                    asked = Some((ProfileAction::Edit, name.clone()));
                }
                egui::Popup::menu(&more).show(|ui| {
                    if let Some(action) = profile_menu(ui) {
                        asked = Some((action, name.clone()));
                    }
                });
            }
        });
        ui.add_space(size::GAP_S);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if widgets::button(ui, tr("New profile")).clicked() {
                self.asking = Some(Ask::New(profiles.unused_name(tr("New profile"))));
            }
        });
        if let Some((action, name)) = asked {
            self.profile_action(action, &name);
        }
    }

    /// One profile's page: the way back, its name and menu, then what it
    /// has, in groups.
    pub(super) fn profile_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(name) = self.editing.as_ref().map(|edit| edit.name.clone()) else {
            return;
        };
        self.profile_heading(ui, &name);
        self.reconnect_notice(ui, ctx, &name);
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        server_group(ui, &mut edit.draft);
        login_group(ui, edit);
        let prefs = edit.draft.session.get_or_insert_with(SessionPrefs::default);
        keeping_group(ui, prefs);
        logs_group(ui, prefs);
        self.plugins_groups(ui);
        ui.add_space(size::GAP_M);
        self.connect_button(ui, ctx, &name);
    }

    /// "‹ Profiles", and the profile's name with its menu.
    fn profile_heading(&mut self, ui: &mut Ui, name: &str) {
        if widgets::quiet_button(ui, &format!("‹ {}", tr("Profiles"))).clicked() {
            self.editing = None;
            return;
        }
        let mut asked = None;
        ui.horizontal(|ui| {
            let title = RichText::new(name)
                .font(theme::font(Weight::Semibold, 18.0))
                .color(theme::current().text);
            ui.add(egui::Label::new(title).selectable(false));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = trf("{0} options", &[name]);
                let more = widgets::icon_button(ui, widgets::Icon::More, &label, false);
                egui::Popup::menu(&more).show(|ui| asked = profile_menu(ui));
            });
        });
        if let Some(action) = asked {
            self.profile_action(action, name);
        }
    }

    /// With the session on screen connected with this profile otherwise
    /// than it now says, that what changed waits for reconnecting.
    fn reconnect_notice(&mut self, ui: &mut Ui, ctx: &egui::Context, name: &str) {
        let tab = &mut self.tabs[self.active];
        if !tab.uses_profile(name) || !tab.connection_outdated() {
            return;
        }
        let mut again = false;
        widgets::notice_bar(
            ui,
            tr("The changes take effect on reconnecting."),
            &mut |ui| {
                again = widgets::primary_button(ui, tr("Reconnect")).clicked();
            },
        );
        if again {
            tab.reconnect(ctx);
            self.show_settings = false;
        }
    }

    /// Connects with this profile: in the session on screen if it is not
    /// connected, or else in a new one.
    fn connect_button(&mut self, ui: &mut Ui, ctx: &egui::Context, name: &str) {
        let mut connect = false;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            connect = widgets::primary_button(ui, tr("Connect")).clicked();
        });
        if !connect {
            return;
        }
        self.keep_profile(ctx, false);
        if self.tabs[self.active].is_connected() {
            self.new_tab(ctx);
        }
        self.tabs[self.active].play(name, ctx);
        self.show_settings = false;
    }

    /// The groups of the plugins: those that come with Muddie, those
    /// installed, and installing one.
    fn plugins_groups(&mut self, ui: &mut Ui) {
        widgets::caption(ui, tr("Plugins"));
        self.included_group(ui);
        ui.add_space(size::GAP_S);
        self.installed_group(ui);
        ui.add_space(size::GAP_S);
        self.install_group(ui);
    }

    /// The plugins that come with Muddie: each with its switch, what it
    /// does, and for one in Luau the way to make it one's own.
    fn included_group(&mut self, ui: &mut Ui) {
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        let mut customize = None;
        widgets::group(ui, |ui| {
            for plugin in crate::bundled::ALL.iter() {
                let draft = &mut edit.draft;
                let mut on = plugin.is_on(&draft.included_on, &draft.included_off);
                let name = tr(plugin.name);
                let switch = widgets::setting_row(ui, name, &mut |ui| {
                    let switch = widgets::switch(ui, &mut on, name);
                    if plugin.code.is_some() {
                        ui.add_space(size::GAP_S);
                        let pressed = widgets::quiet_button(ui, tr("Customize"));
                        let label = trf("Customize {0}", &[name]);
                        pressed.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Button, true, &label)
                        });
                        if pressed.clicked() {
                            customize = Some(plugin);
                        }
                    }
                    switch
                });
                if switch.changed() {
                    plugin.switch(&mut draft.included_on, &mut draft.included_off, on);
                }
                widgets::row_note(ui, tr(plugin.about));
            }
        });
        if let Some(plugin) = customize {
            self.customize(plugin);
        }
    }

    /// The plugins installed: each with its switch in this profile and
    /// its menu, and the way to update them all.
    fn installed_group(&mut self, ui: &mut Ui) {
        let installed = self
            .profiles
            .as_ref()
            .map(Profiles::packages)
            .unwrap_or_default();
        let mut command = None;
        ui.horizontal(|ui| {
            widgets::caption(ui, tr("Installed by you"));
            if !installed.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::quiet_button(ui, tr("Update all")).clicked() {
                        command = Some(
                            installed
                                .iter()
                                .map(|name| format!("/plugin update {name}"))
                                .collect::<Vec<_>>(),
                        );
                    }
                });
            }
        });
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        if installed.is_empty() {
            widgets::row_note(
                ui,
                tr(
                    "No plugins yet. A plugin is a set of scripts someone made for a MUD: a mapper, triggers, gauges.",
                ),
            );
        } else {
            widgets::group(ui, |ui| {
                for name in &installed {
                    if let Some(asked) = installed_row(ui, &mut edit.draft.packages, name) {
                        command = Some(vec![asked]);
                    }
                }
            });
        }
        for command in command.into_iter().flatten() {
            self.tabs[self.active].plugin_command(&command);
        }
    }

    /// Installing a plugin from its address, and what the last commands
    /// about plugins said.
    fn install_group(&mut self, ui: &mut Ui) {
        let mut install = false;
        widgets::group(ui, |ui| {
            widgets::setting_row(ui, tr("Install from"), &mut |ui| {
                let ready = !self.plugin_address.trim().is_empty();
                install = ui
                    .add_enabled_ui(ready, |ui| widgets::button(ui, tr("Install")))
                    .inner
                    .clicked();
                widgets::row_field(
                    ui,
                    &mut self.plugin_address,
                    "https://github.com/…/….git",
                    size::FIELD_L,
                )
            });
            widgets::row_note(
                ui,
                tr(
                    "The address of the plugin's git repository. Only install what you trust: a plugin can send commands as you.",
                ),
            );
        });
        if install {
            let address = std::mem::take(&mut self.plugin_address);
            self.tabs[self.active].plugin_command(&format!("/plugin install {}", address.trim()));
        }
        let (said, busy) = self.tabs[self.active].plugin_answers();
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                widgets::row_note(ui, tr("Working…"));
            });
        }
        for line in said {
            widgets::row_note(ui, &line);
        }
    }

    /// Makes a plugin that comes with Muddie the profile's own: its code
    /// becomes a script of the profile's, to change at will, and the one
    /// that came is switched off so as not to run twice.
    fn customize(&mut self, plugin: &'static crate::bundled::Bundled) {
        let (Some(profiles), Some(edit), Some(code)) =
            (self.profiles.clone(), self.editing.as_mut(), plugin.code)
        else {
            return;
        };
        let path = profiles
            .scripts_dir(&edit.name)
            .join(format!("{}.luau", plugin.id));
        // One made before is kept as it is: it may have been changed.
        if !path.exists()
            && let Err(error) = std::fs::write(&path, code)
        {
            self.settings_error =
                Some(trf("Could not save the script: {0}", &[&error.to_string()]));
            return;
        }
        plugin.switch(
            &mut edit.draft.included_on,
            &mut edit.draft.included_off,
            false,
        );
        let name = edit.name.clone();
        self.save_profile_now();
        let active = self.active;
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            if tab.uses_profile(&name) {
                tab.adopt_script(&path, index == active);
            }
        }
        if self.tabs[active].uses_profile(&name) {
            self.show_settings = false;
        }
        self.tabs[active].note(&trf(
            "{0} is now a script of profile {1}.",
            &[tr(plugin.name), &name],
        ));
    }

    /// Keeps the profile being edited: what is switched at once, what is
    /// typed once typing stops, and all of it when the settings close.
    pub(super) fn keep_profile(&mut self, ctx: &egui::Context, open: bool) {
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
    fn save_profile_now(&mut self) {
        let (Some(profiles), Some(edit)) = (&self.profiles, self.editing.as_mut()) else {
            return;
        };
        edit.settle.clear();
        if let Err(error) = profiles.save(&edit.name, &edit.draft) {
            self.settings_error = Some(trf("Could not save the profile: {0}", &[&error]));
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
    /// from a command, say), takes what it wrote of the plugins.
    fn reread_profile(&mut self) {
        let written = self
            .tabs
            .iter_mut()
            .map(Tab::take_profile_written)
            .fold(false, |any, one| any | one);
        let (Some(profiles), Some(edit), true) = (&self.profiles, self.editing.as_mut(), written)
        else {
            return;
        };
        if let Ok(stored) = profiles.load(&edit.name) {
            for draft in [&mut edit.draft, &mut edit.saved, &mut edit.seen] {
                draft.packages.clone_from(&stored.packages);
                draft.included_on.clone_from(&stored.included_on);
                draft.included_off.clone_from(&stored.included_off);
                draft
                    .included_declined
                    .clone_from(&stored.included_declined);
            }
        }
    }

    /// Puts the password typed in the keyring, under the profile.
    fn keep_password(&mut self) {
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        if edit.password.is_empty() {
            return;
        }
        let password = std::mem::take(&mut edit.password);
        if let Err(error) = crate::secrets::set_password(&edit.name, Some(&password)) {
            self.settings_error = Some(trf(
                "The password could not be saved in the keyring: {0}",
                &[&error],
            ));
        }
    }
}

/// Whether two profiles differ only in what is typed letter by letter,
/// which is saved once typing stops rather than at every key.
fn only_typed(now: &Profile, before: &Profile) -> bool {
    let mut same = now.clone();
    same.host.clone_from(&before.host);
    same.character.clone_from(&before.character);
    same == *before
}

/// Where the profile connects to.
fn server_group(ui: &mut Ui, draft: &mut Profile) {
    widgets::caption(ui, tr("Server"));
    widgets::group(ui, |ui| {
        widgets::setting_row(ui, tr("Host"), &mut |ui| {
            widgets::row_field(ui, &mut draft.host, "mud.example.org", size::FIELD_L)
        });
        widgets::setting_row(ui, tr("Port"), &mut |ui| {
            ui.add(widgets::number(&mut draft.port).range(1..=65535))
        });
        widgets::switch_setting(ui, &mut draft.tls, tr("Secure connection (TLS)"));
        if draft.tls {
            widgets::switch_setting(ui, &mut draft.verify_certificate, tr("Verify certificate"));
            widgets::row_note(
                ui,
                tr("Off only for a server with a certificate of its own making."),
            );
        }
        widgets::setting_row(ui, tr("Text encoding"), &mut |ui| {
            encoding_choice(ui, &mut draft.encoding)
        });
        widgets::row_note(
            ui,
            tr("Automatic suits nearly every MUD: UTF-8, or Latin-1 for what is not."),
        );
    });
}

/// The text encoding, from those offered; one set by hand is shown as it is.
fn encoding_choice(ui: &mut Ui, encoding: &mut String) -> egui::Response {
    let current = encoding.trim().to_lowercase();
    let current = if current.is_empty() {
        "auto".to_owned()
    } else {
        current
    };
    let name = |label: &str, shown: &str| match label {
        "auto" => tr("Automatic").to_owned(),
        _ => shown.to_owned(),
    };
    let chosen = (ENCODINGS.iter())
        .find(|(label, _)| *label == current)
        .map_or(encoding.clone(), |(label, shown)| name(label, shown));
    widgets::dropdown("encoding")
        .selected_text(chosen)
        .width(size::FIELD_L)
        .show_ui(ui, |ui| {
            for (label, shown) in ENCODINGS {
                if widgets::choice(ui, label == current, &name(label, shown)).clicked() {
                    *encoding = label.to_owned();
                }
            }
        })
        .response
}

/// Who to log in as.
fn login_group(ui: &mut Ui, edit: &mut ProfileEdit) {
    widgets::caption(ui, tr("Log in"));
    widgets::group(ui, |ui| {
        widgets::setting_row(ui, tr("Character"), &mut |ui| {
            widgets::row_field(ui, &mut edit.draft.character, "", size::FIELD_L)
        });
        widgets::row_note(ui, tr("Muddie answers the first prompt with it."));
        widgets::setting_row(ui, tr("Password"), &mut |ui| {
            widgets::secret_field(
                ui,
                &mut edit.password,
                tr("Leave empty to keep it"),
                size::FIELD_L,
            )
        });
        widgets::row_note(
            ui,
            tr(
                "Kept in the system's keyring, never in a file, and sent when the server hides typing.",
            ),
        );
    });
}

/// Connecting again when the connection drops, and keeping it alive.
fn keeping_group(ui: &mut Ui, prefs: &mut SessionPrefs) {
    widgets::caption(ui, tr("Keeping the connection"));
    widgets::group(ui, |ui| {
        widgets::switch_setting(
            ui,
            &mut prefs.reconnect,
            tr("Reconnect when the connection is lost"),
        );
        widgets::row_note(ui, tr("When it drops without you closing it."));
        let mut alive = prefs.keepalive_seconds > 0;
        if widgets::switch_setting(ui, &mut alive, tr("Keep the connection alive")).changed() {
            prefs.keepalive_seconds = if alive { 60 } else { 0 };
        }
        if alive {
            widgets::setting_row(ui, tr("Every"), &mut |ui| {
                ui.add(
                    widgets::number(&mut prefs.keepalive_seconds)
                        .range(5..=3600)
                        .suffix(" s"),
                )
            });
        }
        widgets::row_note(
            ui,
            tr("Some servers and routers close a connection that has been quiet for a while."),
        );
    });
}

/// The session's log, and the sounds the MUD asks for.
fn logs_group(ui: &mut Ui, prefs: &mut SessionPrefs) {
    use crate::settings::LogFormat;
    widgets::caption(ui, tr("Logs and sounds"));
    widgets::group(ui, |ui| {
        widgets::switch_setting(ui, &mut prefs.log_sessions, tr("Log every session"));
        widgets::row_note(
            ui,
            tr("Everything shown in the session, written to a file as it comes."),
        );
        if prefs.log_sessions {
            widgets::setting_row(ui, tr("Log format"), &mut |ui| {
                let name = |format: LogFormat| match format {
                    LogFormat::Text => tr("Text"),
                    LogFormat::Html => "HTML",
                };
                widgets::dropdown("log format")
                    .selected_text(name(prefs.log_format))
                    .width(size::FIELD_M)
                    .show_ui(ui, |ui| {
                        for format in [LogFormat::Text, LogFormat::Html] {
                            widgets::choice_value(ui, &mut prefs.log_format, format, name(format));
                        }
                    })
                    .response
            });
            widgets::row_note(ui, tr("HTML keeps the colours."));
        }
        widgets::switch_setting(
            ui,
            &mut prefs.media_downloads,
            tr("Download the sounds a MUD asks to play"),
        );
        widgets::row_note(
            ui,
            tr("A sound the MUD asks for that you do not have is fetched from the web and kept."),
        );
    });
}

/// An installed plugin: its switch in the profile, and its menu. Returns
/// the command its menu asked for.
fn installed_row(ui: &mut Ui, packages: &mut Vec<String>, name: &str) -> Option<String> {
    let mut asked = None;
    let mut enabled = packages.iter().any(|package| package == name);
    let switch = widgets::setting_row(ui, name, &mut |ui| {
        let more =
            widgets::icon_button(ui, widgets::Icon::More, &trf("{0} options", &[name]), false);
        egui::Popup::menu(&more).show(|ui| {
            widgets::menu_width(ui);
            if widgets::menu_item(ui, tr("Update"), "").clicked() {
                asked = Some(format!("/plugin update {name}"));
                ui.close();
            }
            if widgets::menu_item(ui, tr("Remove"), "").clicked() {
                asked = Some(format!("/plugin remove {name}"));
                ui.close();
            }
        });
        ui.add_space(size::GAP_S);
        widgets::switch(ui, &mut enabled, name)
    });
    if switch.changed() {
        packages.retain(|package| package != name);
        if enabled {
            packages.push(name.to_owned());
        }
    }
    asked
}
