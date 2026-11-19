//! One session: its connection bar, output, panels and input line.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, CentralPanel, ComboBox, Key, Label, Modifiers, Panel, RichText, TextEdit,
};
use muddie_core::Line;
use muddie_core::{Command, Config, Event, Session, Target, connect};
use muddie_mapper::{Map, Progress, RoomId, RoomInfo, Walk};
use muddie_script::{Action, Engine};
use tokio::sync::mpsc::UnboundedSender;

use crate::complete::{self, Completion};
use crate::history::History;
use crate::i18n::{tr, trf};
use crate::keypad;
use crate::logger::SessionLog;
use crate::mapview::{MapAction, MapView};
use crate::output::{Output, server_job};
use crate::packages;
use crate::palette;
use crate::panels::{self, Gauges, MsdpVitals};
use crate::profile::{Profile, Profiles};
use crate::rules::{Alias, KeyBinding, Rules, Trigger};
use crate::secrets;
use crate::settings::{LogFormat, Settings};
use crate::theme;
use crate::voice::Voice;
use muddie_sound::{self as sound, Msp, Player};

struct Connection {
    commands: UnboundedSender<Command>,
    events: Receiver<Event>,
}

pub struct Tab {
    host: String,
    port: String,
    tls: bool,
    verify_certificate: bool,
    profiles: Option<Profiles>,
    /// The profile in use, if any.
    profile: Option<String>,
    /// Its packages, kept so that saving the profile does not lose them.
    packages: Vec<String>,
    /// Name typed in the profile bar, to save the profile under.
    profile_name: String,
    /// Scripts given on the command line, run after the profile's.
    extra_scripts: Vec<PathBuf>,
    /// Unterminated text waiting to settle before its triggers run.
    pending_partial: Option<(Line, Instant)>,
    /// Unterminated text whose triggers already ran, and whether they let
    /// it show: the line to show, or `None` if they gagged it.
    settled_partial: Option<(String, Option<Line>)>,
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
    /// Output arrived while the session was not on screen.
    unread: bool,
    /// The settings in force, as of the last change.
    settings: Settings,
    /// The log being written, if logging is on.
    log: Option<SessionLog>,
    sound: Player,
    completion: Completion,
    /// The find bar is showing, and what it searches for.
    find_open: bool,
    find_query: String,
    /// Give the find bar the keyboard on the next frame.
    find_wants_focus: bool,
    /// The user asked to disconnect, so the session must not reconnect.
    user_disconnected: bool,
    /// When to try connecting again after losing the connection, and how
    /// many times in a row that has been tried.
    reconnect_at: Option<Instant>,
    reconnect_attempts: u32,
    /// When something was last sent or received, for keep-alives.
    last_activity: Instant,
    /// The triggers, aliases and keys made in the interface, as saved.
    rules: Rules,
    /// The copy being edited in the rules window.
    rules_draft: Rules,
    rules_open: bool,
    /// The connection window is showing.
    connection_open: bool,
    /// Which of triggers, aliases and keys the rules window shows.
    rules_section: usize,
    /// Label of the text encoding the server uses; `auto` for most.
    encoding: String,
    /// Package installs and updates under way; the flag says to enable
    /// the package when its job ends well.
    package_jobs: Vec<(Receiver<packages::Outcome>, bool)>,
    /// Character to log in as, from the profile.
    character: String,
    /// A new password typed in the bar, saved with the profile.
    new_password: String,
    login: Login,
    /// Keypad keys pressed since the last frame, by name.
    pub keypad_pressed: Vec<&'static str>,
    /// What the screen reader was last asked to announce.
    announcement: String,
}

/// How far an automatic login has got.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Login {
    Idle,
    /// Waiting for the first prompt, to answer it with the name.
    Name(Instant),
    /// Waiting for the server to hide input, to answer with the password.
    Password(Instant),
}

/// How long an automatic login waits for the prompt it expects.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(60);

/// How long unterminated text must stay unchanged before triggers see it
/// as a line. Servers that do not mark their prompts leave them like that.
const PARTIAL_SETTLE: Duration = Duration::from_millis(400);

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
    /// The configuration folder, if the system has one.
    pub profiles: Option<Profiles>,
    /// Profile to load and connect to right away.
    pub profile: Option<String>,
    /// Where to connect right away when no profile is given.
    pub target: Option<Target>,
    pub speech: bool,
    /// Extra script files to run after the profile's, in order.
    pub scripts: Vec<PathBuf>,
}

impl Tab {
    pub fn new(ctx: &egui::Context, startup: Startup, settings: &Settings) -> Self {
        let map = Rc::new(RefCell::new(Map::in_memory()));
        let mut app = Self {
            host: String::new(),
            port: "23".into(),
            tls: false,
            verify_certificate: true,
            profiles: startup.profiles,
            profile: None,
            packages: Vec::new(),
            profile_name: String::new(),
            extra_scripts: startup.scripts,
            pending_partial: None,
            settled_partial: None,
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
            unread: false,
            settings: settings.clone(),
            log: None,
            sound: Player::default(),
            completion: Completion::default(),
            find_open: false,
            find_query: String::new(),
            find_wants_focus: false,
            user_disconnected: false,
            reconnect_at: None,
            reconnect_attempts: 0,
            last_activity: Instant::now(),
            rules: Rules::default(),
            rules_draft: Rules::default(),
            rules_open: false,
            connection_open: false,
            rules_section: 0,
            encoding: "auto".into(),
            package_jobs: Vec::new(),
            character: String::new(),
            new_password: String::new(),
            login: Login::Idle,
            keypad_pressed: Vec::new(),
            announcement: String::new(),
        };
        app.voice.set_enabled(startup.speech);
        match startup.profile {
            Some(name) => {
                if app.select_profile(Some(name)) {
                    app.connect(ctx);
                }
            }
            None => app.load_scripts(),
        }
        if let Some(target) = startup.target {
            app.host = target.host;
            app.port = target.port.to_string();
            app.tls = target.tls;
            app.verify_certificate = target.verify_certificate;
            app.connect(ctx);
        }
        app.settings_changed(settings);
        // With nowhere to connect to yet, start by asking.
        app.connection_open = app.host.trim().is_empty();
        app
    }

    /// What the session's tab is called: its profile, else its server.
    pub fn title(&self) -> String {
        match (&self.profile, self.host.trim()) {
            (Some(profile), _) => profile.clone(),
            (None, "") => tr("New session").to_owned(),
            (None, host) => host.to_owned(),
        }
    }

    /// Whether output arrived since the session was last on screen.
    pub fn has_unread(&self) -> bool {
        self.unread
    }

    /// Takes changed settings into account.
    pub fn settings_changed(&mut self, settings: &Settings) {
        self.settings = settings.clone();
        self.output
            .restyle(settings.scrollback_lines, settings.timestamps);
        self.chat
            .restyle(settings.scrollback_lines, settings.timestamps);
        self.voice.set_rate(settings.speech_rate);
        self.voice.set_language(&settings.speech_language);
        self.voice
            .set_announce(settings.announce_with_screen_reader);
    }

