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
use muddie_mapper::{Map, Progress, RoomId, Walk};
use muddie_script::{Action, Engine, Kind, Switches};
use tokio::sync::mpsc::UnboundedSender;

use crate::complete::{self, Completion};
use crate::history::History;
use crate::i18n::{tr, trf, trn};
use crate::keypad;
use crate::logger::SessionLog;
use crate::mapview::{MapAsk, MapView};
use crate::output::{Output, server_job};
use crate::panels::{self, Gauges};
use crate::profile::{Profile, Profiles};
use crate::rules::Rules;
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
mod language;
mod layout;
mod layout_view;
mod look;
mod media;
mod messages;
mod mxp;
mod panel_sizes;
mod places;
mod plugins;
mod reader;
mod reviewing;
mod scripts;
mod switching;
#[cfg(test)]
mod tests;
mod unprofiled;
mod view;
mod walking;
mod welcome;

pub use messages::Message;

pub use commands::AppRequest;
pub use view::View;

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
    /// The last the server said asks for a password, though it does not
    /// hide what is typed: what is typed next is kept as secret.
    asks_password: bool,
    /// The command line had the keyboard in the frame before.
    line_focused: bool,
    history: History,
    voice: Voice,
    scripts: Engine,
    gauges: Gauges,
    /// The panels around the output, as they are and as they were saved.
    layout: layout::Layout,
    /// What the panels are doing: what each shows, the one being dragged, where
    /// each is, which tab of a place is shown, the channels, and the layout as
    /// last saved.
    panels: layout::Panels,
    /// The line the triggers were last given, for those that show it in
    /// a panel.
    dispatched: Option<Line>,
    /// The session is the one on screen: only it is read aloud.
    on_screen: bool,
    /// Shared with the script engine, which reads and edits it. No borrow
    /// may be held while calling into `scripts`.
    map: Rc<RefCell<Map>>,
    map_view: MapView,
    /// The route being followed, if a walk is under way, and when it last
    /// made progress.
    walk: Option<(Walk, Instant)>,
    /// The map: what the server offered of its own this session, the one
    /// being taken, and whether the user was told the map lost track.
    map_source: walking::MapSource,
    /// What the map changed of itself and is yet to be told, from when the
    /// first of it was: told together a moment later.
    map_told: (Vec<muddie_mapper::Changed>, Option<Instant>),
    /// How a room looks, being chosen, while its dialog is open.
    look: Option<look::Look>,
    /// The area asked to be tidied, while the dialog asks whether to.
    tidy_asked: Option<String>,
    /// The view where the panels are laid out.
    layout_view: layout_view::LayoutView,
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
    /// The editor: whether it is open, the rules and the script being edited,
    /// what is chosen and filtered in it, and what it is in the middle of.
    editor: automation::Editor,
    /// The connection window is showing.
    /// A connection has been asked for at least once; until then the
    /// session shows the welcome screen.
    started: bool,
    /// The saved profiles with where each connects, for the welcome
    /// screen; `None` until read, and again after one is saved.
    welcome_profiles: Option<Vec<(String, String)>>,
    /// The palette of commands: whether it is open, what is typed in it, what
    /// it lists, and what the last command answered.
    palette: commands::Palette,
    /// The protocols the MUD has offered in this connection, and whether
    /// each is in use.
    protocols: Vec<(String, bool)>,
    /// What the client said, to be read again, and how many of those have not
    /// been looked at.
    messages: messages::Messages,
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
    /// The layout and the rules of sessions without a profile were asked
    /// for, knowing whose they are.
    unprofiled_seen: bool,
    /// Whether the command line's cursor is to go to the end of what was
    /// put in it.
    cursor_to_end: bool,
    /// The output as a read-only text, while the keyboard is in it.
    reader: reader::Reader,
    /// The command line is to take the keyboard.
    focus_line: bool,
    /// The view of what the MUD sends: what was seen, which is chosen, how it
    /// is filtered, and whether it is kept outside the view.
    data_view: data::DataView,
    /// The session has no saved profile to keep its variables in, so
    /// they are carried from one start of the scripts to the next.
    variables_passing: bool,
    /// The command left in the command line after it was sent, while it
    /// is still untouched; and whether it has yet to be selected.
    kept_input: Option<String>,
    select_input: bool,
    /// The GMCP modules the connection was last told are wanted.
    supports: Vec<String>,
    /// The switches the player's scripts were last run or stopped by.
    scripts_synced: Option<Switches>,
    /// The errors the scripts gave since they were last restarted.
    script_errors: Vec<String>,
    /// How big the panels and the window are, in characters, and what was told
    /// of it.
    sizes: panel_sizes::Sizes,
    /// The address typed on the welcome screen.
    address: String,
    /// Label of the text encoding the server uses; `auto` for most.
    encoding: String,
    /// The language the MUD is played in, and who said which.
    language: language::GameLanguage,
    /// The plugins that come with Muddie: those switched on, those
    /// switched off, those declined for this MUD, and the one being
    /// offered for it now.
    plugins_on: Vec<String>,
    plugins_off: Vec<String>,
    plugins_declined: Vec<String>,
    /// The plugins that come with Muddie installed in the profile, each
    /// with the mark of what it was made of.
    plugins_installed: std::collections::BTreeMap<String, String>,
    plugin_offer: Option<crate::plugins::Plugin>,
    /// The plugins there are, as last looked for.
    plugins: Vec<crate::plugins::Plugin>,
    /// Plugins being downloaded or updated; the flag says it is one being
    /// downloaded, to be switched on when its job ends well.
    plugin_jobs: Vec<(Receiver<crate::plugins::git::Outcome>, bool)>,
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
    /// Whether the MUD was last told a screen reader reads the session.
    told_screen_reader: bool,
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
            profile_name: String::new(),
            extra_scripts: startup.scripts,
            pending_partial: None,
            settled_partial: None,
            connection: None,
            output: Output::default(),
            input: String::new(),
            input_hidden: false,
            asks_password: false,
            line_focused: false,
            history: History::default(),
            voice: Voice::default(),
            scripts: Engine::new(Rc::clone(&map)).expect("failed to start the script runtime"),
            gauges: Gauges::default(),
            layout: layout::Layout::default(),
            panels: layout::Panels::default(),
            dispatched: None,
            on_screen: true,
            map,
            map_view: MapView::default(),
            walk: None,
            map_source: Default::default(),
            map_told: Default::default(),
            look: None,
            tidy_asked: None,
            layout_view: Default::default(),
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
            editor: automation::Editor::default(),
            started: false,
            welcome_profiles: None,
            palette: commands::Palette::default(),
            protocols: Vec::new(),
            reconnect_wanted: false,
            prefs: crate::profile::SessionPrefs::default(),
            profile_written: false,
            connected_with: None,
            messages: messages::Messages::default(),
            app_request: None,
            unprofiled_seen: false,
            cursor_to_end: false,
            reader: reader::Reader::default(),
            focus_line: false,
            data_view: data::DataView::default(),
            variables_passing: false,
            kept_input: None,
            select_input: false,
            supports: Vec::new(),
            scripts_synced: None,
            script_errors: Vec::new(),
            sizes: panel_sizes::Sizes::default(),
            address: String::new(),
            encoding: "auto".into(),
            language: language::GameLanguage::default(),
            plugins_on: Vec::new(),
            plugins_off: Vec::new(),
            plugins_declined: Vec::new(),
            plugins_installed: Default::default(),
            plugin_offer: None,
            plugins: crate::plugins::all(None),
            plugin_jobs: Vec::new(),
            fetching: media::Fetching::default(),
            mxp_state: mxp::MxpState::default(),
            character: String::new(),
            new_password: String::new(),
            login: Login::Idle,
            keypad_pressed: Vec::new(),
            told_screen_reader: false,
        };
        app.share_layout(settings);
        // Speech first: what is said from here on, connecting included, is
        // heard.
        app.voice.configure(settings);
        if startup.speech {
            app.voice.force_voice();
        }
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
        self.voice.configure(settings);
    }
}

impl Tab {
    /// Runs the session for a frame, on screen or not: events from the
    /// server, timers, and anything waiting on the clock.
    pub fn poll(&mut self, ctx: &egui::Context, on_screen: bool) {
        self.on_screen = on_screen;
        self.voice.hush_on_ctrl(ctx);
        let lines_before = self.output.total_pushed();
        if self.handle_events() {
            ctx.request_repaint();
        }
        let now = Instant::now();
        self.scripts.tick(now);
        self.run_actions();
        self.check_walk(ctx);
        self.check_map_imported(ctx);
        self.check_newer_maps(ctx);
        self.tell_map_changes(ctx, now);
        self.check_partial(ctx);
        self.check_timers(ctx);
        self.check_plugin_jobs(ctx);
        self.tell_screen_reader();
        if let Some(deadline) = self.scripts.next_deadline() {
            ctx.request_repaint_after(deadline.saturating_duration_since(now));
        }
        self.write_down();
        self.unread = !on_screen && (self.unread || self.output.total_pushed() != lines_before);
    }
}
