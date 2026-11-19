//! One session: its connection bar, output, panels and input line.

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
mod commands;
mod data;
mod layout;
mod plugins;
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
    /// The panels around the output, as they are and as they were saved.
    layout: layout::Layout,
    layout_saved: layout::Layout,
    /// What each panel was given to show, by the panel's id.
    panel_text: Vec<(u32, layout::TextContent)>,
    /// The layout mode: panels are made, moved and removed.
    editing_layout: bool,
    /// The panel being dragged to another side, and one to be removed.
    dragged_panel: Option<u32>,
    /// Where each panel was drawn, to drop another before or after it.
    panel_rects: Vec<(u32, egui::Rect)>,
    /// Of the panels that share a place as tabs, by the place: which is
    /// on show; and those that got something while they were not.
    shown_tabs: Vec<(u32, u32)>,
    unread_tabs: Vec<u32>,
    removed_panel: Option<usize>,
    /// Stands in for the channels when no panel shows them.
    no_chat: Output,
    /// The line the triggers were last given, for those that show it in
    /// a panel.
    dispatched: Option<Line>,
    /// Shared with the script engine, which reads and edits it. No borrow
    /// may be held while calling into `scripts`.
    map: Rc<RefCell<Map>>,
    map_view: MapView,
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
    rules_selected: [usize; 7],
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
    /// The palette of commands: which one is chosen, and the text it was
    /// put away over, so that it stays away until that changes.
    /// The gauge being dragged to another place in the editor's list.
    gauge_dragged: Option<usize>,
    palette_chosen: usize,
    /// The protocols the MUD has offered in this connection, and whether
    /// each is in use.
    protocols: Vec<(String, bool)>,
    palette_open: bool,
    palette_query: String,
    /// The palette's field is to take the keyboard, or its cursor to go
    /// to the end of what was put in it.
    palette_focus: bool,
    palette_cursor_end: bool,
    palette_cache: Option<(String, Vec<commands::Item>)>,
    /// What the command being carried out has said so far, and whether
    /// it said it could not do what was asked.
    answer: Vec<commands::Said>,
    answering: bool,
    answer_failed: bool,
    /// What the last command said in a line, shown in the command line
    /// for a moment, and what it listed, shown over it to choose from.
    bar_note: Option<commands::BarNote>,
    results: Option<Vec<commands::Item>>,
    /// Whether the command line's cursor is to go to the end of what was
    /// put in it.
    cursor_to_end: bool,
    /// What checking the scripts being edited said.
    checks: automation::Checks,
    /// What the MUD has sent besides text, by name, to look at.
    data_seen: Vec<data::Seen>,
    /// The debugging view: whether it is on screen, the one chosen in
    /// its list, and what the list is narrowed to.
    debug_open: bool,
    data_selected: usize,
    data_filter: String,
    /// The protocol whose data alone is listed, if one was chosen.
    data_protocol: Option<String>,
    /// Whether what arrives is kept while the debugging view is closed,
    /// and what the connection was last told about tracing.
    data_keep: bool,
    trace_sent: Option<bool>,
    /// The session has no saved profile to keep its variables in, so
    /// they are carried from one start of the scripts to the next.
    variables_passing: bool,
    /// The variable being edited: which, its name as typed, its value as
    /// typed.
    variable_loaded: Option<String>,
    variable_name: String,
    variable_text: String,
    variable_kind: usize,
    variable_number: f64,
    variable_flag: bool,
    variable_items: Vec<(String, String)>,
    /// The group the editor's lists are narrowed to, and those folded.
    group_filter: Option<String>,
    folded_groups: Vec<String>,
    /// The address being typed to install a plugin from, and the row of
    /// the output from which the answers to the settings' plugins are.
    plugin_address: String,
    plugin_said: usize,
    /// The command left in the command line after it was sent, while it
    /// is still untouched; and whether it has yet to be selected.
    kept_input: Option<String>,
    select_input: bool,
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
    /// How many characters fit across and down the text of the game,
    /// what the server was last told of it, and since when it has been
    /// as it is.
    window_cells: (u32, u32),
    window_reported: Option<(u32, u32)>,
    window_settling: Option<((u32, u32), Instant)>,
    /// Tests of what is sent to the server are not about its size: they
    /// switch the telling of it off.
    #[cfg(test)]
    window_untold: bool,
    /// The address typed on the welcome screen.
    address: String,
    /// Whether the region for announcements was in the accessibility tree
    /// on the last frame.
    announce_ready: bool,
    /// Which of triggers, aliases and keys the rules window shows.
    rules_section: usize,
    /// Label of the text encoding the server uses; `auto` for most.
    encoding: String,
    /// The modules of GMCP to ask this MUD for besides the usual ones, as
    /// typed: their names, with commas between.
    gmcp_modules: String,
    /// Package installs and updates under way; the flag says to enable
    /// the package when its job ends well.
    package_jobs: Vec<(Receiver<packages::Outcome>, bool)>,
    /// Where the MUD said its sounds are fetched from, the ones being
    /// fetched, each with what to play once it is here, and the ones
    /// that could not be had, which are not asked for again.
    media_base: Option<String>,
    media_jobs: Vec<MediaJob>,
    media_missing: Vec<String>,
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

/// The modules of GMCP typed in a profile, each as `Name version`: with
/// commas between them, and version 1 for those given none.
fn modules_typed(typed: &str) -> Vec<String> {
    (typed.split(','))
        .map(str::trim)
        .filter(|module| !module.is_empty())
        .map(|module| match module.rsplit_once(' ') {
            Some((_, version)) if version.parse::<u32>().is_ok() => module.to_owned(),
            _ => format!("{module} 1"),
        })
        .collect()
}

