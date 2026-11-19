//! The main window: connection bar, output and input line.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use eframe::egui::{self, CentralPanel, Key, Label, Modifiers, Panel, RichText, TextEdit};
use muddie_core::Line;
use muddie_core::{Command, Config, Event, Session, Target, connect};
use muddie_mapper::{Map, Progress, RoomId, RoomInfo, Walk};
use muddie_script::{Action, Engine};
use tokio::sync::mpsc::UnboundedSender;

use crate::history::History;
use crate::mapview::{MapAction, MapView};
use crate::output::{Output, server_job};
use crate::palette;
use crate::panels::{self, Gauges, MsdpVitals};
use crate::voice::Voice;

struct Connection {
    commands: UnboundedSender<Command>,
    events: Receiver<Event>,
}

pub struct App {
    host: String,
    port: String,
    tls: bool,
    connection: Option<Connection>,
    output: Output,
    input: String,
    input_hidden: bool,
    history: History,
    voice: Voice,
    scripts: Engine,
    gauges: Gauges,
    msdp_vitals: MsdpVitals,
    chat: Output,
    show_chat: bool,
    /// Shared with the script engine, which reads and edits it. No borrow
    /// may be held while calling into `scripts`.
    map: Rc<RefCell<Map>>,
    map_view: MapView,
    show_map: bool,
    /// Panels of text that scripts filled in, by name, in creation order.
    text_panels: Vec<(String, Vec<Line>)>,
    /// The route being followed, if a walk is under way, and when it last
    /// made progress.
    walk: Option<(Walk, Instant)>,
}

/// How long a walk waits for a move to take effect before giving up, as
/// happens at a closed door.
const WALK_TIMEOUT: Duration = Duration::from_secs(6);

/// Most rooms listed when a search matches several.
const MAX_LISTED_ROOMS: usize = 10;

/// Where the map of the MUD at `host:port` is stored, creating the folder.
fn map_path(host: &str, port: u16) -> Option<PathBuf> {
    let data_dir = std::env::var_os("XDG_DATA_HOME")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".local/share")))?;
    let maps = data_dir.join("muddie").join("maps");
    std::fs::create_dir_all(&maps).ok()?;
    let safe_host: String = (host.chars())
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    Some(maps.join(format!("{safe_host}_{port}.sqlite")))
}

