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
    let downloaded = plugin.downloaded;
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
    /// caption. What is done says how it went over the corner.
    pub(in crate::app) fn plugins_groups(&mut self, ui: &mut Ui) {
        // All is done in the profile being edited, by a session of it.
        let Some(name) = self.editing.as_ref().map(|edit| edit.name.clone()) else {
            return;
        };
        let plugins = self.session_of(Some(&name)).plugins().to_vec();
        let downloaded: Vec<&Plugin> = (plugins.iter())
            .filter(|plugin| plugin.downloaded)
            .collect();
        let (mut updating, mut install) = (Vec::new(), false);
        widgets::section_caption_with(ui, tr("Plugins"), |ui| {
            install = widgets::button(ui, tr("Install a plugin…")).clicked();
            if !downloaded.is_empty() && widgets::button(ui, tr("Update all")).clicked() {
                updating.extend(downloaded.iter().map(|plugin| plugin.name.clone()));
            }
        });
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        let (mut again, mut removing) = (None, None);
        widgets::group(ui, |ui| {
            for plugin in &plugins {
                match plugin_row(ui, plugin, &mut edit.draft) {
                    Some(PluginAsk::Again) => again = Some(plugin.clone()),
                    Some(PluginAsk::Update) => updating.push(plugin.name.clone()),
                    Some(PluginAsk::Remove) => removing = Some(plugin.name.clone()),
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
        // Taking one away, files and all, is asked once more.
        if let Some(plugin) = removing {
            self.asking = Some(Ask::RemovePlugin {
                name: plugin,
                profile: Some(name.clone()),
            });
            self.ask_error = None;
        }
        for plugin in updating {
            self.session_of(Some(&name)).update_plugin(&plugin);
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
