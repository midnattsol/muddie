//! How a session looks: the welcome screen before it connects, then the
//! output with its panels, and the session's parts of the menus and of the
//! status bar.

use super::*;
use crate::theme::Weight;
use crate::widgets;

impl Tab {
    /// Draws the session in `ui`.
    pub fn show(&mut self, ui: &mut egui::Ui, _settings: &Settings) {
        if self.recording_key {
            self.record_key(ui);
        } else {
            self.bound_keys(ui);
        }
        self.connection_window(ui.ctx());
        self.gauges_dialog(ui.ctx());
        // The automation view takes the place of the game while open.
        if self.rules_open {
            return self.automation_page(ui);
        }
        if self.debug_open {
            return self.debug_page(ui);
        }
        // Alt+0 reads the gauges aloud.
        if ui.input_mut(|input| input.consume_key(Modifiers::ALT, Key::Num0)) {
            self.voice.stop();
            let summary = self.gauges.summary();
            self.voice.say(if summary.is_empty() {
                tr("No gauges.")
            } else {
                &summary
            });
        }
        // The channels to read back are those of the panel that shows them.
        let channels = (self.layout.panels.iter()).find(|panel| panel.channels);
        let chat = channels
            .and_then(|panel| self.panel_text.iter().find(|(id, _)| *id == panel.id))
            .map_or(&self.no_chat, |(_, content)| &content.log);
        self.voice.update(ui, &self.output, chat);
        if !self.started {
            return self.welcome(ui);
        }
        // Escape leaves the layout mode.
        if self.editing_layout
            && !self.gauges_open
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.editing_layout = false;
        }
        if self.editing_layout {
            self.layout_bar(ui);
        }
        let plain = egui::Frame::new().inner_margin(egui::Margin {
            left: 8,
            right: 8,
            top: 4,
            bottom: 8,
        });
        // The command line is always at the bottom, under any panel.
        Panel::bottom("input")
            .frame(plain)
            .show_separator_line(false)
            .show(ui, |ui| {
                if self.find_open {
                    self.find_bar(ui);
                }
                self.announce(ui);
                self.input_line(ui);
            });
        let asked = self.show_panels(ui);
        self.map_asked(asked);
        self.report_panel(ui.ctx());
        // The output takes what the panels leave.
        let sides =
            |side| (self.layout.panels.iter()).any(|panel| panel.visible && panel.side == side);
        let frame = layout::well(
            if sides(layout::Side::Left) { 4 } else { 8 },
            if sides(layout::Side::Right) { 4 } else { 8 },
        );
        let clicked = CentralPanel::default()
            .frame(frame)
            .show(ui, |ui| self.output.show(ui, "output"));
        // A link the server marked as a command was clicked.
        if let Some(command) = clicked.inner {
            self.send(command);
        }
    }

    /// What a session shows before it has connected anywhere: the saved
    /// profiles, one click away, and a short form for somewhere new.
    fn welcome(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        let profiles = self.welcome_profiles.clone().unwrap_or_else(|| {
            let listed = self.profiles.as_ref().map_or_else(Vec::new, |profiles| {
                let described = profiles.names().into_iter().map(|name| {
                    let place = profiles
                        .load(&name)
                        .map(|profile| format!("{}:{}", profile.host, profile.port));
                    (name, place.unwrap_or_default())
                });
                described.collect()
            });
            self.welcome_profiles = Some(listed.clone());
            listed
        });
        let (mut chosen, mut connect) = (None, false);
        let frame = egui::Frame::new().fill(theme.surface);
        CentralPanel::default().frame(frame).show(ui, |ui| {
            // The last light of the day, low behind everything.
            let sky = ui.max_rect();
            let painter = ui.painter().with_clip_rect(sky);
            let strength = if theme.dark { 0.32 } else { 0.3 };
            let reach = egui::vec2(sky.width() * 0.62, sky.height() * 0.75);
            let low = sky.center_bottom() + egui::vec2(0.0, sky.height() * 0.2);
            widgets::glow(
                &painter,
                low - egui::vec2(sky.width() * 0.22, 0.0),
                reach,
                theme.accent.gamma_multiply(strength * 0.35),
            );
            widgets::glow(
                &painter,
                low + egui::vec2(sky.width() * 0.1, 0.0),
                reach * 0.8,
                theme.accent.gamma_multiply(strength * 0.5),
            );

            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    let width = 520.0_f32.min(ui.available_width() - 32.0);
                    let margin = ((ui.available_width() - width) / 2.0).max(0.0);
                    let rows = profiles.len().div_ceil(2).min(4) as f32;
                    let height = 290.0
                        + if profiles.is_empty() {
                            0.0
                        } else {
                            60.0 + rows * 78.0
                        };
                    ui.add_space(((ui.available_height() - height) * 0.4).max(24.0));
                    ui.horizontal(|ui| {
                        ui.add_space(margin);
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.vertical_centered(|ui| {
                                widgets::logo(ui, 54.0);
                                ui.add_space(10.0);
                                let name = RichText::new("Muddie")
                                    .font(theme::font(Weight::Semibold, 32.0))
                                    .color(theme.text);
                                ui.add(Label::new(name).selectable(false));
                                ui.add_space(6.0);
                                let question = RichText::new(tr("Where do you want to play?"))
                                    .font(theme::font(Weight::Regular, 15.0))
                                    .color(theme.text_dim);
                                ui.add(Label::new(question).selectable(false));
                            });
                            ui.add_space(30.0);

                            // One field takes the whole address; Enter connects.
                            let id = ui.make_persistent_id("address");
                            let focused = ui.memory(|memory| memory.has_focus(id));
                            let edge = if focused { theme.accent } else { theme.border };
                            let pill = egui::Frame::new()
                                .fill(theme.field)
                                .stroke(egui::Stroke::new(1.0, edge))
                                .corner_radius(egui::CornerRadius::same(26))
                                .inner_margin(egui::Margin {
                                    left: 22,
                                    right: 7,
                                    top: 7,
                                    bottom: 7,
                                });
                            pill.show(ui, |ui| {
                                let row = egui::vec2(ui.available_width(), 38.0);
                                let centred = egui::Layout::right_to_left(egui::Align::Center);
                                ui.allocate_ui_with_layout(row, centred, |ui| {
                                    let pressed =
                                        widgets::go_button(ui, tr("Connect"), 38.0).clicked();
                                    let field = ui.add(
                                        TextEdit::singleline(&mut self.address)
                                            .id(id)
                                            .frame(egui::Frame::NONE)
                                            .hint_text(
                                                RichText::new("mud.example.org:4000")
                                                    .color(theme.text_faint),
                                            )
                                            .font(theme::font(Weight::Regular, 16.0))
                                            .align(egui::Align2::LEFT_CENTER)
                                            .desired_width(f32::INFINITY),
                                    );
                                    ui.ctx().accesskit_node_builder(field.id, |node| {
                                        node.set_label(tr("Address"));
                                    });
                                    let entered = field.lost_focus()
                                        && ui.input(|input| input.key_pressed(Key::Enter));
                                    connect = pressed || entered;
                                });
                            });
                            ui.add_space(14.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 9.0;
                                ui.add_space(12.0);
                                widgets::switch(ui, &mut self.tls, tr("Secure connection (TLS)"));
                                let label = RichText::new(tr("Secure connection (TLS)"))
                                    .font(theme::font(Weight::Regular, 13.0))
                                    .color(theme.text_dim);
                                ui.add(Label::new(label).selectable(false));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.add_space(4.0);
                                        if widgets::quiet_button(ui, tr("More options…")).clicked()
                                        {
                                            self.apply_address();
                                            self.connection_open = true;
                                        }
                                    },
                                );
                            });

                            if !profiles.is_empty() {
                                ui.add_space(38.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(4.0);
                                    widgets::caption(ui, tr("Your profiles"));
                                });
                                ui.add_space(10.0);
                                let gap = 10.0;
                                let card = (width - gap) / 2.0;
                                for pair in profiles.chunks(2) {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = gap;
                                        for (name, place) in pair {
                                            if widgets::profile_card(ui, card, name, place)
                                                .clicked()
                                            {
                                                chosen = Some(name.clone());
                                            }
                                        }
                                    });
                                    ui.add_space(gap);
                                }
                            }
                            ui.add_space(24.0);
                        });
                    });
                });
        });
        let ctx = ui.ctx().clone();
        if let Some(name) = chosen {
            if self.select_profile(Some(name)) {
                self.connect(&ctx);
            }
        } else if connect && self.apply_address() {
            self.connect(&ctx);
        }
    }

    /// Takes the host and port from the address typed on the welcome
    /// screen. Says whether there is somewhere to connect to.
    fn apply_address(&mut self) -> bool {
        if let Some((host, port)) = parse_address(&self.address) {
            self.host = host;
            self.port = port;
        }
        !self.host.trim().is_empty()
    }

    /// Does what was asked of the map in its panel.
    fn map_asked(&mut self, asked: Option<MapAction>) {
        match asked {
            Some(MapAction::Walk(room)) => self.walk_to(room),
            Some(MapAction::Tidy(area)) => self.tidy(&area),
            Some(MapAction::Scripts) => self.open_automation(automation::SCRIPTS),
            Some(MapAction::Command(command)) => {
                self.map_command(command);
            }
            Some(MapAction::Avoid(room, avoid)) => {
                let saved = self.map.borrow_mut().set_avoid(room, avoid);
                if let Err(error) = saved {
                    self.note(&trf("Could not save the map: {0}", &[&error.to_string()]));
                }
            }
            Some(MapAction::Move(room, pos)) => {
                let saved = self.map.borrow_mut().move_room(room, pos);
                if let Err(error) = saved {
                    self.note(&trf("Could not save the map: {0}", &[&error.to_string()]));
                }
            }
            None => {}
        }
    }

    /// The widths of the automation view's panes, as last dragged.
    pub fn pane_widths(&self) -> [f32; 2] {
        self.panes
    }

    /// How the connection is doing: a colour for it, and the words.
    pub fn status(&self) -> (egui::Color32, String) {
        let theme = theme::current();
        let place = format!("{}:{}", self.host.trim(), self.port.trim());
        match (&self.connection, self.reconnect_at) {
            (Some(_), _) if self.tls => (theme.good, trf("Connected to {0}, encrypted", &[&place])),
            (Some(_), _) => (theme.good, trf("Connected to {0}", &[&place])),
            (None, Some(at)) => {
                let seconds = at.saturating_duration_since(Instant::now()).as_secs() + 1;
                (
                    theme.terminal[3],
                    crate::i18n::trn(
                        "Reconnecting in {0} second.",
                        "Reconnecting in {0} seconds.",
                        seconds,
                    ),
                )
            }
            (None, None) => (theme.text_faint, tr("Not connected").to_owned()),
        }
    }

    /// The end of the status bar: how the connection is doing, and what
    /// is switched on. Laid out from the right.
    pub fn status_line(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        let small = |text: &str, colour| {
            RichText::new(text)
                .font(theme::font(Weight::Regular, 12.0))
                .color(colour)
        };
        ui.spacing_mut().item_spacing.x = 14.0;
        ui.add_space(4.0);
        ui.label(small(&self.status().1, theme.text_dim));
        if self.voice.enabled() {
            ui.label(small(tr("Speech"), theme.text_faint));
        }
        if self.log.is_some() {
            ui.label(small(tr("Logging"), theme.text_faint));
        }
        if self.walk.is_some() {
            ui.label(small(tr("Walking"), theme.accent));
        }
    }

    /// The session's entries of the Session menu.
    pub fn session_menu(&mut self, ui: &mut egui::Ui) {
        if self.connection.is_some() {
            if widgets::menu_item(ui, tr("Disconnect"), "").clicked() {
                self.disconnect();
                ui.close();
            }
        } else if widgets::menu_item(ui, tr("Connect"), "").clicked() {
            self.connect(&ui.ctx().clone());
            ui.close();
        }
        let offline = self.connection.is_none();
        let details = ui
            .add_enabled_ui(offline, |ui| widgets::menu_item(ui, tr("Connection…"), ""))
            .inner;
        if details.clicked() {
            self.connection_open = true;
            ui.close();
        }
        widgets::menu_separator(ui);
        let mut logging = self.log.is_some();
        if widgets::menu_check(ui, &mut logging, tr("Log this session")).changed() {
            self.map_command("/log");
            ui.close();
        }
        let mut speech = self.voice.enabled();
        if widgets::menu_check(ui, &mut speech, tr("Speech")).changed() {
            self.voice.set_enabled(speech);
            ui.close();
        }
    }

    /// The session's entries of the View menu.
    pub fn view_menu(&mut self, ui: &mut egui::Ui) {
        self.panels_menu(ui);
        widgets::menu_separator(ui);
        if widgets::menu_item(ui, tr("Find…"), "Ctrl+F").clicked() {
            self.find_open = true;
            self.find_wants_focus = true;
            ui.close();
        }
    }

    /// The window with every detail of the connection: where and how to
    /// connect, who to log in as, and the profile to keep it all in.
    fn connection_window(&mut self, ctx: &egui::Context) {
        if !self.connection_open || self.connection.is_some() {
            return;
        }
        let theme = theme::current();
        let (mut open, mut connect, mut save) = (true, false, false);
        widgets::dialog(
            ctx,
            "connection",
            tr("Connection"),
            &mut open,
            460.0,
            |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                widgets::caption(ui, tr("Server"));
                widgets::group(ui, |ui| {
                    widgets::setting_row(ui, tr("Host"), &mut |ui| {
                        widgets::row_field(ui, &mut self.host, "mud.example.org", 230.0)
                    });
                    widgets::setting_row(ui, tr("Port"), &mut |ui| {
                        widgets::row_field(ui, &mut self.port, "23", 80.0)
                    });
                    widgets::switch_setting(ui, &mut self.tls, tr("Secure connection (TLS)"));
                    widgets::row_divider(ui);
                    ui.add_enabled_ui(self.tls, |ui| {
                        widgets::switch_setting(
                            ui,
                            &mut self.verify_certificate,
                            tr("Verify certificate"),
                        );
                    });
                    widgets::setting_row(ui, tr("Text encoding"), &mut |ui| {
                        widgets::row_field(ui, &mut self.encoding, "auto", 120.0)
                    })
                    .on_hover_text(tr(
                        "Only for servers that do not use UTF-8 or Latin-1, e.g. big5 or koi8-r.",
                    ));
                });
                ui.add_space(6.0);
                widgets::caption(ui, tr("Log in automatically"));
                widgets::group(ui, |ui| {
                    widgets::setting_row(ui, tr("Character"), &mut |ui| {
                        widgets::row_field(ui, &mut self.character, "", 230.0)
                    });
                    widgets::setting_row(ui, tr("Password"), &mut |ui| {
                        ui.add(
                            TextEdit::singleline(&mut self.new_password)
                                .password(true)
                                .hint_text(RichText::new(tr("unchanged")).color(theme.text_faint))
                                .margin(egui::Margin::symmetric(9, 6))
                                .desired_width(230.0),
                        )
                    })
                    .on_hover_text(tr(
                        "Kept in the system's keyring when the profile is saved.",
                    ));
                });
                ui.add_space(6.0);
                widgets::caption(ui, tr("Profile"));
                widgets::group(ui, |ui| {
                    widgets::setting_row(ui, tr("Profile name"), &mut |ui| {
                        save = ui.button(tr("Save profile")).clicked();
                        widgets::row_field(ui, &mut self.profile_name, "", 150.0)
                    });
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        connect = widgets::primary_button(ui, tr("Connect")).clicked();
                    });
                });
            },
        );
        self.connection_open = open;
        if save {
            self.save_profile();
        }
        if connect {
            self.connect(ctx);
        }
    }
}

