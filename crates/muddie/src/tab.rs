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
pub use messages::Notice;

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
    /// What the panels are doing: what each shows, the one being dragged, where
    /// each is, which tab of a place is shown, the channels, and the layout as
    /// last saved.
    panels: layout::Panels,
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
    /// Searching the output: whether the bar is open, and what is searched for.
    find: input::Find,
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
    /// The palette of commands: whether it is open, what is typed in it, what
    /// it lists, and what the last command answered.
    palette: commands::Palette,
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
    /// What the client said, to be read again, and how many of those have not
    /// been looked at.
    notices: messages::Notices,
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
    /// What the session asked the window to do, for it to take.
    app_request: Option<commands::AppRequest>,
    /// Whether the command line's cursor is to go to the end of what was
    /// put in it.
    cursor_to_end: bool,
    /// What checking the scripts being edited said.
    checks: automation::Checks,
    /// The view of what the MUD sends: what was seen, which is chosen, how it
    /// is filtered, and whether it is kept outside the view.
    data_view: data::DataView,
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
    /// How big the panels and the window are, in characters, and what was told
    /// of it.
    sizes: panel_sizes::Sizes,
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
    /// The sounds being fetched for the MUD, where it said they are, and those
    /// that could not be had.
    fetching: media::Fetching,
    /// What the MUD named with MXP, and the gauges that follow those values.
    mxp_state: mxp::MxpState,
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
            panels: layout::Panels::default(),
            dispatched: None,
            map,
            map_view: MapView::default(),
            walk: None,
            unread: false,
            settings: settings.clone(),
            log: None,
            sound: Player::default(),
            completion: Completion::default(),
            find: input::Find::default(),
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
            palette: commands::Palette::default(),
            protocols: Vec::new(),
            script_ran: None,
            reconnect_wanted: false,
            prefs: crate::profile::SessionPrefs::default(),
            profile_written: false,
            connected_with: None,
            undo: None,
            gauge_here: None,
            extras_added: (usize::MAX, Vec::new()),
            notices: messages::Notices::default(),
            app_request: None,
            cursor_to_end: false,
            checks: automation::Checks::default(),
            data_view: data::DataView::default(),
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
            sizes: panel_sizes::Sizes::default(),
            address: String::new(),
            announce_ready: false,
            rules_section: 0,
            encoding: "auto".into(),
            included_on: Vec::new(),
            included_off: Vec::new(),
            included_declined: Vec::new(),
            plugin_offer: None,
            package_jobs: Vec::new(),
            fetching: media::Fetching::default(),
            mxp_state: mxp::MxpState::default(),
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
        for (_, content) in &mut self.panels.text {
            content
                .log
                .restyle(settings.scrollback_lines, settings.timestamps);
        }
        for channel in &mut self.panels.channels {
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
