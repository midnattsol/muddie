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
                .color(theme.text_faint)
        };
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
        let mut command = None;
        if installed.is_empty() {
            ui.label(small(tr("No plugins yet. A plugin is a set of scripts someone made for a MUD: a mapper, triggers, gauges.")));
        } else {
            let in_profile = self.profile.is_some();
            widgets::group(ui, |ui| {
                for name in &installed {
                    let mut enabled = self.packages.contains(name);
                    widgets::setting_row(ui, name, &mut |ui| {
                        let switch = ui
                            .add_enabled_ui(in_profile, |ui| {
                                widgets::switch(ui, &mut enabled, name)
                            })
                            .inner;
                        if switch.changed() {
                            let what = if enabled { "enable" } else { "disable" };
                            command = Some(format!("/plugin {what} {name}"));
                        }
                        if ui.button(tr("Update")).clicked() {
                            command = Some(format!("/plugin update {name}"));
                        }
                        let remove = RichText::new(tr("Remove")).color(theme.danger);
                        if ui
                            .add(egui::Button::new(remove).fill(egui::Color32::TRANSPARENT))
                            .clicked()
                        {
                            command = Some(format!("/plugin remove {name}"));
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
            widgets::setting_row(ui, tr("Git address"), &mut |ui| {
                let ready = !self.plugin_address.trim().is_empty();
                if ui
                    .add_enabled(ready, egui::Button::new(tr("Install")))
                    .clicked()
                {
                    command = Some(format!("/plugin install {}", self.plugin_address.trim()));
                    self.plugin_address.clear();
                }
                widgets::row_field(
                    ui,
                    &mut self.plugin_address,
                    "https://github.com/…/….git",
                    250.0,
                )
            });
        });
        ui.label(small(tr("It is downloaded with git into the configuration's packages folder. Only install what you trust: a plugin can send commands as you.")));
        if let Some(command) = command {
            self.map_command(&command);
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
}
