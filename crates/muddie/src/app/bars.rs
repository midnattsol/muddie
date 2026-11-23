//! The bars of the window: at the top, the session's views, the field
//! that opens the palette and the menu; at the bottom, the sessions as
//! tabs, what the one on screen is doing, and its messages.

use super::*;

/// What is done with a session from its tab's menu.
enum SessionAction {
    Reconnect,
    Disconnect,
    EditProfile(String),
    SaveAsProfile,
    Close,
}

impl App {
    /// The bar at the top: the views at the left, the palette's field in
    /// the middle, the menu at the right.
    pub(super) fn top_bar(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        let active = self.active;
        ui.horizontal(|ui| {
            let tab = &mut self.tabs[active];
            // The incoming data is a section of the editor.
            let views = [tr("Game"), tr("Editor"), tr("Layout")];
            let order = [View::Game, View::Editor, View::Layout];
            let shown = match tab.view() {
                View::Data => 1,
                view => order.iter().position(|known| *known == view).unwrap_or(0),
            };
            if let Some(view) = widgets::segmented(ui, &views, shown) {
                tab.set_view(order[view]);
            }
            // Left to right, as the keyboard goes through them.
            self.palette_field(ui, full);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.main_menu(ui)
            });
        });
    }

    /// The field that opens the palette, in the middle of the bar, as
    /// wide as its ends leave it.
    fn palette_field(&mut self, ui: &mut Ui, bar: egui::Rect) {
        let width = (bar.width() - 2.0 * 260.0).clamp(180.0, 380.0);
        let place = egui::Rect::from_center_size(bar.center(), egui::vec2(width, 26.0));
        let keys = crate::shortcuts::PALETTE.written;
        // Where it is, without taking room from the rest of the bar.
        let mut room = ui.new_child(egui::UiBuilder::new().max_rect(place));
        if widgets::palette_button(&mut room, width, keys).clicked() {
            self.tabs[self.active].toggle_palette();
        }
    }

    /// The menu of the window: what is not a session's. Everything in it
    /// is in the palette too.
    fn main_menu(&mut self, ui: &mut Ui) {
        let button = widgets::icon_button(ui, widgets::Icon::Menu, tr("Menu"), false);
        crate::widgets::menu_of(&button).show(|ui| {
            widgets::menu_width(ui);
            let new_session = crate::shortcuts::NEW_SESSION.written;
            if widgets::menu_item(ui, tr("New session"), new_session).clicked() {
                self.new_tab(ui.ctx());
                ui.close();
            }
            let settings = crate::shortcuts::SETTINGS.written;
            if widgets::menu_item(ui, tr("Settings"), settings).clicked() {
                self.open_settings(Section::General);
                ui.close();
            }
            if widgets::menu_item(ui, tr("Profiles"), "").clicked() {
                self.open_profiles(None);
                ui.close();
            }
            widgets::menu_separator(ui);
            if widgets::menu_item(ui, tr("Keyboard shortcuts"), "").clicked() {
                self.show_shortcuts = true;
                ui.close();
            }
            if widgets::menu_item(ui, tr("About Muddie"), "").clicked() {
                self.show_about = true;
                ui.close();
            }
        });
    }

    /// The bar at the bottom: one tab per session, the way to open
    /// another, then what the session on screen is up to and its bell.
    pub(super) fn bottom_bar(&mut self, ui: &mut Ui) {
        let mut asked = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for index in 0..self.tabs.len() {
                if let Some(action) = self.session_tab(ui, index) {
                    asked = Some((index, action));
                }
            }
            if widgets::sign_button(ui, "+", tr("New session"), false).clicked() {
                self.new_tab(ui.ctx());
            }
            let active = self.active;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                messages(ui, &mut self.tabs[active]);
                self.tabs[active].status_line(ui);
            });
        });
        if let Some((index, action)) = asked {
            self.session_action(ui.ctx(), index, action);
        }
    }

    /// A session's tab: a dot for its connection, its name, the cross
    /// that closes it, and its menu on a right click. Returns what its
    /// menu asked for.
    fn session_tab(&mut self, ui: &mut Ui, index: usize) -> Option<SessionAction> {
        let theme = theme::current();
        let active = index == self.active;
        let tab = &self.tabs[index];
        let (title, (dot, state)) = (tab.title(), tab.status());
        let unread = tab.has_unread() && !active;
        let (connected, profile) = (tab.is_connected(), tab.profile_name().map(str::to_owned));
        let fill = if active {
            theme.raised
        } else {
            Color32::TRANSPARENT
        };
        let frame = egui::Frame::new()
            .fill(fill)
            .corner_radius(CornerRadius::same(7))
            .inner_margin(egui::Margin {
                left: 10,
                right: 5,
                top: 3,
                bottom: 3,
            });
        let mut asked = None;
        frame.show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 7.0;
            widgets::status_dot(ui, dot);
            let name = session_name(ui, &title, &state, active, unread);
            if name.clicked() {
                self.active = index;
            }
            crate::widgets::context_menu_of(&name, |ui| {
                asked = session_menu(ui, connected, profile.as_deref())
            });
            let closing = format!("{} {title}", tr("Close session"));
            if widgets::close_button(ui, &closing, 18.0).clicked() {
                asked = Some(SessionAction::Close);
            }
        });
        asked
    }

    /// Does what a session's menu asked for.
    fn session_action(&mut self, ctx: &egui::Context, index: usize, action: SessionAction) {
        match action {
            SessionAction::Reconnect => self.tabs[index].reconnect(ctx),
            SessionAction::Disconnect => self.tabs[index].disconnect(),
            SessionAction::EditProfile(name) => self.edit_profile(&name),
            SessionAction::SaveAsProfile => {
                self.active = index;
                self.ask_save_session();
            }
            SessionAction::Close => self.close_tab(index, ctx),
        }
    }
}