/// Splits an address as people write them — `host:port`, `host port`, or
/// only the host, with or without `telnet://` — into host and port. The
/// port is the telnet one when none is given.
fn parse_address(address: &str) -> Option<(String, String)> {
    let address = address.trim();
    let address = address.strip_prefix("telnet://").unwrap_or(address);
    let address = address.trim_end_matches('/');
    let is_port = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    let split = address
        .rsplit_once(|ch: char| ch == ':' || ch.is_whitespace())
        .filter(|(_, port)| is_port(port));
    let (host, port) = split.unwrap_or((address, "23"));
    let host = host.trim().trim_end_matches(':');
    (!host.is_empty()).then(|| (host.to_owned(), port.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::parse_address;

    #[test]
    fn addresses_are_read_as_people_write_them() {
        let parsed = |address| parse_address(address).map(|(host, port)| format!("{host} {port}"));
        assert_eq!(
            parsed("mud.example.org:4000").unwrap(),
            "mud.example.org 4000"
        );
        assert_eq!(
            parsed("  mud.example.org   4000 ").unwrap(),
            "mud.example.org 4000"
        );
        assert_eq!(
            parsed("telnet://mud.example.org:4000/").unwrap(),
            "mud.example.org 4000"
        );
        assert_eq!(parsed("mud.example.org").unwrap(), "mud.example.org 23");
        assert_eq!(parsed("mud.example.org:").unwrap(), "mud.example.org 23");
        assert_eq!(parsed("   "), None);
    }
}
