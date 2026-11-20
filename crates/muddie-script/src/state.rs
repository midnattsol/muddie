//! What the scripts have registered: their rules, keys, timers and data
//! handlers, and what they remember.

use super::*;

pub(super) struct Matcher {
    pub(crate) id: u64,
    /// The script that made it, which takes it away when run again.
    pub(crate) owner: String,
    pub(crate) regex: Regex,
    pub(crate) callback: Function,
    /// Group it is switched on and off with, if any.
    pub(crate) group: Option<String>,
    /// Removed after firing once.
    pub(crate) once: bool,
}

pub(super) enum Binding {
    Command(String),
    Function(Function),
}

pub(super) struct Timer {
    pub(crate) id: u64,
    pub(crate) owner: String,
    /// The group it belongs to; it is passed over while that is off.
    pub(crate) group: Option<String>,
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
    /// Who bound each key, in which group.
    pub(crate) bound_by: Vec<(String, String, Option<String>)>,
    /// How many characters fit across and down each panel that shows a
    /// drawing, by its name in lower case.
    pub(crate) panel_sizes: HashMap<String, (u32, u32)>,
    pub(crate) triggers: Vec<Matcher>,
    pub(crate) aliases: Vec<Matcher>,
    pub(crate) timers: Vec<Timer>,
    pub(crate) data_handlers: Vec<DataHandler>,
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
    /// What to do for each bound key, by normalised key name.
    /// What each key does, once per group it is bound in.
    pub(crate) bindings: HashMap<String, Vec<(Option<String>, Binding)>>,
    /// Values kept between sessions, and the file they are saved in.
    pub(crate) memory: serde_json::Map<String, serde_json::Value>,
    pub(crate) memory_file: Option<PathBuf>,
    /// Groups switched off with `enable(group, false)`.
    pub(crate) disabled_groups: HashSet<String>,
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