    /// The part of the settings window that belongs to sessions.
    pub fn settings_ui(ui: &mut egui::Ui, settings: &mut Settings) {
        ui.add_space(10.0);
        ui.label(RichText::new(tr("Sessions")).strong());
        egui::Grid::new("sessions")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                let label = ui.label(tr("Lines kept"));
                ui.add(
                    egui::DragValue::new(&mut settings.scrollback_lines)
                        .range(100..=1_000_000)
                        .speed(100),
                )
                .labelled_by(label.id);
                ui.end_row();

                ui.label(tr("Output"));
                ui.checkbox(&mut settings.timestamps, tr("Show the time of each line"));
                ui.end_row();

                ui.label(tr("Log"));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut settings.log_sessions, tr("Log every session"));
                    let name = |format: LogFormat| match format {
                        LogFormat::Text => tr("Text"),
                        LogFormat::Html => "HTML",
                    };
                    ComboBox::from_id_salt("log format")
                        .selected_text(name(settings.log_format))
                        .width(70.0)
                        .show_ui(ui, |ui| {
                            for format in [LogFormat::Text, LogFormat::Html] {
                                ui.selectable_value(&mut settings.log_format, format, name(format));
                            }
                        });
                });
                ui.end_row();

                ui.label(tr("Connection"));
                ui.checkbox(
                    &mut settings.reconnect,
                    tr("Reconnect when the connection is lost"),
                );
                ui.end_row();

                let label = ui.label(tr("Keep-alive"));
                ui.add(
                    egui::DragValue::new(&mut settings.keepalive_seconds)
                        .range(0..=3600)
                        .suffix(" s"),
                )
                .on_hover_text(tr(
                    "Seconds of silence before a keep-alive is sent; 0 sends none.",
                ))
                .labelled_by(label.id);
                ui.end_row();

                let label = ui.label(tr("Speech rate"));
                ui.add(egui::Slider::new(&mut settings.speech_rate, -100..=100))
                    .labelled_by(label.id);
                ui.end_row();

                ui.label(tr("Screen reader"));
                ui.checkbox(
                    &mut settings.announce_with_screen_reader,
                    tr("Voice through the screen reader (also braille)"),
                )
                .on_hover_text(tr(
                    "Instead of speaking directly, hand each line to the screen reader to announce.",
                ));
                ui.end_row();

                let label = ui.label(tr("Speech language"));
                ui.add(
                    TextEdit::singleline(&mut settings.speech_language)
                        .hint_text(tr("system"))
                        .desired_width(70.0),
                )
                .on_hover_text(tr(
                    "A language code such as es or en, so the text is read with the right voice.",
                ))
                .labelled_by(label.id);
                ui.end_row();
            });
    }

    /// Starts the script runtime afresh and runs the user's scripts: the
    /// general one, the profile's packages and scripts, and those given on
    /// the command line.
    fn load_scripts(&mut self) {
        self.scripts =
            Engine::new(Rc::clone(&self.map)).expect("failed to start the script runtime");
        if let Some(profiles) = &self.profiles {
            (self.scripts).use_memory_file(profiles.memory_path(self.profile.as_deref()));
        }
        let mut files = Vec::new();
        if let Some(profiles) = &self.profiles {
            files.extend(profiles.init_script());
            if let Some(name) = &self.profile {
                let profile = Profile {
                    packages: self.packages.clone(),
                    ..Profile::default()
                };
                files.extend(profiles.scripts(name, &profile));
            }
        }
        files.extend(self.extra_scripts.iter().cloned());
        for file in &files {
            self.scripts.load_file(file);
        }
        // The rules made in the interface run last, as one more script.
        if let Some(profiles) = &self.profiles {
            match Rules::load(&profiles.rules_path(self.profile.as_deref())) {
                Ok(rules) => self.rules = rules,
                Err(error) => self.note(&trf("Could not load the rules: {0}", &[&error])),
            }
        }
        self.scripts.exec("rules", &self.rules.to_luau());
        self.run_actions();
    }

    /// Saves the rules edited in the rules window and puts them to work.
    fn save_rules(&mut self) {
        let Some(profiles) = &self.profiles else {
            return self.note(tr(
                "The rules cannot be saved: no configuration folder was found.",
            ));
        };
        match self
            .rules_draft
            .save(&profiles.rules_path(self.profile.as_deref()))
        {
            Ok(()) => {
                self.load_scripts();
                self.note(tr("Rules saved."));
            }
            Err(error) => self.note(&trf("Could not save the rules: {0}", &[&error])),
        }
    }

    /// Switches to a saved profile, or to none. Returns false if the
    /// profile could not be loaded.
    fn select_profile(&mut self, name: Option<String>) -> bool {
        let loaded = match (&name, &self.profiles) {
            (Some(name), Some(profiles)) => profiles.load(name).map(Some),
            (Some(_), None) => Err("no configuration folder was found".to_owned()),
            (None, _) => Ok(None),
        };
        let profile = match loaded {
            Ok(profile) => profile,
            Err(error) => {
                self.note(&trf(
                    "Could not load the profile: {0}",
                    &[&error.to_string()],
                ));
                return false;
            }
        };
        if let Some(profile) = profile {
            self.host = profile.host;
            self.port = profile.port.to_string();
            self.tls = profile.tls;
            self.verify_certificate = profile.verify_certificate;
            self.packages = profile.packages;
            self.character = profile.character;
            self.encoding = profile.encoding;
            self.voice.set_enabled(profile.speech);
        } else {
            self.packages.clear();
        }
        self.profile_name = name.clone().unwrap_or_default();
        self.profile = name;
        self.load_scripts();
        true
    }

    /// Saves the connection settings as the profile named in the bar.
    fn save_profile(&mut self) {
        let name = self.profile_name.trim().to_owned();
        let Some(profiles) = &self.profiles else {
            return self.note(tr(
                "Profiles cannot be saved: no configuration folder was found.",
            ));
        };
        let Ok(port) = self.port.trim().parse::<u16>() else {
            return self.note(tr("The port must be a number from 1 to 65535."));
        };
        let profile = Profile {
            host: self.host.trim().to_owned(),
            port,
            tls: self.tls,
            verify_certificate: self.verify_certificate,
            speech: self.voice.enabled(),
            character: self.character.trim().to_owned(),
            encoding: self.encoding.trim().to_owned(),
            // A profile saved under a new name starts without packages.
            packages: if self.profile.as_deref() == Some(&name) {
                self.packages.clone()
            } else {
                Vec::new()
            },
        };
        match profiles.save(&name, &profile) {
            Ok(()) => {
                let renamed = self.profile.as_deref() != Some(&name);
                self.packages = profile.packages;
                self.profile = Some(name.clone());
                if renamed {
                    self.load_scripts();
                }
                self.note(&trf("Profile {0} saved.", &[&name]));
                let password = std::mem::take(&mut self.new_password);
                if !password.is_empty()
                    && let Err(error) = secrets::set_password(&name, Some(&password))
                {
                    self.note(&trf(
                        "The password could not be saved in the keyring: {0}",
                        &[&error],
                    ));
                }
            }
            Err(error) => self.note(&trf(
                "Could not save the profile: {0}",
                &[&error.to_string()],
            )),
        }
    }

    fn connect(&mut self, ctx: &egui::Context) {
        let host = self.host.trim().to_owned();
        let Ok(port) = self.port.trim().parse::<u16>() else {
            self.note(tr("The port must be a number from 1 to 65535."));
            return;
        };
        if host.is_empty() {
            self.note(tr("Enter a host to connect to."));
            return;
        }
        if muddie_core::decode::Decoder::for_label(&self.encoding).is_none() {
            let unknown = self.encoding.clone();
            self.note(&trf(
                "Unknown text encoding {0}; using the automatic one.",
                &[&unknown],
            ));
        }
        self.user_disconnected = false;
        self.reconnect_at = None;
        self.note(&trf(
            "Connecting to {0}:{1}...",
            &[&host.to_string(), &port.to_string()],
        ));
        self.walk = None;
        // A script may have chosen to keep the map in memory only.
        let stored = if self.scripts.persist_map() {
            // A profile keeps its map in its own folder.
            let path = match (&self.profiles, &self.profile) {
                (Some(profiles), Some(name)) => Some(profiles.map_path(name)),
                _ => map_path(&host, port),
            };
            path.map(|path| Map::open(&path))
        } else {
            Some(Ok(Map::in_memory()))
        };
        let map = match stored {
            Some(Ok(map)) => map,
            Some(Err(error)) => {
                self.note(&trf(
                    "The map will not be saved: {0}",
                    &[&error.to_string()],
                ));
                Map::in_memory()
            }
            None => {
                self.note(tr("The map will not be saved: no data folder was found."));
                Map::in_memory()
            }
        };
        *self.map.borrow_mut() = map;
        let target = Target {
            host,
            port,
            tls: self.tls,
            verify_certificate: self.verify_certificate,
        };
        let Session {
            commands,
            mut events,
        } = connect(
            target,
            Config {
                screen_reader: self.voice.enabled(),
                encoding: self.encoding.clone(),
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
        self.user_disconnected = true;
        self.reconnect_at = None;
        if let Some(connection) = &self.connection {
            let _ = connection.commands.send(Command::Disconnect);
        }
    }

    /// Shows and speaks a message from the client itself.
    pub fn note(&mut self, text: &str) {
        self.show_local(text);
        self.voice.say(text);
    }

    /// Shows a line of the client's own, without speaking it.
    fn show_local(&mut self, text: &str) {
        self.output.push_local(text);
        let time = self.output.time(self.output.len() - 1).to_owned();
        self.logged(|log| log.local(&time, text));
    }

    /// Shows a line from the server.
    fn show_line(&mut self, line: &Line) {
        self.output.push(line);
        let time = self.output.time(self.output.len() - 1).to_owned();
        self.logged(|log| log.line(&time, line));
    }

    /// Writes to the log, if one is open; a failure closes it with a note.
    fn logged(&mut self, write: impl FnOnce(&mut SessionLog) -> std::io::Result<()>) {
        let Some(log) = &mut self.log else {
            return;
        };
        if let Err(error) = write(log) {
            self.log = None;
            self.note(&trf("Logging stopped: {0}", &[&error.to_string()]));
        }
    }

    /// Starts writing the session to a new log file.
    fn start_log(&mut self) {
        let Some(profiles) = &self.profiles else {
            return self.note(tr("Cannot log: no configuration folder was found."));
        };
        match SessionLog::start(
            &profiles.logs_dir(),
            &self.title(),
            self.settings.log_format,
        ) {
            Ok(log) => {
                let path = log.path().display().to_string();
                self.log = Some(log);
                self.note(&trf("Logging to {0}", &[&path]));
            }
            Err(error) => self.note(&trf("Cannot log: {0}", &[&error.to_string()])),
        }
    }

    fn handle_events(&mut self) {
        let Some(connection) = &self.connection else {
            return;
        };
        let events: Vec<Event> = connection.events.try_iter().collect();
        for event in events {
            match event {
                Event::Connected => {
                    self.reconnect_attempts = 0;
                    self.last_activity = Instant::now();
                    if self.settings.log_sessions && self.log.is_none() {
                        self.start_log();
                    }
                    self.note(tr("Connected."));
                    self.login = if self.character.trim().is_empty() {
                        Login::Idle
                    } else {
                        Login::Name(Instant::now())
                    };
                }
                Event::Line(line) | Event::Prompt(line) => {
                    let text = line.text();
                    // A sound request is carried out, not shown.
                    if let Some(request) = sound::parse_msp(&text) {
                        self.output.clear_partial();
                        self.pending_partial = None;
                        match request {
                            Msp::Stop => self.sound.stop_all(),
                            Msp::Play {
                                file,
                                volume,
                                looped,
                            } => {
                                let path = self.sound_file(&file);
                                // A sound the user does not have is no error.
                                let _ = self.sound.play(&path, volume, looped);
                            }
                        }
                        continue;
                    }
                    self.pending_partial = None;
                    // Text whose triggers already ran while it was
                    // unterminated keeps that verdict instead of firing twice.
                    let settled = self
                        .settled_partial
                        .take()
                        .filter(|(fired, _)| *fired == text);
                    let shown = match settled {
                        Some((_, shown)) => shown,
                        None => self.scripts.on_styled_line(&line),
                    };
                    match shown {
                        Some(shown) => {
                            self.show_line(&shown);
                            self.voice.line(&text);
                        }
                        None => {
                            self.output.clear_partial();
                            self.voice.skip_line(&text);
                        }
                    }
                    self.last_activity = Instant::now();
                    self.run_actions();
                    self.login_name();
                }
                Event::Partial(line) => {
                    let text = line.text();
                    let settled =
                        (self.settled_partial.as_ref()).filter(|(fired, _)| *fired == text);
                    match settled {
                        Some((_, None)) => continue,
                        Some((_, Some(_))) => {}
                        None => {
                            self.settled_partial = None;
                            let waiting = (self.pending_partial.as_ref())
                                .is_some_and(|(pending, _)| pending.text() == text);
                            if !waiting {
                                self.pending_partial = Some((line.clone(), Instant::now()));
                            }
                        }
                    }
                    self.output.set_partial(&line);
                    self.voice.partial(&text);
                }
                Event::InputHidden(hidden) => {
                    self.input_hidden = hidden;
                    if hidden {
                        self.login_password();
                    }
                }
                // Not spoken: the user has just typed it.
                Event::Sent(text) => self.show_local(&format!("> {text}")),
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
                    self.pending_partial = None;
                    self.settled_partial = None;
                    self.gauges.clear();
                    self.msdp_vitals.clear();
                    self.sound.stop_all();
                    self.input_hidden = false;
                    self.connection = None;
                    // A connection lost to an error is worth retrying; one
                    // the server or the user closed is not.
                    let lost = error.is_some() && !self.user_disconnected;
                    self.note(&match &error {
                        Some(error) => trf("Disconnected: {0}", &[error]),
                        None => tr("Disconnected.").to_owned(),
                    });
                    if lost && self.settings.reconnect {
                        let wait =
                            Duration::from_secs(2u64.pow(self.reconnect_attempts.min(5)).min(60));
                        self.reconnect_attempts += 1;
                        self.reconnect_at = Some(Instant::now() + wait);
                        self.note(&trf(
                            "Reconnecting in {0} seconds.",
                            &[&wait.as_secs().to_string()],
                        ));
                    }
                    self.log = None;
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
                Action::PlaySound {
                    file,
                    volume,
                    looped,
                } => {
                    let path = self.sound_file(&file);
                    if let Err(error) = self.sound.play(&path, volume, looped) {
                        self.note(&trf("Cannot play the sound: {0}", &[&error]));
                    }
                }
                Action::StopSounds => self.sound.stop_all(),
                Action::Error(message) => {
                    let message = trf("Script error: {0}", &[&message]);
                    message.lines().for_each(|line| self.note(line));
                }
            }
        }
    }

    fn send(&mut self, line: String) {
        match &self.connection {
            Some(connection) => {
                let _ = connection.commands.send(Command::Send(line));
                self.last_activity = Instant::now();
            }
            None => self.note(tr("Not connected.")),
        }
    }

    /// Where a sound a script named is: the path given if it is one, else
    /// the first file of that name in the sound folders.
    fn sound_file(&self, name: &str) -> PathBuf {
        let given = PathBuf::from(name);
        if given.is_absolute() {
            return given;
        }
        let folders = (self.profiles.as_ref())
            .map(|profiles| profiles.sound_dirs(self.profile.as_deref()))
            .unwrap_or_default();
        let found = folders
            .iter()
            .map(|dir| dir.join(name))
            .find(|path| path.is_file());
        found.unwrap_or(given)
    }

    /// Adds a package to the profile's list, or takes it off, saves the
    /// profile and runs the scripts again.
    fn set_package(&mut self, name: &str, enable: bool) {
        let Some(profile) = self.profile.clone() else {
            return self.note(tr(
                "Packages are enabled per profile: save or choose a profile first.",
            ));
        };
        self.packages.retain(|package| package != name);
        if enable {
            self.packages.push(name.to_owned());
        }
        self.profile_name = profile;
        self.save_profile();
        self.load_scripts();
        self.note(&if enable {
            trf("Package {0} enabled.", &[name])
        } else {
            trf("Package {0} disabled.", &[name])
        });
    }

    /// Reports package installs and updates that have ended.
    fn check_packages(&mut self, ctx: &egui::Context) {
        let mut ended = Vec::new();
        self.package_jobs
            .retain(|(job, enable)| match job.try_recv() {
                Ok(outcome) => {
                    ended.push((outcome, *enable));
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
            });
        if !self.package_jobs.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        for (outcome, enable) in ended {
            match outcome {
                Ok(name) if enable => {
                    self.note(&trf("Package {0} installed.", &[&name]));
                    self.set_package(&name, true);
                }
                Ok(name) => {
                    self.note(&trf("Package {0} updated.", &[&name]));
                    self.load_scripts();
                }
                Err(error) => self.note(&trf(
                    "The package could not be installed or updated: {0}",
                    &[&error],
                )),
            }
        }
    }

    /// Reconnects, keeps the connection alive and keeps looped sounds
    /// going, each when its time comes.
    fn check_timers(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if let Some(at) = self.reconnect_at {
            if at <= now {
                self.reconnect_at = None;
                self.connect(ctx);
            } else {
                ctx.request_repaint_after(at - now);
            }
        }
        let keepalive = Duration::from_secs(self.settings.keepalive_seconds.into());
        if let (Some(connection), false) = (&self.connection, keepalive.is_zero()) {
            let idle = now.saturating_duration_since(self.last_activity);
            if idle >= keepalive {
                let _ = connection.commands.send(Command::KeepAlive);
                self.last_activity = now;
                ctx.request_repaint_after(keepalive);
            } else {
                ctx.request_repaint_after(keepalive - idle);
            }
        }
        self.sound.poll();
        if self.sound.has_loops() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    /// Runs the binding of any key pressed with Ctrl or Alt, or of a
    /// function key, before anything else gets to use the key.
    fn bound_keys(&mut self, ui: &mut egui::Ui) {
        let pressed: Vec<(Key, Modifiers)> = ui.input(|input| {
            let keys = input.events.iter().filter_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            });
            keys.collect()
        });
        // Keypad keys come from the window layer, as egui cannot tell them
        // from the main block's. A bound one must not also type its digit.
        let held = ui.input(|input| input.modifiers);
        for name in std::mem::take(&mut self.keypad_pressed) {
            let mut full = String::new();
            for (down, modifier) in [
                (held.ctrl, "ctrl+"),
                (held.alt, "alt+"),
                (held.shift, "shift+"),
            ] {
                if down {
                    full.push_str(modifier);
                }
            }
            full.push_str(name);
            if self.scripts.on_key(&full) {
                ui.input_mut(|input| {
                    input
                        .events
                        .retain(|event| !keypad::is_event_of(name, event))
                });
                self.run_actions();
            }
        }
        for (key, modifiers) in pressed {
            let name = key.name().to_lowercase();
            let is_function_key =
                name.len() > 1 && name.starts_with('f') && name[1..].parse::<u8>().is_ok();
            if !(modifiers.ctrl || modifiers.alt || is_function_key) {
                continue;
            }
            let mut full = String::new();
            for (held, modifier) in [
                (modifiers.ctrl, "ctrl+"),
                (modifiers.alt, "alt+"),
                (modifiers.shift, "shift+"),
            ] {
                if held {
                    full.push_str(modifier);
                }
            }
            full.push_str(&name);
            if self.scripts.on_key(&full) {
                ui.input_mut(|input| input.consume_key(modifiers, key));
                self.run_actions();
            }
        }
    }

    /// The window for editing triggers, aliases and keys without a script.
    fn rules_window(&mut self, ctx: &egui::Context) {
        if !self.rules_open {
            return;
        }
        let theme = theme::current();
        let (mut open, mut save) = (true, false);
        egui::Window::new(tr("Triggers, aliases and keys"))
            .open(&mut open)
            .collapsible(false)
            .default_size([780.0, 460.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (section, name) in [tr("Triggers"), tr("Aliases"), tr("Keys")]
                        .into_iter()
                        .enumerate()
                    {
                        ui.selectable_value(&mut self.rules_section, section, name);
                    }
                });
                ui.separator();
                let draft = &mut self.rules_draft;
                egui::ScrollArea::vertical()
                    .max_height(340.0)
                    .auto_shrink(false)
                    .show(ui, |ui| match self.rules_section {
                        0 => edit_rules(ui, &mut draft.triggers, trigger_editor),
                        1 => edit_rules(ui, &mut draft.aliases, alias_editor),
                        _ => edit_rules(ui, &mut draft.keys, key_editor),
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr("Add")).clicked() {
                        match self.rules_section {
                            0 => draft.triggers.push(Trigger {
                                enabled: true,
                                ..Trigger::default()
                            }),
                            1 => draft.aliases.push(Alias {
                                enabled: true,
                                ..Alias::default()
                            }),
                            _ => draft.keys.push(KeyBinding {
                                enabled: true,
                                ..KeyBinding::default()
                            }),
                        }
                    }
                    let changed = *draft != self.rules;
                    save = ui
                        .add_enabled(changed, egui::Button::new(tr("Save")))
                        .clicked();
                    if changed {
                        ui.label(RichText::new(tr("Unsaved changes")).color(theme.text_dim));
                    }
                });
            });
        self.rules_open = open;
        if save {
            self.save_rules();
        }
    }

    /// Hands new text to the screen reader: a label too small to see, marked
    /// as a live region, which screen readers announce when it changes, by
    /// voice and in braille.
    fn announce(&mut self, ui: &mut egui::Ui) {
        if !self.voice.announcing() {
            return;
        }
        let lines = self.voice.take_announcements();
        if !lines.is_empty() {
            let mut text = lines.join("\n");
            // The same text twice running would not count as a change.
            if text == self.announcement {
                text.push('\u{200b}');
            }
            self.announcement = text;
        }
        let label = RichText::new(&self.announcement)
            .size(1.0)
            .color(egui::Color32::TRANSPARENT);
        let response = ui.add(Label::new(label).selectable(false));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_live(egui::accesskit::Live::Polite);
        });
    }

    /// The bar for searching the scrollback, shown above the input line.
    fn find_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let label = ui.label(tr("Find"));
            let field = ui
                .add(TextEdit::singleline(&mut self.find_query).desired_width(260.0))
                .labelled_by(label.id);
            if std::mem::take(&mut self.find_wants_focus) {
                field.request_focus();
            }
            let enter = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
            let forward = enter && ui.input(|input| input.modifiers.shift);
            let previous = ui.button(tr("Previous")).clicked() || (enter && !forward);
            let next = ui.button(tr("Next")).clicked() || forward;
            if previous || next {
                let found = self.output.find(&self.find_query, next);
                match found {
                    Some(row) => {
                        let text = self.output.text(row).trim().to_owned();
                        self.voice.stop();
                        self.voice.say(&text);
                    }
                    None => self.voice.say(tr("Not found.")),
                }
                if enter {
                    field.request_focus();
                }
            }
            let escape = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
            if ui.button(tr("Close")).clicked() || escape {
                self.find_open = false;
                self.output.end_find();
            }
        });
    }

    /// The server reported the room the player is in.
    fn entered_room(&mut self, json: &str) {
        let Some(info) = RoomInfo::from_json(json) else {
            return;
        };
        let room = info.id;
        let saved = self.map.borrow_mut().visit(info);
        if let Err(error) = saved {
            self.note(&trf("Could not save the map: {0}", &[&error.to_string()]));
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
                return self.note(tr("The map does not know where you are yet."));
            };
            map.route(from, room).map(|route| (from, route))
        };
        let Some((from, route)) = route else {
            return self.note(tr("No known route to that room."));
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
                self.note(tr("Arrived."));
            }
            Progress::Lost => {
                self.walk = None;
                self.note(tr("Walk stopped: this is not the room the route expected."));
            }
        }
    }

    /// Runs the triggers on unterminated text that has stopped changing,
    /// which is how a prompt looks when the server does not mark it.
    fn check_partial(&mut self, ctx: &egui::Context) {
        let Some((_, since)) = &self.pending_partial else {
            return;
        };
        let waited = since.elapsed();
        if waited < PARTIAL_SETTLE {
            return ctx.request_repaint_after(PARTIAL_SETTLE - waited);
        }
        let Some((line, _)) = self.pending_partial.take() else {
            return;
        };
        let text = line.text();
        let shown = self.scripts.on_styled_line(&line);
        match &shown {
            Some(shown) => self.output.set_partial(shown),
            None => {
                self.output.clear_partial();
                self.voice.skip_partial(&text);
            }
        }
        self.settled_partial = Some((text, shown));
        self.run_actions();
        self.login_name();
    }

    /// Answers the first prompt after connecting with the profile's
    /// character name, if it has one.
    fn login_name(&mut self) {
        let Login::Name(since) = self.login else {
            return;
        };
        if since.elapsed() > LOGIN_TIMEOUT {
            self.login = Login::Idle;
            return;
        }
        // Only a prompt counts: text the server left unterminated.
        if self.settled_partial.is_none() {
            return;
        }
        self.login = Login::Password(Instant::now());
        self.send(self.character.trim().to_owned());
    }

    /// Answers a hidden prompt after the name with the password kept in
    /// the keyring for the profile.
    fn login_password(&mut self) {
        let Login::Password(since) = self.login else {
            return;
        };
        self.login = Login::Idle;
        let (true, Some(profile)) = (since.elapsed() <= LOGIN_TIMEOUT, self.profile.clone()) else {
            return;
        };
        match secrets::password(&profile) {
            Ok(Some(password)) => self.send(password),
            Ok(None) => {}
            Err(error) => self.note(&trf(
                "The password could not be read from the keyring: {0}",
                &[&error],
            )),
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
            self.note(tr("Walk stopped: the last move had no effect."));
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
                    trf(
                        "{0} (#{1}) in {2}. Exits: {3}.",
                        &[
                            &room.name,
                            &room.id.to_string(),
                            &room.area,
                            &exits.join(", "),
                        ],
                    )
                });
                match place {
                    Some(place) => self.note(&place),
                    None => self.note(tr("The map does not know where you are yet.")),
                }
            }
            "/find" if !argument.is_empty() => {
                let rooms = self.matching_rooms(argument);
                if rooms.is_empty() {
                    self.note(tr("No room matches."));
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
                        [] => self.note(tr("No room matches.")),
                        [room] => self.walk_to(room),
                        _ => {
                            self.note(tr("Several rooms match; use /go with a room number:"));
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
                    Some((Ok(_), true, name)) => {
                        self.note(&trf("Routes will avoid {0}.", &[&name.to_string()]))
                    }
                    Some((Ok(_), false, name)) => {
                        self.note(&trf(
                            "Routes may pass through {0} again.",
                            &[&name.to_string()],
                        ));
                    }
                    Some((Err(error), ..)) => {
                        self.note(&trf("Could not save the map: {0}", &[&error.to_string()]))
                    }
                    None => self.note(tr("No such room; use /avoid with a room number.")),
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
                    Some((Ok(_), true)) => self.note(tr("Routes will not use that exit.")),
                    Some((Ok(_), false)) => self.note(tr("Routes may use that exit again.")),
                    Some((Err(error), _)) => {
                        self.note(&trf("Could not save the map: {0}", &[&error.to_string()]))
                    }
                    None => self.note(tr("This room has no such exit.")),
                }
            }
            // Draws the current area again from scratch.
            "/tidy" => {
                let area = self.map.borrow().current().map(|room| room.area.clone());
                match area {
                    Some(area) => self.tidy(&area),
                    None => self.note(tr("The map does not know where you are yet.")),
                }
            }
            "/packages" => {
                let installed = self
                    .profiles
                    .as_ref()
                    .map(Profiles::packages)
                    .unwrap_or_default();
                if installed.is_empty() {
                    self.note(tr(
                        "No script packages are installed. Use /install with a git address.",
                    ));
                }
                for package in installed {
                    let state = if self.packages.contains(&package) {
                        tr("enabled in this profile")
                    } else {
                        tr("not enabled; use /enable")
                    };
                    self.note(&format!("{package}: {state}"));
                }
            }
            // Downloads a package from a git repository and enables it.
            "/install" if !argument.is_empty() => {
                let (address, given) = argument.split_once(' ').unwrap_or((argument, ""));
                let name = match given.trim() {
                    "" => packages::name_from_address(address),
                    given => crate::profile::valid_name(given).then(|| given.to_owned()),
                };
                let folder = self.profiles.as_ref().map(Profiles::packages_dir);
                match (folder, name) {
                    (Some(folder), Some(name)) => {
                        self.note(&trf("Installing {0}...", &[&name]));
                        let job = packages::install(&folder, &name, address);
                        self.package_jobs.push((job, true));
                    }
                    (None, _) => self.note(tr(
                        "Packages need a configuration folder, and none was found.",
                    )),
                    (_, None) => self.note(tr(
                        "Give the package a name after the address: letters, digits, - and _.",
                    )),
                }
            }
            "/update" if !argument.is_empty() => {
                match self.profiles.as_ref().map(Profiles::packages_dir) {
                    Some(folder) => {
                        self.note(&trf("Updating {0}...", &[argument]));
                        let job = packages::update(&folder, argument);
                        self.package_jobs.push((job, false));
                    }
                    None => self.note(tr(
                        "Packages need a configuration folder, and none was found.",
                    )),
                }
            }
            "/enable" | "/disable" if !argument.is_empty() => {
                let enable = command == "/enable";
                let installed = self
                    .profiles
                    .as_ref()
                    .map(Profiles::packages)
                    .unwrap_or_default();
                if enable && !installed.iter().any(|package| package == argument) {
                    self.note(tr("No such package is installed; see /packages."));
                } else {
                    self.set_package(argument, enable);
                }
            }
            "/reload" => {
                self.load_scripts();
                self.note(tr("Scripts reloaded."));
            }
            // Starts or stops logging the session.
            "/log" => match self.log.take() {
                Some(log) => {
                    let path = log.path().display().to_string();
                    self.note(&trf("Logging stopped; the log is {0}", &[&path]));
                }
                None => self.start_log(),
            },
            "/stop" => {
                let stopped = self.walk.take().is_some();
                self.note(if stopped {
                    tr("Walk stopped.")
                } else {
                    tr("Not walking.")
                });
            }
            _ => return false,
        }
        true
    }

    fn tidy(&mut self, area: &str) {
        let moved = self.map.borrow_mut().relayout(area);
        match moved {
            Ok(moved) => self.note(&trf(
                "Tidied the map: {0} rooms moved.",
                &[&moved.to_string()],
            )),
            Err(error) => self.note(&trf("Could not save the map: {0}", &[&error.to_string()])),
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

    /// The session's bar: profile, connect and the panel switches. The
    /// connection's details live in their own window.
    fn connection_bar(&mut self, ui: &mut egui::Ui) {
        let connected = self.connection.is_some();
        let theme = theme::current();
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!connected, |ui| {
                let names = self
                    .profiles
                    .as_ref()
                    .map(Profiles::names)
                    .unwrap_or_default();
                let mut chosen = self.profile.clone();
                let label = ui.label(RichText::new(tr("Profile")).color(theme.text_dim));
                ComboBox::from_id_salt("profile")
                    .selected_text(chosen.as_deref().unwrap_or(tr("(none)")))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut chosen, None, tr("(none)"));
                        for name in names {
                            ui.selectable_value(&mut chosen, Some(name.clone()), name);
                        }
                    })
                    .response
                    .labelled_by(label.id);
                if chosen != self.profile {
                    self.select_profile(chosen);
                }
            });
            if connected {
                if ui.button(tr("Disconnect")).clicked() {
                    self.disconnect();
                }
            } else {
                if ui.button(tr("Connect")).clicked() {
                    self.connect(ui.ctx());
                }
                if ui.button(tr("Connection")).clicked() {
                    self.connection_open = !self.connection_open;
                }
            }
            let host = self.host.trim();
            if !host.is_empty() {
                let address = format!("{host}:{}", self.port.trim());
                ui.label(RichText::new(address).color(theme.text_dim));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut speech = self.voice.enabled();
                if ui.checkbox(&mut speech, tr("Speech")).changed() {
                    self.voice.set_enabled(speech);
                }
                ui.checkbox(&mut self.show_map, tr("Map"));
                ui.checkbox(&mut self.show_chat, tr("Chat"));
                if ui.button(tr("Triggers")).clicked() {
                    self.rules_draft = self.rules.clone();
                    self.rules_open = true;
                }
            });
        });
    }

    /// The window with the connection's details: where to connect, how,
    /// who to log in as, and the profile to keep it all in.
    fn connection_window(&mut self, ctx: &egui::Context) {
        if !self.connection_open || self.connection.is_some() {
            return;
        }
        let (mut open, mut connect, mut save) = (true, false, false);
        egui::Window::new(tr("Connection"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -40.0])
            .show(ctx, |ui| {
                egui::Grid::new("connection").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                    let label = ui.label(tr("Host"));
                    ui.add(TextEdit::singleline(&mut self.host).desired_width(260.0))
                        .labelled_by(label.id);
                    ui.end_row();

                    let label = ui.label(tr("Port"));
                    ui.add(TextEdit::singleline(&mut self.port).desired_width(70.0))
                        .labelled_by(label.id);
                    ui.end_row();

                    ui.label(tr("Security"));
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.tls, "TLS");
                        ui.add_enabled(self.tls, egui::Checkbox::new(&mut self.verify_certificate, tr("Verify certificate")));
                    });
                    ui.end_row();

                    let label = ui.label(tr("Encoding"));
                    ui.add(TextEdit::singleline(&mut self.encoding).hint_text("auto").desired_width(110.0))
                        .on_hover_text(tr("Only for servers that do not use UTF-8 or Latin-1, e.g. big5 or koi8-r."))
                        .labelled_by(label.id);
                    ui.end_row();

                    let label = ui.label(tr("Character"));
                    ui.add(TextEdit::singleline(&mut self.character).desired_width(180.0))
                        .labelled_by(label.id);
                    ui.end_row();

                    let label = ui.label(tr("Password"));
                    ui.add(
                        TextEdit::singleline(&mut self.new_password)
                            .password(true)
                            .hint_text(tr("unchanged"))
                            .desired_width(180.0),
                    )
                    .on_hover_text(tr("Kept in the system's keyring when the profile is saved."))
                    .labelled_by(label.id);
                    ui.end_row();

                    let label = ui.label(tr("Profile name"));
                    ui.horizontal(|ui| {
                        ui.add(TextEdit::singleline(&mut self.profile_name).desired_width(180.0))
                            .labelled_by(label.id);
                        save = ui.button(tr("Save profile")).clicked();
                    });
                    ui.end_row();
                });
                ui.add_space(6.0);
                // A row of its own, so the layout does not stretch down.
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let accent = theme::current().accent;
                        let button = egui::Button::new(RichText::new(tr("Connect")).strong())
                            .stroke(egui::Stroke::new(1.0, accent));
                        connect = ui.add(button).clicked();
                    });
                });
            });
        self.connection_open = open;
        if save {
            self.save_profile();
        }
        if connect {
            self.connection_open = false;
            self.connect(ctx);
        }
    }

    fn input_line(&mut self, ui: &mut egui::Ui) {
        let id = ui.make_persistent_id("command input");
        // Tab completes the word being typed. With nothing typed it is
        // left alone, to move between controls as usual.
        let focused = ui.memory(|memory| memory.has_focus(id));
        let completing = focused
            && !self.input.is_empty()
            && !self.input_hidden
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Tab));
        if completing {
            // Most recent first: the latest output, then what was typed.
            let rows = (0..self.output.len()).rev().take(200);
            let seen: Vec<&str> = rows
                .flat_map(|row| complete::words(self.output.text(row)))
                .collect();
            let typed = self
                .history
                .entries()
                .iter()
                .rev()
                .flat_map(|entry| complete::words(entry));
            self.completion
                .complete(&mut self.input, seen.into_iter().chain(typed));
        }
        let has_text = !self.input.is_empty();
        let mut output = TextEdit::singleline(&mut self.input)
            .id(id)
            .font(palette::font())
            .password(self.input_hidden)
            // Keeps Tab from moving to the next control while there is a
            // word to complete.
            .lock_focus(has_text)
            .hint_text(tr("Type a command"))
            .desired_width(f32::INFINITY)
            .show(ui);
        if completing {
            // Put the cursor after the completed word.
            let end = egui::text::CCursor::new(self.input.chars().count());
            output
                .state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(end)));
            output.state.store(ui.ctx(), id);
        }
        let response = output.response;
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label(tr("Command"));
        });
        if response.has_focus() {
            let (up, down, search, find) = ui.input_mut(|input| {
                (
                    input.consume_key(Modifiers::NONE, Key::ArrowUp),
                    input.consume_key(Modifiers::NONE, Key::ArrowDown),
                    input.consume_key(Modifiers::CTRL, Key::R),
                    input.consume_key(Modifiers::CTRL, Key::F),
                )
            });
            if up {
                self.history.older(&mut self.input);
            } else if down {
                self.history.newer(&mut self.input);
            } else if search {
                self.history.search(&mut self.input);
            }
            if find {
                self.find_open = true;
                self.find_wants_focus = true;
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

impl Tab {
    /// Runs the session for a frame, on screen or not: events from the
    /// server, timers, and anything waiting on the clock.
    pub fn poll(&mut self, ctx: &egui::Context, _settings: &Settings, on_screen: bool) {
        let lines_before = self.output.total_pushed();
        self.handle_events();
        let now = Instant::now();
        self.scripts.tick(now);
        self.run_actions();
        self.check_walk(ctx);
        self.check_partial(ctx);
        self.check_timers(ctx);
        self.check_packages(ctx);
        if let Some(deadline) = self.scripts.next_deadline() {
            ctx.request_repaint_after(deadline.saturating_duration_since(now));
        }
        self.unread = !on_screen && (self.unread || self.output.total_pushed() != lines_before);
    }

    /// Draws the session in `ui`.
    pub fn show(&mut self, ui: &mut egui::Ui, _settings: &Settings) {
        self.bound_keys(ui);
        self.rules_window(ui.ctx());
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
        Panel::top("connection").show(ui, |ui| self.connection_bar(ui));
        if self.show_chat && self.chat.len() > 0 {
            Panel::top("chat")
                .resizable(true)
                .default_size(84.0)
                .min_size(48.0)
                .show(ui, |ui| {
                    self.chat.show(ui, "chat");
                });
        }
        Panel::bottom("input").show(ui, |ui| {
            if !self.gauges.is_empty() {
                self.gauges.show(ui);
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
        let theme = theme::current();
        let frame = egui::Frame::new()
            .fill(theme.background)
            .inner_margin(egui::Margin::symmetric(10, 6));
        let clicked = CentralPanel::default()
            .frame(frame)
            .show(ui, |ui| self.output.show(ui, "output"));
        // A link the server marked as a command was clicked.
        if let Some(command) = clicked.inner {
            self.send(command);
        }
    }
}

/// A labelled text field for the rules window.
fn rule_field(ui: &mut egui::Ui, label: &'static str, value: &mut String, width: f32) {
    let label = ui.label(RichText::new(tr(label)).color(theme::current().text_dim));
    ui.add(TextEdit::singleline(value).desired_width(width))
        .labelled_by(label.id);
}

/// Lays out one rule's fields; the last argument draws its delete button.
type RuleEditor<T> = fn(&mut egui::Ui, &mut T, &mut dyn FnMut(&mut egui::Ui));

/// Lists `rules` for editing, each in its own frame. An editor lays out a
/// rule's fields, and puts `delete` where the rule's delete button goes.
fn edit_rules<T>(ui: &mut egui::Ui, rules: &mut Vec<T>, editor: RuleEditor<T>) {
    if rules.is_empty() {
        ui.label(
            RichText::new(tr("Nothing here yet. Use Add to make one."))
                .color(theme::current().text_dim),
        );
    }
    let mut delete = None;
    for (index, rule) in rules.iter_mut().enumerate() {
        ui.push_id(index, |ui| {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                editor(ui, rule, &mut |ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr("Delete")).clicked() {
                            delete = Some(index);
                        }
                    });
                });
            });
        });
    }
    if let Some(index) = delete {
        rules.remove(index);
    }
}

