//! One profile's page: the way back, its name and menu, a notice when what
//! changed waits for reconnecting, its groups, and connecting with it.

use super::*;

impl App {
    /// One profile's page: the way back, its name and menu, then what it
    /// has, in groups.
    pub(in crate::app) fn profile_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let Some(name) = self.editing.as_ref().map(|edit| edit.name.clone()) else {
            return;
        };
        self.profile_heading(ui, &name);
        self.reconnect_notice(ui, ctx, &name);
        let Some(edit) = self.editing.as_mut() else {
            return;
        };
        server_group(ui, &mut edit.draft);
        login_group(ui, edit);
        let prefs = edit.draft.session.get_or_insert_with(SessionPrefs::default);
        keeping_group(ui, prefs);
        logs_group(ui, prefs);
        self.plugins_groups(ui);
        ui.add_space(size::GAP_M);
        self.connect_button(ui, ctx, &name);
    }

    /// "‹ Profiles", and the profile's name with its menu.
    pub(in crate::app) fn profile_heading(&mut self, ui: &mut Ui, name: &str) {
        if widgets::quiet_button(ui, &format!("‹ {}", tr("Profiles"))).clicked() {
            self.editing = None;
            return;
        }
        let mut asked = None;
        ui.horizontal(|ui| {
            let title = RichText::new(name)
                .font(theme::font(Weight::Semibold, 18.0))
                .color(theme::current().text);
            ui.add(egui::Label::new(title).selectable(false));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = trf("{0} options", &[name]);
                let more = widgets::icon_button(ui, widgets::Icon::More, &label, false);
                egui::Popup::menu(&more).show(|ui| asked = profile_menu(ui));
            });
        });
        if let Some(action) = asked {
            self.profile_action(action, name);
        }
    }

    /// With the session on screen connected with this profile otherwise
    /// than it now says, that what changed waits for reconnecting.
    pub(in crate::app) fn reconnect_notice(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        name: &str,
    ) {
        let tab = &mut self.tabs[self.active];
        if !tab.uses_profile(name) || !tab.connection_outdated() {
            return;
        }
        let mut again = false;
        widgets::notice_bar(
            ui,
            tr("The changes take effect on reconnecting."),
            &mut |ui| {
                again = widgets::primary_button(ui, tr("Reconnect")).clicked();
            },
        );
        if again {
            tab.reconnect(ctx);
            self.show_settings = false;
        }
    }

    /// Connects with this profile: in the session on screen if it is not
    /// connected, or else in a new one.
    pub(in crate::app) fn connect_button(&mut self, ui: &mut Ui, ctx: &egui::Context, name: &str) {
        let mut connect = false;
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            connect = widgets::primary_button(ui, tr("Connect")).clicked();
        });
        if !connect {
            return;
        }
        self.keep_profile(ctx, false);
        if self.tabs[self.active].is_connected() {
            self.new_tab(ctx);
        }
        self.tabs[self.active].play(name, ctx);
        self.show_settings = false;
    }
}
