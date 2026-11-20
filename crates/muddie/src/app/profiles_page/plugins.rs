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

impl App {
    /// The groups of the plugins: those that come with Muddie, those
    /// installed, and installing one.
    pub(in crate::app) fn plugins_groups(&mut self, ui: &mut Ui) {
        widgets::section_caption(ui, tr("Plugins"));
        self.included_group(ui);
        self.installed_group(ui);
    }

    /// The plugins that come with Muddie: each with its switch, what it
    /// does, and for one in Luau the way to make it one's own.
    pub(in crate::app) fn included_group(&mut self, ui: &mut Ui) {
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
    /// its menu, the way to update them all, and to install another; and
    /// what the last commands about plugins said.
    pub(in crate::app) fn installed_group(&mut self, ui: &mut Ui) {
        let installed = (self.profiles.as_ref())
            .map(Profiles::packages)
            .unwrap_or_default();
        let mut commands = Vec::new();
        widgets::section_caption_with(ui, tr("Installed by you"), |ui| {
            if !installed.is_empty() && widgets::quiet_button(ui, tr("Update all")).clicked() {
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
        let address = &mut self.plugin_address;
        widgets::group(ui, |ui| {
            if installed.is_empty() {
                ui.add_space(12.0);
                let none = "No plugins yet. A plugin is a set of scripts someone made for a MUD: a mapper, triggers, gauges.";
                widgets::row_note(ui, tr(none));
            }
            for name in &installed {
                commands.extend(installed_row(ui, &mut edit.draft.packages, name));
            }
            commands.extend(install_row(ui, address));
        });
        for command in commands {
            self.tabs[self.active].plugin_command(&command);
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
    pub(in crate::app) fn customize(&mut self, plugin: &'static crate::bundled::Bundled) {
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
}

/// Installing a plugin from the address of its repository. Returns the
/// command that installs it, when asked.
fn install_row(ui: &mut Ui, address: &mut String) -> Option<String> {
    let mut install = false;
    widgets::setting_row(ui, tr("Install from"), &mut |ui| {
        let ready = !address.trim().is_empty();
        install = ui
            .add_enabled_ui(ready, |ui| widgets::button(ui, tr("Install")))
            .inner
            .clicked();
        widgets::row_field(ui, address, "https://github.com/…/….git", size::FIELD_L)
    });
    widgets::row_note(
        ui,
        tr(
            "The address of the plugin's git repository. Only install what you trust: a plugin can send commands as you.",
        ),
    );
    install.then(|| format!("/plugin install {}", std::mem::take(address).trim()))
}
