//! One session: its connection bar, output, panels and input line.

use std::cell::RefCell;
use std::collections::HashMap;
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
    /// The channels of the MUD's that have been heard of: each with what
    /// was said on it, and whether something was that has not been seen.
    channels: Vec<layout::Channel>,
    /// The channel each panel of channels is showing; none is all.
    channel_shown: Vec<(u32, String)>,
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
    /// When the scripts were last run from the editor, and what kept
    /// them from running, if anything did.
    script_ran: Option<(String, Option<String>)>,
    /// The connection is being closed to be opened again.
    reconnect_wanted: bool,
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
    panel_sizes: Vec<PanelSize>,
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
    /// The plugins that come with Muddie: those switched on, those
    /// switched off, those declined for this MUD, and the one being
    /// offered for it now.
    included_on: Vec<String>,
    included_off: Vec<String>,
    included_declined: Vec<String>,
    plugin_offer: Option<&'static crate::bundled::Bundled>,
    /// Package installs and updates under way; the flag says to enable
    /// the package when its job ends well.
    package_jobs: Vec<(Receiver<packages::Outcome>, bool)>,
    /// Where the MUD said its sounds are fetched from, the ones being
    /// fetched, each with what to play once it is here, and the ones
    /// that could not be had, which are not asked for again.
    media_base: Option<String>,
    media_jobs: Vec<MediaJob>,
    media_missing: Vec<String>,
    /// What the server named with MXP, and the gauges it asked for: what
    /// each is called, the values it follows, its colour and whether it
    /// is a bar.
    mxp_values: HashMap<String, String>,
    mxp_gauges: Vec<(String, String, String, String, bool)>,
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
    let maps = crate::profile::user_dir(crate::profile::UserDir::Data)?.join("maps");
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

/// A sound being fetched: where the result comes, its address, and what
/// to play once it is here.
type MediaJob = (
    Receiver<Result<PathBuf, String>>,
    String,
    Option<crate::media::Play>,
);

/// How many characters fit across and down a panel that can show a
/// drawing, what scripts were last told of it, and since when it has
/// been as it is.
struct PanelSize {
    id: u32,
    /// Whether it is showing a drawing, or is the map's: only then are
    /// scripts told when its size changes, though they can always ask.
    drawing: bool,
    cells: (u32, u32),
    told: Option<(u32, u32)>,
    settling: Option<((u32, u32), Instant)>,
}

/// What the rules made in the interface are run as: one owner, to take
/// them away together when they are run again.
const RULES: &str = "rules";