/// A session's name on its tab, said with how it is doing.
fn session_name(
    ui: &mut Ui,
    title: &str,
    state: &str,
    active: bool,
    unread: bool,
) -> egui::Response {
    let theme = theme::current();
    let colour = match (active, unread) {
        (true, _) => theme.text,
        (false, true) => widgets::fresh_colour(),
        (false, false) => theme.text_dim,
    };
    let weight = if active || unread {
        Weight::Medium
    } else {
        Weight::Regular
    };
    let label = RichText::new(title)
        .font(theme::font(weight, 12.5))
        .color(colour);
    let name = ui
        .add(
            egui::Label::new(label)
                .selectable(false)
                .sense(Sense::click()),
        )
        .on_hover_text(state);
    ui.ctx().accesskit_node_builder(name.id, |node| {
        node.set_role(egui::accesskit::Role::Tab);
        node.set_selected(active);
        node.set_label(format!("{}, {state}", widgets::fresh_said(title, unread)));
    });
    name
}

/// A session's menu: connecting, its profile, and closing it.
fn session_menu(ui: &mut Ui, connected: bool, profile: Option<&str>) -> Option<SessionAction> {
    widgets::menu_width(ui);
    let mut asked = None;
    let mut item = |ui: &mut Ui, label: &str, keys: &str, action: SessionAction| {
        if widgets::menu_item(ui, label, keys).clicked() {
            asked = Some(action);
            ui.close();
        }
    };
    let connect = if connected {
        tr("Reconnect")
    } else {
        tr("Connect")
    };
    item(
        ui,
        connect,
        crate::shortcuts::RECONNECT.written,
        SessionAction::Reconnect,
    );
    if connected {
        item(ui, tr("Disconnect"), "", SessionAction::Disconnect);
    }
    widgets::menu_separator(ui);
    match profile {
        Some(name) => item(
            ui,
            tr("Edit profile"),
            "",
            SessionAction::EditProfile(name.to_owned()),
        ),
        None => item(ui, tr("Save as profile"), "", SessionAction::SaveAsProfile),
    }
    widgets::menu_separator(ui);
    item(
        ui,
        tr("Close session"),
        crate::shortcuts::CLOSE_SESSION.written,
        SessionAction::Close,
    );
    asked
}

/// The bell of what the client said in a session, marked while some have
/// not been looked at; pressed, the list of them, each with when it was
/// said, to read again or copy.
fn messages(ui: &mut Ui, tab: &mut Tab) {
    let theme = theme::current();
    let (said, unread) = tab.messages();
    let label = match unread {
        0 => tr("Messages").to_owned(),
        count => crate::i18n::trn("Messages, {0} new", "Messages, {0} new", count as u64),
    };
    let bell = widgets::icon_button(ui, widgets::Icon::Bell, &label, false);
    if unread > 0 {
        let at = bell.rect.right_top() + egui::vec2(-7.0, 7.0);
        ui.painter().circle_filled(at, 3.5, theme.accent);
    }
    let said: Vec<_> = said.iter().rev().take(60).cloned().collect();
    let mut clear = false;
    let shown = crate::widgets::menu_of(&bell).width(420.0).show(|ui| {
        clear = messages_list(ui, &said);
    });
    if shown.is_some() {
        tab.messages_read();
    }
    if clear {
        tab.clear_notices();
    }
}

/// The messages, newest first, with the way to clear them. Returns
/// whether clearing was asked for.
fn messages_list(ui: &mut Ui, said: &[crate::tab::Message]) -> bool {
    let theme = theme::current();
    let mut clear = false;
    ui.horizontal(|ui| {
        widgets::menu_heading(ui, tr("Messages"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !said.is_empty() {
                clear = widgets::quiet_button(ui, tr("Clear")).clicked();
            }
        });
    });
    if said.is_empty() {
        widgets::menu_heading(ui, tr("No messages."));
        return false;
    }
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .show(ui, |ui| {
            widgets::keep_clear_of_bar(ui);
            for message in said {
                ui.horizontal_top(|ui| {
                    let time = RichText::new(&message.time)
                        .font(egui::FontId::monospace(11.5))
                        .color(theme.text_faint);
                    ui.add(egui::Label::new(time).selectable(false));
                    let colour = if message.problem {
                        theme.danger
                    } else {
                        theme.text
                    };
                    ui.add(
                        egui::Label::new(RichText::new(&message.text).color(colour))
                            .wrap()
                            .selectable(true),
                    );
                });
            }
        });
    clear
}
