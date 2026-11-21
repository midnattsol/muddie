//! A profile's plugins: those that come with Muddie, those installed, and
//! installing one.

use super::*;

/// An installed plugin: its switch in the profile, and its menu. Returns
/// the command its menu asked for.
pub(in crate::app) fn installed_row(
    ui: &mut Ui,
    packages: &mut Vec<String>,
    name: &str,
) -> Option<String> {
    let mut asked = None;
    let mut enabled = packages.iter().any(|package| package == name);
    let switch = widgets::setting_row(ui, name, None, &mut |ui| {
        let switch = widgets::switch(ui, &mut enabled, name);
        ui.add_space(size::GAP_S);
        let more =
            widgets::icon_button(ui, widgets::Icon::More, &trf("{0} options", &[name]), false);
        crate::widgets::menu_of(&more).show(|ui| {
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
        switch
    });
    if switch.changed() {
        packages.retain(|package| package != name);
        if enabled {
            packages.push(name.to_owned());
        }
    }
    asked
}

/// A plugin that comes with Muddie: its switch, what it does, and for one
/// installed in the profile, a menu to install it again. Returns whether
/// that was asked.
fn included_row(ui: &mut Ui, plugin: &crate::bundled::Bundled, draft: &mut Profile) -> bool {
    let mut on = plugin.is_on(&draft.included_on, &draft.included_off);
    let name = tr(plugin.name);
    let mut again = false;
    let switch = widgets::setting_row(ui, name, Some(tr(plugin.about)), &mut |ui| {
        let switch = widgets::switch(ui, &mut on, name);
        if plugin.installs() && draft.included_installed.contains_key(plugin.id) {
            ui.add_space(size::GAP_S);
            let more =
                widgets::icon_button(ui, widgets::Icon::More, &trf("{0} options", &[name]), false);
            crate::widgets::menu_of(&more).show(|ui| {
                widgets::menu_width(ui);
                if widgets::menu_item(ui, tr("Install again"), "").clicked() {
                    again = true;
                    ui.close();
                }
            });
        }
        switch
    });
    if switch.changed() {
        plugin.switch(&mut draft.included_on, &mut draft.included_off, on);
    }
    again
}

impl App {
    /// The plugins: those that come with Muddie, and those installed.
    pub(in crate::app) fn plugins_groups(&mut self, ui: &mut Ui) {
        self.included_group(ui);
        self.installed_group(ui);
    }

    /// The plugins that come with Muddie.
    pub(in crate::app) fn included_group(&mut self, ui: &mut Ui) {
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        let mut again = None;
        widgets::section(ui, tr("Included"), |ui| {
            for plugin in crate::bundled::ALL.iter() {
                if included_row(ui, plugin, &mut edit.draft) {
                    again = Some(plugin);
                }
            }
        });
        if let Some(plugin) = again {
            self.install_again(plugin);
        }
    }

    /// The plugins installed, each with its switch in this profile and its
    /// menu; updating them all and installing another, beside their
    /// caption; and what the last commands about plugins said.
    pub(in crate::app) fn installed_group(&mut self, ui: &mut Ui) {
        let installed = (self.profiles.as_ref())
            .map(Profiles::packages)
            .unwrap_or_default();
        let (mut commands, mut install) = (Vec::new(), false);
        widgets::section_caption_with(ui, tr("Installed"), |ui| {
            install = widgets::button(ui, tr("Install from git")).clicked();
            if !installed.is_empty() && widgets::button(ui, tr("Update all")).clicked() {
                commands.extend(
                    installed
                        .iter()
                        .map(|name| format!("/plugin update {name}")),
                );
            }
        });
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        if installed.is_empty() {
            widgets::page_note(ui, tr("None installed."));
        } else {
            widgets::group(ui, |ui| {
                for name in &installed {
                    commands.extend(installed_row(ui, &mut edit.draft.packages, name));
                }
            });
        }
        if install {
            self.asking = Some(Ask::Install(String::new()));
            self.ask_error = None;
        }
        for command in commands {
            self.tabs[self.active].plugin_command(&command);
        }
        self.plugin_answers(ui);
    }

    /// What the last commands about plugins said, under their card, and
    /// whether one is still at work.
    fn plugin_answers(&mut self, ui: &mut Ui) {
        let (said, busy) = self.tabs[self.active].plugin_answers();
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

    /// Installs a plugin that comes with Muddie again in the profile being
    /// edited, in the sessions that use it: what it installed is put back
    /// as it came, and what was added is kept.
    pub(in crate::app) fn install_again(&mut self, plugin: &'static crate::bundled::Bundled) {
        let Some(name) = self.editing.as_ref().map(|edit| edit.name.clone()) else {
            return;
        };
        for tab in self.tabs.iter_mut().filter(|tab| tab.uses_profile(&name)) {
            tab.install_included_again(plugin);
        }
    }
}
