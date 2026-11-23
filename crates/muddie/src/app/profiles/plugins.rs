//! A profile's plugins: those that come with Muddie and those downloaded,
//! each switched on and off alike, and downloading one.

use super::*;
use crate::plugins::Plugin;

/// What a plugin's menu asked for.
enum PluginAsk {
    /// Install it again, as it comes.
    Again,
    /// Bring one downloaded up to date.
    Update,
    /// Take one downloaded out of the profile.
    Remove,
}

/// A plugin: its switch in the profile, what it does, and its menu: for
/// one installed, installing it again; for one downloaded, updating and
/// removing it.
fn plugin_row(ui: &mut Ui, plugin: &Plugin, draft: &mut Profile) -> Option<PluginAsk> {
    let mut on = plugin.is_on(&draft.plugins_on, &draft.plugins_off);
    let (name, about) = (plugin.shown_name(), plugin.about());
    let installed = draft.plugins_installed.contains_key(&plugin.id);
    let downloaded = plugin.included.is_none();
    let mut asked = None;
    let switch = widgets::setting_row(ui, &name, Some(&about), &mut |ui| {
        let switch = widgets::switch(ui, &mut on, &name);
        if installed || downloaded {
            ui.add_space(size::GAP_S);
            let more = widgets::icon_button(
                ui,
                widgets::Icon::More,
                &trf("{0} options", &[&name]),
                false,
            );
            crate::widgets::menu_of(&more).show(|ui| {
                widgets::menu_width(ui);
                let items = [
                    (installed, PluginAsk::Again, tr("Install again")),
                    (downloaded, PluginAsk::Update, tr("Update")),
                    (downloaded, PluginAsk::Remove, tr("Remove…")),
                ];
                for (offered, ask, label) in items {
                    if offered && widgets::menu_item(ui, label, "").clicked() {
                        asked = Some(ask);
                        ui.close();
                    }
                }
            });
        }
        switch
    });
    if switch.changed() {
        plugin.switch(&mut draft.plugins_on, &mut draft.plugins_off, on);
    }
    asked
}

impl App {
    /// The plugins, each with its switch in this profile and its menu;
    /// downloading another and updating those downloaded, beside their
    /// caption; and what the last commands about plugins said.
    pub(in crate::app) fn plugins_groups(&mut self, ui: &mut Ui) {
        // All is done in the profile being edited, by a session of it.
        let Some(name) = self.editing.as_ref().map(|edit| edit.name.clone()) else {
            return;
        };
        let plugins = self.session_of(Some(&name)).plugins().to_vec();
        let downloaded: Vec<&Plugin> = (plugins.iter())
            .filter(|plugin| plugin.included.is_none())
            .collect();
        let (mut commands, mut install) = (Vec::new(), false);
        widgets::section_caption_with(ui, tr("Plugins"), |ui| {
            install = widgets::button(ui, tr("Install a plugin…")).clicked();
            if !downloaded.is_empty() && widgets::button(ui, tr("Update all")).clicked() {
                commands.extend(
                    (downloaded.iter()).map(|plugin| format!("/plugin update {}", plugin.name)),
                );
            }
        });
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        let mut again = None;
        widgets::group(ui, |ui| {
            for plugin in &plugins {
                match plugin_row(ui, plugin, &mut edit.draft) {
                    Some(PluginAsk::Again) => again = Some(plugin.clone()),
                    Some(PluginAsk::Update) => {
                        commands.push(format!("/plugin update {}", plugin.name));
                    }
                    Some(PluginAsk::Remove) => {
                        commands.push(format!("/plugin remove {}", plugin.name));
                    }
                    None => {}
                }
            }
        });
        if let Some(plugin) = again {
            self.install_again(&plugin);
        }
        if install {
            self.asking = Some(Ask::Install {
                address: String::new(),
                profile: Some(name.clone()),
            });
            self.ask_error = None;
        }
        for command in commands {
            self.session_of(Some(&name)).plugin_command(&command);
        }
        self.plugin_answers(ui, &name);
    }

    /// What the last commands about plugins said, under their card, and
    /// whether one is still at work.
    fn plugin_answers(&mut self, ui: &mut Ui, name: &str) {
        let (said, busy) = self.session_of(Some(name)).plugin_answers();
        if !busy && said.is_empty() {
            return;
        }
        ui.add_space(size::GAP_S);
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                widgets::page_note(ui, tr("Working…"));
            });
        }
        for line in said {
            widgets::page_note(ui, &line);
        }
    }

    /// Installs a plugin again in the profile being edited, in the
    /// sessions that use it: what it installed is put back as it came, and
    /// what was added is kept.
    pub(in crate::app) fn install_again(&mut self, plugin: &Plugin) {
        let Some(name) = self.editing.as_ref().map(|edit| edit.name.clone()) else {
            return;
        };
        // Installed once, in the profile; the others playing it run its
        // scripts again.
        self.session_of(Some(&name)).install_plugin_again(plugin);
        let first = self.tabs.iter().position(|tab| tab.uses_profile(&name));
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            if tab.uses_profile(&name) && Some(index) != first {
                tab.load_scripts();
            }
        }
    }
}
