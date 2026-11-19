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

mod automation;
mod view;

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
    /// A connection has been asked for at least once; until then the
    /// session shows the welcome screen.
    started: bool,
    /// The saved profiles with where each connects, for the welcome
    /// screen; `None` until read, and again after one is saved.
    welcome_profiles: Option<Vec<(String, String)>>,
    /// The rule selected in each section of the automation window.
    rules_selected: [usize; 5],
    /// The script files that can be edited, and the one being edited:
    /// its file, its name as typed, its text, and that text as saved.
    script_files: Vec<PathBuf>,
    script_loaded: Option<PathBuf>,
    script_name: String,
    script_text: String,
    script_saved: String,
    script_seen: String,
    script_changed_at: Option<Instant>,
    /// What the automation window's search box holds.
    rules_filter: String,
    /// The draft as last seen, and when it last changed: edits are saved
    /// once they stop for a moment.
    rules_seen: Rules,
    rules_changed_at: Option<Instant>,
    /// A sample line or command to try the selected rule on.
    rules_sample: String,
    /// The next key pressed becomes the selected macro's key.
    recording_key: bool,
    /// The widths of the automation view's first two panes, and how tall
    /// its details are.
    panes: [f32; 2],
    detail_height: f32,
    /// GMCP modules scripts asked to announce, besides the usual ones.
    supports: Vec<String>,
    /// Words scripts gave to complete with, under the name of each list.
    completions: Vec<(String, Vec<String>)>,
    /// The size in characters of the panel beside the output: as it is,
    /// as scripts were last told, and since when it has been as it is.
    panel_cells: (u32, u32),
    panel_reported: Option<(u32, u32)>,
    panel_settling: Option<((u32, u32), Instant)>,
    /// The address typed on the welcome screen.
    address: String,
    /// Whether the region for announcements was in the accessibility tree
    /// on the last frame.
    announce_ready: bool,
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
            show_chat: settings.show_chat,
            map,
            map_view: MapView::default(),
            show_map: settings.show_map,
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
            started: false,
            welcome_profiles: None,
            rules_selected: [0; 5],
            script_files: Vec::new(),
            script_loaded: None,
            script_name: String::new(),
            script_text: String::new(),
            script_saved: String::new(),
            script_seen: String::new(),
            script_changed_at: None,
            rules_filter: String::new(),
            rules_seen: Rules::default(),
            rules_changed_at: None,
            rules_sample: String::new(),
            recording_key: false,
            panes: settings.automation_panes,
            detail_height: 600.0,
            supports: Vec::new(),
            completions: Vec::new(),
            panel_cells: (0, 0),
            panel_reported: None,
            panel_settling: None,
            address: String::new(),
            announce_ready: false,
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
    pub fn settings_ui(ui: &mut egui::Ui, settings: &mut Settings, section: usize) {
        use crate::widgets::{group, setting_row as row, switch_setting as switch_row};
        if section == 1 {
            group(ui, |ui| {
                row(ui, tr("Lines kept"), &mut |ui| {
                    ui.add(
                        egui::DragValue::new(&mut settings.scrollback_lines)
                            .range(100..=1_000_000)
                            .speed(100),
                    )
                });
                switch_row(
                    ui,
                    &mut settings.timestamps,
                    tr("Show the time of each line"),
                );
            });
        }
        if section == 2 {
            group(ui, |ui| {
                switch_row(ui, &mut settings.log_sessions, tr("Log every session"));
                row(ui, tr("Log format"), &mut |ui| {
                    let name = |format: LogFormat| match format {
                        LogFormat::Text => tr("Text"),
                        LogFormat::Html => "HTML",
                    };
                    ComboBox::from_id_salt("log format")
                        .selected_text(name(settings.log_format))
                        .width(110.0)
                        .show_ui(ui, |ui| {
                            for format in [LogFormat::Text, LogFormat::Html] {
                                ui.selectable_value(&mut settings.log_format, format, name(format));
                            }
                        })
                        .response
                });
                switch_row(
                    ui,
                    &mut settings.reconnect,
                    tr("Reconnect when the connection is lost"),
                );
                row(ui, tr("Keep-alive"), &mut |ui| {
                    ui.add(
                        egui::DragValue::new(&mut settings.keepalive_seconds)
                            .range(0..=3600)
                            .suffix(" s"),
                    )
                    .on_hover_text(tr(
                        "Seconds of silence before a keep-alive is sent; 0 sends none.",
                    ))
                });
            });
        }
        if section == 3 {
            group(ui, |ui| {
                row(ui, tr("Speech rate"), &mut |ui| {
                    ui.add(egui::Slider::new(&mut settings.speech_rate, -100..=100))
                });
                row(ui, tr("Speech language"), &mut |ui| {
                    ui.add(
                    TextEdit::singleline(&mut settings.speech_language)
                        .hint_text(tr("system"))
                        .margin(egui::Margin::symmetric(9, 6))
                        .desired_width(110.0),
                )
                .on_hover_text(tr(
                    "A language code such as es or en, so the text is read with the right voice.",
                ))
                });
                switch_row(
                ui,
                &mut settings.announce_with_screen_reader,
                tr("Voice through the screen reader (also braille)"),
            )
            .on_hover_text(tr(
                "Instead of speaking directly, hand each line to the screen reader to announce.",
            ));
            });
        }
    }

    /// Starts the script runtime afresh and runs the user's scripts: the
    /// general one, the profile's packages and scripts, and those given on
    /// the command line.
    fn load_scripts(&mut self) {
        // The scripts say again what they want announced and are told
        // again how big the panel is.
        self.supports.clear();
        self.completions.clear();
        self.panel_reported = None;
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
        // Each on its own, so a mistake in one says which and spares the rest.
        for (name, code) in self.rules.chunks() {
            self.scripts.exec(&name, &code);
        }
        self.run_actions();
    }

    /// Saves the rules edited in the rules window and puts them to work.
    fn save_rules(&mut self) {
        let Some(profiles) = &self.profiles else {
            // Nowhere to keep them: they still work for this session.
            self.rules = self.rules_draft.clone();
            return self.load_scripts();
        };
        let saved = self
            .rules_draft
            .save(&profiles.rules_path(self.profile.as_deref()));
        match saved {
            Ok(()) => self.load_scripts(),
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
                self.welcome_profiles = None;
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
        self.started = true;
        self.connection_open = false;
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
        } = connect(target, {
            let mut config = Config {
                screen_reader: self.voice.enabled(),
                encoding: self.encoding.clone(),
                ..Config::default()
            };
            config.gmcp_supports.extend(self.supports.iter().cloned());
            config
        });

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
                    // The server has yet to hear how big the panel is.
                    self.panel_reported = None;
                    self.scripts.on_event("connect", "");
                    self.run_actions();
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
                    self.scripts.on_event("disconnect", "");
                    self.run_actions();
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
                Action::Gauge { name, level, look } => {
                    let look = (look != Default::default()).then_some(look);
                    self.gauges.set_with(&name, level, look);
                }
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
                Action::Supports(modules) => {
                    let new: Vec<String> = modules
                        .into_iter()
                        .filter(|module| !self.supports.contains(module))
                        .collect();
                    // Already connected: the server is told of the additions.
                    if let (Some(connection), false) = (&self.connection, new.is_empty()) {
                        let data = serde_json::to_string(&new).unwrap_or_default();
                        let package = "Core.Supports.Add".to_owned();
                        let _ = connection.commands.send(Command::Gmcp { package, data });
                    }
                    self.supports.extend(new);
                }
                Action::Completions { name, words } => {
                    // A list keeps its place when a newer one replaces it.
                    let known = self.completions.iter().position(|(list, _)| *list == name);
                    match (known, words.is_empty()) {
                        (Some(index), true) => drop(self.completions.remove(index)),
                        (Some(index), false) => self.completions[index].1 = words,
                        (None, false) => self.completions.push((name, words)),
                        (None, true) => {}
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

    /// Hands new text to the screen reader: a label too small to see, marked
    /// as a live region, which screen readers announce when it changes, by
    /// voice and in braille.
    fn announce(&mut self, ui: &mut egui::Ui) {
        if !self.voice.announcing() {
            return;
        }
        // A screen reader is only told of changes to a region it already
        // knows, so text waits until the region has been in the tree.
        let lines = if self.announce_ready {
            self.voice.take_announcements()
        } else {
            Vec::new()
        };
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
        let in_tree = ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_live(egui::accesskit::Live::Polite);
        });
        self.announce_ready = in_tree.is_some();
        if !self.announce_ready {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
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

    /// The size in characters of a panel `size` points wide and tall, in
    /// the output's typeface.
    fn cells(ui: &egui::Ui, size: egui::Vec2) -> (u32, u32) {
        let font = palette::font();
        let (wide, tall) =
            ui.fonts_mut(|fonts| (fonts.glyph_width(&font, 'm'), fonts.row_height(&font)));
        (
            (size.x / wide).floor().max(0.0) as u32,
            (size.y / tall).floor().max(0.0) as u32,
        )
    }

    /// Tells scripts how big the panel is once it has stopped changing,
    /// so a window being resized does not flood the server.
    pub(super) fn report_panel(&mut self, ctx: &egui::Context) {
        const SETTLED: Duration = Duration::from_millis(250);
        let cells = self.panel_cells;
        if self.panel_reported == Some(cells) || cells.0 == 0 {
            return self.panel_settling = None;
        }
        match self.panel_settling {
            Some((settling, since)) if settling == cells && since.elapsed() >= SETTLED => {
                self.panel_settling = None;
                self.panel_reported = Some(cells);
                let size = format!(r#"{{"columns": {}, "rows": {}}}"#, cells.0, cells.1);
                self.scripts.on_event("panel", &size);
                self.run_actions();
            }
            Some((settling, _)) if settling == cells => ctx.request_repaint_after(SETTLED),
            _ => {
                self.panel_settling = Some((cells, Instant::now()));
                ctx.request_repaint_after(SETTLED);
            }
        }
    }

    /// The right-hand panel: text panels from scripts, then the map.
    fn side_panel(&mut self, ui: &mut egui::Ui) -> Option<MapAction> {
        // What a drawing sent by the server has to fit in: the panel, less
        // its title.
        let room = ui.available_size() - egui::vec2(0.0, 22.0);
        self.panel_cells = Self::cells(ui, room);
        let has_map = self.map.borrow().current().is_some();
        let alone = self.text_panels.len() == 1 && !has_map;
        for (index, (name, lines)) in self.text_panels.iter().enumerate() {
            crate::widgets::caption(ui, name);
            // Never wrapped: this is often a drawing made of characters,
            // which is also why it is centred as one block.
            let galleys: Vec<_> = lines
                .iter()
                .map(|line| ui.painter().layout_job(server_job(line)))
                .collect();
            let widest = galleys
                .iter()
                .map(|galley| galley.size().x)
                .fold(0.0, f32::max);
            let height: f32 = galleys.iter().map(|galley| galley.size().y).sum();
            let left = ((ui.available_width() - widest) / 2.0).max(0.0);
            if alone {
                ui.add_space(((ui.available_height() - height) / 2.0).max(0.0));
            }
            ui.horizontal(|ui| {
                ui.add_space(left);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for galley in galleys {
                        ui.add(Label::new(galley).extend());
                    }
                });
            });
            if index + 1 < self.text_panels.len() || has_map {
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(2.0);
            }
        }
        let map = self.map.borrow();
        if map.current().is_some() {
            return self.map_view.show(ui, &map, self.walk.is_some());
        }
        if !self.text_panels.is_empty() {
            return None;
        }
        // Nothing to show yet: say why, and where to go from here.
        let theme = theme::current();
        crate::widgets::caption(ui, tr("Map"));
        ui.add_space((ui.available_height() * 0.28).max(12.0));
        let mut scripts = false;
        ui.vertical_centered(|ui| {
            let title = RichText::new(tr("No map yet")).font(theme::font(theme::Weight::Medium, 14.5)).color(theme.text);
            ui.add(Label::new(title).selectable(false));
            ui.add_space(4.0);
            let about = tr("It is drawn as you walk, if the MUD says which room you are in. If yours tells it another way, or draws the map itself, a script can take it from there.");
            ui.label(RichText::new(about).font(theme::font(theme::Weight::Regular, 12.5)).color(theme.text_dim));
            ui.add_space(12.0);
            scripts = ui.button(tr("Set up a mapper…")).clicked();
        });
        scripts.then_some(MapAction::Scripts)
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
            let given =
                (self.completions.iter()).flat_map(|(_, words)| words.iter().map(String::as_str));
            self.completion
                .complete(&mut self.input, seen.into_iter().chain(typed).chain(given));
        }
        let has_text = !self.input.is_empty();
        let theme = theme::current();
        let edge = if focused {
            theme.accent.gamma_multiply(0.7)
        } else {
            theme.border
        };
        let line = egui::Frame::new()
            .fill(theme.background)
            .stroke(egui::Stroke::new(1.0, edge))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::symmetric(14, 8));
        let shown = line.show(ui, |ui| {
            let height = ui.fonts_mut(|fonts| fonts.row_height(&palette::font())) + 4.0;
            let row = egui::vec2(ui.available_width(), height);
            let centred = egui::Layout::left_to_right(egui::Align::Center);
            ui.allocate_ui_with_layout(row, centred, |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (mark, _) =
                    ui.allocate_exact_size(egui::vec2(8.0, height), egui::Sense::hover());
                let edit = TextEdit::singleline(&mut self.input)
                    .id(id)
                    .frame(egui::Frame::NONE)
                    .font(palette::font())
                    .password(self.input_hidden)
                    // Keeps Tab from moving to the next control while there
                    // is a word to complete.
                    .lock_focus(has_text)
                    .hint_text(
                        // In the typeface of what will be typed, so it sits
                        // on the same line.
                        RichText::new(tr("Type a command"))
                            .font(palette::font())
                            .color(theme.text_faint),
                    )
                    .align(egui::Align2::LEFT_CENTER)
                    .desired_width(f32::INFINITY)
                    .show(ui);
                // The prompt mark is drawn, not typed, and against the text
                // itself: level with the middle of its lowercase letters.
                // The field's own middle: the same with and without text in it.
                let line = edit.response.rect.center().y;
                let x_height = ui.fonts_mut(|fonts| fonts.row_height(&palette::font())) * 0.08;
                let (centre, arm) = (egui::pos2(mark.center().x, line + x_height), 4.0);
                let chevron = vec![
                    centre + egui::vec2(-arm * 0.5, -arm),
                    centre + egui::vec2(arm * 0.5, 0.0),
                    centre + egui::vec2(-arm * 0.5, arm),
                ];
                let stroke = egui::Stroke::new(1.6, theme.accent);
                ui.painter().add(egui::Shape::line(chevron, stroke));
                edit
            })
            .inner
        });
        let mut output = shown.inner;
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
        // A new session opens on the welcome screen; most tests are about
        // a session in use.
        harness.state_mut().tab_mut().started = true;
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
    fn map_drawn_by_the_server_is_asked_for_at_the_panels_size_and_shown() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        let example = include_str!("../../../examples/scripts/drawn-map.luau");
        app.scripts.exec("drawn-map", example);
        app.run_actions();
        assert_eq!(app.supports, ["Map.Render 1"]);
        let (events, mut sent) = fake_connection(app);
        events.send(Event::Connected).unwrap();
        // The size goes out once the panel has kept it for a moment.
        harness.run_steps(2);
        assert!(sent.try_recv().is_err());
        std::thread::sleep(Duration::from_millis(300));
        harness.run_steps(2);
        let Ok(Command::Gmcp { package, data }) = sent.try_recv() else {
            panic!("the panel's size was not sent");
        };
        assert_eq!(package, "Map.Render.Size");
        let size: serde_json::Value = serde_json::from_str(&data).unwrap();
        let (width, height) = (
            size["width"].as_u64().unwrap(),
            size["height"].as_u64().unwrap(),
        );
        assert!(
            (20..80).contains(&width) && (5..=60).contains(&height),
            "{data}"
        );
        assert!(sent.try_recv().is_err(), "sent once");

        let drawing = r##"{"width": 5, "height": 3, "lines": ["  #  ", "#-@-#", "  #  "]}"##;
        let render = Event::Gmcp {
            package: "Map.Render".into(),
            data: drawing.into(),
        };
        events.send(render).unwrap();
        harness.run_steps(3);
        let app = harness.state().tab();
        assert_eq!(app.text_panels[0].0, "Map");
        assert_eq!(app.text_panels[0].1.len(), 3);
        assert!(harness.query_by_label("#-@-#").is_some());
    }

    #[test]
    fn tab_completes_with_words_the_server_gives() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        let example = include_str!("../../../examples/scripts/completions.luau");
        app.scripts.exec("completions", example);
        app.run_actions();
        let (events, _sent) = fake_connection(app);
        let words = |package: &str, data: &str| Event::Gmcp {
            package: package.into(),
            data: data.into(),
        };
        events
            .send(words(
                "Dev.Completions",
                r#"{"words": ["teleport", "tell"]}"#,
            ))
            .unwrap();
        events
            .send(words(
                "Dev.Completions.Ids",
                r#"{"words": ["temple_guard"]}"#,
            ))
            .unwrap();
        // A newer list replaces the one of the same package only.
        events
            .send(words("Dev.Completions", r#"{"words": ["teleport"]}"#))
            .unwrap();
        harness.run_steps(2);
        harness.state_mut().tab_mut().input = "te".into();
        harness.run_steps(2);
        for expected in ["teleport", "temple_guard", "te"] {
            harness.key_press(Key::Tab);
            harness.run_steps(2);
            assert_eq!(harness.state().tab().input, expected);
        }
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
        harness.state_mut().tab_mut().open_automation(0);
        harness.run_steps(4);
        harness.get_by_label("Add").click();
        harness.run_steps(2);
        let app = harness.state_mut().tab_mut();
        app.rules_draft.triggers[0].pattern = "you are hungry".into();
        app.rules_draft.triggers[0].send = "eat bread".into();
        // Edits are saved once they stop for a moment, with no button.
        harness.run_steps(2);
        assert!(!dir.join("rules.toml").exists(), "not while still editing");
        std::thread::sleep(Duration::from_millis(1000));
        harness.run_steps(2);
        assert_eq!(
            Rules::load(&dir.join("rules.toml")).unwrap().triggers.len(),
            1
        );
        // Closing the window saves at once.
        let app = harness.state_mut().tab_mut();
        app.rules_draft.triggers[0].name = "Hunger".into();
        app.close_automation();
        assert_eq!(
            Rules::load(&dir.join("rules.toml")).unwrap().triggers[0].name,
            "Hunger"
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
    fn macro_key_is_recorded_from_the_next_key_pressed() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        app.open_automation(2);
        app.rules_draft.keys.push(KeyBinding {
            enabled: true,
            send: "look".into(),
            ..KeyBinding::default()
        });
        app.recording_key = true;
        harness.run_steps(3);
        // A plain letter cannot be bound, so it is not taken.
        harness.key_press(Key::A);
        harness.run_steps(2);
        assert!(harness.state().tab().recording_key);
        harness.key_press_modifiers(Modifiers::CTRL | Modifiers::SHIFT, Key::K);
        harness.run_steps(2);
        let app = harness.state_mut().tab_mut();
        assert_eq!(app.rules_draft.keys[0].key, "ctrl+shift+k");
        assert!(!app.recording_key);
        // The keypad is recorded by its own names.
        app.recording_key = true;
        app.keypad_pressed = vec!["numpad8"];
        harness.run_steps(2);
        assert_eq!(harness.state().tab().rules_draft.keys[0].key, "numpad8");
        // Escape leaves the key as it was.
        harness.state_mut().tab_mut().recording_key = true;
        harness.key_press(Key::Escape);
        harness.run_steps(2);
        let app = harness.state().tab();
        assert_eq!(
            (app.rules_draft.keys[0].key.as_str(), app.recording_key),
            ("numpad8", false)
        );
    }

    #[test]
    fn welcome_screen_lists_the_profiles_and_gives_way_once_connecting() {
        let dir = std::env::temp_dir().join(format!("muddie-app-welcome-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let profiles = Profiles::at(dir.clone());
        let profile = Profile {
            host: "reinos.example.org".into(),
            port: 4000,
            ..Profile::default()
        };
        profiles.save("Reinos", &profile).unwrap();
        let mut harness = window(Some(profiles));
        harness.state_mut().tab_mut().started = false;
        harness.run_steps(3);
        assert!(harness.query_by_label("Reinos").is_some());
        assert!(
            harness
                .query_by_label("Where do you want to play?")
                .is_some()
        );
        // The session's own controls are not there yet.
        assert!(harness.query_by_label("Command").is_none());
        harness.state_mut().tab_mut().started = true;
        harness.run_steps(3);
        assert!(harness.query_by_label("Command").is_some());
        assert!(
            harness
                .query_by_label("Where do you want to play?")
                .is_none()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn arriving_lines_do_not_flash_the_view_of_the_latest_lines() {
        let mut harness = window(None);
        for burst in 0..12 {
            let app = harness.state_mut().tab_mut();
            (0..9).for_each(|n| app.show_local(&format!("burst {burst} line {n}")));
            // Frame by frame: the view must not come up even for one.
            for _ in 0..3 {
                harness.step();
                assert!(
                    harness.query_by_label("Latest lines").is_none(),
                    "shown during burst {burst}"
                );
            }
        }
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
        app.started = true;
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
        // What a new session opens on.
        let dir = std::env::temp_dir().join(format!("muddie-shot-welcome-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let profiles = Profiles::at(dir.clone());
        for (name, host, port) in [
            ("Reinos de Leyenda", "rlmud.org", 23),
            ("Aardwolf", "aardmud.org", 4000),
            ("Discworld", "discworld.starturtle.net", 4242),
        ] {
            let profile = Profile {
                host: host.into(),
                port,
                ..Profile::default()
            };
            profiles.save(name, &profile).unwrap();
        }
        let mut welcome = window(Some(profiles));
        welcome.state_mut().tab_mut().started = false;
        welcome.run_steps(3);
        save(&mut welcome, "welcome");
        welcome.state_mut().tab_mut().connection_open = true;
        welcome.run_steps(4);
        save(&mut welcome, "connection");
        std::fs::remove_dir_all(&dir).unwrap();

        let mut settings = demo_window();
        settings.state_mut().open_settings();
        settings.run_steps(4);
        save(&mut settings, "settings");
        // The automation window, with rules of each kind to look at.
        let mut rules = demo_window();
        rules.set_size(egui::vec2(1500.0, 900.0));
        let app = rules.state_mut().tab_mut();
        app.rules.triggers.push(Trigger {
            name: "Tells".into(),
            enabled: true,
            pattern: "tells you".into(),
            speak: "a tell".into(),
            highlight: "yellow".into(),
            ..Trigger::default()
        });
        app.rules.triggers.push(Trigger {
            name: "Fight back".into(),
            enabled: true,
            pattern: r"^(\w+) attacks you".into(),
            regex: true,
            send: "kill %1\nshield".into(),
            sound: "alert.ogg".into(),
            script: "-- Only the dangerous ones\nlocal feared = {Orc = true, Troll = true}\nif feared[m[1]] and gmcp(\"Char.Vitals\").hp < 40 then\n\tsend(\"flee\")\n\techo(\"Too weak for \" .. m[1])\nend".into(),
            ..Trigger::default()
        });
        app.rules.triggers.push(Trigger {
            enabled: false,
            pattern: "The wind howls".into(),
            gag: true,
            ..Trigger::default()
        });
        app.rules.keys.push(KeyBinding {
            name: "North".into(),
            enabled: true,
            key: "numpad8".into(),
            send: "north".into(),
            ..KeyBinding::default()
        });
        app.open_automation(0);
        app.rules_selected[0] = 1;
        app.rules_sample = "Orc attacks you with a rusty axe!".into();
        rules.run_steps(4);
        save(&mut rules, "automation");
        let app = rules.state_mut().tab_mut();
        app.rules_section = 2;
        rules.run_steps(3);
        save(&mut rules, "macros");
        let app = rules.state_mut().tab_mut();
        app.rules_draft.gauges.push(crate::rules::Gauge {
            name: "Salud".into(),
            enabled: true,
            value: "Char.Vitals.health.current".into(),
            max: "Char.Vitals.health.max".into(),
            colour: "red".into(),
            segments: true,
            low: 30,
        });
        app.rules_section = 3;
        rules.run_steps(3);
        save(&mut rules, "gauges");
        // The menus, as someone using Spanish sees them.
        for menu in ["Sesión", "Ver", "Mapa"] {
            let mut menus = connected_demo_window();
            crate::i18n::set_language(crate::settings::Language::Spanish);
            menus.run_steps(2);
            menus.get_by_label(menu).click();
            menus.run_steps(4);
            save(&mut menus, &format!("menu-{menu}"));
            crate::i18n::set_language(crate::settings::Language::English);
        }
        // A session whose MUD has reported no room: the map says so.
        let mut unmapped = window(None);
        fake_connection(unmapped.state_mut().tab_mut());
        unmapped
            .state_mut()
            .tab_mut()
            .show_local("Bienvenido a Fimbulvetr.");
        unmapped.run_steps(3);
        save(&mut unmapped, "unmapped");
        // The command line with nothing typed in it.
        let mut empty = connected_demo_window();
        empty.state_mut().tab_mut().input.clear();
        empty.run_steps(3);
        save(&mut empty, "empty");
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
        harness.state_mut().tab_mut().map_view.all_areas = true;
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
