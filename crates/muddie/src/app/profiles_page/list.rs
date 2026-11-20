//! The saved profiles in a list, each with its menu, and making one.

use super::*;

impl App {
    /// The saved profiles, each with where it leads, whether a session
    /// plays with it, and its menu; and the way to make another.
    pub(in crate::app) fn profiles_list(&mut self, ui: &mut Ui) {
        let Some(profiles) = self.profiles.clone() else {
            widgets::page_note(
                ui,
                tr("Profiles need a configuration folder, and none was found."),
            );
            return;
        };
        let names = profiles.names();
        if names.is_empty() {
            widgets::page_note(
                ui,
                tr("No profiles yet. A profile keeps a MUD's connection, scripts, map and layout."),
            );
        }
        let mut asked = None;
        if !names.is_empty() {
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
        }
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
}
