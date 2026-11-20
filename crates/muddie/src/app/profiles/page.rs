//! The profile chosen, at the right of the Profiles window: its colour,
//! name, menu and connecting with it, which says when what changed waits
//! for reconnecting; and its parts, each in a tab.

use super::*;

impl App {
    /// The profile chosen, or what to do when there is none.
    pub(in crate::app) fn profile_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(name) = self.editing.as_ref().map(|edit| edit.name.clone()) else {
            return self.no_profile(ui);
        };
        self.profile_heading(ui, ctx, &name);
        ui.add_space(size::GAP_M);
        let names = ProfileTab::ALL.map(ProfileTab::name);
        let current = ProfileTab::ALL
            .iter()
            .position(|tab| *tab == self.profile_tab);
        if let Some(chosen) = widgets::segmented(ui, &names, current.unwrap_or(0)) {
            self.profile_tab = ProfileTab::ALL[chosen];
        }
        ui.add_space(size::GAP_M);
        egui::ScrollArea::vertical()
            .id_salt(("profile", self.profile_tab))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Room at the right for the bar it scrolls by.
                let room = egui::Margin {
                    right: 14,
                    ..egui::Margin::ZERO
                };
                egui::Frame::new().inner_margin(room).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = size::GAP_S;
                    self.profile_tab_page(ui);
                    if let Some(error) = &self.settings_error {
                        ui.colored_label(theme::current().danger, error);
                    }
                    ui.add_space(size::GAP_M);
                });
            });
    }

    /// The tab of the profile on show.
    fn profile_tab_page(&mut self, ui: &mut Ui) {
        if self.profile_tab == ProfileTab::Plugins {
            return self.plugins_groups(ui);
        }
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        match self.profile_tab {
            ProfileTab::Connection => {
                server_group(ui, &mut edit.draft);
                login_group(ui, edit);
            }
            ProfileTab::Session => {
                let prefs = edit.draft.session.get_or_insert_with(SessionPrefs::default);
                keeping_group(ui, prefs);
                logs_group(ui, prefs);
            }
            ProfileTab::Plugins => {}
        }
    }

    /// With no profile yet: what one is, and making the first.
    fn no_profile(&mut self, ui: &mut Ui) {
        ui.add_space(size::GAP_L);
        widgets::page_note(
            ui,
            tr("No profiles yet. A profile keeps a MUD's connection, scripts, map and layout."),
        );
        ui.add_space(size::GAP_M);
        if widgets::primary_button(ui, tr("New profile")).clicked() {
            let name =
                (self.profiles.as_ref()).map(|profiles| profiles.unused_name(tr("New profile")));
            self.asking = name.map(Ask::New);
            self.ask_error = None;
        }
    }

    /// The profile's colour and name, whether a session plays with it,
    /// and connecting with it and its menu at the right.
    pub(in crate::app) fn profile_heading(&mut self, ui: &mut Ui, ctx: &egui::Context, name: &str) {
        let theme = theme::current();
        let in_use = self.tabs.iter().any(|tab| tab.uses_profile(name));
        let mut asked = None;
        ui.horizontal(|ui| {
            let (tile, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
            widgets::initial_tile(ui.painter(), tile, name, theme.surface);
            ui.add_space(size::GAP_S);
            let title = RichText::new(name)
                .font(theme::font(Weight::Semibold, 20.0))
                .color(theme.text);
            ui.add(egui::Label::new(title).selectable(false));
            if in_use {
                widgets::badge(ui, tr("In use"), true);
            }
            if self.waits_to_reconnect(name) {
                widgets::badge(ui, tr("Changes pending"), false);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = trf("{0} options", &[name]);
                let more = widgets::icon_button(ui, widgets::Icon::More, &label, false);
                egui::Popup::menu(&more).show(|ui| asked = profile_menu(ui));
                ui.add_space(size::GAP_S);
                self.connect_button(ui, ctx, name);
            });
        });
        if let Some(action) = asked {
            self.profile_action(action, name);
        }
    }

    /// Connects with this profile: in the session on screen if it is not
    /// connected, or else in a new one.
    /// With the session on screen connected with this profile otherwise
    /// than it now says, what changed takes effect on reconnecting: it says
    /// so in its place, and connecting is reconnecting.
    pub(in crate::app) fn connect_button(&mut self, ui: &mut Ui, ctx: &egui::Context, name: &str) {
        if self.waits_to_reconnect(name) {
            let again = widgets::primary_button(ui, tr("Reconnect"))
                .on_hover_text(tr("The changes take effect on reconnecting."));
            if again.clicked() {
                self.tabs[self.active].reconnect(ctx);
                self.show_profiles = false;
            }
            return;
        }
        if !widgets::primary_button(ui, tr("Connect")).clicked() {
            return;
        }
        self.keep_profile(ctx, false);
        if self.tabs[self.active].is_connected() {
            self.new_tab(ctx);
        }
        self.tabs[self.active].play(name, ctx);
        self.show_profiles = false;
    }

    /// Whether the session on screen plays with this profile as it was,
    /// so that what changed waits for reconnecting.
    fn waits_to_reconnect(&self, name: &str) -> bool {
        let tab = &self.tabs[self.active];
        tab.uses_profile(name) && tab.connection_outdated()
    }
}
