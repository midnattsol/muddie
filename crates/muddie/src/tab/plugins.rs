//! The plugins' part of the settings: scripts made by others, installed
//! from a git repository and switched on per profile.

use super::*;
use crate::theme::Weight;
use crate::widgets;

impl Tab {
    /// Lists the installed plugins, each with its switch for this
    /// session's profile, and takes the address of one to install.
    pub fn plugins_ui(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        let small = |text: &str| {
            RichText::new(text)
                .font(theme::font(Weight::Regular, 12.0))
                .color(theme.text_dim)
        };
        // Those that come with Muddie: on or off in this session, and
        // kept so with its profile; one in Luau can be made one's own.
        widgets::caption(ui, tr("Included with Muddie"));
        let mut customize = None;
        widgets::group(ui, |ui| {
            for plugin in crate::bundled::ALL.iter() {
                let mut on = self.included(plugin);
                let name = tr(plugin.name);
                let switch = widgets::setting_row(ui, name, &mut |ui| {
                    let switch = widgets::switch(ui, &mut on, name);
                    if plugin.code.is_some() {
                        ui.add_space(theme::size::GAP_S);
                        let mine = trf("Customize {0}", &[name]);
                        let pressed = widgets::quiet_button(ui, tr("Customize"));
                        pressed.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Button, true, &mine)
                        });
                        if pressed.clicked() {
                            customize = Some(plugin);
                        }
                    }
                    switch
                });
                if switch.changed() {
                    self.set_included(plugin, on);
                }
                widgets::row_note(ui, tr(plugin.about));
            }
        });
        if let Some(plugin) = customize {
            self.customize(plugin);
        }
        ui.add_space(theme::size::GAP_M);
        let Some(profiles) = self.profiles.clone() else {
            ui.label(small(tr(
                "Plugins need a configuration folder, and none was found.",
            )));
            return;
        };
        let installed = profiles.packages();
        // What it is told to do here is said in the output; the lines that
        // adds are shown here too.
        let before = self.output.len();
        let mut command: Option<Vec<String>> = None;
        ui.horizontal(|ui| {
            widgets::caption(ui, tr("Installed by you"));
            if !installed.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::quiet_button(ui, tr("Update all")).clicked() {
                        command = Some(
                            installed
                                .iter()
                                .map(|name| format!("/plugin update {name}"))
                                .collect(),
                        );
                    }
                });
            }
        });
        if installed.is_empty() {
            ui.label(small(tr("No plugins yet. A plugin is a set of scripts someone made for a MUD: a mapper, triggers, gauges.")));
        } else {
            let in_profile = self.profile.is_some();
            widgets::group(ui, |ui| {
                for name in &installed {
                    let mut enabled = self.packages.contains(name);
                    widgets::setting_row(ui, name, &mut |ui| {
                        // The rest of what can be done with it, in its menu.
                        let more = widgets::icon_button(
                            ui,
                            widgets::Icon::More,
                            &trf("{0} options", &[name]),
                            false,
                        );
                        egui::Popup::menu(&more).show(|ui| {
                            widgets::menu_width(ui);
                            if widgets::menu_item(ui, tr("Update"), "").clicked() {
                                command = Some(vec![format!("/plugin update {name}")]);
                                ui.close();
                            }
                            if widgets::menu_item(ui, tr("Remove"), "").clicked() {
                                command = Some(vec![format!("/plugin remove {name}")]);
                                ui.close();
                            }
                        });
                        ui.add_space(theme::size::GAP_S);
                        let switch = ui
                            .add_enabled_ui(in_profile, |ui| {
                                widgets::switch(ui, &mut enabled, name)
                            })
                            .inner;
                        if switch.changed() {
                            let what = if enabled { "enable" } else { "disable" };
                            command = Some(vec![format!("/plugin {what} {name}")]);
                        }
                        switch
                    });
                }
            });
            if !in_profile {
                ui.label(small(tr(
                    "Plugins are enabled per profile: save or choose a profile first.",
                )));
            }
        }
        ui.add_space(6.0);
        widgets::caption(ui, tr("Install a plugin"));
        widgets::group(ui, |ui| {
            widgets::setting_row(ui, tr("Address"), &mut |ui| {
                let ready = !self.plugin_address.trim().is_empty();
                let install = ui.add_enabled_ui(ready, |ui| widgets::button(ui, tr("Install")));
                if install.inner.clicked() {
                    command = Some(vec![format!(
                        "/plugin install {}",
                        self.plugin_address.trim()
                    )]);
                    self.plugin_address.clear();
                }
                widgets::row_field(
                    ui,
                    &mut self.plugin_address,
                    "https://github.com/…/….git",
                    theme::size::FIELD_L,
                )
            });
            widgets::row_note(
                ui,
                tr(
                    "The address of the plugin's git repository. Only install what you trust: a plugin can send commands as you.",
                ),
            );
        });
        if let Some(commands) = command {
            for command in commands {
                self.map_command(&command);
            }
            self.plugin_said = before;
        }
        if !self.package_jobs.is_empty() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(small(tr("Working…")));
            });
        }
        // The answers to what was last asked for here.
        let from = self.plugin_said.min(self.output.len());
        if from > 0 || !self.package_jobs.is_empty() {
            for row in from..self.output.len() {
                ui.label(
                    RichText::new(self.output.text(row))
                        .font(theme::font(Weight::Regular, 12.5))
                        .color(theme.text_dim),
                );
            }
        }
    }

    /// Makes a plugin that comes with Muddie one's own: its code becomes
    /// a script of the profile's, to change at will, and the one that
    /// came is switched off so as not to run twice. The script is opened
    /// in the editor.
    fn customize(&mut self, plugin: &'static crate::bundled::Bundled) {
        let (Some(profiles), Some(profile), Some(code)) =
            (self.profiles.clone(), self.profile.clone(), plugin.code)
        else {
            return self.note(tr(
                "Save the session as a profile first: the script is kept with it.",
            ));
        };
        let path = profiles
            .scripts_dir(&profile)
            .join(format!("{}.luau", plugin.id));
        // One made before is kept as it is: it may have been changed.
        if !path.exists() {
            let written = (path.parent().map_or(Ok(()), std::fs::create_dir_all))
                .and_then(|()| std::fs::write(&path, code));
            if let Err(error) = written {
                return self.note(&trf(
                    "Could not save the script: {0}",
                    &[&error.to_string()],
                ));
            }
        }
        self.set_included(plugin, false);
        self.scripts.reload_file(&path);
        self.run_actions();
        self.open_automation(automation::SCRIPTS);
        if let Some(index) = self.script_files.iter().position(|file| *file == path) {
            self.select_rule(automation::SCRIPTS, index);
        }
        self.app_request = Some(AppRequest::CloseSettings);
        self.note(&trf(
            "{0} is now a script of yours, in the editor.",
            &[tr(plugin.name)],
        ));
    }
}