/// What to do when the window opens.
pub struct Startup {
    /// Where to connect right away, if anywhere.
    pub target: Option<Target>,
    pub speech: bool,
    /// Script files to run, in order.
    pub scripts: Vec<PathBuf>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        cc.egui_ctx.all_styles_mut(|style| {
            style.visuals.panel_fill = palette::BACKGROUND;
        });
        let map = Rc::new(RefCell::new(Map::in_memory()));
        let mut app = Self {
            host: String::new(),
            port: "23".into(),
            tls: false,
            connection: None,
            output: Output::default(),
            input: String::new(),
            input_hidden: false,
            history: History::default(),
            voice: Voice::default(),
            scripts: Engine::new(Rc::clone(&map)).expect("failed to start the script runtime"),
            gauges: Gauges::default(),
            msdp_vitals: MsdpVitals::default(),
            chat: Output::default(),
            show_chat: true,
            map,
            map_view: MapView::default(),
            show_map: true,
            text_panels: Vec::new(),
            walk: None,
        };
        app.voice.set_enabled(startup.speech);
        for script in &startup.scripts {
            app.scripts.load_file(script);
        }
        app.run_actions();
        if let Some(target) = startup.target {
            app.host = target.host;
            app.port = target.port.to_string();
            app.tls = target.tls;
            app.connect(&cc.egui_ctx);
        }
        app
    }

    fn connect(&mut self, ctx: &egui::Context) {
        let host = self.host.trim().to_owned();
        let Ok(port) = self.port.trim().parse::<u16>() else {
            self.note("The port must be a number from 1 to 65535.");
            return;
        };
        if host.is_empty() {
            self.note("Enter a host to connect to.");
            return;
        }
        self.note(&format!("Connecting to {host}:{port}..."));
        self.walk = None;
        // A script may have chosen to keep the map in memory only.
        let stored = if self.scripts.persist_map() {
            map_path(&host, port).map(|path| Map::open(&path))
        } else {
            Some(Ok(Map::in_memory()))
        };
        let map = match stored {
            Some(Ok(map)) => map,
            Some(Err(error)) => {
                self.note(&format!("The map will not be saved: {error}"));
                Map::in_memory()
            }
            None => {
                self.note("The map will not be saved: no data folder was found.");
                Map::in_memory()
            }
        };
        *self.map.borrow_mut() = map;
        let target = Target {
            host,
            port,
            tls: self.tls,
        };
        let Session {
            commands,
            mut events,
        } = connect(
            target,
            Config {
                screen_reader: self.voice.enabled(),
                ..Config::default()
            },
        );

        // Forward events to the UI thread and wake it up for each one.
        let (sender, receiver) = channel();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                if sender.send(event).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        self.connection = Some(Connection {
            commands,
            events: receiver,
        });
    }

    fn disconnect(&mut self) {
        if let Some(connection) = &self.connection {
            let _ = connection.commands.send(Command::Disconnect);
        }
    }

    /// Shows and speaks a message from the client itself.
    fn note(&mut self, text: &str) {
        self.output.push_local(text);
        self.voice.say(text);
    }

    fn handle_events(&mut self) {
        let Some(connection) = &self.connection else {
            return;
        };
        let events: Vec<Event> = connection.events.try_iter().collect();
        for event in events {
            match event {
                Event::Connected => self.note("Connected."),
                Event::Line(line) | Event::Prompt(line) => {
                    let text = line.text();
                    if self.scripts.on_line(&text) {
                        self.output.push(&line);
                        self.voice.line(&text);
                    } else {
                        self.output.clear_partial();
                        self.voice.skip_line(&text);
                    }
                    self.run_actions();
                }
                Event::Partial(line) => {
                    self.output.set_partial(&line);
                    self.voice.partial(&line.text());
                }
                Event::InputHidden(hidden) => self.input_hidden = hidden,
                // Not spoken: the user has just typed it.
                Event::Sent(text) => self.output.push_local(&format!("> {text}")),
                Event::Gmcp { package, data } => {
                    // Built-in panels first, so scripts can override them.
                    if package.eq_ignore_ascii_case("Char.Vitals") {
                        for (name, value, max) in panels::vitals(&data) {
                            self.gauges.set(name, Some((value, max)));
                        }
                    } else if package.eq_ignore_ascii_case("Comm.Channel.Text") {
                        for line in panels::channel_text(&data) {
                            self.chat.push(&line);
                        }
                    } else if package.eq_ignore_ascii_case("Room.Info") && self.scripts.native_map()
                    {
                        self.entered_room(&data);
                    }
                    self.scripts.on_gmcp(&package, &data);
                    self.run_actions();
                }
                Event::Msdp { variable, data } => {
                    if let Some((name, value, max)) = self.msdp_vitals.update(&variable, &data) {
                        self.gauges.set(name, Some((value, max)));
                    }
                    if variable == "ROOM" && self.scripts.native_map() {
                        self.entered_room(&data);
                    }
                    self.scripts.on_msdp(&variable, &data);
                    self.run_actions();
                }
                Event::Mssp(status) => self.scripts.on_mssp(&status),
                Event::Disconnected(error) => {
                    self.walk = None;
                    self.gauges.clear();
                    self.msdp_vitals.clear();
                    self.note(&match error {
                        Some(error) => format!("Disconnected: {error}"),
                        None => "Disconnected.".to_owned(),
                    });
                    self.input_hidden = false;
                    self.connection = None;
                }
            }
        }
    }

    /// Carries out what scripts asked for since the last call.
    fn run_actions(&mut self) {
        for action in self.scripts.take_actions() {
            match action {
                Action::Send(text) => self.send(text),
                Action::Echo(text) => text.lines().for_each(|line| self.note(line)),
                Action::Speak { text, interrupt } => {
                    if interrupt {
                        self.voice.stop();
                    }
                    self.voice.say(&text);
                }
                Action::Gmcp { package, data } => {
                    if let Some(connection) = &self.connection {
                        let _ = connection.commands.send(Command::Gmcp { package, data });
                    }
                }
                Action::Msdp { variable, values } => {
                    if let Some(connection) = &self.connection {
                        let _ = connection.commands.send(Command::Msdp { variable, values });
                    }
                }
                Action::Gauge { name, level } => self.gauges.set(&name, level),
                Action::Walk(room) => self.walk_to(room),
                Action::Entered(room) => self.arrived_in(room),
                Action::Panel { name, text } => {
                    let index = self
                        .text_panels
                        .iter()
                        .position(|(panel, _)| *panel == name);
                    match (index, text) {
                        (Some(index), Some(text)) => {
                            self.text_panels[index].1 = panels::styled_lines(&text);
                        }
                        (None, Some(text)) => {
                            self.text_panels.push((name, panels::styled_lines(&text)));
                        }
                        (Some(index), None) => {
                            self.text_panels.remove(index);
                        }
                        (None, None) => {}
                    }
                }
                Action::Chat(text) => {
                    for line in panels::styled_lines(&text) {
                        self.chat.push(&line);
                    }
                }
                Action::Error(message) => {
                    let message = format!("Script error: {message}");
                    message.lines().for_each(|line| self.note(line));
                }
            }
        }
    }

    fn send(&mut self, line: String) {
        match &self.connection {
            Some(connection) => {
                let _ = connection.commands.send(Command::Send(line));
            }
            None => self.note("Not connected."),
        }
    }

    /// The server reported the room the player is in.
    fn entered_room(&mut self, json: &str) {
        let Some(info) = RoomInfo::from_json(json) else {
            return;
        };
        let room = info.id;
        let saved = self.map.borrow_mut().visit(info);
        if let Err(error) = saved {
            self.note(&format!("Could not save the map: {error}"));
        }
        self.arrived_in(room);
    }

    /// The map now has the player in `room`: continue a walk, if any.
    fn arrived_in(&mut self, room: RoomId) {
        if let Some((walk, _)) = &mut self.walk {
            let progress = walk.entered(room);
            self.follow(progress);
        }
    }

    fn walk_to(&mut self, room: RoomId) {
        let route = {
            let map = self.map.borrow();
            let Some(from) = map.current().map(|current| current.id) else {
                drop(map);
                return self.note("The map does not know where you are yet.");
            };
            map.route(from, room).map(|route| (from, route))
        };
        let Some((from, route)) = route else {
            return self.note("No known route to that room.");
        };
        let walk = Walk::new(from, route);
        let progress = walk.start();
        self.walk = Some((walk, Instant::now()));
        self.follow(progress);
    }

    fn follow(&mut self, progress: Progress) {
        match progress {
            Progress::Go(commands) => {
                if let Some((_, moved_at)) = &mut self.walk {
                    *moved_at = Instant::now();
                }
                // An exit may need several commands, as in "open door;n".
                for command in commands.split(';') {
                    self.send(command.trim().to_owned());
                }
            }
            Progress::Waiting => {}
            Progress::Arrived => {
                self.walk = None;
                self.note("Arrived.");
            }
            Progress::Lost => {
                self.walk = None;
                self.note("Walk stopped: this is not the room the route expected.");
            }
        }
    }

    /// Gives up a walk whose last move had no effect for too long.
    fn check_walk(&mut self, ctx: &egui::Context) {
        let Some((_, moved_at)) = &self.walk else {
            return;
        };
        let waited = moved_at.elapsed();
        if waited >= WALK_TIMEOUT {
            self.walk = None;
            self.note("Walk stopped: the last move had no effect.");
        } else {
            ctx.request_repaint_after(WALK_TIMEOUT - waited);
        }
    }

    /// Handles the client's own map commands. Returns false if `line` is
    /// not one of them.
    fn map_command(&mut self, line: &str) -> bool {
        let (command, argument) = line.split_once(' ').unwrap_or((line, ""));
        let argument = argument.trim();
        match command {
            "/where" => {
                let place = self.map.borrow().current().map(|room| {
                    let exits: Vec<&str> = room.exits.keys().map(String::as_str).collect();
                    format!(
                        "{} (#{}) in {}. Exits: {}.",
                        room.name,
                        room.id,
                        room.area,
                        exits.join(", ")
                    )
                });
                match place {
                    Some(place) => self.note(&place),
                    None => self.note("The map does not know where you are yet."),
                }
            }
            "/find" if !argument.is_empty() => {
                let rooms = self.matching_rooms(argument);
                if rooms.is_empty() {
                    self.note("No room matches.");
                }
                rooms.iter().for_each(|room| self.note(room));
            }
            "/go" if !argument.is_empty() => {
                if let Ok(room) = argument.parse::<RoomId>() {
                    self.walk_to(room);
                } else {
                    let found: Vec<RoomId> = (self.map.borrow().find(argument).iter())
                        .map(|room| room.id)
                        .collect();
                    match found[..] {
                        [] => self.note("No room matches."),
                        [room] => self.walk_to(room),
                        _ => {
                            self.note("Several rooms match; use /go with a room number:");
                            let rooms = self.matching_rooms(argument);
                            rooms.iter().for_each(|room| self.note(room));
                        }
                    }
                }
            }
            // Toggles whether routes may pass through a room: the given
            // one, or the current one.
            "/avoid" => {
                let toggled = {
                    let mut map = self.map.borrow_mut();
                    let room = match argument.parse::<RoomId>() {
                        Ok(id) => map.room(id),
                        Err(_) if argument.is_empty() => map.current(),
                        Err(_) => None,
                    };
                    let target = room.map(|room| (room.id, !room.avoid, room.name.clone()));
                    target.map(|(id, avoid, name)| (map.set_avoid(id, avoid), avoid, name))
                };
                match toggled {
                    Some((Ok(_), true, name)) => self.note(&format!("Routes will avoid {name}.")),
                    Some((Ok(_), false, name)) => {
                        self.note(&format!("Routes may pass through {name} again."));
                    }
                    Some((Err(error), ..)) => {
                        self.note(&format!("Could not save the map: {error}"))
                    }
                    None => self.note("No such room; use /avoid with a room number."),
                }
            }
            // Toggles whether routes may use an exit of the current room.
            "/lock" if !argument.is_empty() => {
                let toggled = {
                    let mut map = self.map.borrow_mut();
                    let here = map.current().map(|room| room.id);
                    let direction = muddie_mapper::canonical_direction(argument);
                    let exit = here.and_then(|id| map.room(id)?.exits.get(&direction).cloned());
                    here.zip(exit).map(|(id, exit)| {
                        let locked = !exit.locked;
                        (
                            map.edit_exit(id, &direction, |exit| exit.locked = locked),
                            locked,
                        )
                    })
                };
                match toggled {
                    Some((Ok(_), true)) => self.note("Routes will not use that exit."),
                    Some((Ok(_), false)) => self.note("Routes may use that exit again."),
                    Some((Err(error), _)) => self.note(&format!("Could not save the map: {error}")),
                    None => self.note("This room has no such exit."),
                }
            }
            // Draws the current area again from scratch.
            "/tidy" => {
                let area = self.map.borrow().current().map(|room| room.area.clone());
                match area {
                    Some(area) => self.tidy(&area),
                    None => self.note("The map does not know where you are yet."),
                }
            }
            "/stop" => {
                let stopped = self.walk.take().is_some();
                self.note(if stopped {
                    "Walk stopped."
                } else {
                    "Not walking."
                });
            }
            _ => return false,
        }
        true
    }

    fn tidy(&mut self, area: &str) {
        let moved = self.map.borrow_mut().relayout(area);
        match moved {
            Ok(moved) => self.note(&format!("Tidied the map: {moved} rooms moved.")),
            Err(error) => self.note(&format!("Could not save the map: {error}")),
        }
    }

    /// The right-hand panel: text panels from scripts, then the map.
    fn side_panel(&mut self, ui: &mut egui::Ui) -> Option<MapAction> {
        for (name, lines) in &self.text_panels {
            ui.label(RichText::new(name).weak());
            for line in lines {
                // Never wrapped: this is often a drawing made of characters.
                ui.add(Label::new(server_job(line)).extend());
            }
            ui.separator();
        }
        let map = self.map.borrow();
        if map.current().is_some() {
            self.map_view.show(ui, &map)
        } else {
            None
        }
    }

    /// Up to [`MAX_LISTED_ROOMS`] rooms matching `query`, one line each.
    fn matching_rooms(&self, query: &str) -> Vec<String> {
        (self.map.borrow().find(query).iter())
            .take(MAX_LISTED_ROOMS)
            .map(|room| format!("#{}: {} ({})", room.id, room.name, room.area))
            .collect()
    }

    fn submit(&mut self) {
        let line = std::mem::take(&mut self.input);
        // What follows answers this command, so drop what was still queued.
        self.voice.stop();
        if self.input_hidden {
            // A password: scripts and the history never see it.
            self.send(line);
        } else {
            self.history.add(&line);
            if !self.map_command(&line) {
                self.scripts.on_input(&line);
                self.run_actions();
            }
        }
    }

    fn connection_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let connected = self.connection.is_some();
            ui.add_enabled_ui(!connected, |ui| {
                let label = ui.label("Host");
                ui.add(TextEdit::singleline(&mut self.host).desired_width(220.0))
                    .labelled_by(label.id);
                let label = ui.label("Port");
                ui.add(TextEdit::singleline(&mut self.port).desired_width(60.0))
                    .labelled_by(label.id);
                ui.checkbox(&mut self.tls, "TLS");
            });
            if connected {
                if ui.button("Disconnect").clicked() {
                    self.disconnect();
                }
            } else if ui.button("Connect").clicked() {
                self.connect(ui.ctx());
            }
            ui.checkbox(&mut self.show_chat, "Chat");
            ui.checkbox(&mut self.show_map, "Map");
            let mut speech = self.voice.enabled();
            if ui.checkbox(&mut speech, "Speech").changed() {
                self.voice.set_enabled(speech);
            }
        });
    }

    fn input_line(&mut self, ui: &mut egui::Ui) {
        let response = ui.add(
            TextEdit::singleline(&mut self.input)
                .font(palette::font())
                .password(self.input_hidden)
                .hint_text("Type a command")
                .desired_width(f32::INFINITY),
        );
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label("Command");
        });
        if response.has_focus() {
            let (up, down) = ui.input_mut(|input| {
                (
                    input.consume_key(Modifiers::NONE, Key::ArrowUp),
                    input.consume_key(Modifiers::NONE, Key::ArrowDown),
                )
            });
            if up {
                self.history.older(&mut self.input);
            } else if down {
                self.history.newer(&mut self.input);
            }
        }
        let submitted = response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        if submitted {
            self.submit();
        }
        // The input owns the keyboard unless the user moved focus elsewhere.
        if submitted || ui.memory(|memory| memory.focused().is_none()) {
            response.request_focus();
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_events();
        let now = Instant::now();
        self.scripts.tick(now);
        self.run_actions();
        self.check_walk(ui.ctx());
        if let Some(deadline) = self.scripts.next_deadline() {
            ui.ctx()
                .request_repaint_after(deadline.saturating_duration_since(now));
        }
        // Alt+0 reads the gauges aloud.
        if ui.input_mut(|input| input.consume_key(Modifiers::ALT, Key::Num0)) {
            self.voice.stop();
            let summary = self.gauges.summary();
            self.voice.say(if summary.is_empty() {
                "No gauges."
            } else {
                &summary
            });
        }
        self.voice.update(ui, &self.output);
        Panel::top("connection").show(ui, |ui| self.connection_bar(ui));
        if self.show_chat && self.chat.len() > 0 {
            Panel::top("chat")
                .resizable(true)
                .default_size(130.0)
                .min_size(48.0)
                .show(ui, |ui| self.chat.show(ui));
        }
        Panel::bottom("input").show(ui, |ui| {
            if !self.gauges.is_empty() {
                self.gauges.show(ui);
            }
            self.input_line(ui);
        });
        let has_map = self.map.borrow().current().is_some();
        if self.show_map && (has_map || !self.text_panels.is_empty()) {
            let action = Panel::right("map")
                .resizable(true)
                .default_size(320.0)
                .min_size(160.0)
                .show(ui, |ui| self.side_panel(ui))
                .inner;
            match action {
                Some(MapAction::Walk(room)) => self.walk_to(room),
                Some(MapAction::Tidy(area)) => self.tidy(&area),
                Some(MapAction::Avoid(room, avoid)) => {
                    let saved = self.map.borrow_mut().set_avoid(room, avoid);
                    if let Err(error) = saved {
                        self.note(&format!("Could not save the map: {error}"));
                    }
                }
                Some(MapAction::Move(room, pos)) => {
                    let saved = self.map.borrow_mut().move_room(room, pos);
                    if let Err(error) = saved {
                        self.note(&format!("Could not save the map: {error}"));
                    }
                }
                None => {}
            }
        }
        CentralPanel::default().show(ui, |ui| self.output.show(ui));
    }
}