/// Things by their names, each with whether it is switched on.
type Switched = Vec<(String, bool)>;

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
            channels: Vec::new(),
            channel_shown: Vec::new(),
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
            script_ran: None,
            reconnect_wanted: false,
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
            panel_sizes: Vec::new(),
            window_cells: (0, 0),
            window_reported: None,
            window_settling: None,
            #[cfg(test)]
            window_untold: false,
            address: String::new(),
            announce_ready: false,
            rules_section: 0,
            encoding: "auto".into(),
            included_on: Vec::new(),
            included_off: Vec::new(),
            included_declined: Vec::new(),
            plugin_offer: None,
            package_jobs: Vec::new(),
            media_base: None,
            media_jobs: Vec::new(),
            media_missing: Vec::new(),
            mxp_values: HashMap::new(),
            mxp_gauges: Vec::new(),
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
        for channel in &mut self.channels {
            channel
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
                row(ui, tr("Scrollback lines"), &mut |ui| {
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
        let (saved_before, session) = self.group_states();
        // The scripts say again what they want announced and are told
        // again how big the panel is.
        self.supports.clear();
        self.completions.clear();
        self.panels_untold();
        // Variables belong to the session. With a saved profile they are
        // kept with it; without one they last as long as the session, and
        // go with it into a profile if it is then saved as one.
        let passing = std::mem::take(&mut self.variables_passing).then(|| self.scripts.variables());
        self.scripts =
            Engine::new(Rc::clone(&self.map)).expect("failed to start the script runtime");
        // The new runtime knows as much of the panels as the old one did.
        let sizes: Vec<(u32, (u32, u32))> = (self.panel_sizes.iter())
            .map(|size| (size.id, size.cells))
            .collect();
        for (id, cells) in sizes {
            self.panel_size_known(id, cells);
        }
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
        // The plugins that come with Muddie and are on, before the
        // player's own scripts, which may build on them.
        for plugin in &crate::bundled::ALL {
            if let (Some(code), true) = (plugin.code, self.included(plugin)) {
                let name = format!("{}.luau", plugin.id);
                (self.scripts).exec_owned(&crate::bundled::owner(plugin), &name, code);
            }
        }
        // The rules made in the interface, which also say which of the
        // scripts are switched off.
        if let Some(profiles) = &self.profiles {
            match Rules::load(&profiles.rules_path(self.profile.as_deref())) {
                Ok(rules) => self.rules = rules,
                Err(error) => self.note(&trf("Could not load the rules: {0}", &[&error])),
            }
        }
        // The player's own scripts that are switched off are not run;
        // a plugin has its own way of being switched off.
        let own: Vec<PathBuf> = (self.profiles.as_ref())
            .map(|profiles| profiles.editable_scripts(self.profile.as_deref()))
            .unwrap_or_default();
        files.retain(|file| {
            !own.contains(file)
                || !self
                    .rules
                    .scripts_off
                    .contains(&crate::rules::script_key(file))
        });
        for file in &files {
            self.scripts.load_file(file);
        }
        // The rules run last, as one more script.
        self.run_rules(&saved_before, session);
        self.run_actions();
    }

    /// Whether each group is on: as the rules have it saved, and as it
    /// is in this session, where a script or the player may have
    /// switched it.
    fn group_states(&self) -> (Switched, Switched) {
        let saved = (self.rules.group_names().into_iter())
            .map(|name| {
                let on = self.rules.group_enabled(&name);
                (name, on)
            })
            .collect();
        let session = (self.scripts.groups().into_iter())
            .map(|name| {
                let on = self.scripts.group_enabled(&name);
                (name, on)
            })
            .collect();
        (saved, session)
    }

    /// Runs the rules made in the interface, in place of those that were
    /// running. Groups switched for the session keep how they were left,
    /// unless what is saved for them is what changed.
    fn run_rules(&mut self, saved_before: &[(String, bool)], session: Switched) {
        self.scripts.unload(RULES);
        // Each on its own, so a mistake in one says which and spares the rest.
        for (name, code) in self.rules.chunks() {
            self.scripts.exec_owned(RULES, &name, &code);
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
    }

    /// Puts the rules as they are now saved to work, without running the
    /// scripts again: what those keep is not theirs to lose because a
    /// rule changed. A script switched on or off since is run or stopped.
    fn reload_rules(&mut self, saved: Option<Rules>) {
        let (saved_before, session) = self.group_states();
        let off_before = self.rules.scripts_off.clone();
        if let Some(rules) = saved {
            self.rules = rules;
        }
        let own: Vec<PathBuf> = (self.profiles.as_ref())
            .map(|profiles| profiles.editable_scripts(self.profile.as_deref()))
            .unwrap_or_default();
        for file in own.iter().filter(|file| file.is_file()) {
            let key = crate::rules::script_key(file);
            match (
                off_before.contains(&key),
                self.rules.scripts_off.contains(&key),
            ) {
                (false, true) => self.scripts.unload_file(file),
                (true, false) => self.scripts.load_file(file),
                _ => {}
            }
        }
        self.run_rules(&saved_before, session);
        self.run_actions();
    }

    /// Saves the rules edited in the rules window and puts them to work.
    fn save_rules(&mut self) {
        let draft = self.rules_draft.clone();
        let Some(profiles) = &self.profiles else {
            // Nowhere to keep them: they still work for this session.
            return self.reload_rules(Some(draft));
        };
        let saved = draft.save(&profiles.rules_path(self.profile.as_deref()));
        match saved {
            Ok(()) => self.reload_rules(Some(draft)),
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
            self.included_on = profile.included_on;
            self.included_off = profile.included_off;
            self.included_declined = profile.included_declined;
            self.voice.set_enabled(profile.speech);
        } else {
            self.packages.clear();
            self.included_on.clear();
            self.included_off.clear();
            self.included_declined.clear();
        }
        self.plugin_offer = None;
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
            included_on: self.included_on.clone(),
            included_off: self.included_off.clone(),
            included_declined: self.included_declined.clone(),
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
            // What many MUDs send the same way is asked for only while
            // the plugin that makes something of it is on.
            if !self.conventions() {
                let ours =
                    |module: &String| module.starts_with("Core ") || module.starts_with("Client.");
                config.gmcp_supports.retain(ours);
                config.msdp_report.clear();
            }
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

    /// Connects again: at once if there is no connection, and if there
    /// is one, once it has been closed.
    pub(super) fn reconnect(&mut self, ctx: &egui::Context) {
        if self.host.trim().is_empty() {
            return;
        }
        if self.connection.is_some() {
            self.disconnect();
            self.reconnect_wanted = true;
        } else {
            self.connect(ctx);
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

    /// Does what a link of the server's that was clicked asks for.
    pub(super) fn link_clicked(&mut self, clicked: crate::output::Clicked) {
        match clicked {
            crate::output::Clicked::Send(command) => self.send(command),
            // To be finished in the command line, with a space to go on.
            crate::output::Clicked::Prompt(command) => {
                self.input = format!("{command} ");
                self.kept_input = None;
                self.cursor_to_end = true;
            }
        }
    }

    /// Carries out what an MXP tag asked of the client besides text.
    fn mxp(&mut self, asked: muddie_core::Mxp, recording: bool) {
        use muddie_core::Mxp;
        match asked {
            // A value by a name: kept, told to the scripts, and followed
            // by the gauges the server asked for.
            Mxp::Value { name, value } => {
                if recording {
                    self.saw(
                        "MXP",
                        &name,
                        &serde_json::Value::String(value.clone()).to_string(),
                    );
                }
                let told = serde_json::json!({"name": name, "value": value}).to_string();
                self.scripts.on_event("mxp", &told);
                self.run_actions();
                self.mxp_values.insert(name.to_lowercase(), value);
                self.mxp_gauges_follow();
            }
            Mxp::Gauge {
                entity,
                max,
                caption,
                colour,
                bar,
            } => {
                let called = if caption.is_empty() {
                    entity.clone()
                } else {
                    caption
                };
                self.mxp_gauges.retain(|gauge| gauge.0 != called);
                self.mxp_gauges.push((
                    called,
                    entity.to_lowercase(),
                    max.to_lowercase(),
                    colour,
                    bar,
                ));
                self.mxp_gauges_follow();
            }
            // A frame is a panel of text, by its name.
            Mxp::Frame {
                name,
                open,
                title: _,
                align,
            } => {
                if name.trim().is_empty() {
                    return;
                }
                let known = (self.layout.panels.iter())
                    .any(|panel| panel.name.eq_ignore_ascii_case(name.trim()));
                if !known && !open {
                    return;
                }
                let id = self.panel_called(&name);
                if let Some(panel) = self.layout.panel_mut(id) {
                    panel.visible = open;
                    if !known {
                        panel.side = match align.as_str() {
                            "left" => layout::Side::Left,
                            "top" => layout::Side::Top,
                            "bottom" => layout::Side::Bottom,
                            _ => layout::Side::Right,
                        };
                    }
                }
            }
            Mxp::Clear(name) => {
                if !name.trim().is_empty() {
                    let id = self.panel_called(&name);
                    self.text_of(id).log.clear();
                }
            }
            Mxp::Sound(request) => {
                if let Some(request) = sound::parse_msp(&request) {
                    self.msp(request);
                }
            }
            Mxp::Expire(name) => {
                self.output.expire(&name);
                let ids: Vec<u32> = self.layout.panels.iter().map(|panel| panel.id).collect();
                for id in ids {
                    self.text_of(id).log.expire(&name);
                }
            }
            // Said, not done: where to connect is the player's to choose.
            Mxp::Relocate { host, port } => {
                self.note(&trf(
                    "The MUD asks to go on at {0}, port {1}.",
                    &[&host, &port],
                ));
            }
            // The name and the password of the profile, as when logging
            // in by itself.
            Mxp::User => {
                let name = self.character.trim().to_owned();
                if !name.is_empty() {
                    self.send(name);
                    self.login = Login::Password(Instant::now());
                }
            }
            Mxp::Password => {
                if !self.character.trim().is_empty() {
                    self.login = Login::Password(Instant::now());
                    self.login_password();
                }
            }
        }
    }

    /// Sets the gauges the server asked for with MXP to the values they
    /// follow, as far as those are known and are numbers.
    fn mxp_gauges_follow(&mut self) {
        for (called, entity, max, colour, bar) in self.mxp_gauges.clone() {
            let number = |name: &str| {
                let known = self
                    .mxp_values
                    .get(name)
                    .map(String::as_str)
                    .unwrap_or(name);
                known.trim().parse::<f64>().ok()
            };
            let Some(value) = number(&entity) else {
                continue;
            };
            let max = number(&max).unwrap_or(0.0);
            let look = muddie_script::GaugeLook {
                colour,
                kind: if bar && max > 0.0 { "bar" } else { "number" }.to_owned(),
                ..muddie_script::GaugeLook::default()
            };
            self.gauges
                .set_with(&called, Some((value, max)), Some(look));
        }
    }

    fn show_line(&mut self, line: &Line) {
        // What the server sent to a frame goes to the panel of its name.
        if let Some(frame) = &line.dest {
            return self.add_to_panel(&frame.clone(), line);
        }
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
                    self.offer_plugin_for(&self.host.clone(), "");
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
                    // The server has yet to hear how big the panels are,
                    // and the text of the game.
                    self.panels_untold();
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
                    // What many MUDs send the same way first, if its plugin
                    // is on, so that scripts can override it.
                    let common = self.conventions();
                    if common && package.eq_ignore_ascii_case("Char.Vitals") {
                        for (name, value, max) in panels::vitals(&data) {
                            self.gauges.set(name, Some((value, max)));
                        }
                    } else if let Some(message) = crate::media::parse(&package, &data) {
                        self.media(message);
                    } else if common && package.eq_ignore_ascii_case("Comm.Channel.Text") {
                        let name = panels::channel_name(&data);
                        self.channel_said(&name, "", panels::channel_text(&data));
                    } else if common
                        && package.eq_ignore_ascii_case("Room.Info")
                        && self.scripts.native_map()
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
                    let common = self.conventions();
                    if common
                        && let Some((name, value, max)) = self.msdp_vitals.update(&variable, &data)
                    {
                        self.gauges.set(name, Some((value, max)));
                    }
                    if common && variable == "ROOM" && self.scripts.native_map() {
                        self.entered_room(&data);
                    }
                    if recording {
                        self.saw_data(&variable, true, &data);
                    }
                    self.scripts.on_msdp(&variable, &data);
                    self.run_actions();
                }
                Event::Mssp(status) => {
                    let named = status
                        .iter()
                        .find(|(field, _)| field.eq_ignore_ascii_case("NAME"));
                    if let Some((_, name)) = named {
                        self.offer_plugin_for("", &name.clone());
                    }
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
                Event::Mxp(asked) => self.mxp(asked, recording),
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
                    self.mxp_values.clear();
                    self.mxp_gauges.clear();
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
                    // Closed to be opened again: as soon as can be.
                    if std::mem::take(&mut self.reconnect_wanted) {
                        self.reconnect_at = Some(Instant::now());
                    }
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
                Action::Supports(modules) => self.announce_modules(modules),
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
                Action::Channel { name, text, panel } => {
                    let lines = text.as_deref().map(panels::styled_lines);
                    self.channel_said(&name, &panel, lines.unwrap_or_default());
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
                    message.lines().for_each(|line| self.refuse(line));
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

    /// Whether a plugin that comes with Muddie is on in this session.
    pub(super) fn included(&self, plugin: &crate::bundled::Bundled) -> bool {
        let listed = |list: &[String]| list.iter().any(|id| id == plugin.id);
        match plugin.default_on {
            true => !listed(&self.included_off),
            false => listed(&self.included_on),
        }
    }

    /// Whether what many MUDs send the same way is made something of.
    fn conventions(&self) -> bool {
        crate::bundled::find(crate::bundled::COMMON).is_some_and(|plugin| self.included(plugin))
    }

    /// Switches a plugin that comes with Muddie on or off, at once and
    /// without running the other scripts again, and keeps that with the
    /// profile if there is one.
    pub(super) fn set_included(&mut self, plugin: &'static crate::bundled::Bundled, on: bool) {
        let id = plugin.id.to_owned();
        self.included_on.retain(|known| *known != id);
        self.included_off.retain(|known| *known != id);
        match (plugin.default_on, on) {
            (false, true) => self.included_on.push(id),
            (true, false) => self.included_off.push(id),
            _ => {}
        }
        if self.plugin_offer == Some(plugin) {
            self.plugin_offer = None;
        }
        let owner = crate::bundled::owner(plugin);
        self.scripts.unload(&owner);
        if let (Some(code), true) = (plugin.code, on) {
            let name = format!("{}.luau", plugin.id);
            self.scripts.exec_owned(&owner, &name, code);
        }
        // The one that is part of the program asks the MUD for its own.
        if plugin.id == crate::bundled::COMMON && on {
            let usual = muddie_core::Config::default().gmcp_supports;
            self.announce_modules(usual);
        }
        self.run_actions();
        self.keep_included();
    }

    /// Saves which plugins are on with the profile, if there is one;
    /// without one they are as set for as long as the session lasts.
    fn keep_included(&mut self) {
        let (Some(profiles), Some(name)) = (&self.profiles, &self.profile) else {
            return;
        };
        // What is saved of the profile, with only this changed: what is
        // being typed in the connection's form is not this one's to save.
        let saved = profiles.load(name).and_then(|mut profile| {
            profile.included_on.clone_from(&self.included_on);
            profile.included_off.clone_from(&self.included_off);
            profile
                .included_declined
                .clone_from(&self.included_declined);
            (profiles.save(name, &profile)).map_err(|error| error.to_string())
        });
        if let Err(error) = saved {
            self.note(&trf("Could not save the profile: {0}", &[&error]));
        }
    }

    /// Offers the plugin made for the MUD at `host`, or that gives itself
    /// `name`, unless it is on already or was declined for this profile.
    fn offer_plugin_for(&mut self, host: &str, name: &str) {
        let Some(plugin) = crate::bundled::for_mud(host, name) else {
            return;
        };
        let declined = self.included_declined.iter().any(|id| id == plugin.id);
        if self.included(plugin) || declined || self.plugin_offer == Some(plugin) {
            return;
        }
        self.plugin_offer = Some(plugin);
        self.voice.say(&trf(
            "There is a plugin for {0}. It is offered above the command line.",
            &[tr(plugin.name)],
        ));
    }

    /// The plugin offered for this MUD was declined: it is not offered
    /// again in this profile, and stays in the settings.
    fn decline_plugin(&mut self) {
        if let Some(plugin) = self.plugin_offer.take() {
            self.included_declined.push(plugin.id.to_owned());
            self.keep_included();
        }
    }

    /// The offer of a plugin for the MUD connected to, over the command
    /// line: what it does, and a button to switch it on and one not to.
    pub(super) fn plugin_notice(&mut self, ui: &mut egui::Ui) {
        let Some(plugin) = self.plugin_offer else {
            return;
        };
        let theme = theme::current();
        let card = egui::Frame::new()
            .fill(theme.raised)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(12, 6));
        let (mut enable, mut decline) = (false, false);
        card.show(ui, |ui| {
            ui.horizontal(|ui| {
                let what = trf("There is a plugin for {0}.", &[tr(plugin.name)]);
                ui.label(RichText::new(what).color(theme.text));
                ui.label(RichText::new(tr(plugin.about)).color(theme.text_dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    decline = crate::widgets::quiet_button(ui, tr("Not now")).clicked();
                    enable = crate::widgets::button(ui, tr("Enable")).clicked();
                });
            });
        });
        ui.add_space(4.0);
        if enable {
            self.set_included(plugin, true);
        }
        if decline {
            self.decline_plugin();
        }
    }

    /// Tells the MUD of GMCP modules it has not been told of: at once if
    /// there is a connection, and on connecting.
    fn announce_modules(&mut self, modules: Vec<String>) {
        let new: Vec<String> = modules
            .into_iter()
            .filter(|module| !self.supports.contains(module))
            .collect();
        if let (Some(connection), false) = (&self.connection, new.is_empty()) {
            let data = serde_json::to_string(&new).unwrap_or_default();
            let package = "Core.Supports.Add".to_owned();
            let _ = connection.commands.send(Command::Gmcp { package, data });
        }
        self.supports.extend(new);
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
        if ui.input_mut(|input| input.consume_key(Modifiers::ALT, Key::R)) {
            self.reconnect(&ui.ctx().clone());
        }
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

    /// Notes how many characters fit in the panel `id`, as it is drawn.
    /// Scripts can ask for it at once; they are told of a change once it
    /// has settled.
    pub(super) fn panel_measured(&mut self, id: u32, cells: (u32, u32), drawing: bool) {
        match self.panel_sizes.iter_mut().find(|size| size.id == id) {
            Some(size) => {
                size.drawing = drawing;
                if size.cells == cells {
                    return;
                }
                size.cells = cells;
            }
            None => self.panel_sizes.push(PanelSize {
                id,
                drawing,
                cells,
                told: None,
                settling: None,
            }),
        }
        self.panel_size_known(id, cells);
    }

    /// Lets scripts ask for the size of the panel `id`, by its name, and
    /// as `map` if it is the map's.
    fn panel_size_known(&mut self, id: u32, cells: (u32, u32)) {
        let Some(panel) = self.layout.panel(id) else {
            return;
        };
        self.scripts.set_panel_size(&panel.name, cells.0, cells.1);
        if panel.kind == layout::Kind::Map {
            self.scripts.set_panel_size("map", cells.0, cells.1);
        }
    }

    /// Scripts are to be told the size of every panel again.
    fn panels_untold(&mut self) {
        for size in &mut self.panel_sizes {
            size.told = None;
        }
    }

    /// Tells scripts how big each panel is once it has stopped changing,
    /// so a window being resized does not flood the server.
    pub(super) fn report_panel(&mut self, ctx: &egui::Context) {
        const SETTLED: Duration = Duration::from_millis(250);
        let mut settled = Vec::new();
        for size in &mut self.panel_sizes {
            let cells = size.cells;
            if size.told == Some(cells) || cells.0 == 0 || !size.drawing {
                size.settling = None;
                continue;
            }
            match size.settling {
                Some((settling, since)) if settling == cells && since.elapsed() >= SETTLED => {
                    size.settling = None;
                    size.told = Some(cells);
                    settled.push((size.id, cells));
                }
                Some((settling, _)) if settling == cells => ctx.request_repaint_after(SETTLED),
                _ => {
                    size.settling = Some((cells, Instant::now()));
                    ctx.request_repaint_after(SETTLED);
                }
            }
        }
        for (id, cells) in settled {
            // Which panel it is: by its name, and whether it is the map's,
            // whatever that one is called.
            let panel = self.layout.panel(id);
            let name = panel.map(|panel| panel.name.clone()).unwrap_or_default();
            let map = panel.is_some_and(|panel| panel.kind == layout::Kind::Map);
            let size = serde_json::json!({
                "name": name,
                "map": map,
                "columns": cells.0,
                "rows": cells.1,
            });
            self.scripts.on_event("panel", &size.to_string());
            self.run_actions();
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
        let cells = Self::cells(ui, room);
        self.panel_measured(id, cells, true);
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
mod tests;
