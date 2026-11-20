//! The saved profiles in a list, at the left of the Profiles window, and
//! making another.

use super::*;

impl App {
    /// The saved profiles, each with its colour, where it leads and
    /// whether a session plays with it; the one chosen lit up. Its menu
    /// opens on a right click. Under them, making another.
    pub(in crate::app) fn profiles_list(&mut self, ui: &mut Ui) {
        let Some(profiles) = self.profiles.clone() else {
            widgets::page_note(
                ui,
                tr("Profiles need a configuration folder, and none was found."),
            );
            return;
        };
        let chosen = self.editing.as_ref().map(|edit| edit.name.clone());
        let mut asked = None;
        let room = ui.available_height() - size::CONTROL - size::GAP_M;
        egui::ScrollArea::vertical()
            .id_salt("profiles list")
            .max_height(room)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for name in profiles.names() {
                    let place = (profiles.load(&name).ok())
                        .map(|profile| format!("{}:{}", profile.host, profile.port))
                        .unwrap_or_default();
                    let in_use = self.tabs.iter().any(|tab| tab.uses_profile(&name));
                    let selected = chosen.as_deref() == Some(name.as_str());
                    let row = widgets::profile_row(ui, selected, &name, &place, in_use);
                    if row.clicked() {
                        asked = Some((ProfileAction::Edit, name.clone()));
                    }
                    row.context_menu(|ui| {
                        if let Some(action) = profile_menu(ui) {
                            asked = Some((action, name.clone()));
                        }
                    });
                }
            });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            if widgets::button(ui, tr("New profile")).clicked() {
                self.asking = Some(Ask::New(profiles.unused_name(tr("New profile"))));
                self.ask_error = None;
            }
        });
        if let Some((action, name)) = asked {
            self.profile_action(action, &name);
        }
    }
}
