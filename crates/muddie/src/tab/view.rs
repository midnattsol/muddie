//! How a session looks: the welcome screen before it connects, then the
//! output with its panels, and the session's parts of the menus and of the
//! status bar.

use super::*;
use crate::theme::Weight;

impl Tab {
    /// Draws the session in `ui`.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        if self.editor.recording_key {
            self.record_key(ui);
        } else {
            self.bound_keys(ui);
        }
        self.gauge_window(ui.ctx());
        self.look_window(ui.ctx());
        self.tidy_window(ui.ctx());
        // The palette of commands, over whichever view is shown.
        self.palette(ui.ctx());
        // The layout and the editor take the place of the game while open.
        if self.layout_view.open {
            if self.unprofiled_page(ui, unprofiled::Unprofiled::Layout) {
                return;
            }
            return self.layout_page(ui);
        }
        if self.editor.open {
            if self.unprofiled_page(ui, unprofiled::Unprofiled::Editor) {
                return;
            }
            return self.automation_page(ui);
        }
        // Alt+M (Ctrl+M on a Mac) says where the player is, by the map;
        // Alt+E explores it.
        if ui.input_mut(|input| crate::shortcuts::WHERE.pressed(input)) {
            self.say_where_aloud();
        }
        if ui.input_mut(|input| crate::shortcuts::EXPLORE.pressed(input)) {
            self.explore_map();
        }
        // Alt+0 (Ctrl+0 on a Mac) reads the gauges aloud.
        if ui.input_mut(|input| crate::shortcuts::GAUGES.pressed(input)) {
            let summary = self.gauges_said();
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
    fn map_asked(&mut self, asked: Option<MapAsk>) {
        match asked {
            Some(MapAsk::Walk(room)) => self.walk_to(room),
            Some(MapAsk::Tidy(area)) => drop(self.run_command(&format!("/map tidy {area}"))),
            Some(MapAsk::Scripts) => self.open_automation(automation::SCRIPTS),
            Some(MapAsk::Selected(room, came)) => {
                let told = {
                    let map = self.map.borrow();
                    let room = map.room(room);
                    room.map(|room| self.map_view.said_of(&map, room, came))
                };
                if let Some(told) = told {
                    self.voice.say_now(&told);
                }
            }
            Some(MapAsk::Way(room)) => self.say_route(&room.to_string()),
            Some(MapAsk::NoExit(way)) => {
                let way = crate::mapview::direction_words(&self.map.borrow(), way);
                self.voice.say_now(&trf("No exit {0}.", &[&way]));
            }
            Some(MapAsk::Say(said)) => self.voice.say_now(&said),
            Some(MapAsk::Info(room)) => drop(self.run_command(&format!("/map info {room}"))),
            Some(MapAsk::Mark(room, name)) => self.mark_room(room, &name),
            Some(MapAsk::Look(room)) => self.open_look(room),
            Some(MapAsk::Note(room, note)) => self.write_note(room, &note),
            Some(MapAsk::Leave(typed)) => {
                self.input.push_str(&typed);
                self.cursor_to_end = true;
            }
            Some(MapAsk::Command(command)) => {
                self.run_command(command);
            }
            Some(MapAsk::Avoid(room, avoid)) => self.avoid_room(room, avoid),
            Some(MapAsk::Move(room, pos)) => {
                let saved = self.map.borrow_mut().move_room(room, pos);
                self.map_saved(saved);
            }
            None => {}
        }
    }

    /// The widths of the automation view's panes and the heights of its
    /// editors of code, as last made.
    pub fn layout(&self) -> ([f32; 2], [f32; 2]) {
        (self.editor.panes, self.editor.code.heights)
    }

    /// Lays out the automation view as `settings` say: as it was made in
    /// whichever session it was.
    pub fn share_layout(&mut self, settings: &Settings) {
        self.editor.panes = settings.automation_panes;
        self.editor.code.heights = settings.code_heights;
    }

    /// The folders open in the editor, and the profile they are kept for;
    /// None until they are those of this session's profile, which
    /// `share_open_folders` makes them.
    pub fn open_folders(&self) -> Option<(String, &[(crate::rules::Kind, String)])> {
        let key = self.profile.clone().unwrap_or_default();
        (self.editor.opened_for.as_ref() == Some(&key)).then_some((key, &self.editor.opened[..]))
    }

    /// Opens the folders the settings keep open for this session's profile,
    /// and those of what is chosen.
    pub fn share_open_folders(&mut self, settings: &Settings) {
        let key = self.profile.clone().unwrap_or_default();
        self.editor.opened = settings.open_folders.get(&key).cloned().unwrap_or_default();
        self.editor.opened_for = Some(key);
        self.reveal_chosen();
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
        // How the connection is: a dot of its colour, a lock if it is
        // encrypted, and where to; the whole sentence for a screen reader.
        let (colour, said) = self.status();
        let connected = self.connection.is_some();
        let shown = match connected {
            true => format!("{}:{}", self.host.trim(), self.port.trim()),
            false => said.clone(),
        };
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let place = ui.label(small(&shown, theme.text_dim)).on_hover_text(&said);
            (ui.ctx()).accesskit_node_builder(place.id, |node| node.set_label(said.clone()));
            if connected && self.tls {
                crate::widgets::glyph(ui, crate::widgets::Glyph::Lock, 10.5, theme.text_faint);
            }
            let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 3.5, colour);
        });
        // What reads aloud, by its name: the screen reader or the voice.
        if let Some(name) = self.voice.through().filter(|_| self.voice.enabled()) {
            ui.label(small(&name, theme.text_faint))
                .on_hover_text(trf("Active voice: {0}.", &[&name]));
        }
        if self.log.is_some() {
            ui.label(small(tr("Logging"), theme.text_faint));
        }
        if self.walk.is_some() {
            ui.label(small(tr("On the way"), theme.accent));
        }
    }
}

/// The views of a session, each over the whole window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Game,
    /// The editor, where it was left.
    Editor,
    /// The editor on what the MUD sends.
    Data,
    Layout,
}

impl Tab {
    /// Which of its views the session is on.
    pub fn view(&self) -> View {
        match (self.layout_view.open, self.editor.open, self.data_open()) {
            (true, ..) => View::Layout,
            (_, _, true) => View::Data,
            (_, true, false) => View::Editor,
            (false, false, false) => View::Game,
        }
    }

    /// Goes to one of the session's views.
    pub fn set_view(&mut self, view: View) {
        match view {
            View::Game => {
                self.layout_view.open = false;
                if self.editor.open {
                    self.close_automation();
                }
            }
            View::Editor => self.reopen_automation(),
            View::Data => self.open_automation(automation::DATA),
            View::Layout => self.open_layout_view(None),
        }
    }
}