/// A sound being fetched: where the result comes, its address, and what
/// to play once it is here.
type MediaJob = (
    Receiver<Result<PathBuf, String>>,
    String,
    Option<crate::media::Play>,
);

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
            layout: layout::Layout::default(),
            layout_saved: layout::Layout::default(),
            panel_text: Vec::new(),
            editing_layout: false,
            dragged_panel: None,
            panel_rects: Vec::new(),
            shown_tabs: Vec::new(),
            unread_tabs: Vec::new(),
            removed_panel: None,
            no_chat: Output::default(),
            dispatched: None,
            map,
            map_view: MapView::default(),
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
            rules_selected: [0; 7],
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
            gauge_dragged: None,
            palette_chosen: 0,
            protocols: Vec::new(),
            palette_open: false,
            palette_query: String::new(),
            palette_focus: false,
            palette_cursor_end: false,
            palette_cache: None,
            answer: Vec::new(),
            answering: false,
            answer_failed: false,
            bar_note: None,
            results: None,
            cursor_to_end: false,
            checks: automation::Checks::default(),
            data_seen: Vec::new(),
            debug_open: false,
            data_selected: 0,
            data_filter: String::new(),
            data_protocol: None,
            data_keep: false,
            trace_sent: Some(false),
            variables_passing: false,
            variable_loaded: None,
            variable_name: String::new(),
            variable_text: String::new(),
            variable_kind: 0,
            variable_number: 0.0,
            variable_flag: false,
            variable_items: Vec::new(),
            group_filter: None,
            folded_groups: Vec::new(),
            plugin_address: String::new(),
            plugin_said: 0,
            kept_input: None,
            select_input: false,
            panes: settings.automation_panes,
            detail_height: 600.0,
            supports: Vec::new(),
            completions: Vec::new(),
            panel_cells: (0, 0),
            panel_reported: None,
            panel_settling: None,
            window_cells: (0, 0),
            window_reported: None,
            window_settling: None,
            #[cfg(test)]
            window_untold: false,
            address: String::new(),
            announce_ready: false,
            rules_section: 0,
            encoding: "auto".into(),
            gmcp_modules: String::new(),
            package_jobs: Vec::new(),
            media_base: None,
            media_jobs: Vec::new(),
            media_missing: Vec::new(),
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
        app.load_layout();
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
        for (_, content) in &mut self.panel_text {
            content
                .log
                .restyle(settings.scrollback_lines, settings.timestamps);
        }
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
                        crate::widgets::number(&mut settings.scrollback_lines)
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
                switch_row(
                    ui,
                    &mut settings.keep_command,
                    tr("Keep the command after sending it"),
                )
                .on_hover_text(tr(
                    "It stays selected: Enter sends it again, and typing replaces it.",
                ));
                switch_row(
                    ui,
                    &mut settings.media_downloads,
                    tr("Download the sounds a MUD asks to play"),
                );
                switch_row(ui, &mut settings.log_sessions, tr("Log every session"));
                row(ui, tr("Log format"), &mut |ui| {
                    let name = |format: LogFormat| match format {
                        LogFormat::Text => tr("Text"),
                        LogFormat::Html => "HTML",
                    };
                    crate::widgets::dropdown("log format")
                        .selected_text(name(settings.log_format))
                        .width(110.0)
                        .show_ui(ui, |ui| {
                            for format in [LogFormat::Text, LogFormat::Html] {
                                crate::widgets::choice_value(
                                    ui,
                                    &mut settings.log_format,
                                    format,
                                    name(format),
                                );
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
                        crate::widgets::number(&mut settings.keepalive_seconds)
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
                    ui.add(crate::widgets::slider(
                        &mut settings.speech_rate,
                        -100..=100,
                    ))
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
        // Groups switched for the session keep how they were left, unless
        // what is saved for them is what changed.
        let saved_before: Vec<(String, bool)> = (self.rules.group_names().into_iter())
            .map(|name| (self.rules.group_enabled(&name), name))
            .map(|(on, name)| (name, on))
            .collect();
        let session: Vec<(String, bool)> = (self.scripts.groups().into_iter())
            .map(|name| (self.scripts.group_enabled(&name), name))
            .map(|(on, name)| (name, on))
            .collect();
        // The scripts say again what they want announced and are told
        // again how big the panel is.
        self.supports.clear();
        self.completions.clear();
        self.panel_reported = None;
        // Variables belong to the session. With a saved profile they are
        // kept with it; without one they last as long as the session, and
        // go with it into a profile if it is then saved as one.
        let passing = std::mem::take(&mut self.variables_passing).then(|| self.scripts.variables());
        self.scripts =
            Engine::new(Rc::clone(&self.map)).expect("failed to start the script runtime");
        match (&self.profiles, &self.profile) {
            (Some(profiles), Some(profile)) => {
                (self.scripts).use_memory_file(profiles.memory_path(Some(profile)));
            }
            _ => self.variables_passing = true,
        }
        let known: Vec<String> = self
            .scripts
            .variables()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        for (name, value) in passing.into_iter().flatten() {
            if !known.contains(&name) {
                let _ = self.scripts.set_variable(&name, Some(value));
            }
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
        for (name, on) in session {
            let before = saved_before
                .iter()
                .find(|(known, _)| *known == name)
                .is_none_or(|(_, on)| *on);
            if before == self.rules.group_enabled(&name) {
                self.scripts.set_group(&name, on);
            }
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
            self.gmcp_modules = profile.gmcp_modules.join(", ");
            self.voice.set_enabled(profile.speech);
        } else {
            self.packages.clear();
        }
        self.profile_name = name.clone().unwrap_or_default();
        self.profile = name;
        self.load_layout();
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
            gmcp_modules: modules_typed(&self.gmcp_modules),
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
            for module in modules_typed(&self.gmcp_modules) {
                if !config.gmcp_supports.contains(&module) {
                    config.gmcp_supports.push(module);
                }
            }
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
                // A moment later rather than at once: asked for at once,
                // egui draws a second frame after the first, and what
                // arrives together is drawn together this way.
                ctx.request_repaint_after(Duration::from_millis(30));
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
        // What a command typed with `/` says is its answer, shown at the
        // command line; anything else is a line of the client's.
        if self.answering {
            self.answer.push(commands::Said {
                text: text.to_owned(),
                about: String::new(),
                run: None,
            });
        } else {
            self.show_local(text);
        }
        self.voice.say(text);
    }

    /// Shows a line of the client's own, without speaking it.
    fn show_local(&mut self, text: &str) {
        self.output.push_local(text);
        let time = self.output.time(self.output.len() - 1).to_owned();
        self.logged(|log| log.local(&time, text));
    }

    /// Shows a line from the server.
    /// How many lines the scrollback holds.
    pub fn lines(&self) -> usize {
        self.output.len()
    }

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
        self.sync_trace();
        let recording = self.recording();
        let Some(connection) = &self.connection else {
            return;
        };
        let events: Vec<Event> = connection.events.try_iter().collect();
        for event in events {
            match event {
                Event::Connected => {
                    self.protocols.clear();
                    // A new connection passes over what the last was
                    // told to say.
                    self.trace_sent = Some(false);
                    self.reconnect_attempts = 0;
                    self.last_activity = Instant::now();
                    if self.settings.log_sessions && self.log.is_none() {
                        self.start_log();
                    }
                    self.note(tr("Connected."));
                    // The server has yet to hear how big the panel is, and
                    // the text of the game.
                    self.panel_reported = None;
                    self.window_reported = None;
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
                        if recording {
                            let said = serde_json::Value::String(text.clone()).to_string();
                            self.saw("MSP", "sound", &said);
                        }
                        self.output.clear_partial();
                        self.pending_partial = None;
                        self.msp(request);
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
                        None => {
                            let shown = self.scripts.on_styled_line(&line);
                            self.dispatched = Some(shown.clone().unwrap_or_else(|| line.clone()));
                            shown
                        }
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
                    } else if let Some(message) = crate::media::parse(&package, &data) {
                        self.media(message);
                    } else if package.eq_ignore_ascii_case("Comm.Channel.Text") {
                        for line in panels::channel_text(&data) {
                            self.channel_line(&line);
                        }
                    } else if package.eq_ignore_ascii_case("Room.Info") && self.scripts.native_map()
                    {
                        self.entered_room(&data);
                    }
                    if recording {
                        self.saw_data(&package, false, &data);
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
                    if recording {
                        self.saw_data(&variable, true, &data);
                    }
                    self.scripts.on_msdp(&variable, &data);
                    self.run_actions();
                }
                Event::Mssp(status) => {
                    // Shown with the rest of what the MUD sends, if looked at.
                    if recording {
                        let fields: serde_json::Map<String, serde_json::Value> = (status.iter())
                            .map(|(name, value)| (name.clone(), value.clone().into()))
                            .collect();
                        self.saw(
                            "MSSP",
                            "MSSP",
                            &serde_json::Value::Object(fields).to_string(),
                        );
                    }
                    self.scripts.on_mssp(&status);
                }
                Event::Other {
                    protocol,
                    name,
                    data,
                } => {
                    if recording {
                        self.saw(
                            &protocol,
                            &name,
                            &serde_json::Value::String(data).to_string(),
                        );
                    }
                }
                Event::Protocol { name, on } => {
                    match self.protocols.iter_mut().find(|(known, _)| *known == name) {
                        Some(known) => known.1 = on,
                        None => self.protocols.push((name, on)),
                    }
                }
                Event::Disconnected(error) => {
                    self.scripts.on_event("disconnect", "");
                    self.run_actions();
                    self.walk = None;
                    self.pending_partial = None;
                    self.settled_partial = None;
                    self.gauges.clear();
                    self.msdp_vitals.clear();
                    self.sound.stop_all();
                    self.media_base = None;
                    self.media_jobs.clear();
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
                Action::Panel { name, text } => self.show_in_panel(&name, text.as_deref()),
                Action::PanelAdd { name, text } => {
                    for line in panels::styled_lines(&text) {
                        self.add_to_panel(&name, &line);
                    }
                }
                Action::ToPanel(name) => {
                    if let Some(line) = self.dispatched.clone() {
                        self.add_to_panel(&name, &line);
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
                        self.channel_line(&line);
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
                "Plugins are enabled per profile: save or choose a profile first.",
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
            trf("Plugin {0} enabled.", &[name])
        } else {
            trf("Plugin {0} disabled.", &[name])
        });
    }

    /// Deletes an installed plugin, in every profile's eyes.
    fn remove_plugin(&mut self, name: &str) {
        let folder = self.profiles.as_ref().map(Profiles::packages_dir);
        let installed = self
            .profiles
            .as_ref()
            .map(Profiles::packages)
            .unwrap_or_default();
        let Some(folder) = folder.filter(|_| installed.iter().any(|plugin| plugin == name)) else {
            return self.note(tr("No such plugin is installed; see /plugin list."));
        };
        // Its name came from the list of folders there, so this is one.
        match std::fs::remove_dir_all(folder.join(name)) {
            Ok(()) => {
                self.note(&trf("Plugin {0} removed.", &[name]));
                if self.packages.iter().any(|plugin| plugin == name) {
                    self.set_package(name, false);
                }
            }
            Err(error) => self.note(&trf(
                "The plugin could not be removed: {0}",
                &[&error.to_string()],
            )),
        }
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
                    self.bar(&trf("Plugin {0} installed.", &[&name]), false);
                    self.set_package(&name, true);
                }
                Ok(name) => {
                    self.bar(&trf("Plugin {0} updated.", &[&name]), false);
                    self.load_scripts();
                }
                Err(error) => self.bar(
                    &trf(
                        "The plugin could not be installed or updated: {0}",
                        &[&error],
                    ),
                    true,
                ),
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
        self.media_fetched();
        self.sound.poll();
        if self.sound.has_loops() || !self.media_jobs.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    /// Carries out a message of `Client.Media`: a sound or music the MUD
    /// asks for, by name, with where to fetch it if it is not here.
    fn media(&mut self, message: crate::media::Media) {
        use crate::media::Media;
        match message {
            Media::Default(url) => self.media_base = Some(url),
            Media::Load { name, url } => {
                if let Some((address, None)) = self.media_file(&name, url.as_deref()) {
                    self.media_fetch(address, None);
                }
            }
            Media::Play(play) => self.media_play(play),
            Media::Stop(stop) => self
                .sound
                .stop_where(|label| stop.matches(&label.name, &label.kind, &label.tag, &label.key)),
        }
    }

    /// Carries out a request of the MUD Sound Protocol, which says the
    /// same things in its own way: a sound of a kind, with a priority,
    /// or the one music there is at a time.
    fn msp(&mut self, request: Msp) {
        let kind_of = |music: bool| if music { "music" } else { "sound" };
        match request {
            Msp::Stop { music, url } => {
                if let Some(url) = url {
                    self.media_base = Some(url);
                }
                let kind = kind_of(music);
                self.sound.stop_where(|label| label.kind == kind);
            }
            Msp::Play {
                music,
                file,
                volume,
                times,
                priority,
                keep_going,
                kind,
                url,
            } => {
                // A sound of a kind may be in the folder of that kind.
                let inside = (!kind.is_empty()).then(|| format!("{kind}/{file}"));
                let names = inside.into_iter().chain([file.clone()]);
                // One that stands for several is one of those there are.
                let name = (names.clone())
                    .find_map(|name| self.sound_among(&name))
                    .or_else(|| names.clone().next())
                    .unwrap_or(file);
                // An address may be of the folder, or of the file itself.
                let url = url.map(|url| match url.rsplit_once('/') {
                    Some((folder, last)) if crate::media::safe_name(last) => folder.to_owned(),
                    _ => url,
                });
                self.media_play(crate::media::Play {
                    name,
                    url,
                    kind: kind_of(music).to_owned(),
                    tag: kind,
                    // There is one music at a time.
                    key: if music {
                        "msp music".to_owned()
                    } else {
                        String::new()
                    },
                    volume,
                    loops: times,
                    keep_going,
                    priority,
                    start: 0,
                    finish: None,
                    fade_in: 0,
                    fade_out: 0,
                });
            }
        }
    }

    /// The sound among the player's own that `name` stands for, if it has
    /// `*` or `?` in it: one of those there are, a different one each
    /// time. `None` if it has neither, or there is none.
    fn sound_among(&self, name: &str) -> Option<String> {
        if !name.contains(['*', '?']) || !crate::media::safe_name(&name.replace(['*', '?'], "x")) {
            return None;
        }
        let (folder, pattern) = match name.rsplit_once('/') {
            Some((folder, pattern)) => (folder, pattern),
            None => ("", name),
        };
        let dirs = (self.profiles.as_ref())
            .map(|profiles| profiles.sound_dirs(self.profile.as_deref()))
            .unwrap_or_default();
        let mut found: Vec<String> = (dirs.iter())
            .filter_map(|dir| std::fs::read_dir(dir.join(folder)).ok())
            .flat_map(|entries| entries.flatten())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|file| muddie_sound::name_matches(pattern, file))
            .collect();
        found.sort();
        found.dedup();
        if found.is_empty() {
            return None;
        }
        // Whichever the clock says: no two in a row need be alike.
        let turn = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.subsec_nanos() as usize);
        let file = found.swap_remove(turn % found.len());
        Some(if folder.is_empty() {
            file
        } else {
            format!("{folder}/{file}")
        })
    }

    /// Plays a sound or music asked for by name, from the player's own
    /// or the web, as its priority and its key allow.
    fn media_play(&mut self, play: crate::media::Play) {
        let same = |label: &muddie_sound::Label| label.name == play.name;
        // Music that is playing goes on, unless told otherwise.
        let music = |label: &muddie_sound::Label| same(label) && label.kind == "music";
        if play.keep_going && self.sound.is_playing(music) {
            return;
        }
        // Under a key there is one thing at a time.
        if !play.key.is_empty() {
            let key = play.key.clone();
            self.sound
                .stop_where(|label| label.key == key && !same(label));
        }
        // What matters more is not played over, and stops what matters
        // less; what was given no priority does neither.
        if play.priority > 0 {
            let priority = play.priority;
            if self.sound.is_playing(|label| label.priority > priority) {
                return;
            }
            self.sound
                .stop_where(|label| label.priority > 0 && label.priority < priority);
        }
        match self.media_file(&play.name, play.url.as_deref()) {
            Some((_, Some(path))) => self.media_start(&path, &play),
            Some((address, None)) => self.media_fetch(address, Some(play)),
            None => {}
        }
    }

    /// Where the sound `name` is: among the player's own sounds, or
    /// already fetched, which gives its path; or on the web, which gives
    /// only its address. `None` if it is neither, or not safe to name.
    fn media_file(&self, name: &str, url: Option<&str>) -> Option<(String, Option<PathBuf>)> {
        if !crate::media::safe_name(name) {
            return None;
        }
        let own = self.sound_file(name);
        if own.is_file() {
            return Some((String::new(), Some(own)));
        }
        let address = crate::media::address(url.or(self.media_base.as_deref())?, name)?;
        let kept = crate::media::kept_at(&crate::media::cache_dir()?, &address);
        let here = kept.is_file().then_some(kept);
        Some((address, here))
    }

    /// Starts fetching a sound, to play `then` once it is here, unless
    /// fetching is switched off, it is being fetched, or it failed before.
    fn media_fetch(&mut self, address: String, then: Option<crate::media::Play>) {
        let busy = self
            .media_jobs
            .iter()
            .any(|(_, known, _)| *known == address);
        if !self.settings.media_downloads || busy || self.media_missing.contains(&address) {
            return;
        }
        let Some(cache) = crate::media::cache_dir() else {
            return;
        };
        // No more at once than a MUD could reasonably ask for.
        if self.media_jobs.len() < 8 {
            let job = crate::media::fetch(&cache, &address);
            self.media_jobs.push((job, address, then));
        }
    }

    /// Plays what has been fetched since the last look, and remembers
    /// what could not be, saying so once.
    fn media_fetched(&mut self) {
        let mut ended = Vec::new();
        self.media_jobs
            .retain_mut(|(job, address, then)| match job.try_recv() {
                Ok(outcome) => {
                    ended.push((outcome, address.clone(), then.take()));
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
            });
        for (outcome, address, then) in ended {
            match (outcome, then) {
                (Ok(path), Some(play)) => self.media_start(&path, &play),
                (Ok(_), None) => {}
                (Err(error), _) => {
                    self.media_missing.push(address);
                    self.note(&trf("A sound could not be fetched: {0}", &[&error]));
                }
            }
        }
    }

    fn media_start(&mut self, path: &std::path::Path, play: &crate::media::Play) {
        let label = muddie_sound::Label {
            name: play.name.clone(),
            kind: play.kind.clone(),
            tag: play.tag.clone(),
            key: play.key.clone(),
            priority: play.priority,
        };
        let options = muddie_sound::Options {
            volume: play.volume,
            times: play.loops,
            start: play.start,
            finish: play.finish,
            fade_in: play.fade_in,
            fade_out: play.fade_out,
        };
        // A sound that cannot be played is no error.
        let _ = self.sound.play_with(path, options, label);
    }

    /// Runs the binding of any key pressed with Ctrl or Alt, or of a
    /// function key, before anything else gets to use the key.
    fn bound_keys(&mut self, ui: &mut egui::Ui) {
        if ui.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::M)) {
            let shown = self.map_shown();
            self.set_map_shown(!shown);
        }
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
            let previous =
                crate::widgets::button(ui, tr("Previous")).clicked() || (enter && !forward);
            let next = crate::widgets::button(ui, tr("Next")).clicked() || forward;
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
            if crate::widgets::button(ui, tr("Close")).clicked() || escape {
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
        self.dispatched = Some(shown.clone().unwrap_or_else(|| line.clone()));
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
        let Some((command, argument)) = client_command(line) else {
            return false;
        };
        let argument = argument.as_str();
        match command {
            // The palette is the list of commands: open it, at the group
            // if one was given.
            "/help" | "/map" | "/plugin" => {
                self.palette_open = true;
                self.palette_focus = true;
                self.palette_cursor_end = true;
                self.palette_query = match command {
                    "/help" => String::new(),
                    group => format!("{} ", &group[1..]),
                };
            }
            "/map-show" | "/map-hide" => self.set_map_shown(command == "/map-show"),
            // Switches a group for this session, or lists them all.
            "/group" => {
                let (name, wanted) = match argument.rsplit_once(' ') {
                    Some((name, "on")) => (name.trim(), Some(true)),
                    Some((name, "off")) => (name.trim(), Some(false)),
                    _ => (argument, None),
                };
                let known = self.scripts.groups();
                if name.is_empty() {
                    if known.is_empty() {
                        self.refuse(tr(
                            "There are no groups. Give a rule a group in the editor.",
                        ));
                    }
                    for group in known {
                        let state = if self.scripts.group_enabled(&group) {
                            tr("on")
                        } else {
                            tr("off")
                        };
                        self.offer(&group, state, &format!("/group {group}"));
                    }
                } else if let Some(group) =
                    known.iter().find(|group| group.eq_ignore_ascii_case(name))
                {
                    let on = wanted.unwrap_or(!self.scripts.group_enabled(group));
                    self.scripts.set_group(group, on);
                    let said = if on {
                        "Group {0} on."
                    } else {
                        "Group {0} off."
                    };
                    self.note(&trf(said, &[group]));
                } else {
                    self.refuse(&trf(
                        "There is no group called {0}; /group lists them.",
                        &[name],
                    ));
                }
            }
            "/remove" if !argument.is_empty() => self.remove_plugin(argument),
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
                    None => self.refuse(tr("The map does not know where you are yet.")),
                }
            }
            "/find" if !argument.is_empty() => {
                if !self.offer_rooms(argument) {
                    self.refuse(tr("No room matches."));
                }
            }
            "/go" if !argument.is_empty() => {
                if let Ok(room) = argument.parse::<RoomId>() {
                    self.walk_to(room);
                } else {
                    let found: Vec<RoomId> = (self.map.borrow().find(argument).iter())
                        .map(|room| room.id)
                        .collect();
                    match found[..] {
                        [] => self.refuse(tr("No room matches.")),
                        [room] => self.walk_to(room),
                        // Several: listed, to choose the one meant.
                        _ => {
                            self.offer_rooms(argument);
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
                        self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()]))
                    }
                    None => self.refuse(tr("No such room; use /map avoid with a room number.")),
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
                        self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()]))
                    }
                    None => self.refuse(tr("This room has no such exit.")),
                }
            }
            // Draws the current area again from scratch.
            "/tidy" => {
                let area = self.map.borrow().current().map(|room| room.area.clone());
                match area {
                    Some(area) => self.tidy(&area),
                    None => self.refuse(tr("The map does not know where you are yet.")),
                }
            }
            "/packages" => {
                let installed = self
                    .profiles
                    .as_ref()
                    .map(Profiles::packages)
                    .unwrap_or_default();
                if installed.is_empty() {
                    self.refuse(tr(
                        "No plugins are installed. Use /plugin install with a git address.",
                    ));
                }
                for package in installed {
                    let on = self.packages.contains(&package);
                    let (state, switch) = if on {
                        (tr("enabled in this profile"), "disable")
                    } else {
                        (tr("not enabled; use /plugin enable"), "enable")
                    };
                    self.offer(&package, state, &format!("/plugin {switch} {package}"));
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
                        "Plugins need a configuration folder, and none was found.",
                    )),
                    (_, None) => self.note(tr(
                        "Give the plugin a name after the address: letters, digits, - and _.",
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
                        "Plugins need a configuration folder, and none was found.",
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
                    self.note(tr("No such plugin is installed; see /plugin list."));
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
            Err(error) => self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()])),
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

    /// Tells the server how many characters fit across and down the text
    /// of the game (NAWS), on connecting and once a change of size has
    /// settled, so that it wraps and pages what it sends to fit.
    pub(super) fn report_window(&mut self, ctx: &egui::Context) {
        const SETTLED: Duration = Duration::from_millis(250);
        let cells = self.window_cells;
        #[cfg(test)]
        if self.window_untold {
            return;
        }
        let Some(connection) = &self.connection else {
            return self.window_settling = None;
        };
        if self.window_reported == Some(cells) || cells.0 == 0 || cells.1 == 0 {
            return self.window_settling = None;
        }
        match self.window_settling {
            Some((settling, since)) if settling == cells && since.elapsed() >= SETTLED => {
                self.window_settling = None;
                self.window_reported = Some(cells);
                let size = |cells: u32| u16::try_from(cells).unwrap_or(u16::MAX);
                let _ = (connection.commands).send(Command::Resize(size(cells.0), size(cells.1)));
            }
            Some((settling, _)) if settling == cells => ctx.request_repaint_after(SETTLED),
            _ => {
                self.window_settling = Some((cells, Instant::now()));
                ctx.request_repaint_after(SETTLED);
            }
        }
    }

    /// What the map's panel shows: the map; failing that, what a script
    /// put there, such as a map the MUD draws itself; failing that, why
    /// there is nothing yet.
    fn map_body(&mut self, ui: &mut egui::Ui, id: u32) -> Option<MapAction> {
        // What a drawing sent by the server has to fit in: the panel, less
        // its title.
        let room = ui.available_size() - egui::vec2(0.0, 22.0);
        self.panel_cells = Self::cells(ui, room);
        if self.map.borrow().current().is_some() {
            let map = self.map.borrow();
            return self.map_view.show(ui, &map, self.walk.is_some());
        }
        let drawn = !self.text_of(id).fixed.is_empty() || self.text_of(id).log.len() > 0;
        if drawn {
            crate::widgets::caption(ui, tr("Map"));
            self.text_body(ui, id, true);
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
            scripts = crate::widgets::button(ui, tr("Set up a mapper…")).clicked();
            ui.add_space(4.0);
            if crate::widgets::quiet_button(ui, tr("Hide the map")).clicked() {
                self.set_map_shown(false);
            }
        });
        scripts.then_some(MapAction::Scripts)
    }

    /// Up to [`MAX_LISTED_ROOMS`] rooms matching `query`, one line each.
    /// Lists the rooms whose name contains `query`, each to be walked to
    /// by choosing it. Returns whether there were any.
    fn offer_rooms(&mut self, query: &str) -> bool {
        let rooms: Vec<(RoomId, String, String)> = (self.map.borrow().find(query).iter())
            .take(MAX_LISTED_ROOMS)
            .map(|room| (room.id, room.name.to_string(), room.area.to_string()))
            .collect();
        for (id, name, area) in &rooms {
            self.offer(&format!("#{id}: {name}"), area, &format!("/map walk {id}"));
        }
        !rooms.is_empty()
    }

    fn submit(&mut self) {
        let line = std::mem::take(&mut self.input);
        // The command stays, selected, to be sent again or typed over.
        // Never a password.
        // Never a password, and not a command of the client: that is not
        // something to send again and again.
        let keep = self.settings.keep_command
            && !self.input_hidden
            && !line.is_empty()
            && client_command(&line).is_none();
        self.kept_input = keep.then(|| line.clone());
        if let Some(kept) = &self.kept_input {
            self.input.clone_from(kept);
            self.select_input = true;
        }
        // What follows answers this command, so drop what was still queued.
        self.voice.stop();
        if self.input_hidden {
            // A password: scripts and the history never see it.
            self.send(line);
        } else {
            self.history.add(&line);
            // A command of the client answers at the command line.
            if !self.run_command(&line) {
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
        // What the last command said takes the place of the hint; with
        // something typed, it is said at the other end of the line.
        let said = self.bar_said(ui);
        let hint = match &said {
            Some((text, colour)) => RichText::new(text).font(palette::font()).color(*colour),
            None => RichText::new(tr("Type a command"))
                .font(palette::font())
                .color(theme.text_faint),
        };
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
                    // In the typeface of what will be typed, so it sits
                    // on the same line.
                    .hint_text(hint)
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
        if std::mem::take(&mut self.select_input) {
            let all = egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(self.input.chars().count()),
            );
            output.state.cursor.set_char_range(Some(all));
            output.state.clone().store(ui.ctx(), id);
        }
        // Once it is typed over or edited, it is a new command.
        if self
            .kept_input
            .as_ref()
            .is_some_and(|kept| *kept != self.input)
        {
            self.kept_input = None;
        }
        if completing || std::mem::take(&mut self.cursor_to_end) {
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
            // A command left from before is the newest in the history: the
            // arrows start from it, not from what its text happens to match.
            if (up || down || search) && self.kept_input.take().is_some() {
                self.input.clear();
                if up {
                    self.history.older(&mut self.input);
                }
            }
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
        if has_text && let Some((text, colour)) = said {
            let bar = shown.response.rect;
            let at = bar.right_center() - egui::vec2(14.0, 0.0);
            let font = theme::font(crate::theme::Weight::Regular, 13.0);
            let galley = ui.painter().layout_no_wrap(text, font, colour);
            let place = egui::Rect::from_min_size(
                egui::pos2(at.x - galley.size().x, at.y - galley.size().y / 2.0),
                galley.size(),
            );
            // Over whatever of the typed text reaches that far.
            ui.painter()
                .rect_filled(place.expand2(egui::vec2(10.0, 2.0)), 4.0, theme.background);
            ui.painter().galley(place.min, galley, colour);
        }
        // Typing puts away what the last command answered.
        if response.changed() {
            self.bar_note = None;
        }
        let submitted = response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        if submitted {
            self.submit();
        }
        // The input owns the keyboard unless the user moved focus elsewhere.
        if !self.palette_open && (submitted || ui.memory(|memory| memory.focused().is_none())) {
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

/// The client's own commands, as they are typed, and what each is for.
/// They are in English whatever the language of the interface, like the
/// words of a programming language.
const COMMANDS: [(&str, &str); 21] = [
    ("/map where", "Says where you are"),
    (
        "/map find <text>",
        "Lists the rooms whose name contains the text",
    ),
    ("/map walk <room>", "Walks to a room, by name or number"),
    ("/map stop", "Stops walking"),
    (
        "/map avoid [number]",
        "Keeps routes out of a room, or lets them in again",
    ),
    (
        "/map lock <direction>",
        "Keeps routes from an exit of this room, or lets them use it again",
    ),
    ("/map tidy", "Draws this area again from scratch"),
    ("/map show", "Shows the map panel"),
    ("/map hide", "Hides the map panel"),
    ("/plugin list", "Lists the installed plugins"),
    (
        "/plugin install <git address> [name]",
        "Installs a plugin and enables it",
    ),
    ("/plugin update <name>", "Brings a plugin up to date"),
    ("/plugin enable <name>", "Enables a plugin in this profile"),
    (
        "/plugin disable <name>",
        "Disables a plugin in this profile",
    ),
    ("/plugin remove <name>", "Deletes a plugin"),
    (
        "/group",
        "Lists the groups of rules and whether they are on",
    ),
    (
        "/group <name> [on|off]",
        "Switches a group on or off for this session",
    ),
    ("/log", "Starts or stops logging the session"),
    ("/reload", "Runs the scripts and rules again"),
    ("/lua <code>", "Runs a line of Luau"),
    ("/help", "Lists these commands"),
];

/// Reads a line as a command of the client: what it does, under the name
/// the code knows it by, and what it was given. Anything else is for the
/// MUD.
fn client_command(line: &str) -> Option<(&'static str, String)> {
    let mut words = line.split_whitespace();
    let (group, what) = (words.next()?, words.next().unwrap_or_default());
    let rest = words.collect::<Vec<_>>().join(" ");
    let command = match (group, what) {
        ("/map", "where") => "/where",
        ("/map", "find") => "/find",
        ("/map", "walk") => "/go",
        ("/map", "stop") => "/stop",
        ("/map", "avoid") => "/avoid",
        ("/map", "lock") => "/lock",
        ("/map", "tidy") => "/tidy",
        ("/map", "show") => "/map-show",
        ("/map", "hide") => "/map-hide",
        ("/map", _) => "/map",
        ("/plugin", "list") => "/packages",
        ("/plugin", "install") => "/install",
        ("/plugin", "update") => "/update",
        ("/plugin", "enable") => "/enable",
        ("/plugin", "disable") => "/disable",
        ("/plugin", "remove") => "/remove",
        ("/plugin", _) => "/plugin",
        ("/group", _) => {
            let after = line.trim_start().strip_prefix("/group").unwrap_or_default();
            return Some(("/group", after.trim().to_owned()));
        }
        ("/log", _) => "/log",
        ("/reload", _) => "/reload",
        ("/help", _) => "/help",
        _ => return None,
    };
    Some((command, rest))
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
            crate::i18n::set_language("en");
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
        app.window_untold = true;
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
        // However busy the machine: it is waited for, not timed.
        let mut answer = None;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(100));
            harness.run_steps(2);
            answer = sent.try_recv().ok();
            if answer.is_some() {
                break;
            }
        }
        let Some(Command::Gmcp { package, data }) = answer else {
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
        // It is the map's own panel that shows it, not one more.
        assert_eq!(app.layout.panels.len(), 3);
        let map = app
            .layout
            .panels
            .iter()
            .find(|panel| panel.kind == layout::Kind::Map)
            .unwrap();
        let drawn = app.panel_text.iter().find(|(id, _)| *id == map.id).unwrap();
        assert_eq!(drawn.1.fixed.len(), 3);
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
    fn sent_command_stays_selected_to_repeat_or_type_over() {
        let mut harness = window(None);
        let (_events, mut sent) = fake_connection(harness.state_mut().tab_mut());
        let send = |text: &str| Command::Send(text.into());
        harness.state_mut().tab_mut().input = "north".into();
        harness.run_steps(2);
        // Enter sends it and leaves it there; Enter again sends it again.
        for _ in 0..3 {
            harness.key_press(Key::Enter);
            harness.run_steps(3);
            assert_eq!(sent.try_recv().unwrap(), send("north"));
            assert_eq!(harness.state().tab().input, "north");
        }
        // It is selected: what is typed takes its place.
        harness.get_by_label("Command").type_text("look");
        harness.run_steps(3);
        assert_eq!(harness.state().tab().input, "look");
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        assert_eq!(sent.try_recv().unwrap(), send("look"));
        // The arrows browse from it as the newest command, not as a search.
        harness.key_press(Key::ArrowUp);
        harness.run_steps(3);
        assert_eq!(harness.state().tab().input, "north");
        // With the setting off, the line empties as it used to.
        let app = harness.state_mut().tab_mut();
        app.settings.keep_command = false;
        app.input = "south".into();
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        assert_eq!(sent.try_recv().unwrap(), send("south"));
        assert_eq!(harness.state().tab().input, "");
    }

    #[test]
    fn client_commands_are_grouped() {
        let read = |line| client_command(line).map(|(command, rest)| format!("{command} {rest}"));
        assert_eq!(
            read("/map walk Temple of Mota").unwrap(),
            "/go Temple of Mota"
        );
        assert_eq!(
            read("/plugin install https://example.org/x.git mine").unwrap(),
            "/install https://example.org/x.git mine"
        );
        assert_eq!(read("/map").unwrap(), "/map ");
        // The names of before are gone, and what is not ours is the MUD's.
        assert_eq!(read("/where"), None);
        assert_eq!(read("/say hello"), None);
        assert_eq!(read("north"), None);

        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        assert!(app.map_command("/map hide") && !app.map_shown());
        assert!(app.map_command("/map show") && app.map_shown());
        // The palette is the list of the commands: asking for help opens it.
        assert!(app.map_command("/help") && app.palette_open);
        app.toggle_palette();
        // Groups: listed, and switched for the session by name.
        app.scripts
            .exec("t", r#"bind("f9", "kick", {group = "Combat"})"#);
        assert!(app.map_command("/group combat off") && !app.scripts.group_enabled("Combat"));
        assert!(app.map_command("/group Combat") && app.scripts.group_enabled("Combat"));
        assert!(app.map_command("/group"));
        assert!(output_lines(app).iter().any(|line| line == "Combat: on"));
        // An incomplete group opens the palette at what it has, instead
        // of going to the MUD.
        assert!(app.map_command("/map dance"));
        assert!(app.palette_open && app.palette_query == "map ");
    }

    #[test]
    fn commands_are_chosen_from_a_palette_and_answer_at_the_line() {
        let mut harness = window(None);
        harness.run_steps(2);
        let before = output_lines(harness.state().tab()).len();
        // Typing a slash in the command line offers nothing over it.
        harness.state_mut().tab_mut().input = "/pl".into();
        harness.run_steps(3);
        assert!(harness.query_by_label_contains("plugin install").is_none());
        harness.state_mut().tab_mut().input.clear();
        // The palette lists the commands, each said for what it is.
        harness.state_mut().tab_mut().toggle_palette();
        harness.run_steps(3);
        harness.get_by_label_contains("map where");
        harness.state_mut().tab_mut().palette_query = "pl".into();
        harness.run_steps(3);
        harness.get_by_label_contains("plugin install");
        assert!(harness.query_by_label_contains("map where").is_none());
        // The arrows choose; Enter on one that takes something more
        // completes it, to type that after.
        harness.key_press(Key::ArrowDown);
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(2);
        let app = harness.state().tab();
        assert!(app.palette_open);
        assert_eq!(app.palette_query, "plugin install ");
        // Escape closes it.
        harness.key_press(Key::Escape);
        harness.run_steps(2);
        assert!(!harness.state().tab().palette_open);
        assert!(harness.query_by_label_contains("plugin install").is_none());
        // One Enter runs what is chosen, typed in full or not, and closes
        // the palette. A line of answer is said in the command line, and
        // the lines of the game are as they were.
        let app = harness.state_mut().tab_mut();
        app.scripts
            .exec("t", r#"bind("f9", "kick", {group = "Combat"})"#);
        app.toggle_palette();
        app.palette_query = "map hi".into();
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        let app = harness.state().tab();
        assert!(!app.map_shown() && !app.palette_open);
        assert_eq!(output_lines(app).len(), before);
        let app = harness.state_mut().tab_mut();
        app.toggle_palette();
        app.palette_query = "group Com".into();
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        let app = harness.state().tab();
        assert!(!app.scripts.group_enabled("Combat"));
        let note = app.bar_note.clone().expect("a line of answer");
        assert_eq!(
            (note.text.as_str(), note.problem),
            ("Group Combat off.", false)
        );
        assert_eq!(output_lines(app).len(), before);
        // Left alone, it goes by itself after a moment.
        harness.run_steps(40);
        assert!(harness.state().tab().bar_note.is_none());
        // What a command lists stays in the palette, to choose from:
        // Enter on a group switches it.
        let app = harness.state_mut().tab_mut();
        app.toggle_palette();
        app.palette_query = "gr".into();
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        let app = harness.state().tab();
        assert!(app.palette_open && app.results.is_some());
        harness.get_by_label("Combat. off");
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        let app = harness.state().tab();
        assert!(app.scripts.group_enabled("Combat") && !app.palette_open);
        // Typed in the command line after a slash, a command is carried
        // out the same, and is not kept there to be sent again. What
        // could not be done stays said, until something is typed.
        harness.state_mut().tab_mut().input = "/map find nowhere".into();
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(40);
        let app = harness.state().tab();
        assert!(app.input.is_empty());
        let note = app.bar_note.clone().expect("a refusal");
        assert_eq!(
            (note.text.as_str(), note.problem),
            ("No room matches.", true)
        );
    }

    #[test]
    fn panels_are_kept_fed_by_triggers_and_moved_about() {
        let dir = std::env::temp_dir().join(format!("muddie-app-panels-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut harness = window(Some(Profiles::at(dir.clone())));
        let app = harness.state_mut().tab_mut();
        // What a profile starts with.
        let kinds: Vec<layout::Kind> = app.layout.panels.iter().map(|panel| panel.kind).collect();
        assert_eq!(
            kinds,
            [layout::Kind::Text, layout::Kind::Map, layout::Kind::Gauges]
        );
        // A trigger shows its line in a panel, which is made if need be,
        // and with the line hidden it is only there.
        app.rules.triggers.push(Trigger {
            enabled: true,
            pattern: "tells you".into(),
            panel: "Tells".into(),
            gag: true,
            ..Trigger::default()
        });
        for (name, code) in app.rules.chunks() {
            app.scripts.exec(&name, &code);
        }
        let (events, _sent) = fake_connection(app);
        let line = |text: &str| Event::Line(panels::styled_lines(text).remove(0));
        events
            .send(line("\x1b[33mAna tells you: hola\x1b[0m"))
            .unwrap();
        events.send(line("A plain line")).unwrap();
        harness.run_steps(3);
        let app = harness.state_mut().tab_mut();
        let tells = app
            .layout
            .panels
            .iter()
            .find(|panel| panel.name == "Tells")
            .expect("made")
            .clone();
        let content = &app
            .panel_text
            .iter()
            .find(|(id, _)| *id == tells.id)
            .unwrap()
            .1;
        assert_eq!(
            (content.log.len(), content.log.text(0)),
            (1, "Ana tells you: hola")
        );
        assert_eq!(output_lines(app), ["A plain line"]);
        // The channels go to the panel that shows them.
        app.channel_line(&panels::styled_lines("[ooc] Luis: buenas").remove(0));
        let chat = app.layout.panels[0].id;
        assert_eq!(
            app.panel_text
                .iter()
                .find(|(id, _)| *id == chat)
                .unwrap()
                .1
                .log
                .len(),
            1
        );
        // Moved to another side and hidden, it is all there the next time.
        let index = app
            .layout
            .panels
            .iter()
            .position(|panel| panel.id == tells.id)
            .unwrap();
        app.layout.panels[index].side = layout::Side::Left;
        app.set_map_shown(false);
        harness.run_steps(3);
        assert!(
            harness.query_by_label("Where am I?").is_none(),
            "the map is hidden"
        );
        let kept = layout::Layout::load(&dir.join("layout.toml")).unwrap();
        assert_eq!(kept.panels[index].side, layout::Side::Left);
        assert!(
            !kept
                .panels
                .iter()
                .any(|panel| panel.kind == layout::Kind::Map && panel.visible)
        );
        // Two panels on one side share it, each with its part.
        let app = harness.state_mut().tab_mut();
        app.set_map_shown(true);
        let map = app
            .layout
            .panels
            .iter()
            .position(|panel| panel.kind == layout::Kind::Map)
            .unwrap();
        app.layout.panels[index].side = app.layout.panels[map].side;
        harness.run_steps(3);
        let app = harness.state().tab();
        let rect = |id| {
            app.panel_rects
                .iter()
                .find(|(known, _)| *known == id)
                .unwrap()
                .1
        };
        let (above, below) = (rect(app.layout.panels[map].id), rect(tells.id));
        assert!((above.width() - below.width()).abs() < 1.0 && above.bottom() < below.top());
        assert!(
            (above.height() - below.height()).abs() < 2.0,
            "equal weights, equal parts"
        );
        // Joined as tabs they share one place, and the one out of sight
        // says so when something comes for it.
        let app = harness.state_mut().tab_mut();
        app.layout.panels[index].stack = 1;
        app.layout.panels[map].stack = 1;
        harness.run_steps(3);
        let app = harness.state_mut().tab_mut();
        let rect = |app: &Tab, id| {
            app.panel_rects
                .iter()
                .find(|(known, _)| *known == id)
                .unwrap()
                .1
        };
        assert_eq!(rect(app, app.layout.panels[map].id), rect(app, tells.id));
        let first = app.layout.panels[map.min(index)].id;
        let hidden = if first == tells.id {
            app.layout.panels[map].id
        } else {
            tells.id
        };
        assert!(app.shown_tabs.contains(&(1, first)));
        if hidden == tells.id {
            app.add_to_panel(
                "Tells",
                &panels::styled_lines("Luis tells you: eh").remove(0),
            );
            assert_eq!(app.unread_tabs, [tells.id]);
            harness.get_by_label("Tells").click();
            harness.run_steps(3);
            let app = harness.state().tab();
            assert!(app.shown_tabs.contains(&(1, tells.id)) && app.unread_tabs.is_empty());
        }
        let app = harness.state_mut().tab_mut();
        app.layout.panels[index].stack = 0;
        app.layout.panels[map].stack = 0;
        harness.run_steps(3);
        // Its size is as it was drawn, not a little more each time.
        let before = harness.state().tab().layout.panels[index].size;
        harness.run_steps(5);
        assert_eq!(harness.state().tab().layout.panels[index].size, before);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn variables_are_the_sessions_and_kept_only_with_a_saved_profile() {
        let dir = std::env::temp_dir().join(format!("muddie-app-vars-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut harness = window(Some(Profiles::at(dir.clone())));
        let app = harness.state_mut().tab_mut();
        // No profile: they last through the scripts starting again, and
        // nothing is written anywhere.
        app.scripts.exec("t", r#"vars.target = "orc""#);
        app.load_scripts();
        assert_eq!(
            app.scripts.variables(),
            [("target".to_owned(), serde_json::json!("orc"))]
        );
        assert!(!dir.join("data.json").exists());
        // Another session does not see them.
        let mut other = window(Some(Profiles::at(dir.clone())));
        assert!(other.state_mut().tab_mut().scripts.variables().is_empty());
        // Saved as a profile, they go with it, and are there the next time.
        let app = harness.state_mut().tab_mut();
        app.host = "mud.example.org".into();
        app.port = "4000".into();
        app.profile_name = "Reinos".into();
        app.save_profile();
        assert_eq!(app.profile.as_deref(), Some("Reinos"));
        assert_eq!(app.scripts.variables().len(), 1);
        let mut later = window(Some(Profiles::at(dir.clone())));
        let app = later.state_mut().tab_mut();
        assert!(app.select_profile(Some("Reinos".into())));
        assert_eq!(
            app.scripts.variables(),
            [("target".to_owned(), serde_json::json!("orc"))]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn what_the_mud_sends_is_kept_to_look_at_and_made_into_gauges() {
        let mut harness = window(None);
        let (events, _sent) = fake_connection(harness.state_mut().tab_mut());
        // Kept from the start, as when it is asked to go on outside the
        // debugging view.
        harness.state_mut().tab_mut().data_keep = true;
        let vitals = |hp: u32| Event::Gmcp {
            package: "Char.Vitals".into(),
            data: format!(r#"{{"health": {{"current": {hp}, "max": 100}}}}"#),
        };
        events.send(vitals(40)).unwrap();
        events.send(vitals(62)).unwrap();
        let hunger = Event::Msdp {
            variable: "HUNGER".into(),
            data: "\"7\"".into(),
        };
        events.send(hunger).unwrap();
        harness.run_steps(3);
        let app = harness.state_mut().tab_mut();
        let seen: Vec<(&str, bool, u32)> = app
            .data_seen
            .iter()
            .map(|seen| (seen.name.as_str(), seen.msdp, seen.count))
            .collect();
        assert_eq!(seen, [("Char.Vitals", false, 2), ("HUNGER", true, 1)]);
        assert!(
            app.data_seen[0].json.contains("62") && app.data_seen[0].earlier[0].1.contains("40")
        );
        // The editor shows it, and a number there becomes a gauge with
        // its maximum found, in a panel of gauges.
        app.set_view(2);
        harness.run_steps(3);
        assert!(
            harness
                .query_by_label("Char.Vitals.health.current")
                .is_some()
        );
        harness
            .get_all_by_label("Make a gauge")
            .next()
            .unwrap()
            .click();
        harness.run_steps(3);
        let app = harness.state_mut().tab_mut();
        let made = app
            .rules_draft
            .gauges
            .last()
            .expect("a gauge was made")
            .clone();
        assert_eq!(
            (made.name.as_str(), made.value.as_str(), made.max.as_str()),
            (
                "health",
                "Char.Vitals.health.current",
                "Char.Vitals.health.max"
            )
        );
        assert_eq!(app.view(), 1, "it is shown in the editor to be finished");
        // Saved, it follows the value, in the panel it was put in.
        app.close_automation();
        events.send(vitals(55)).unwrap();
        harness.run_steps(3);
        let app = harness.state().tab();
        assert!(app.gauge_is_in("health", made.panel));
        assert!(app.gauges.summary().contains("health 55 of 100"));
    }

    #[test]
    fn long_scrollback_is_not_laid_out_all_at_once_when_the_text_changes() {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        (0..6000).for_each(|n| {
            app.show_local(&format!(
                "line {n} of a long session, long enough to matter"
            ))
        });
        harness.run_steps(3);
        // A change of the text's size: every row may be another height.
        let mut settings = harness.state().tab().settings.clone();
        settings.font_size = 19.0;
        let ctx = harness.ctx.clone();
        crate::theme::apply(&ctx, &settings).unwrap();
        harness.state_mut().tab_mut().settings_changed(&settings);
        harness.step();
        // One frame does not pay for six thousand rows: it lays out a
        // share, and what is on the screen.
        let measured = harness.state().tab().output.measured();
        assert!(measured < 3000, "{measured} rows were laid out in a frame");
        // And the end stays in view while the rest catches up.
        harness.run_steps(40);
        assert!(harness.query_by_label("Latest lines").is_none());
    }

    #[test]
    fn a_variable_is_edited_as_its_type() {
        let mut harness = window(None);
        harness.set_size(egui::vec2(1400.0, 800.0));
        let app = harness.state_mut().tab_mut();
        app.scripts
            .set_variable("bag", Some(serde_json::json!({"gold": 5})))
            .unwrap();
        app.open_automation(5);
        harness.run_steps(3);
        // A table: a key and a value a row, added to and typed over.
        harness.get_by_label("Add a field").click();
        harness.run_steps(2);
        let app = harness.state_mut().tab_mut();
        app.variable_items[1] = ("keys".into(), "3".into());
        harness.run_steps(2);
        let kept = |app: &Tab| app.scripts.variables()[0].1.clone();
        assert_eq!(
            kept(harness.state().tab()),
            serde_json::json!({"gold": 5, "keys": 3})
        );
        // Made an array, it keeps its values and loses their keys.
        harness.get_by_label("array").click();
        harness.run_steps(3);
        assert_eq!(kept(harness.state().tab()), serde_json::json!([5, 3]));
        // A boolean is a switch.
        harness.get_by_label("boolean").click();
        harness.run_steps(3);
        assert_eq!(kept(harness.state().tab()), serde_json::json!(true));
        harness.get_by_label("Value").click();
        harness.run_steps(3);
        assert_eq!(kept(harness.state().tab()), serde_json::json!(false));
    }

    /// Says what a frame costs in the views that could be slow, to look
    /// for what to make faster:
    /// `cargo test -p muddie frame_costs -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn frame_costs() {
        let cost = |harness: &mut Harness<'_, App>, what: &str| {
            harness.run_steps(3);
            let started = Instant::now();
            for _ in 0..30 {
                harness.step();
            }
            println!(
                "{what}: {:.2} ms a frame",
                started.elapsed().as_secs_f64() * 1000.0 / 30.0
            );
        };
        let mut harness = window(None);
        harness.set_size(egui::vec2(1600.0, 1000.0));
        cost(&mut harness, "empty session");
        let app = harness.state_mut().tab_mut();
        for n in 0..20_000 {
            app.show_local(&format!(
                "\x1b[32mline {n}\x1b[0m of a long session, with some text to wrap now and then"
            ));
        }
        cost(&mut harness, "20000 lines of scrollback");
        let app = harness.state_mut().tab_mut();
        for n in 0..2000 {
            let line = panels::styled_lines(&format!("[ooc] someone says thing {n}")).remove(0);
            app.channel_line(&line);
            app.add_to_panel("Combat", &line);
        }
        app.gauges.set("HP", Some((80.0, 100.0)));
        cost(&mut harness, "with panels of 2000 lines and gauges");
        let app = harness.state_mut().tab_mut();
        for n in 0..300 {
            app.rules.triggers.push(Trigger {
                name: format!("Trigger {n}"),
                enabled: true,
                pattern: format!("pattern {n}"),
                send: "say hi".into(),
                group: format!("Group {}", n % 6),
                ..Trigger::default()
            });
        }
        app.open_automation(0);
        cost(&mut harness, "editor with 300 triggers");
        let app = harness.state_mut().tab_mut();
        for n in 0..120 {
            app.saw_data(
                &format!("Pkg.Number{n}"),
                false,
                r#"{"a": 1, "b": {"c": [1, 2, 3], "d": "text"}}"#,
            );
        }
        app.set_view(2);
        cost(&mut harness, "debug with 120 packages");
    }

    #[test]
    fn a_burst_of_lines_is_measured_a_share_a_frame() {
        let mut harness = window(None);
        harness.run_steps(2);
        let app = harness.state_mut().tab_mut();
        (0..6000).for_each(|n| app.show_local(&format!("line {n} of a burst of text")));
        // Laid out all at once, each row would hold its laid out text for
        // the frame; a share is, the newest, which are the ones in view.
        harness.step();
        let share = crate::output::REMEASURED_PER_FRAME;
        let measured = harness.state().tab().output.measured();
        assert!(
            measured > 0 && measured <= share + 100,
            "{measured} rows measured in a frame"
        );
        // The view follows to the end, where the newest already are; a
        // frame or two later, as the size of what it holds settles.
        harness.run_steps(3);
        assert!(harness.query_by_label("Latest lines").is_none());
        harness.get_by_label("line 5999 of a burst of text");
        // The rest follow, and the end stays in view meanwhile.
        harness.run_steps(6000 / share + 2);
        assert_eq!(
            harness.state().tab().output.measured(),
            harness.state().tab().output.len()
        );
        assert!(harness.query_by_label("Latest lines").is_none());
    }

    #[test]
    fn gauges_are_shown_in_the_order_of_the_editor() {
        let mut harness = window(None);
        harness.set_size(egui::vec2(1400.0, 800.0));
        let app = harness.state_mut().tab_mut();
        let gauge = |name: &str| crate::rules::Gauge {
            name: name.into(),
            value: format!("Char.{name}"),
            ..crate::rules::Gauge::starter()
        };
        app.rules.gauges = vec![gauge("HP"), gauge("MP"), gauge("Gold")];
        // The MUD reports them in an order of its own.
        for name in ["Gold", "HP", "MP"] {
            app.gauges.set(name, Some((1.0, 2.0)));
        }
        harness.run_steps(3);
        assert_eq!(
            harness.state().tab().gauges.summary(),
            "HP 1 of 2, MP 1 of 2, Gold 1 of 2"
        );
        // Moved in the editor with Alt and an arrow, and saved, the panel
        // follows.
        let app = harness.state_mut().tab_mut();
        app.open_automation(automation::GAUGES);
        app.select_rule(automation::GAUGES, 2);
        harness.run_steps(3);
        harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowUp);
        harness.run_steps(3);
        harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowUp);
        harness.run_steps(3);
        let names = |app: &Tab| -> Vec<String> {
            (app.rules_draft.gauges.iter())
                .map(|gauge| gauge.name.clone())
                .collect()
        };
        assert_eq!(names(harness.state().tab()), ["Gold", "HP", "MP"]);
        // At the top it stays; down it goes, and no further than the end.
        harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowUp);
        harness.run_steps(3);
        assert_eq!(names(harness.state().tab()), ["Gold", "HP", "MP"]);
        harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowDown);
        harness.run_steps(3);
        assert_eq!(names(harness.state().tab()), ["HP", "Gold", "MP"]);
        // Dropped before another, as dragging does.
        let app = harness.state_mut().tab_mut();
        app.move_gauge(2, 0);
        assert_eq!(names(app), ["MP", "HP", "Gold"]);
        app.close_automation();
        harness.run_steps(3);
        assert_eq!(
            harness.state().tab().gauges.summary(),
            "MP 1 of 2, HP 1 of 2, Gold 1 of 2"
        );
    }

    #[test]
    fn what_arrives_is_kept_only_while_it_is_looked_at() {
        let mut harness = window(None);
        let (events, mut sent) = fake_connection(harness.state_mut().tab_mut());
        let vitals = || Event::Gmcp {
            package: "Char.Vitals".into(),
            data: r#"{"hp": 5}"#.into(),
        };
        // With the debugging view closed, nothing is kept.
        events.send(vitals()).unwrap();
        harness.run_steps(3);
        assert!(harness.state().tab().data_seen.is_empty());
        // Open, it is: by the protocol each thing came by. The connection
        // is told to say what it otherwise passes over.
        harness.state_mut().tab_mut().set_view(2);
        harness.run_steps(2);
        let told: Vec<Command> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
        assert!(matches!(told.last(), Some(Command::Trace(true))));
        events.send(vitals()).unwrap();
        let tag = Event::Other {
            protocol: "MXP".into(),
            name: "send".into(),
            data: "<send href=\"look\">".into(),
        };
        events.send(tag).unwrap();
        events
            .send(Event::Protocol {
                name: "GMCP".into(),
                on: true,
            })
            .unwrap();
        harness.run_steps(3);
        let kept = |app: &Tab| -> Vec<String> {
            (app.data_seen.iter())
                .map(|seen| format!("{} {}", seen.protocol, seen.name))
                .collect()
        };
        assert_eq!(
            kept(harness.state().tab()),
            ["GMCP Char.Vitals", "MXP send"]
        );
        harness.get_by_label("GMCP: In use");
        // Choosing a protocol lists what came by it alone.
        harness.get_by_label("MXP: Not offered by this MUD").click();
        harness.run_steps(3);
        assert!(harness.query_by_label("Char.Vitals").is_none());
        // In the list, and laid out beside it.
        assert!(harness.query_all_by_label("send").count() >= 2);
        // Closed again, it stops, unless asked to go on.
        harness.state_mut().tab_mut().set_view(0);
        harness.run_steps(2);
        events.send(vitals()).unwrap();
        harness.run_steps(3);
        assert_eq!(harness.state().tab().data_seen[0].count, 1);
        harness.state_mut().tab_mut().data_keep = true;
        events.send(vitals()).unwrap();
        harness.run_steps(3);
        assert_eq!(harness.state().tab().data_seen[0].count, 2);
    }

    #[test]
    fn the_server_is_told_how_much_text_fits() {
        let mut harness = window(None);
        harness.set_size(egui::vec2(1100.0, 700.0));
        let (_events, mut sent) = fake_connection(harness.state_mut().tab_mut());
        harness.state_mut().tab_mut().window_untold = false;
        // Once the size has been the same for a moment, it is told.
        let mut told = |harness: &mut Harness<'_, App>| {
            for _ in 0..40 {
                harness.run_steps(2);
                while let Ok(command) = sent.try_recv() {
                    if let Command::Resize(wide, tall) = command {
                        return (wide, tall);
                    }
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            panic!("the size was not told");
        };
        let first = told(&mut harness);
        assert!(
            first.0 > 40 && first.1 > 10,
            "{first:?} is not the size of the text"
        );
        // A wider and shorter window fits more across and less down.
        harness.set_size(egui::vec2(1500.0, 500.0));
        let second = told(&mut harness);
        assert!(
            second.0 > first.0 && second.1 < first.1,
            "{first:?} then {second:?}"
        );
    }

    #[test]
    fn modules_of_gmcp_are_typed_with_commas_and_get_a_version() {
        assert_eq!(
            modules_typed("Comm, Char.Ficha 2 ,, Char.Stats"),
            ["Comm 1", "Char.Ficha 2", "Char.Stats 1"]
        );
        assert!(modules_typed("  ").is_empty());
    }

    #[test]
    fn sounds_a_mud_asks_for_are_played_from_the_players_own_or_fetched() {
        let dir = std::env::temp_dir().join(format!("muddie-media-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let profiles = Profiles::at(dir.clone());
        let sounds = profiles.sound_dirs(None).remove(0);
        std::fs::create_dir_all(&sounds).unwrap();
        std::fs::write(sounds.join("own.wav"), b"RIFF").unwrap();
        let mut harness = window(Some(profiles));
        let app = harness.state_mut().tab_mut();
        // One the player has is found there, whatever address is given.
        let found = app.media_file("own.wav", Some("https://mud.example.org/s"));
        assert_eq!(found, Some((String::new(), Some(sounds.join("own.wav")))));
        // One they do not have is on the web, at the address the message
        // gives, or the one given for all of them.
        assert_eq!(app.media_file("far.wav", None), None);
        app.media(crate::media::Media::Default(
            "https://mud.example.org/sounds/".into(),
        ));
        let (address, here) = app.media_file("far.wav", None).unwrap();
        assert_eq!(address, "https://mud.example.org/sounds/far.wav");
        assert_eq!(here, None);
        // Nothing is looked for outside its folder, nor that is no sound.
        assert_eq!(app.media_file("../own.wav", None), None);
        assert_eq!(app.media_file("/etc/passwd", None), None);
        assert_eq!(app.media_file("script.sh", None), None);
        // With fetching switched off, nothing is asked of the web; nor is
        // what could not be had before.
        app.settings.media_downloads = false;
        app.media_fetch(address.clone(), None);
        assert!(app.media_jobs.is_empty());
        app.settings.media_downloads = true;
        app.media_missing.push(address.clone());
        app.media_fetch(address, None);
        assert!(app.media_jobs.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_sound_of_the_sound_protocol_is_found_by_kind_and_by_pattern() {
        let dir = std::env::temp_dir().join(format!("muddie-msp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let profiles = Profiles::at(dir.clone());
        let sounds = profiles.sound_dirs(None).remove(0);
        std::fs::create_dir_all(sounds.join("combat")).unwrap();
        for name in [
            "combat/hit1.wav",
            "combat/hit2.wav",
            "combat/miss.wav",
            "door.wav",
        ] {
            std::fs::write(sounds.join(name), b"RIFF").unwrap();
        }
        let mut harness = window(Some(profiles));
        let app = harness.state_mut().tab_mut();
        // A name with a star is one of the files it stands for.
        let found = app.sound_among("combat/hit*.wav").unwrap();
        assert!(
            ["combat/hit1.wav", "combat/hit2.wav"].contains(&found.as_str()),
            "{found}"
        );
        assert_eq!(app.sound_among("d??r.wav").as_deref(), Some("door.wav"));
        assert_eq!(app.sound_among("combat/parry*.wav"), None);
        // One without is looked for as it is, and none out of the folder.
        assert_eq!(app.sound_among("door.wav"), None);
        assert_eq!(app.sound_among("../*.wav"), None);
        // Stopping may say where files are fetched from when none is given.
        app.msp(Msp::Stop {
            music: true,
            url: Some("http://mud.example/m/".into()),
        });
        assert_eq!(app.media_base.as_deref(), Some("http://mud.example/m/"));
        std::fs::remove_dir_all(&dir).unwrap();
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
                crate::i18n::set_language("en");
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
            app.channel_line(&line);
        }
        app.show_in_panel("Minimapa", Some("  #\n#-\x1b[1;33m@\x1b[0m-#\n  #"));
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
        settings.state_mut().open_settings(1);
        settings.run_steps(4);
        save(&mut settings, "settings");
        settings.state_mut().open_settings(0);
        settings.run_steps(4);
        save(&mut settings, "settings-general");
        // The same text in each of the fonts that come with the program.
        for (name, _) in crate::theme::BUNDLED_FONTS {
            let mut fonts = connected_demo_window();
            let ctx = fonts.ctx.clone();
            fonts
                .state_mut()
                .use_font(&ctx, &crate::theme::bundled_font(name));
            fonts.run_steps(4);
            save(&mut fonts, &format!("font-{}", name.replace(' ', "")));
        }
        // The plugins, with one installed and enabled and one not.
        let dir = std::env::temp_dir().join(format!("muddie-shot-plugins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for plugin in ["aardwolf-mapper", "combat-gauges"] {
            std::fs::create_dir_all(dir.join("packages").join(plugin)).unwrap();
            std::fs::write(dir.join("packages").join(plugin).join("init.luau"), "").unwrap();
        }
        let profiles = Profiles::at(dir.clone());
        profiles.save("Aardwolf", &Profile::default()).unwrap();
        let mut plugins = window(Some(profiles));
        let app = plugins.state_mut().tab_mut();
        app.profile = Some("Aardwolf".into());
        app.packages = vec!["aardwolf-mapper".into()];
        plugins.state_mut().open_settings(5);
        plugins.run_steps(4);
        save(&mut plugins, "plugins");
        std::fs::remove_dir_all(&dir).unwrap();
        // The automation window, with rules of each kind to look at.
        let mut rules = demo_window();
        rules.set_size(egui::vec2(1500.0, 900.0));
        let app = rules.state_mut().tab_mut();
        app.rules.triggers.push(Trigger {
            name: "Tells".into(),
            group: "Social".into(),
            enabled: true,
            pattern: "tells you".into(),
            speak: "a tell".into(),
            highlight: "yellow".into(),
            ..Trigger::default()
        });
        app.rules.triggers.push(Trigger {
            name: "Fight back".into(),
            group: "Combat".into(),
            enabled: true,
            pattern: r"^(\w+) attacks you".into(),
            regex: true,
            send: "kill %1\nshield".into(),
            sound: "alert.ogg".into(),
            lua: true,
            script: "-- Only the dangerous ones\nsend(\"kill \" .. m[1])\nlocal feared = {Orc = true, Troll = true}\nif feared[m[1]] and gmcp(\"Char.Vitals\").hp < 40 then\n\tsend(\"flee\")\n\techo(\"Too weak for \" .. m[1])\nend".into(),
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
        // The same with a mistake in the script, and then as commands.
        let app = rules.state_mut().tab_mut();
        let good = app.rules_draft.triggers[1].script.clone();
        app.rules_draft.triggers[1].script =
            good.replace("then\n\tsend(\"flee\")", "\n\tsend(\"flee\"");
        rules.run_steps(3);
        save(&mut rules, "mistake");
        let app = rules.state_mut().tab_mut();
        app.rules_draft.triggers[1].script = good;
        app.rules_draft.triggers[1].lua = false;
        rules.run_steps(3);
        save(&mut rules, "commands");
        rules.state_mut().tab_mut().rules_draft.triggers[1].lua = true;
        let app = rules.state_mut().tab_mut();
        app.rules_section = 2;
        rules.run_steps(3);
        save(&mut rules, "macros");
        let app = rules.state_mut().tab_mut();
        app.rules_draft.timers.push(crate::rules::Timer {
            name: "Guardar".into(),
            tenths: 3000,
            send: "save".into(),
            ..crate::rules::Timer::default()
        });
        app.rules_section = 3;
        rules.run_steps(3);
        save(&mut rules, "timers");
        let app = rules.state_mut().tab_mut();
        app.scripts
            .set_variable("objetivo", Some(serde_json::json!("orco jefe")))
            .unwrap();
        app.scripts
            .set_variable("muertes", Some(serde_json::json!(12)))
            .unwrap();
        app.scripts
            .set_variable(
                "enemigo",
                Some(serde_json::json!({"nombre": "troll", "vida": 340, "jefe": true})),
            )
            .unwrap();
        app.scripts
            .set_variable("ruta", Some(serde_json::json!(["norte", "norte", "este"])))
            .unwrap();
        app.rules_section = 5;
        app.rules_selected[5] = 0;
        rules.run_steps(3);
        save(&mut rules, "variables");
        rules.state_mut().tab_mut().rules_selected[5] = 3;
        rules.run_steps(3);
        save(&mut rules, "variable-array");
        let app = rules.state_mut().tab_mut();
        app.saw_data(
            "Char.Vitals",
            false,
            r#"{"health": {"current": 40, "max": 100}}"#,
        );
        app.saw_data("Char.Vitals", false, r#"{"health": {"current": 62, "max": 100}, "stamina": {"current": 80, "max": 90}, "focus": {"current": 12, "max": 30}}"#);
        app.saw_data(
            "Map.Render",
            false,
            r##"{"width": 5, "height": 3, "lines": ["  #  ", "#-@-#", "  #  "]}"##,
        );
        app.saw_data(
            "Room.Info",
            false,
            r#"{"num": 2, "name": "Plaza Mayor", "exits": {"n": 5, "e": 3}}"#,
        );
        app.saw("MXP", "send", r#""<send href=\"mirar\">""#);
        app.protocols = vec![
            ("GMCP".into(), true),
            ("MXP".into(), true),
            ("MSSP".into(), true),
            ("ATCP".into(), false),
        ];
        app.set_view(2);
        rules.run_steps(3);
        save(&mut rules, "data");
        // The menus, as someone using Spanish sees them.
        for menu in ["Sesión", "Preferencias"] {
            let mut menus = connected_demo_window();
            crate::i18n::set_language("es");
            menus.run_steps(2);
            menus.get_by_label(menu).click();
            menus.run_steps(4);
            save(&mut menus, &format!("menu-{menu}"));
            crate::i18n::set_language("en");
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
        // The layout mode, and the gauges made from it.
        let mut layout = connected_demo_window();
        layout.set_size(egui::vec2(1500.0, 900.0));
        let app = layout.state_mut().tab_mut();
        app.editing_layout = true;
        // The chat at the right, under the map, sharing its side.
        let mut chat = app.layout.panels.remove(0);
        chat.side = layout::Side::Right;
        app.layout.panels.push(chat);
        // And two panels of text under it in one place, as tabs.
        app.add_to_panel(
            "Combate",
            &panels::styled_lines("\x1b[31mUn orco te ataca\x1b[0m con su hacha.").remove(0),
        );
        app.add_to_panel(
            "Tells",
            &panels::styled_lines("\x1b[33mAna te dice: ¿vienes?\x1b[0m").remove(0),
        );
        for panel in &mut app.layout.panels {
            if panel.kind == layout::Kind::Text {
                panel.side = layout::Side::Right;
                panel.stack = 1;
            }
        }
        app.layout.panels.retain(|panel| panel.name != "Minimapa");
        layout.run_steps(4);
        save(&mut layout, "layout");
        let app = layout.state_mut().tab_mut();
        let gauges = app
            .layout
            .panels
            .iter()
            .find(|panel| panel.kind == layout::Kind::Gauges)
            .unwrap()
            .id;
        app.open_gauges(gauges);
        app.rules_draft.gauges.push(crate::rules::Gauge {
            name: "Salud".into(),
            value: "Char.Vitals.health.current".into(),
            max: "Char.Vitals.health.max".into(),
            colour: "green".into(),
            steps: vec![
                crate::rules::Step {
                    under: 60,
                    colour: "yellow".into(),
                },
                crate::rules::Step {
                    under: 30,
                    colour: "red".into(),
                },
            ],
            ..crate::rules::Gauge::starter()
        });
        app.rules_draft.gauges.push(crate::rules::Gauge {
            name: "Envenenado".into(),
            value: "Char.Status.poisoned".into(),
            kind: crate::rules::GaugeKind::Light,
            colour: "red".into(),
            off_colour: "green".into(),
            ..crate::rules::Gauge::starter()
        });
        app.rules_selected[4] = 0;
        layout.run_steps(4);
        save(&mut layout, "gauges");
        layout.state_mut().tab_mut().rules_selected[4] = 1;
        layout.run_steps(4);
        save(&mut layout, "gauge-light");
        // The palette of commands over the command line, and an answer.
        let mut palette = connected_demo_window();
        let app = palette.state_mut().tab_mut();
        app.scripts.exec(
            "t",
            r#"bind("f9", "kick", {group = "Combate"}) bind("f8", "hi", {group = "Social"})"#,
        );
        app.toggle_palette();
        palette.run_steps(8);
        save(&mut palette, "palette");
        palette.state_mut().tab_mut().palette_query = "map".into();
        palette.run_steps(4);
        save(&mut palette, "palette-typed");
        palette.state_mut().tab_mut().palette_query = "gr".into();
        palette.run_steps(3);
        palette.key_press(Key::Enter);
        palette.run_steps(4);
        save(&mut palette, "answer");
        palette.key_press(Key::Enter);
        palette.run_steps(3);
        save(&mut palette, "bar-note");
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
