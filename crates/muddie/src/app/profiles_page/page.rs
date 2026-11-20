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
        self.profile_heading(ui, ctx, &name);
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
        ui.add_space(size::GAP_L);
    }

    /// "‹ Profiles", and the profile's name, with where it leads under it,
    /// connecting with it and its menu at the right.
    pub(in crate::app) fn profile_heading(&mut self, ui: &mut Ui, ctx: &egui::Context, name: &str) {
        if widgets::quiet_button(ui, &format!("‹ {}", tr("Profiles"))).clicked() {
            self.editing = None;
            return;
        }
        ui.add_space(size::GAP_S);
        let place =
            (self.editing.as_ref()).map(|edit| format!("{}:{}", edit.draft.host, edit.draft.port));
        let theme = theme::current();
        let mut asked = None;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let title = RichText::new(name)
                    .font(theme::font(Weight::Semibold, 22.0))
                    .color(theme.text);
                ui.add(egui::Label::new(title).selectable(false));
                let place = RichText::new(place.unwrap_or_default()).color(theme.text_dim);
                ui.add(egui::Label::new(place).selectable(false));
            });
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
        let connect = widgets::primary_button(ui, tr("Connect")).clicked();
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
