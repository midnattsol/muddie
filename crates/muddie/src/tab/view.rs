//! How a session looks: the welcome screen before it connects, then the
//! output with its panels, and the session's parts of the menus and of the
//! status bar.

use super::*;
use crate::theme::Weight;

impl Tab {
    /// Draws the session in `ui`.
    pub fn show(&mut self, ui: &mut egui::Ui, _settings: &Settings) {
        if self.editor.recording_key {
            self.record_key(ui);
        } else {
            self.bound_keys(ui);
        }
        self.gauge_window(ui.ctx());
        // The palette of commands, over whichever view is shown.
        self.palette(ui.ctx());
        // The automation view takes the place of the game while open.
        if self.editor.open {
            return self.automation_page(ui);
        }
        // Alt+M says where the player is, by the map.
        if ui.input_mut(|input| input.consume_key(Modifiers::ALT, Key::M)) {
            self.say_where_aloud();
        }
        // Alt+0 reads the gauges aloud.
        if ui.input_mut(|input| input.consume_key(Modifiers::ALT, Key::Num0)) {
            let summary = self.gauges.summary();
            self.voice.say_now(if summary.is_empty() {
                tr("No gauges.")
            } else {
                &summary
            });
        }
        // The channels to read back are those of the panel that shows them.
        let channels = (self.layout.panels.iter()).find(|panel| panel.channels);
        let chat = channels
            .and_then(|panel| self.panels.text.iter().find(|(id, _)| *id == panel.id))
            .map_or(&self.panels.no_chat, |(_, content)| &content.log);
        self.voice.update(ui, &self.output, chat);
        if !self.started {
            return self.welcome(ui);
        }
        let plain = egui::Frame::new().inner_margin(egui::Margin {
            left: 8,
            right: 8,
            top: 4,
            bottom: 8,
        });
        // The command line is at the bottom, under any panel, and is
        // reached last with the keyboard, as it is read.
        let id = ui.make_persistent_id("command line");
        crate::widgets::below_last(
            ui,
            id,
            self,
            |this, ui| {
                let asked = this.show_panels(ui, Self::show_output);
                this.map_asked(asked);
                this.report_panel(ui.ctx());
            },
            |this, ui| {
                Panel::bottom("input")
                    .frame(plain)
                    .show_separator_line(false)
                    .show(ui, |ui| {
                        if this.find.open {
                            this.find_bar(ui);
                        }
                        this.input_line(ui);
                    })
                    .response
                    .rect
                    .height()
            },
        );
    }

    /// The output, which takes what the panels leave.
    fn show_output(&mut self, ui: &mut egui::Ui) {
        let sides = |side| {
            (self.layout.panels.iter())
                .any(|panel| self.layout.on_screen(panel) && panel.side == side)
        };
        let frame = layout::well(
            if sides(layout::Side::Left) { 4 } else { 8 },
            if sides(layout::Side::Right) { 4 } else { 8 },
        );
        let clicked = CentralPanel::default().frame(frame).show(ui, |ui| {
            // What fits in it is what the server is told, less the
            // place of the scroll bar.
            let room = ui.available_size() - egui::vec2(12.0, 0.0);
            self.sizes.window = Self::cells(ui, room);
            self.output.show(ui, "output")
        });
        self.report_window(ui.ctx());
        // A link the server marked as a command was clicked.
        if let Some(clicked) = clicked.inner {
            self.link_clicked(clicked);
        }
    }

    /// Does what was asked of the map in its panel.
    fn map_asked(&mut self, asked: Option<MapAction>) {
        match asked {
            Some(MapAction::Walk(room)) => self.walk_to(room),
            Some(MapAction::Tidy(area)) => self.tidy(&area),
            Some(MapAction::Scripts) => self.open_automation(automation::SCRIPTS),
            Some(MapAction::Command(command)) => {
                self.do_client_command(command);
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
        self.editor.panes
    }

    /// How the connection is doing: a colour for it, and the words.
    /// What speaks now in this session, by its name.
    pub fn speech_through(&self) -> Option<String> {
        self.voice.through()
    }

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
        // What reads aloud, by its name: the screen reader or the voice.
        if let Some(name) = self.voice.through().filter(|_| self.voice.enabled()) {
            ui.label(small(&name, theme.text_faint))
                .on_hover_text(trf("Speaking through {0}.", &[&name]));
        }
        if self.log.is_some() {
            ui.label(small(tr("Logging"), theme.text_faint));
        }
        if self.walk.is_some() {
            ui.label(small(tr("Walking"), theme.accent));
        }
    }
}

impl Tab {
    /// Which of its views the session is on: the game, the editor, or
    /// the editor on what the MUD sends.
    pub fn view(&self) -> usize {
        match (self.editor.open, self.data_open()) {
            (_, true) => 2,
            (true, false) => 1,
            (false, false) => 0,
        }
    }

    /// Goes to one of the session's views: the game, the editor where it
    /// was left, or the editor on what the MUD sends.
    pub fn set_view(&mut self, view: usize) {
        match view {
            0 if self.editor.open => self.close_automation(),
            1 => self.reopen_automation(),
            2 => self.open_automation(automation::DATA),
            _ => {}
        }
    }
}
