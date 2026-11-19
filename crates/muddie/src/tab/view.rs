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
        self.automation_window(ui.ctx());
        self.connection_window(ui.ctx());
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
        self.voice.update(ui, &self.output, &self.chat);
        if !self.started {
            return self.welcome(ui);
        }
        let theme = theme::current();
        let bar = |vertical: i8| {
            egui::Frame::new()
                .fill(theme.surface)
                .inner_margin(egui::Margin::symmetric(14, vertical))
        };
        if self.show_chat && self.chat.len() > 0 {
            Panel::top("chat")
                .frame(bar(8))
                .resizable(true)
                .default_size(96.0)
                .min_size(56.0)
                .show(ui, |ui| {
                    widgets::caption(ui, tr("Chat"));
                    self.chat.show(ui, "chat");
                });
        }
        Panel::bottom("input")
            .frame(bar(10))
            .show_separator_line(false)
            .show(ui, |ui| {
                if !self.gauges.is_empty() {
                    self.gauges.show(ui);
                    ui.add_space(2.0);
                }
                if self.find_open {
                    self.find_bar(ui);
                }
                self.announce(ui);
                self.input_line(ui);
            });
        let has_map = self.map.borrow().current().is_some();
        if self.show_map && (has_map || !self.text_panels.is_empty()) {
            let action = Panel::right("map")
                .frame(bar(12))
                .resizable(true)
                .default_size(330.0)
                .min_size(200.0)
                .show(ui, |ui| self.side_panel(ui))
                .inner;
            match action {
                Some(MapAction::Walk(room)) => self.walk_to(room),
                Some(MapAction::Tidy(area)) => self.tidy(&area),
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
        let frame = egui::Frame::new()
            .fill(theme.background)
            .inner_margin(egui::Margin::symmetric(16, 10));
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
        let frame = egui::Frame::new().fill(theme.background);
        CentralPanel::default().frame(frame).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    let width = 440.0_f32.min(ui.available_width() - 32.0);
                    let margin = ((ui.available_width() - width) / 2.0).max(0.0);
                    ui.add_space((ui.available_height() * 0.14).clamp(24.0, 120.0));
                    ui.horizontal(|ui| {
                        ui.add_space(margin);
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            ui.label(
                                RichText::new("muddie")
                                    .font(theme::font(Weight::Semibold, 34.0))
                                    .color(theme.text),
                            );
                            let question = RichText::new(tr("Where do you want to play?"))
                                .font(theme::font(Weight::Regular, 15.0))
                                .color(theme.text_dim);
                            ui.label(question);
                            ui.add_space(26.0);
                            if !profiles.is_empty() {
                                widgets::caption(ui, tr("Your profiles"));
                                ui.spacing_mut().item_spacing.y = 2.0;
                                for (name, place) in &profiles {
                                    if widgets::list_row(ui, false, name, place, true).clicked() {
                                        chosen = Some(name.clone());
                                    }
                                }
                                ui.add_space(22.0);
                            }
                            widgets::caption(ui, tr("New connection"));
                            ui.add_space(2.0);
                            widgets::card(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 10.0;
                                ui.horizontal(|ui| {
                                    ui.scope(|ui| {
                                        ui.set_width(width - 28.0 - 108.0);
                                        widgets::field(
                                            ui,
                                            tr("Host"),
                                            &mut self.host,
                                            "mud.example.org",
                                        );
                                    });
                                    ui.scope(|ui| {
                                        ui.set_width(100.0);
                                        widgets::field(ui, tr("Port"), &mut self.port, "23");
                                    });
                                });
                                widgets::switch_row(
                                    ui,
                                    &mut self.tls,
                                    tr("Secure connection (TLS)"),
                                );
                                ui.horizontal(|ui| {
                                    connect = widgets::primary_button(ui, tr("Connect")).clicked();
                                    if widgets::quiet_button(ui, tr("More options…")).clicked() {
                                        self.connection_open = true;
                                    }
                                });
                            });
                            ui.add_space(24.0);
                        });
                    });
                });
        });
        let ctx = ui.ctx().clone();
        if let Some(name) = chosen
            && self.select_profile(Some(name))
        {
            self.connect(&ctx);
        } else if connect {
            self.connect(&ctx);
        }
    }

    /// The session's line of the status bar: how the connection is doing
    /// on the left, what is switched on at the right.
    pub fn status_bar(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        let small = |text: String, colour| {
            RichText::new(text)
                .font(theme::font(Weight::Regular, 12.0))
                .color(colour)
        };
        ui.horizontal(|ui| {
            let place = format!("{}:{}", self.host.trim(), self.port.trim());
            let (colour, text) = match (&self.connection, self.reconnect_at) {
                (Some(_), _) if self.tls => {
                    (theme.good, trf("Connected to {0}, encrypted", &[&place]))
                }
                (Some(_), _) => (theme.good, trf("Connected to {0}", &[&place])),
                (None, Some(at)) => {
                    let seconds = at.saturating_duration_since(Instant::now()).as_secs() + 1;
                    (
                        theme.terminal[3],
                        trf("Reconnecting in {0} seconds.", &[&seconds.to_string()]),
                    )
                }
                (None, None) => (theme.text_faint, tr("Not connected").to_owned()),
            };
            widgets::status_dot(ui, colour);
            ui.label(small(text, theme.text_dim));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                if self.voice.enabled() {
                    ui.label(small(tr("Speech").to_owned(), theme.text_dim));
                }
                if self.log.is_some() {
                    ui.label(small(tr("Logging").to_owned(), theme.text_dim));
                }
                if self.walk.is_some() {
                    ui.label(small(tr("Walking").to_owned(), theme.accent));
                }
                if let Some(profile) = &self.profile {
                    ui.label(small(profile.clone(), theme.text_faint));
                }
            });
        });
    }

    /// The session's entries of the Session menu.
    pub fn session_menu(&mut self, ui: &mut egui::Ui) {
        if self.connection.is_some() {
            if ui.button(tr("Disconnect")).clicked() {
                self.disconnect();
                ui.close();
            }
        } else if ui.button(tr("Connect")).clicked() {
            self.connect(&ui.ctx().clone());
            ui.close();
        }
        if ui
            .add_enabled(
                self.connection.is_none(),
                egui::Button::new(tr("Connection…")),
            )
            .clicked()
        {
            self.connection_open = true;
            ui.close();
        }
        ui.separator();
        let mut logging = self.log.is_some();
        if ui.checkbox(&mut logging, tr("Log this session")).changed() {
            self.map_command("/log");
            ui.close();
        }
        let mut speech = self.voice.enabled();
        if ui.checkbox(&mut speech, tr("Speech")).changed() {
            self.voice.set_enabled(speech);
            ui.close();
        }
    }

    /// The session's entries of the View menu.
    pub fn view_menu(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.show_chat, tr("Chat panel"));
        ui.checkbox(&mut self.show_map, tr("Map panel"));
        ui.separator();
        if ui
            .add(egui::Button::new(tr("Find…")).shortcut_text("Ctrl+F"))
            .clicked()
        {
            self.find_open = true;
            self.find_wants_focus = true;
            ui.close();
        }
    }

    /// The Automation menu.
    pub fn automation_menu(&mut self, ui: &mut egui::Ui) {
        for (section, name) in [tr("Triggers…"), tr("Aliases…"), tr("Macros…")]
            .into_iter()
            .enumerate()
        {
            if ui.button(name).clicked() {
                self.open_automation(section);
                ui.close();
            }
        }
        ui.separator();
        if ui.button(tr("Reload scripts")).clicked() {
            self.map_command("/reload");
            ui.close();
        }
        if ui.button(tr("List script packages")).clicked() {
            self.map_command("/packages");
            ui.close();
        }
    }

    /// The Map menu.
    pub fn map_menu(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.map_view.all_areas, tr("Show all areas"));
        ui.separator();
        for (name, command) in [
            (tr("Where am I?"), "/where"),
            (tr("Tidy this area"), "/tidy"),
            (tr("Avoid this room in routes"), "/avoid"),
            (tr("Stop walking"), "/stop"),
        ] {
            if ui.button(name).clicked() {
                self.map_command(command);
                ui.close();
            }
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
        egui::Window::new(tr("Connection"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(400.0);
                ui.spacing_mut().item_spacing.y = 10.0;
                widgets::caption(ui, tr("Server"));
                ui.horizontal(|ui| {
                    ui.scope(|ui| {
                        ui.set_width(292.0);
                        widgets::field(ui, tr("Host"), &mut self.host, "mud.example.org");
                    });
                    ui.scope(|ui| {
                        ui.set_width(100.0);
                        widgets::field(ui, tr("Port"), &mut self.port, "23");
                    });
                });
                widgets::switch_row(ui, &mut self.tls, tr("Secure connection (TLS)"));
                ui.add_enabled_ui(self.tls, |ui| {
                    widgets::switch_row(ui, &mut self.verify_certificate, tr("Verify certificate"));
                });
                widgets::field(ui, tr("Text encoding"), &mut self.encoding, "auto").on_hover_text(
                    tr("Only for servers that do not use UTF-8 or Latin-1, e.g. big5 or koi8-r."),
                );
                ui.add_space(4.0);
                widgets::caption(ui, tr("Log in automatically"));
                ui.columns(2, |columns| {
                    widgets::field(&mut columns[0], tr("Character"), &mut self.character, "");
                    columns[1].vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        let name = ui.label(
                            RichText::new(tr("Password"))
                                .font(theme::font(Weight::Regular, 12.0))
                                .color(theme.text_dim),
                        );
                        ui.add(
                            TextEdit::singleline(&mut self.new_password)
                                .password(true)
                                .hint_text(RichText::new(tr("unchanged")).color(theme.text_faint))
                                .margin(egui::Margin::symmetric(9, 6))
                                .desired_width(f32::INFINITY),
                        )
                        .on_hover_text(tr(
                            "Kept in the system's keyring when the profile is saved.",
                        ))
                        .labelled_by(name.id);
                    });
                });
                ui.add_space(4.0);
                widgets::caption(ui, tr("Profile"));
                ui.horizontal(|ui| {
                    ui.scope(|ui| {
                        ui.set_width(270.0);
                        widgets::text_box(ui, &mut self.profile_name, tr("Profile name"));
                    });
                    save = ui.button(tr("Save profile")).clicked();
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        connect = widgets::primary_button(ui, tr("Connect")).clicked();
                    });
                });
            });
        self.connection_open = open;
        if save {
            self.save_profile();
        }
        if connect {
            self.connect(ctx);
        }
    }
}