fn trigger_editor(ui: &mut egui::Ui, trigger: &mut Trigger, delete: &mut dyn FnMut(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut trigger.enabled, tr("On"));
        rule_field(ui, "When a line has", &mut trigger.pattern, 280.0);
        ui.checkbox(&mut trigger.regex, tr("Regular expression"));
        delete(ui);
    });
    ui.horizontal(|ui| {
        rule_field(ui, "Send", &mut trigger.send, 170.0);
        rule_field(ui, "Say", &mut trigger.speak, 120.0);
        rule_field(ui, "Play", &mut trigger.sound, 90.0);
        rule_field(ui, "Colour", &mut trigger.highlight, 64.0);
        ui.checkbox(&mut trigger.gag, tr("Hide the line"));
    });
}

fn alias_editor(ui: &mut egui::Ui, alias: &mut Alias, delete: &mut dyn FnMut(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut alias.enabled, tr("On"));
        rule_field(ui, "Typing", &mut alias.pattern, 150.0);
        ui.checkbox(&mut alias.regex, tr("Regular expression"));
        rule_field(ui, "Sends", &mut alias.send, 260.0);
        delete(ui);
    });
}

fn key_editor(ui: &mut egui::Ui, key: &mut KeyBinding, delete: &mut dyn FnMut(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut key.enabled, tr("On"));
        let label = ui.label(RichText::new(tr("Key")).color(theme::current().text_dim));
        ui.add(
            TextEdit::singleline(&mut key.key)
                .hint_text("f5, ctrl+k")
                .desired_width(110.0),
        )
        .labelled_by(label.id);
        rule_field(ui, "Sends", &mut key.send, 300.0);
        delete(ui);
    });
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};
    use muddie_mapper::Pos;

    use super::*;
    use crate::app::App;

    /// A window that is not drawn, using `profiles` as its configuration.
    fn window<'a>(profiles: Option<Profiles>) -> Harness<'a, App> {
        // Tests read the interface's text, so it must not follow the
        // language of whatever machine they run on.
        let english = |cc: &eframe::CreationContext<'_>, startup| {
            let app = App::new(cc, startup, Default::default());
            crate::i18n::set_language(crate::settings::Language::English);
            app
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1100.0, 700.0))
            .build_eframe(|cc| {
                let startup = Startup {
                    profiles,
                    profile: None,
                    target: None,
                    speech: false,
                    scripts: Vec::new(),
                };
                english(cc, startup)
            });
        // A new session opens asking where to connect; most tests are
        // about something else and want that window out of the way.
        harness.state_mut().tab_mut().connection_open = false;
        harness.run_steps(2);
        harness
    }

    fn output_lines(app: &Tab) -> Vec<String> {
        (0..app.output.len())
            .map(|row| app.output.text(row).to_owned())
            .collect()
    }

    /// Puts the window in the state of having a live connection, and
    /// returns the two ends a real session would hold.
    fn fake_connection(
        app: &mut Tab,
    ) -> (
        std::sync::mpsc::Sender<Event>,
        tokio::sync::mpsc::UnboundedReceiver<Command>,
    ) {
        let (events, receiver) = channel();
        let (commands, sent) = tokio::sync::mpsc::unbounded_channel();
        app.connection = Some(Connection {
            commands,
            events: receiver,
        });
        (events, sent)
    }

    #[test]
    fn profile_is_saved_from_the_connection_window_and_brings_its_scripts() {
        let dir = std::env::temp_dir().join(format!("muddie-app-profile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut harness = window(Some(Profiles::at(dir.clone())));
        let app = harness.state_mut().tab_mut();
        app.host = "mud.example.org".into();
        app.port = "4000".into();
        app.tls = true;
        app.verify_certificate = false;
        app.profile_name = "Reinos".into();
        app.connection_open = true;
        harness.run_steps(4);
        harness.get_by_label("Save profile").click();
        harness.run_steps(2);
        let saved = Profiles::at(dir.clone()).load("Reinos").unwrap();
        assert_eq!((saved.host.as_str(), saved.port), ("mud.example.org", 4000));
        assert!(saved.tls && !saved.verify_certificate);
        assert_eq!(harness.state().tab().profile.as_deref(), Some("Reinos"));

        // Scripts dropped in the profile's folder run when it is selected.
        std::fs::write(
            dir.join("profiles/Reinos/scripts/hello.luau"),
            "echo('hola desde el perfil')",
        )
        .unwrap();
        let app = harness.state_mut().tab_mut();
        assert!(app.select_profile(None));
        app.host.clear();
        assert!(app.select_profile(Some("Reinos".into())));
        assert_eq!(app.host, "mud.example.org");
        assert!(output_lines(app).contains(&"hola desde el perfil".to_owned()));
        assert!(!app.select_profile(Some("Missing".into())));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn typed_commands_are_split_and_reach_the_server() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        let (_events, mut sent) = fake_connection(app);
        app.input = "n;n;look".into();
        app.submit();
        let commands: Vec<Command> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
        let expected = ["n", "n", "look"].map(|command| Command::Send(command.into()));
        assert_eq!(commands, expected);
    }

    #[test]
    fn unmarked_prompt_fires_triggers_once_it_settles() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        app.scripts.exec(
            "test",
            r#"trigger("^Name: $", function() send("gandalf") end)
            trigger("^Spam> $", function() gag() end)"#,
        );
        let (events, mut sent) = fake_connection(app);
        let partial = |text: &str| Event::Partial(panels::styled_lines(text).remove(0));
        events.send(partial("Name: ")).unwrap();
        harness.run_steps(2);
        assert!(sent.try_recv().is_err(), "must wait for the text to settle");
        std::thread::sleep(PARTIAL_SETTLE + Duration::from_millis(50));
        harness.run_steps(2);
        assert_eq!(sent.try_recv().unwrap(), Command::Send("gandalf".into()));
        // The server then closes the same text as a line: no second firing.
        events
            .send(Event::Prompt(panels::styled_lines("Name: ").remove(0)))
            .unwrap();
        harness.run_steps(2);
        assert!(sent.try_recv().is_err());
        assert_eq!(output_lines(harness.state().tab()), ["Name: "]);

        // A gagged prompt disappears and stays hidden when completed.
        events.send(partial("Spam> ")).unwrap();
        harness.run_steps(2);
        std::thread::sleep(PARTIAL_SETTLE + Duration::from_millis(50));
        harness.run_steps(2);
        events
            .send(Event::Prompt(panels::styled_lines("Spam> ").remove(0)))
            .unwrap();
        harness.run_steps(2);
        assert_eq!(output_lines(harness.state().tab()), ["Name: "]);
    }

    #[test]
    fn lost_connection_is_retried_but_a_closed_one_is_not() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        app.host = "127.0.0.1".into();
        let (events, _sent) = fake_connection(app);
        events
            .send(Event::Disconnected(Some("connection reset".into())))
            .unwrap();
        harness.run_steps(2);
        let app = harness.state_mut().tab_mut();
        assert!(app.reconnect_at.is_some());
        assert!(
            output_lines(app)
                .iter()
                .any(|line| line.starts_with("Reconnecting in 1 "))
        );

        // Closed by the server without an error: nothing to retry.
        app.reconnect_at = None;
        let (events, _sent) = fake_connection(app);
        events.send(Event::Disconnected(None)).unwrap();
        harness.run_steps(2);
        assert!(harness.state().tab().reconnect_at.is_none());

        // Asked for by the user: not retried even if it ends in an error.
        let app = harness.state_mut().tab_mut();
        let (events, _sent) = fake_connection(app);
        app.disconnect();
        events
            .send(Event::Disconnected(Some("broken pipe".into())))
            .unwrap();
        harness.run_steps(2);
        assert!(harness.state().tab().reconnect_at.is_none());
    }

    #[test]
    fn idle_connection_gets_a_keep_alive() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        app.settings.keepalive_seconds = 1;
        let (_events, mut sent) = fake_connection(app);
        app.last_activity = Instant::now() - Duration::from_secs(2);
        harness.run_steps(2);
        assert_eq!(sent.try_recv().unwrap(), Command::KeepAlive);
        assert!(sent.try_recv().is_err(), "only one until idle again");
    }

    #[test]
    fn bound_function_key_sends_its_command() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        app.scripts
            .exec("test", r#"bind("f5", "look") bind("ctrl+k", "kick")"#);
        let (_events, mut sent) = fake_connection(app);
        harness.key_press(Key::F5);
        harness.run_steps(2);
        harness.key_press_modifiers(Modifiers::CTRL, Key::K);
        harness.run_steps(2);
        let commands: Vec<Command> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
        assert_eq!(
            commands,
            [Command::Send("look".into()), Command::Send("kick".into())]
        );
    }

    #[test]
    fn bound_keypad_key_sends_its_command_and_types_nothing() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        app.scripts.exec("test", r#"bind("numpad8", "north")"#);
        let (_events, mut sent) = fake_connection(app);
        // What the window layer and egui each make of one press of keypad 8.
        app.keypad_pressed = vec!["numpad8", "numpad2"];
        harness.event(egui::Event::Text("8".into()));
        harness.event(egui::Event::Text("2".into()));
        harness.run_steps(2);
        assert_eq!(sent.try_recv().unwrap(), Command::Send("north".into()));
        // The bound key typed nothing; the unbound one typed its digit.
        assert_eq!(harness.state().tab().input, "2");
    }

    #[test]
    fn lines_are_announced_through_a_live_region_when_asked() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        app.voice.set_announce(true);
        app.voice.set_enabled(true);
        let (events, _sent) = fake_connection(app);
        // Let the notice that speech is on be announced first.
        harness.run_steps(2);
        assert_eq!(harness.state().tab().announcement, "Speech on.");
        events
            .send(Event::Line(
                panels::styled_lines("An orc arrives.").remove(0),
            ))
            .unwrap();
        harness.run_steps(2);
        // The line is in the scrollback too; the announcement is the node
        // marked as a live region.
        let announced = (harness.query_all_by_label("An orc arrives."))
            .filter(|node| node.accesskit_node().live() == egui::accesskit::Live::Polite)
            .count();
        assert_eq!(announced, 1);
        // The same line again still counts as a change to announce.
        events
            .send(Event::Line(
                panels::styled_lines("An orc arrives.").remove(0),
            ))
            .unwrap();
        harness.run_steps(2);
        assert_eq!(
            harness.state().tab().announcement,
            "An orc arrives.\u{200b}"
        );
        // Speaking directly instead leaves nothing to announce.
        harness.state_mut().tab_mut().voice.set_announce(false);
        harness.run_steps(2);
        assert!(!harness.state().tab().voice.announcing());
    }

    #[test]
    fn tab_completes_from_recent_output() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        app.show_local("A goblin sharpens a golden axe.");
        app.input = "kill go".into();
        harness.run_steps(2);
        harness.key_press(Key::Tab);
        harness.run_steps(2);
        assert_eq!(harness.state().tab().input, "kill goblin");
        harness.key_press(Key::Tab);
        harness.run_steps(2);
        assert_eq!(harness.state().tab().input, "kill golden");
    }

    #[test]
    fn session_is_logged_when_asked() {
        let dir = std::env::temp_dir().join(format!("muddie-app-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut harness = window(Some(Profiles::at(dir.clone())));
        let app = harness.state_mut().tab_mut();
        app.host = "mud.example.org".into();
        assert!(app.map_command("/log"));
        let (events, _sent) = fake_connection(app);
        events
            .send(Event::Line(
                panels::styled_lines("\x1b[31mAn orc\x1b[0m arrives").remove(0),
            ))
            .unwrap();
        harness.run_steps(2);
        let app = harness.state_mut().tab_mut();
        let path = app.log.as_ref().unwrap().path().to_owned();
        assert!(app.map_command("/log"));
        assert!(app.log.is_none());
        let log = std::fs::read_to_string(path).unwrap();
        assert!(log.contains("An orc arrives"), "{log}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rules_made_in_the_window_are_saved_and_take_effect() {
        let dir = std::env::temp_dir().join(format!("muddie-app-rules-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut harness = window(Some(Profiles::at(dir.clone())));
        harness.get_by_label("Triggers").click();
        harness.run_steps(4);
        assert!(
            harness.state().tab().rules_open,
            "the button opens the window"
        );
        harness.get_by_label("Add").click();
        harness.run_steps(2);
        let app = harness.state_mut().tab_mut();
        app.rules_draft.triggers[0].pattern = "you are hungry".into();
        app.rules_draft.triggers[0].send = "eat bread".into();
        harness.run_steps(2);
        harness.get_by_label("Save").click();
        harness.run_steps(2);
        assert_eq!(
            Rules::load(&dir.join("rules.toml")).unwrap().triggers.len(),
            1
        );

        let app = harness.state_mut().tab_mut();
        let (events, mut sent) = fake_connection(app);
        events
            .send(Event::Line(
                panels::styled_lines("Suddenly you are hungry.").remove(0),
            ))
            .unwrap();
        harness.run_steps(2);
        assert_eq!(sent.try_recv().unwrap(), Command::Send("eat bread".into()));
        // Reloading scripts keeps the rules working.
        assert!(harness.state_mut().tab_mut().map_command("/reload"));
        events
            .send(Event::Line(
                panels::styled_lines("Again you are hungry.").remove(0),
            ))
            .unwrap();
        harness.run_steps(2);
        assert_eq!(sent.try_recv().unwrap(), Command::Send("eat bread".into()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sound_requests_from_the_server_are_not_shown() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        let (events, _sent) = fake_connection(app);
        for text in [
            "before",
            "!!SOUND(missing.wav V=50)",
            "!!MUSIC(Off)",
            "after",
        ] {
            events
                .send(Event::Line(panels::styled_lines(text).remove(0)))
                .unwrap();
        }
        harness.run_steps(2);
        assert_eq!(output_lines(harness.state().tab()), ["before", "after"]);
    }

    #[test]
    fn character_name_answers_the_first_prompt() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        app.character = "Gandalf".into();
        let (events, mut sent) = fake_connection(app);
        events.send(Event::Connected).unwrap();
        events
            .send(Event::Line(
                panels::styled_lines("Welcome to the realm").remove(0),
            ))
            .unwrap();
        events
            .send(Event::Partial(
                panels::styled_lines("By what name are you known? ").remove(0),
            ))
            .unwrap();
        harness.run_steps(2);
        assert!(sent.try_recv().is_err(), "a banner line is not a prompt");
        std::thread::sleep(PARTIAL_SETTLE + Duration::from_millis(50));
        harness.run_steps(2);
        assert_eq!(sent.try_recv().unwrap(), Command::Send("Gandalf".into()));
        // Without a profile there is no password to send.
        events.send(Event::InputHidden(true)).unwrap();
        harness.run_steps(2);
        assert!(sent.try_recv().is_err());
        assert_eq!(harness.state().tab().login, Login::Idle);
    }

    /// A window with every panel in use, as after some minutes of play.
    fn demo_window<'a>() -> Harness<'a, App> {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1100.0, 700.0))
            .wgpu()
            .build_eframe(|cc| {
                let startup = Startup {
                    profiles: None,
                    profile: None,
                    target: None,
                    speech: false,
                    scripts: Vec::new(),
                };
                let app = App::new(cc, startup, Default::default());
                crate::i18n::set_language(crate::settings::Language::English);
                app
            });
        let app = harness.state_mut().tab_mut();
        app.connection_open = false;
        app.host = "mud.example.org".into();
        app.port = "4000".into();
        app.tls = true;
        app.profile_name = "Reinos".into();
        app.note("Connected.");
        let text = "\x1b[1;32mPlaza Mayor\x1b[0m\n\
            Una plaza amplia con una fuente en el centro. Al norte se alza la \x1b[33mtorre\x1b[0m \
            del reloj y al este se abre un callejón estrecho que huele a pan recién hecho.\n\
            \x1b[36mSalidas: norte, este, sur\x1b[0m\n\
            \x1b[31mUn orco te ataca\x1b[0m con su \x1b[1;35mhacha rúnica\x1b[0m.\n\
            \x1b[38;5;208mAna te dice: ¿vienes a la torre?\x1b[0m";
        for line in panels::styled_lines(text) {
            app.output.push(&line);
        }
        app.output.push_local("> matar orco");
        app.output
            .set_partial(&panels::styled_lines("HP:80 MP:20> ")[0]);
        app.gauges.set("HP", Some((80.0, 100.0)));
        app.gauges.set("MP", Some((20.0, 50.0)));
        app.gauges.set("MV", Some((140.0, 150.0)));
        for line in
            panels::styled_lines("\x1b[33m[ooc] Ana: hola a todos\x1b[0m\n[ooc] Luis: buenas")
        {
            app.chat.push(&line);
        }
        app.text_panels.push((
            "Minimapa".into(),
            panels::styled_lines("  #\n#-\x1b[1;33m@\x1b[0m-#\n  #"),
        ));
        app.input = "matar orco".into();
        {
            let mut map = app.map.borrow_mut();
            let room = |id, name: &str, area: &str, exits: &[(&str, RoomId)]| RoomInfo {
                id,
                name: name.into(),
                area: area.into(),
                exits: (exits.iter())
                    .map(|(direction, to)| ((*direction).to_owned(), *to))
                    .collect(),
                pos: None,
            };
            for info in [
                room(1, "Puerta", "ciudad", &[("n", 2)]),
                room(
                    2,
                    "Plaza Mayor",
                    "ciudad",
                    &[("s", 1), ("n", 3), ("e", 4), ("w", 9)],
                ),
                room(3, "Torre", "ciudad", &[("s", 2), ("u", 5), ("portal", 20)]),
                room(4, "Callejón", "ciudad", &[("w", 2), ("e", 6), ("ne", 7)]),
                room(6, "Panadería", "ciudad", &[("w", 4)]),
                room(7, "Patio", "ciudad", &[("sw", 4), ("n", 8)]),
                room(8, "Camino", "afueras", &[("s", 7)]),
                room(4, "Callejón", "ciudad", &[("w", 2), ("e", 6), ("ne", 7)]),
                room(
                    2,
                    "Plaza Mayor",
                    "ciudad",
                    &[("s", 1), ("n", 3), ("e", 4), ("w", 9)],
                ),
            ] {
                map.visit(info).unwrap();
            }
            map.set_avoid(6, true).unwrap();
            map.edit_exit(4, "ne", |exit| exit.locked = true).unwrap();
        }
        harness.run_steps(3);
        harness
    }

    /// The same window as it looks during play, with the connection open.
    fn connected_demo_window<'a>() -> Harness<'a, App> {
        let mut harness = demo_window();
        // Dropping the far ends is fine: nothing is sent or received here.
        fake_connection(harness.state_mut().tab_mut());
        harness.run_steps(3);
        harness
    }

    /// Renders the demo window to the PNG named by `MUDDIE_SCREENSHOT`, to
    /// look at the result. Needs a GPU or software Vulkan, so it is not
    /// part of the normal test run:
    /// `MUDDIE_SCREENSHOT=/tmp/muddie.png cargo test -p muddie -- --ignored`
    #[test]
    #[ignore = "needs a graphics adapter; writes an image for visual review"]
    fn screenshot() {
        let path = std::env::var_os("MUDDIE_SCREENSHOT").map(PathBuf::from);
        let save = |harness: &mut Harness<'_, App>, name: &str| {
            let image = harness.render().expect("rendering failed");
            if let Some(path) = &path {
                let path = path.with_extension(format!("{name}.png"));
                image.save(path).expect("could not write the screenshot");
            }
        };
        // Before connecting, the connection's details have their window.
        let mut disconnected = demo_window();
        disconnected.state_mut().tab_mut().connection_open = true;
        disconnected.run_steps(4);
        save(&mut disconnected, "disconnected");
        disconnected.state_mut().tab_mut().connection_open = false;
        disconnected.run_steps(2);
        disconnected.get_by_label("Settings").click();
        disconnected.run_steps(4);
        save(&mut disconnected, "settings");
        // The rules window, with one rule of each kind to look at.
        let mut rules = demo_window();
        let app = rules.state_mut().tab_mut();
        app.rules.triggers.push(Trigger {
            enabled: true,
            pattern: "tells you".into(),
            speak: "a tell".into(),
            highlight: "yellow".into(),
            ..Trigger::default()
        });
        app.rules.triggers.push(Trigger {
            enabled: true,
            pattern: r"^(\w+) attacks you".into(),
            regex: true,
            send: "kill %1".into(),
            sound: "alert.ogg".into(),
            ..Trigger::default()
        });
        rules.run_steps(2);
        rules.get_by_label("Triggers").click();
        rules.run_steps(4);
        save(&mut rules, "rules");
        // The find bar and the view of the latest lines while scrolled back.
        let mut find = connected_demo_window();
        let app = find.state_mut().tab_mut();
        (0..60).for_each(|n| app.show_local(&format!("line {n} of filler to scroll through")));
        app.find_open = true;
        app.find_query = "orco".into();
        find.run_steps(3);
        find.get_by_label("Previous").click();
        find.run_steps(4);
        save(&mut find, "find");
        // The other themes.
        for (theme, name) in [
            (crate::settings::ThemeName::Light, "light"),
            (crate::settings::ThemeName::Contrast, "contrast"),
        ] {
            let mut themed = connected_demo_window();
            let ctx = themed.ctx.clone();
            themed.state_mut().use_theme(&ctx, theme);
            themed.run_steps(3);
            save(&mut themed, name);
        }
        let ctx = find.ctx.clone();
        find.state_mut()
            .use_theme(&ctx, crate::settings::ThemeName::Dark);
        let mut harness = connected_demo_window();
        assert!(harness.query_by_label("Connection").is_none());
        let image = harness.render().expect("rendering failed");
        assert_eq!((image.width(), image.height()), (1100, 700));
        if let Some(path) = &path {
            image.save(path).expect("could not write the screenshot");
        }
        // A second view: every area on the level.
        harness.get_by_label("All areas").click();
        harness.run_steps(3);
        let image = harness.render().expect("rendering failed");
        if let Some(path) = &path {
            (image.save(path.with_extension("all-areas.png")))
                .expect("could not write the screenshot");
        }
        // A third: the menu of the room under the pointer. The map is
        // centred on the player's room, so that is the room it opens on.
        harness.get_by_label("Map view").click_secondary();
        harness.run_steps(3);
        assert!(harness.query_by_label("Avoid in routes").is_some());
        let image = harness.render().expect("rendering failed");
        if let Some(path) = &path {
            let path = path.with_extension("menu.png");
            image.save(path).expect("could not write the screenshot");
        }
        // Choosing to avoid the room marks it on the map.
        harness.get_by_label("Avoid in routes").click();
        harness.run_steps(3);
        assert!(harness.state().tab().map.borrow().room(2).unwrap().avoid);
        // The map placed the rooms as walked: the patio is north-east of
        // the alley, one cell east of the square.
        let map = harness.state().tab().map.borrow();
        assert_eq!(map.room(7).unwrap().pos, Pos { x: 2, y: 2, z: 0 });
    }
}
