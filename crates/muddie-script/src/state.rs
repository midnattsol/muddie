//! What the scripts have registered: their rules, keys, timers and data
//! handlers, and what they remember.

use super::*;

/// A trigger's or an alias's pattern: checked when it is made, and
/// compiled the first time it is tried on a line. Most triggers are seldom
/// tried, as the set of all their patterns says which may match, and one
/// compiled, with `\w`, every letter of every language, holds some 100 KB.
pub(crate) struct Pattern {
    source: String,
    compiled: std::cell::OnceCell<Option<Regex>>,
}

impl Pattern {
    /// The pattern, if it is one; else what is wrong with it, as the
    /// regex crate says it.
    pub(crate) fn new(source: &str) -> Result<Self, String> {
        if regex_syntax::Parser::new().parse(source).is_err() {
            let wrong = Regex::new(source).err().map(|error| error.to_string());
            return Err(wrong.unwrap_or_else(|| "not a pattern".to_owned()));
        }
        Ok(Self {
            source: source.to_owned(),
            compiled: std::cell::OnceCell::new(),
        })
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.source
    }

    /// Whether it was compiled yet.
    #[cfg(test)]
    pub(crate) fn is_compiled(&self) -> bool {
        self.compiled.get().is_some()
    }

    /// The pattern compiled, the first time it is asked for; None if it
    /// cannot be, too big as it is.
    pub(crate) fn regex(&self) -> Option<&Regex> {
        self.compiled
            .get_or_init(|| Regex::new(&self.source).ok())
            .as_ref()
    }
}

pub(super) struct Matcher {
    pub(crate) id: u64,
    /// The script that made it, which takes it away when run again.
    pub(crate) owner: String,
    pub(crate) pattern: Pattern,
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
/// A gauge a script shows: its name, and what it is switched by.
pub(crate) struct Shown {
    pub(crate) owner: String,
    pub(crate) name: String,
    pub(crate) belongs: Belongs,
}

pub(super) struct Bound {
    /// What `cancel` takes it away by.
    pub(crate) id: u64,
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
    /// The tags it carries: it is told nothing while one is off.
    pub(crate) tags: Vec<String>,
}

impl DataHandler {
    /// Whether it is told what comes: none of its tags is switched off.
    pub(crate) fn on(&self, switches: &Switches) -> bool {
        let belongs = Belongs {
            place: None,
            tags: self.tags.clone(),
        };
        switches.active(Kind::Script, &belongs)
    }
}

/// Something a script offers to do with the map, which the user chooses
/// from the map's menu or the palette.
pub(super) struct MapAction {
    pub(crate) owner: String,
    /// What the user chooses it by, by language code (see `tr`).
    pub(crate) label: Vec<(String, String)>,
    /// For one that switches something on or off, whether it is on.
    pub(crate) checked: Option<bool>,
    pub(crate) callback: Function,
}

/// A function a script gave to be called once the client knows something
/// it went to find out.
pub(super) struct Awaited {
    pub(crate) id: u64,
    pub(crate) owner: String,
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

/// The key handlers of a variable of `vars` are kept under: its name as it
/// is written, for variables are told apart by case.
pub(super) fn var_key(name: &str) -> String {
    format!("var {name}")
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
    pub(crate) aliases: Vec<Matcher>,
    pub(crate) timers: Vec<Timer>,
    pub(crate) data_handlers: Vec<DataHandler>,
    /// What the scripts offer to do with the map, in the order offered.
    pub(crate) map_actions: Vec<MapAction>,
    /// What the scripts wait to be told, by the id the client answers.
    pub(crate) awaited: Vec<Awaited>,
    /// The GMCP modules each script asked for, as `(owner, "Name version")`.
    pub(crate) supports: Vec<(String, String)>,
    /// The MSDP variables each script asked to be told of, as
    /// `(owner, variable)`.
    pub(crate) reports: Vec<(String, String)>,
    /// Data last received per GMCP package or MSDP variable.
    pub(crate) data: HashMap<String, Value>,
    /// The GMCP packages that send only what changed, as `(owner, key)`:
    /// what comes is merged into what was kept.
    pub(crate) merged: Vec<(String, String)>,
    /// The server's MSSP status, as `(name, value)` pairs.
    pub(crate) mssp: Vec<(String, String)>,
    pub(crate) actions: Vec<Action>,
    /// Set by `map.native(false)`, with the script that did.
    pub(crate) map_native_off: Option<String>,
    /// Set by `map.persist(false)`.
    pub(crate) map_memory_only: bool,
    /// Set by `gag()` while a line's triggers run.
    pub(crate) gag: bool,
    /// The line whose triggers are running, with its styles.
    pub(crate) current_line: Option<Line>,
    /// Byte range of the text the running trigger matched.
    pub(crate) current_match: Option<(usize, usize)>,
    /// Words to offer when completing, as `(owner, name, words)`, in the
    /// order the names were first given.
    pub(crate) completions: Vec<(String, String, Vec<String>)>,
    /// The gauges the scripts show, by what they are switched by.
    pub(crate) gauges: Vec<Shown>,
    /// What each key does, by its normalised name.
    pub(crate) bindings: HashMap<String, Vec<Bound>>,
    /// Values kept between sessions, and the file they are saved in.
    pub(crate) memory: serde_json::Map<String, serde_json::Value>,
    pub(crate) memory_file: Option<PathBuf>,
    /// The variables changed since their handlers were last told, in the
    /// order they changed.
    pub(crate) vars_changed: Vec<String>,
    /// How deep in script code the engine is: handlers of variables are
    /// told when it comes out, so a script sees all its changes made.
    pub(crate) running: u32,
    /// Whether handlers of variables are being told now.
    pub(crate) telling: bool,
    /// What is switched off for the session.
    pub(crate) switches: Switches,
    /// The modules asked for or the switches changed since the frontend
    /// last asked.
    pub(crate) changed: bool,
    /// The scripts there are, as the frontend tells: to list them and
    /// their tags, as it is the frontend that runs them.
    pub(crate) scripts: Vec<Belongs>,
    /// Set by `separator()`, with the script that did: `None` in it turns
    /// command splitting off; none set is the usual `;`.
    pub(crate) separator: Option<(String, Option<String>)>,
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

/// The triggers' patterns in one set. With many patterns it takes a moment
/// to make, so it is made on a thread of its own; until it is, the set
/// before says for the patterns it had, and the others are tried each,
/// with the same result.
#[derive(Default)]
pub(crate) struct TriggerSet {
    /// The triggers it was last asked for.
    pub(crate) ids: Vec<u64>,
    /// The set there is, and the patterns it was made of; None if they
    /// could not be put in one, or until it is made.
    pub(crate) set: Option<regex::RegexSet>,
    pub(crate) patterns: Vec<String>,
    /// Where each trigger's pattern is in the set; None for one not in it.
    pub(crate) places: Vec<Option<usize>>,
    /// The set being made of the triggers' patterns, as they now are.
    pub(crate) making: Option<std::sync::mpsc::Receiver<(Vec<String>, Option<regex::RegexSet>)>>,
}
