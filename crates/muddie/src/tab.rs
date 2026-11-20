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
use crate::settings::Settings;
use crate::theme;
use crate::voice::Voice;
use muddie_sound::{self as sound, Msp, Player};

mod automation;
mod client_commands;
mod commands;
mod connection;
mod data;
mod events;
mod input;
mod its_profile;
mod keys;
mod layout;
mod media;
mod messages;
mod mxp;
mod panel_sizes;
mod plugins;
mod scripts;
#[cfg(test)]
mod tests;
mod view;
mod walking;

use client_commands::{COMMANDS, client_command};
use media::MediaJob;
pub use messages::Notice;
use panel_sizes::PanelSize;

pub use commands::AppRequest;

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
    /// A connection has been asked for at least once; until then the
    /// session shows the welcome screen.
    started: bool,
    /// The saved profiles with where each connects, for the welcome
    /// screen; `None` until read, and again after one is saved.
    welcome_profiles: Option<Vec<(String, String)>>,
    /// The rule selected in each section of the automation window.
    rules_selected: [usize; 8],
    /// The script files that can be edited, and the one being edited:
    /// its file, its name as typed, its text, and that text as saved.
    script_files: Vec<PathBuf>,
    script_loaded: Option<PathBuf>,
    script_name: String,
    script_text: String,
    script_saved: String,
    script_seen: String,
    /// When the script being edited last changed, to save it after.
    script_settle: crate::autosave::Settle,
    /// What the automation window's search box holds.
    rules_filter: String,
    /// The draft as last seen, and when it last changed: edits are saved
    /// once they stop for a moment.
    rules_seen: Rules,
    /// When the rules being edited last changed, to save them after.
    rules_settle: crate::autosave::Settle,
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
    /// The trigger whose details are shown, and what it was asked to do
    /// besides its commands though that is still empty.
    extras_added: (usize, Vec<automation::Extra>),
    /// The gauge whose details are open over the game.
    gauge_here: Option<usize>,
    /// What was last deleted in the editor, to have it back.
    undo: Option<automation::Undo>,
    /// What the client said, to be read again, and how many of those
    /// have not been looked at.
    notices: Vec<Notice>,
    notices_unread: usize,
    /// How its sessions go, kept with the profile: those of the settings
    /// with none.
    prefs: crate::profile::SessionPrefs,
    /// The session wrote its profile, for the settings to read it again.
    profile_written: bool,
    /// Where and how the session connected, to tell when the profile's
    /// fields say otherwise and connecting again would apply them.
    connected_with: Option<String>,
    /// The connection is being closed to be opened again.
    reconnect_wanted: bool,
    palette_open: bool,
    /// The palette opened in this frame, by a press that is not to close it.
    palette_fresh: bool,
    /// What the session asked the window to do, for it to take.
    app_request: Option<commands::AppRequest>,
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
            layout: layout::Layout::default(),
            layout_saved: layout::Layout::default(),
            panel_text: Vec::new(),
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
            started: false,
            welcome_profiles: None,
            rules_selected: [0; 8],
            script_files: Vec::new(),
            script_loaded: None,
            script_name: String::new(),
            script_text: String::new(),
            script_saved: String::new(),
            script_seen: String::new(),
            script_settle: Default::default(),
            rules_filter: String::new(),
            rules_seen: Rules::default(),
            rules_settle: Default::default(),
            rules_sample: String::new(),
            recording_key: false,
            gauge_dragged: None,
            palette_chosen: 0,
            protocols: Vec::new(),
            script_ran: None,
            reconnect_wanted: false,
            prefs: crate::profile::SessionPrefs::default(),
            profile_written: false,
            connected_with: None,
            undo: None,
            gauge_here: None,
            extras_added: (usize::MAX, Vec::new()),
            notices: Vec::new(),
            notices_unread: 0,
            palette_open: false,
            palette_fresh: false,
            app_request: None,
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
        // Without a profile, the session goes as the settings say.
        if self.profile.is_none() {
            self.prefs = crate::profile::SessionPrefs::from(settings);
        }
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
