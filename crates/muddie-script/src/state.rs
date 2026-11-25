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

pub(super) enum MacroAction {
    Command(String),
    Function(Function),
}

/// What a script sets that outlives the call, to be undone when it stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Set {
    /// The map learning from what is seen (`map.learn`).
    Learning,
    /// Words for directions (`map.directions`).
    Directions,
    /// How the MUD writes room names (`map.names`).
    Naming,
    /// The language the MUD is played in (`map.language`).
    Language,
    /// A panel's text, by its name (`panel`).
    Panel(String),
    /// A channel's tab, by its name (`channel`).
    Channel(String),
}

impl State {
    /// Notes that the script running set `what`.
    pub(crate) fn set(&mut self, what: Set) {
        let owner = self.owner.clone();
        self.set_by.retain(|(known, _)| *known != what);
        self.set_by.push((what, owner));
    }
}

/// A gauge a script shows: its name, and what it is switched by.
pub(crate) struct Shown {
    pub(crate) owner: String,
    pub(crate) name: String,
    pub(crate) belongs: Belongs,
}

/// What a key does, for whom: the script that bound it, and what it is
/// switched by. A key can be bound several times, to do one thing while
/// one is on and another while another is: the first on does.
pub(super) struct Macro {
    pub(crate) id: u64,
    pub(crate) owner: String,
    pub(crate) belongs: Belongs,
    pub(crate) action: MacroAction,
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
    /// What is switched off by itself, of what scripts made.
    pub(crate) paused: std::collections::HashSet<u64>,
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
    /// A screen reader is reading the client: for `screen_reader()`.
    pub(crate) screen_reader: bool,
    pub(crate) actions: Vec<Action>,
    /// Set by `map.native(false)`, with the script that did.
    pub(crate) map_native_off: Option<String>,
    /// Set by `map.persist(false)`, with the script that did.
    pub(crate) map_memory_only: Option<String>,
    /// What scripts set that is the map's or the session's, each by what it
    /// is and the script that set it last: to be undone when that script
    /// stops. One of [`Set`].
    pub(crate) set_by: Vec<(Set, String)>,
    /// The line the triggers run on is shown, but not read aloud
    /// (`silence()`).
    pub(crate) silenced: bool,
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
    pub(crate) macros: HashMap<String, Vec<Macro>>,
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

impl DataHandler {
    pub(super) fn covers(&self, key: &str) -> bool {
        match key.strip_prefix(self.key.as_str()) {
            Some(rest) => rest.is_empty() || rest.starts_with('.'),
            None => false,
        }
    }
}

impl State {
    /// Notes that a variable changed, for its handlers to be told.
    pub(super) fn var_changed(&mut self, name: &str) {
        if !self.vars_changed.iter().any(|known| known == name) {
            self.vars_changed.push(name.to_owned());
        }
    }

    pub(super) fn next_id(&mut self) -> u64 {
        self.last_id += 1;
        self.last_id
    }

    /// Writes the remembered values to their file, if they have one.
    pub(super) fn save_memory(&self) -> Result<(), String> {
        let Some(file) = &self.memory_file else {
            return Ok(());
        };
        // Written to a temporary file first, so a crash cannot leave the
        // real one half written.
        let text = serde_json::to_string_pretty(&self.memory).map_err(|error| error.to_string())?;
        let temporary = file.with_extension("json.tmp");
        std::fs::write(&temporary, text)
            .and_then(|()| std::fs::rename(&temporary, file))
            .map_err(|error| format!("could not save {}: {error}", file.display()))
    }

    /// `text` with each `$name` that is a remembered value replaced by
    /// it. A `$` before anything else is left as it is.
    pub(super) fn expand(&self, text: &str) -> String {
        let mut expanded = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find('$') {
            expanded.push_str(&rest[..at]);
            let after = &rest[at + 1..];
            let end = after
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .unwrap_or(after.len());
            let name = &after[..end];
            let starts_as_a_name =
                name.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_');
            let Some(mut value) = self.memory.get(name).filter(|_| starts_as_a_name) else {
                expanded.push('$');
                expanded.push_str(name);
                rest = &after[end..];
                continue;
            };
            // `$name.key` goes into a table, and `$name.2` into a list,
            // for as long as there is something of that name in it.
            let mut used = end;
            while let Some(inside) = after[used..].strip_prefix('.') {
                let key_end = inside
                    .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                    .unwrap_or(inside.len());
                let key = &inside[..key_end];
                let found = match value {
                    serde_json::Value::Object(fields) => fields.get(key),
                    serde_json::Value::Array(items) => key
                        .parse::<usize>()
                        .ok()
                        .and_then(|place| items.get(place.checked_sub(1)?)),
                    _ => None,
                };
                let Some(found) = found.filter(|_| !key.is_empty()) else {
                    break;
                };
                value = found;
                used += 1 + key_end;
            }
            match value {
                serde_json::Value::String(text) => expanded.push_str(text),
                // A list as its items, one after another.
                serde_json::Value::Array(items) => {
                    let shown: Vec<String> = (items.iter())
                        .map(|item| {
                            item.as_str()
                                .map_or_else(|| item.to_string(), str::to_owned)
                        })
                        .collect();
                    expanded.push_str(&shown.join(" "));
                }
                other => expanded.push_str(&other.to_string()),
            }
            rest = &after[used..];
        }
        expanded.push_str(rest);
        expanded
    }

    /// What there is of a kind, by what it is switched by: those made by
    /// the scripts, and for scripts, those the frontend told of.
    fn belongings(&self, kind: Kind) -> Vec<&Belongs> {
        match kind {
            Kind::Trigger => self
                .triggers
                .iter()
                .map(|matcher| &matcher.belongs)
                .collect(),
            Kind::Alias => self
                .aliases
                .iter()
                .map(|matcher| &matcher.belongs)
                .collect(),
            Kind::Macro => (self.macros.values().flatten())
                .map(|bound| &bound.belongs)
                .collect(),
            Kind::Timer => self.timers.iter().map(|timer| &timer.belongs).collect(),
            Kind::Gauge => self.gauges.iter().map(|gauge| &gauge.belongs).collect(),
            Kind::Script => self.scripts.iter().collect(),
        }
    }

    /// The places there are of a kind, with the folders they are in, in
    /// alphabetical order.
    pub(super) fn place_names(&self, kind: Kind) -> Vec<String> {
        let places = (self.belongings(kind).into_iter())
            .filter_map(|belongs| belongs.place.as_deref())
            .flat_map(switches::places_up);
        let mut names: Vec<String> = places.map(str::to_owned).collect();
        names.sort();
        names.dedup();
        names
    }

    /// The tags there are, those anything carries and those switched off,
    /// in alphabetical order.
    pub(super) fn tag_names(&self) -> Vec<String> {
        let handlers = self
            .data_handlers
            .iter()
            .flat_map(|handler| handler.tags.iter());
        let carried = (Kind::ALL.iter())
            .flat_map(|&kind| self.belongings(kind))
            .flat_map(|belongs| belongs.tags.iter())
            .chain(handlers)
            .map(|tag| tag_key(tag))
            .collect::<Vec<String>>();
        let mut names: Vec<String> = carried
            .into_iter()
            .chain(self.switches.tags.iter().cloned())
            .collect();
        names.retain(|name| !name.is_empty());
        names.sort();
        names.dedup();
        names
    }
}
