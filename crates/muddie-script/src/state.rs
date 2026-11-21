//! What the scripts have registered: their rules, keys, timers and data
//! handlers, and what they remember.

use super::*;

pub(super) struct Matcher {
    pub(crate) id: u64,
    /// The script that made it, which takes it away when run again.
    pub(crate) owner: String,
    pub(crate) regex: Regex,
    pub(crate) callback: Function,
    /// What it is switched on and off by.
    pub(crate) belongs: Belongs,
    /// Removed after firing once.
    pub(crate) once: bool,
}

pub(super) enum Binding {
    Command(String),
    Function(Function),
}

/// What a key does, for whom: the script that bound it, and what it is
/// switched by. A key can be bound several times, to do one thing while
/// one is on and another while another is: the first on does.
pub(super) struct Bound {
    pub(crate) owner: String,
    pub(crate) belongs: Belongs,
    pub(crate) binding: Binding,
}

pub(super) struct Timer {
    pub(crate) id: u64,
    pub(crate) owner: String,
    /// What it is switched by; it lets its turn pass while it is off.
    pub(crate) belongs: Belongs,
    pub(crate) due: Instant,
    /// `Some` for repeating timers.
    pub(crate) period: Option<Duration>,
    pub(crate) callback: Function,
}

/// A callback for GMCP or MSDP data.
pub(super) struct DataHandler {
    pub(crate) id: u64,
    pub(crate) owner: String,
    /// Key of the data it wants, as produced for the data store. For GMCP it
    /// also matches packages under it.
    pub(crate) key: String,
    pub(crate) callback: Function,
}

/// Store key of a client event; like [`msdp_key`], it cannot collide with
/// a GMCP package.
pub(super) fn event_key(name: &str) -> String {
    format!("event {}", name.to_lowercase())
}

/// Store key of an MSDP variable. The space cannot occur in a GMCP package
/// name, so the two kinds never collide.
pub(super) fn msdp_key(variable: &str) -> String {
    format!("msdp {}", variable.to_lowercase())
}

/// State shared between the engine and the functions scripts call.
#[derive(Default)]
pub(super) struct State {
    pub(crate) last_id: u64,
    /// The script whose code is running: what it makes is its own, to
    /// be taken away when that script alone is run again.
    pub(crate) owner: String,
    /// How many characters fit across and down each panel that shows a
    /// drawing, by its name in lower case.
    pub(crate) panel_sizes: HashMap<String, (u32, u32)>,
    pub(crate) triggers: Vec<Matcher>,
    /// The triggers' patterns in one set, to find in one pass which may
    /// match a line.
    pub(crate) trigger_set: TriggerSet,
    /// Patterns as they were compiled, kept for when the rules are put to
    /// work again: each takes a while, and most are as they were.
    pub(crate) compiled: HashMap<String, regex::Regex>,
    pub(crate) aliases: Vec<Matcher>,
    pub(crate) timers: Vec<Timer>,
    pub(crate) data_handlers: Vec<DataHandler>,
    /// The GMCP modules each script asked for, as `(owner, "Name version")`.
    pub(crate) supports: Vec<(String, String)>,
    /// Data last received per GMCP package or MSDP variable.
    pub(crate) data: HashMap<String, Value>,
    /// The server's MSSP status, as `(name, value)` pairs.
    pub(crate) mssp: Vec<(String, String)>,
    pub(crate) actions: Vec<Action>,
    /// Set by `map.native(false)`.
    pub(crate) map_native_off: bool,
    /// Set by `map.persist(false)`.
    pub(crate) map_memory_only: bool,
    /// Set by `gag()` while a line's triggers run.
    pub(crate) gag: bool,
    /// The line whose triggers are running, with its styles.
    pub(crate) current_line: Option<Line>,
    /// Byte range of the text the running trigger matched.
    pub(crate) current_match: Option<(usize, usize)>,
    /// What each key does, by its normalised name.
    pub(crate) bindings: HashMap<String, Vec<Bound>>,
    /// Values kept between sessions, and the file they are saved in.
    pub(crate) memory: serde_json::Map<String, serde_json::Value>,
    pub(crate) memory_file: Option<PathBuf>,
    /// What is switched off for the session.
    pub(crate) switches: Switches,
    /// The modules asked for or the switches changed since the frontend
    /// last asked.
    pub(crate) changed: bool,
    /// The scripts there are, as the frontend tells: to list them and
    /// their tags, as it is the frontend that runs them.
    pub(crate) scripts: Vec<Belongs>,
    /// Set by `separator()`: `Some(None)` turns command splitting off.
    pub(crate) separator: Option<Option<String>>,
    /// Folders `require` looks in: those scripts were loaded from.
    pub(crate) module_dirs: Vec<PathBuf>,
    /// What each required module returned.
    pub(crate) modules: HashMap<String, Value>,
    /// Modules being loaded, to refuse circular requires.
    pub(crate) loading: Vec<String>,
    /// When the script currently running must be stopped.
    pub(crate) deadline: Option<Instant>,
}

/// A match captured before its callback runs, so callbacks are free to add
/// or remove triggers while a line is being dispatched.
pub(super) struct Hit {
    pub(crate) id: u64,
    pub(crate) owner: String,
    pub(crate) once: bool,
    /// Byte range of the whole match.
    pub(crate) range: (usize, usize),
    pub(crate) callback: Function,
    pub(crate) groups: Vec<Option<String>>,
    pub(crate) named: Vec<(String, String)>,
}

/// The triggers' patterns in one set, by the ids of the triggers it is made
/// of. With many patterns it takes a moment to make, so it is made on a
/// thread of its own; until it is ready, lines are tried on each trigger,
/// with the same result, only slower.
#[derive(Default)]
pub(crate) struct TriggerSet {
    pub(crate) ids: Vec<u64>,
    /// None if the patterns could not be put in one set, or until it is.
    pub(crate) set: Option<regex::RegexSet>,
    pub(crate) making: Option<std::sync::mpsc::Receiver<Option<regex::RegexSet>>>,
}
