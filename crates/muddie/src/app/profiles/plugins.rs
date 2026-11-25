//! A profile's plugins: those that come with Muddie, the official ones and
//! those from git, each switched on and off alike, and downloading one.

use super::*;
use crate::plugins::{Origin, Plugin};

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
/// one installed, installing it again; for one from git, updating and
/// removing it.
fn plugin_row(ui: &mut Ui, plugin: &Plugin, draft: &mut Profile) -> Option<PluginAsk> {
    let mut on = plugin.is_on(&draft.plugins_on, &draft.plugins_off);
    let (name, about) = (plugin.shown_name(), plugin.about());
    let installed = draft.plugins_installed.contains_key(&plugin.id);
    let git = plugin.is_git();
    let mut asked = None;
    let switch = widgets::setting_row(ui, &name, Some(&about), &mut |ui| {
        let switch = widgets::switch(ui, &mut on, &name);
        if installed || git {
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
                    (git, PluginAsk::Update, tr("Update")),
                    (git, PluginAsk::Remove, tr("Remove…")),
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
    // That on is all it takes is said: there is nothing else to install.
    let said = match plugin.origin {
        Origin::Included => trf(
            "{0} Comes with Muddie: on, it sets itself up when you connect.",
            &[&about],
        ),
        Origin::Official => trf(
            "{0} Official, kept up to date by Muddie: on, it sets itself up when you connect.",
            &[&about],
        ),
        Origin::Git => trf(
            "{0} Downloaded from git: on, it sets itself up when you connect.",
            &[&about],
        ),
    };
    (ui.ctx()).accesskit_node_builder(switch.id, |node| node.set_description(said));
    if switch.changed() {
        plugin.switch(&mut draft.plugins_on, &mut draft.plugins_off, on);
        // Switched off, it is not switched on again by itself on
        // connecting to its MUD.
        draft.plugins_declined.retain(|id| *id != plugin.id);
        if !on {
            draft.plugins_declined.push(plugin.id.clone());
        }
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
        let from_git: Vec<&Plugin> = (plugins.iter()).filter(|plugin| plugin.is_git()).collect();
        let (mut updating, mut install, mut check) = (Vec::new(), false, false);
        widgets::section_caption(ui, tr("Plugins"));
        widgets::page_note(
            ui,
            tr("Switch a plugin on to use it: there is nothing else to install."),
        );
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
        // Those others make, after: what is seldom wanted, last.
        ui.add_space(size::GAP_S);
        ui.horizontal(|ui| {
            install = widgets::button(ui, tr("Install from git…")).clicked();
            if !from_git.is_empty() && widgets::button(ui, tr("Update all")).clicked() {
                updating.extend(from_git.iter().map(|plugin| plugin.name.clone()));
            }
            check = widgets::button(ui, tr("Check for plugin updates")).clicked();
        });
        widgets::page_note(
            ui,
            tr(
                "Official plugins are kept up to date by Muddie. For plugins others make, install them from the address of their git repository.",
            ),
        );
        if check {
            self.look_for_plugin_updates();
        }
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
