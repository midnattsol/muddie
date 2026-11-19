//! Triggers, aliases, timers and the Luau runtime that scripts them.
//!
//! The [`Engine`] has no I/O of its own: the frontend feeds it output lines,
//! typed input and the clock, and carries out the [`Action`]s it returns.
//!
//! Functions available to scripts:
//!
//! - `send(text)`: send a command to the MUD (aliases are not applied).
//! - `echo(text)`: show a line in the output.
//! - `speak(text, {interrupt = true})`: say something through speech.
//! - `trigger(pattern, fn, options)`: call `fn(m, line)` for each output
//!   line matching the regular expression. `m[0]` is the whole match,
//!   `m[1]`... the groups, and named groups are also available as `m.name`.
//!   The optional `options` table takes `group` (a name to switch several
//!   on and off together) and `once` (remove it after it fires).
//! - `gag()`: inside a trigger, hide the line that fired it.
//! - `line_spans()`: inside a trigger, the line as a list of runs of text
//!   with their style: `text`, `fg` and `bg` (a palette number, an
//!   `"#rrggbb"` string, or `nil` for the default), `bold`, `italic`,
//!   `underline` and `inverse`.
//! - `highlight(fg, bg)`: inside a trigger, recolour the text that matched.
//!   A colour is a name (`"red"`, `"bright blue"`...), `"#rrggbb"` or a
//!   palette number; `nil` leaves that colour as it is.
//! - `alias(pattern, fn, options)`: call `fn(m, input)` instead of sending
//!   typed input that matches; same options as `trigger`.
//! - `enable(group, enabled)`: switch a group of triggers and aliases on
//!   or off.
//! - `separator(text)`: the text that separates several commands typed on
//!   one line (`;` by default, doubled to type it literally); `nil` turns
//!   splitting off.
//! - `bind(key, action)`: run `action` (a command to send, or a function)
//!   when `key` is pressed, e.g. `bind("f5", "look")` or
//!   `bind("ctrl+shift+k", function() ... end)`; `unbind(key)` removes it.
//! - `remember(key, value)` and `recall(key)`: keep values between
//!   sessions, per profile. Values can be anything JSON can hold.
//! - `play(file, {volume = 0.8, loop = false})`: play a sound file, by
//!   path or by name in the profile's `sounds` folder; `stop_sounds()`.
//! - `require(name)`: load `name.luau` (or `name/init.luau`) from a folder
//!   scripts were loaded from, once, and return what it returns.
//! - `after(seconds, fn)` and `every(seconds, fn)`: timers.
//! - `on_gmcp(package, fn)`: call `fn(data, package)` when a GMCP message
//!   for that package or one under it arrives (`"Char"` covers
//!   `Char.Vitals`).
//! - `gmcp(package)`: the data last received for exactly that package.
//! - `send_gmcp(package, data)`: send a GMCP message; `data` is optional.
//! - `on_msdp(variable, fn)`, `msdp(variable)` and
//!   `send_msdp(command, ...)`: the same for MSDP, e.g.
//!   `send_msdp("REPORT", "HEALTH")`. Values arrive as strings.
//! - `mssp()`: the server's MSSP status as a table, empty if it sent none.
//! - `on_event(name, fn)`: call `fn` when something happens in the client:
//!   `"connect"`, `"disconnect"`, or `"panel"` with the size in characters
//!   of the panel beside the output.
//! - `supports(module, ...)`: announce GMCP modules to the server.
//! - `completions(name, words)`: words for Tab to offer.
//! - `gauge(name, value, max, options)`: show or update a bar in the status
//!   strip; `gauge(name)` removes it. `options` may give a `colour`, a
//!   `style` (`"segments"`) and `low`, a percentage to warn under.
//! - `chat(text)`: add a line to the chat panel.
//! - `panel(name, text)`: show `text` (several lines, colours allowed) in
//!   a panel of its own, replacing what it showed before; `panel(name)`
//!   removes it. For anything the server sends ready to display, such as
//!   a map it draws itself.
//!
//! The `map` table reads and edits the automapper's map. A room is given
//! by its number, or by the text the server identifies it with.
//!
//! - `map.visit(room)`: the player is now in `room`, a table with `id` and
//!   optionally `name`, `area`, `exits` (direction to room id) and `x`,
//!   `y`, `z`. This is how a script maps a MUD the built-in loader does not
//!   understand, from whatever the server sends.
//! - `map.native(false)`: turn the built-in loader (GMCP `Room.Info`, MSDP
//!   `ROOM`) off, leaving the map to the script.
//! - `map.persist(false)`: keep the map in memory only instead of saving
//!   it to a file, for servers that send the map themselves.
//! - `map.clear()`: forget every room, for instance before loading the
//!   area the server has just sent.
//! - `map.set_room(room)`: add or update a room without moving there.
//! - `map.room(id)`: a room as such a table, the current one if `id` is
//!   omitted; `nil` if unknown.
//! - `map.find(text)`: the rooms whose name contains `text`.
//! - `map.path(to, from)`: the directions to walk, from the current room
//!   if `from` is omitted; `nil` if there is no known route.
//! - `map.walk(to)`: walk there.
//! - `map.set_exit(from, direction, to, options)`: add an exit of your
//!   own or change where one leads. `map.edit_exit(from, direction,
//!   options)` changes an existing exit. The options, all optional, are
//!   `cost` (extra cost for routes), `locked` (routes never use it) and
//!   `command` (what a walk sends instead of the direction, such as
//!   `"open door;n"`; `""` clears it). `map.exit(from, direction)` returns
//!   an exit as a table with those fields plus `to` and `custom`.
//! - `map.remove_exit(from, direction)`, `map.remove_room(id)` and
//!   `map.move(id, x, y, z)`.
//! - `map.relayout(area)`: draw an area again from scratch (the current
//!   one if omitted); returns how many rooms moved.
//! - `map.set_weight(id, weight)`: how costly routes consider entering the
//!   room (1 is normal), and `map.avoid(id, true)`: keep routes out of it.
//!   A room table reports them as `weight` and `avoid`.
//! - `map.areas()`: the names of all areas.
//! - `map.id(text)`: the room number used for a text identifier.
//!
//! - `cancel(id)`: remove the trigger, alias, timer or GMCP handler with the
//!   id that the functions above returned.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::serde::SerializeOptions;
use mlua::{Function, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, Variadic, VmState};
use muddie_core::{Color, Line};
use muddie_mapper::{Exit, Map, Pos, Room, RoomId, RoomInfo, room_id};
use regex::Regex;

/// Longest a single script run or callback may take before it is stopped,
/// so a faulty script cannot freeze the client.
const TIME_BUDGET: Duration = Duration::from_millis(500);
const MEMORY_LIMIT: usize = 256 << 20;
const MIN_TIMER_INTERVAL: Duration = Duration::from_millis(10);

/// Something a script asked the client to do.
/// How a gauge is drawn, as far as the script cares to say.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GaugeLook {
    /// A colour name or `#rrggbb`; empty lets the client choose.
    pub colour: String,
    /// Drawn as a row of cells instead of a continuous bar.
    pub segments: bool,
    /// The percentage under which the numbers turn to a warning colour.
    pub low: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Send(String),
    Echo(String),
    Speak {
        text: String,
        interrupt: bool,
    },
    /// Send a GMCP message; `data` is JSON, or empty for none.
    Gmcp {
        package: String,
        data: String,
    },
    /// Set the named gauge to `(value, max)`, or remove it.
    Gauge {
        name: String,
        level: Option<(f64, f64)>,
        look: GaugeLook,
    },
    /// Add a line to the chat panel.
    Chat(String),
    /// Announce these GMCP modules to the server, as `"Name version"`.
    Supports(Vec<String>),
    /// Words to offer when completing, under a name; none removes them.
    Completions {
        name: String,
        words: Vec<String>,
    },
    /// Set the text of the named panel, or remove the panel.
    Panel {
        name: String,
        text: Option<String>,
    },
    /// Walk to the room with this number.
    Walk(RoomId),
    /// A script moved the player to this room on the map.
    Entered(RoomId),
    /// Send an MSDP message such as `REPORT` with variable names.
    Msdp {
        variable: String,
        values: Vec<String>,
    },
    /// Play a sound file at `volume` (0 to 1), repeating if `looped`.
    PlaySound {
        file: String,
        volume: f32,
        looped: bool,
    },
    StopSounds,
    /// A script failed; the message is for the user.
    Error(String),
}

struct Matcher {
    id: u64,
    regex: Regex,
    callback: Function,
    /// Group it is switched on and off with, if any.
    group: Option<String>,
    /// Removed after firing once.
    once: bool,
}

enum Binding {
    Command(String),
    Function(Function),
}

struct Timer {
    id: u64,
    due: Instant,
    /// `Some` for repeating timers.
    period: Option<Duration>,
    callback: Function,
}

/// A callback for GMCP or MSDP data.
struct DataHandler {
    id: u64,
    /// Key of the data it wants, as produced for the data store. For GMCP it
    /// also matches packages under it.
    key: String,
    callback: Function,
}

impl DataHandler {
    fn covers(&self, key: &str) -> bool {
        match key.strip_prefix(self.key.as_str()) {
            Some(rest) => rest.is_empty() || rest.starts_with('.'),
            None => false,
        }
    }
}

/// Store key of a client event; like [`msdp_key`], it cannot collide with
/// a GMCP package.
fn event_key(name: &str) -> String {
    format!("event {}", name.to_lowercase())
}

/// Store key of an MSDP variable. The space cannot occur in a GMCP package
/// name, so the two kinds never collide.
fn msdp_key(variable: &str) -> String {
    format!("msdp {}", variable.to_lowercase())
}

/// State shared between the engine and the functions scripts call.
#[derive(Default)]
struct State {
    last_id: u64,
    triggers: Vec<Matcher>,
    aliases: Vec<Matcher>,
    timers: Vec<Timer>,
    data_handlers: Vec<DataHandler>,
    /// Data last received per GMCP package or MSDP variable.
    data: HashMap<String, Value>,
    /// The server's MSSP status, as `(name, value)` pairs.
    mssp: Vec<(String, String)>,
    actions: Vec<Action>,
    /// Set by `map.native(false)`.
    map_native_off: bool,
    /// Set by `map.persist(false)`.
    map_memory_only: bool,
    /// Set by `gag()` while a line's triggers run.
    gag: bool,
    /// The line whose triggers are running, with its styles.
    current_line: Option<Line>,
    /// Byte range of the text the running trigger matched.
    current_match: Option<(usize, usize)>,
    /// What to do for each bound key, by normalised key name.
    bindings: HashMap<String, Binding>,
    /// Values kept between sessions, and the file they are saved in.
    memory: serde_json::Map<String, serde_json::Value>,
    memory_file: Option<PathBuf>,
    /// Groups switched off with `enable(group, false)`.
    disabled_groups: HashSet<String>,
    /// Set by `separator()`: `Some(None)` turns command splitting off.
    separator: Option<Option<String>>,
    /// Folders `require` looks in: those scripts were loaded from.
    module_dirs: Vec<PathBuf>,
    /// What each required module returned.
    modules: HashMap<String, Value>,
    /// Modules being loaded, to refuse circular requires.
    loading: Vec<String>,
    /// When the script currently running must be stopped.
    deadline: Option<Instant>,
}

impl State {
    fn next_id(&mut self) -> u64 {
        self.last_id += 1;
        self.last_id
    }
}

/// A match captured before its callback runs, so callbacks are free to add
/// or remove triggers while a line is being dispatched.
struct Hit {
    id: u64,
    once: bool,
    /// Byte range of the whole match.
    range: (usize, usize),
    callback: Function,
    groups: Vec<Option<String>>,
    named: Vec<(String, String)>,
}

pub struct Engine {
    lua: Lua,
    state: Rc<RefCell<State>>,
    /// The automapper's map, shared with the frontend.
    map: Rc<RefCell<Map>>,
    /// Script files loaded so far, in order, for `/reload`.
    files: Vec<PathBuf>,
}

impl Engine {
    /// Starts a runtime whose scripts read and edit `map`. The frontend
    /// must not hold a borrow of the map while it calls into the engine.
    pub fn new(map: Rc<RefCell<Map>>) -> mlua::Result<Self> {
        // No `io`, `package` or `debug`: scripts cannot touch files, load
        // other code from disk or escape the sandbox. Luau's `os` only has
        // clock and date functions.
        let libs = StdLib::COROUTINE
            | StdLib::TABLE
            | StdLib::OS
            | StdLib::STRING
            | StdLib::UTF8
            | StdLib::BIT
            | StdLib::MATH
            | StdLib::BUFFER;
        let lua = Lua::new_with(libs, LuaOptions::default())?;
        // mlua always installs a `require` that reads any file on disk.
        lua.globals().raw_remove("require")?;
        lua.set_memory_limit(MEMORY_LIMIT)?;
        let state = Rc::new(RefCell::new(State::default()));
        install_api(&lua, &state)?;
        install_map_api(&lua, &state, &map)?;

        let watched = Rc::clone(&state);
        lua.set_interrupt(move |_| {
            let overdue = watched
                .try_borrow()
                .is_ok_and(|state| state.deadline.is_some_and(|at| Instant::now() > at));
            if overdue {
                Err(mlua::Error::runtime("script ran too long and was stopped"))
            } else {
                Ok(VmState::Continue)
            }
        });
        // Freezes the API and the standard library; scripts' own globals go
        // to a separate table layered on top.
        lua.sandbox(true)?;
        Ok(Self {
            map,
            lua,
            state,
            files: Vec::new(),
        })
    }

    /// Runs a script file and remembers it for `/reload`.
    pub fn load_file(&mut self, path: &Path) {
        if !self.files.iter().any(|loaded| loaded == path) {
            self.files.push(path.to_owned());
        }
        if let Some(dir) = path.parent() {
            let dirs = &mut self.state.borrow_mut().module_dirs;
            if !dirs.iter().any(|known| known == dir) {
                dirs.push(dir.to_owned());
            }
        }
        match std::fs::read_to_string(path) {
            Ok(code) => {
                let name = path.file_name().unwrap_or(path.as_os_str());
                self.exec(&name.to_string_lossy(), &code);
            }
            Err(error) => self.error(format!("cannot read {}: {error}", path.display())),
        }
    }

    /// Runs a chunk of Luau code; `name` identifies it in error messages.
    pub fn exec(&mut self, name: &str, code: &str) {
        // The "@" makes errors read `name:line: message`.
        let chunk = self.lua.load(code).set_name(format!("@{name}"));
        self.guarded(|| chunk.exec());
    }

    /// Offers a completed output line to the triggers. Returns false if a
    /// trigger gagged it, meaning it must not be shown or spoken.
    pub fn on_line(&mut self, line: &str) -> bool {
        self.dispatch_line(line, None).is_some()
    }

    /// Like [`Self::on_line`], with the line's colours available to the
    /// triggers through `line_spans()`.
    /// Returns the line to show, which triggers may have recoloured, or
    /// `None` if one gagged it.
    pub fn on_styled_line(&mut self, line: &Line) -> Option<Line> {
        self.dispatch_line(&line.text(), Some(line.clone()))
    }

    /// Handles a key press, named as in `bind` (see [`key_name`]). Returns
    /// whether the key was bound, in which case nothing else should use it.
    pub fn on_key(&mut self, key: &str) -> bool {
        let action = match self.state.borrow().bindings.get(&key_name(key)) {
            Some(Binding::Command(command)) => Ok(command.clone()),
            Some(Binding::Function(function)) => Err(function.clone()),
            None => return false,
        };
        match action {
            Ok(command) => self.on_input(&command),
            Err(function) => {
                self.guarded(|| function.call::<()>(()));
            }
        }
        true
    }

    /// Keeps the values of `remember` in `file`, loading what it holds.
    pub fn use_memory_file(&mut self, file: PathBuf) {
        let loaded = match std::fs::read_to_string(&file) {
            Ok(text) => serde_json::from_str(&text).map_err(|error| error.to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
            Err(error) => Err(error.to_string()),
        };
        let mut state = self.state.borrow_mut();
        match loaded {
            Ok(memory) => {
                state.memory = memory;
                state.memory_file = Some(file);
            }
            // Left unset, so the unreadable file is not overwritten.
            Err(error) => {
                let message = format!(
                    "remembered values not loaded from {}: {error}",
                    file.display()
                );
                state.actions.push(Action::Error(message));
            }
        }
    }

    fn dispatch_line(&mut self, text: &str, styled: Option<Line>) -> Option<Line> {
        {
            let mut state = self.state.borrow_mut();
            state.gag = false;
            state.current_line = styled;
        }
        for hit in self.hits(text, |state| &state.triggers, usize::MAX) {
            let still_there = (self.state.borrow().triggers.iter()).any(|t| t.id == hit.id);
            if still_there {
                self.fire(&hit, text);
            }
        }
        let mut state = self.state.borrow_mut();
        let line = state.current_line.take();
        if std::mem::take(&mut state.gag) {
            return None;
        }
        Some(line.unwrap_or_default())
    }

    /// Handles a line typed by the user: a client command, or one or more
    /// commands, each of them an alias or sent as it is.
    pub fn on_input(&mut self, input: &str) {
        if let Some(code) = input.strip_prefix("/lua ") {
            return self.exec("input", code);
        }
        if input == "/reload" {
            return self.reload();
        }
        let separator = match &self.state.borrow().separator {
            Some(chosen) => chosen.clone(),
            None => Some(";".to_owned()),
        };
        for command in split_commands(input, separator.as_deref()) {
            match self.hits(&command, |state| &state.aliases, 1).pop() {
                Some(hit) => self.fire(&hit, &command),
                None => self.push(Action::Send(command)),
            }
        }
    }

    /// Records a GMCP message and runs the handlers for its package.
    /// `json` is the message's payload, empty if it had none.
    pub fn on_gmcp(&mut self, package: &str, json: &str) {
        self.on_data(package.to_lowercase(), package, json);
    }

    /// Records an MSDP variable and runs the handlers for it. `json` is the
    /// value converted to JSON.
    pub fn on_msdp(&mut self, variable: &str, json: &str) {
        self.on_data(msdp_key(variable), variable, json);
    }

    /// Tells scripts of something that happened in the client itself,
    /// such as `connect` or the `panel` changing size. `json` is what
    /// their handlers are given, empty for nothing.
    pub fn on_event(&mut self, name: &str, json: &str) {
        self.on_data(event_key(name), name, json);
    }

    /// Remembers the server's MSSP status for `mssp()`.
    pub fn on_mssp(&mut self, status: &[(String, String)]) {
        self.state.borrow_mut().mssp = status.to_vec();
    }

    /// Stores out-of-band data under `key` and calls the handlers covering
    /// it with the data and `name`.
    fn on_data(&mut self, key: String, name: &str, json: &str) {
        let data = if json.is_empty() {
            Value::Nil
        } else {
            // JSON null becomes nil and arrays plain tables, as scripts expect.
            let options = SerializeOptions::new()
                .serialize_none_to_null(false)
                .serialize_unit_to_null(false)
                .set_array_metatable(false);
            let parsed = serde_json::from_str::<serde_json::Value>(json)
                .map_err(|error| error.to_string())
                .and_then(|value| {
                    (self.lua.to_value_with(&value, options)).map_err(|error| error.to_string())
                });
            match parsed {
                Ok(data) => data,
                Err(error) => return self.error(format!("bad data for {name}: {error}")),
            }
        };
        let callbacks: Vec<(u64, Function)> = {
            let mut state = self.state.borrow_mut();
            state.data.insert(key.clone(), data.clone());
            (state.data_handlers.iter())
                .filter(|handler| handler.covers(&key))
                .map(|handler| (handler.id, handler.callback.clone()))
                .collect()
        };
        for (id, callback) in callbacks {
            let still_there = (self.state.borrow().data_handlers.iter()).any(|h| h.id == id);
            if still_there {
                self.guarded(|| callback.call::<()>((data.clone(), name)));
            }
        }
    }

    /// Runs the timers that are due at `now`.
    pub fn tick(&mut self, now: Instant) {
        let due: Vec<(u64, Function)> = (self.state.borrow().timers.iter())
            .filter(|timer| timer.due <= now)
            .map(|timer| (timer.id, timer.callback.clone()))
            .collect();
        for (id, callback) in due {
            let ok = self.guarded(|| callback.call::<()>(()));
            let mut state = self.state.borrow_mut();
            // The callback may have cancelled its own timer.
            let Some(index) = state.timers.iter().position(|timer| timer.id == id) else {
                continue;
            };
            match state.timers[index].period {
                // A repeating timer that fails would fail forever: drop it.
                Some(period) if ok => state.timers[index].due = now + period,
                _ => {
                    state.timers.remove(index);
                }
            }
        }
    }

    /// Whether the built-in map loader should run, which it does unless a
    /// script turned it off with `map.native(false)`.
    pub fn native_map(&self) -> bool {
        !self.state.borrow().map_native_off
    }

    /// Whether the map should be saved to a file, which it is unless a
    /// script chose otherwise with `map.persist(false)`.
    pub fn persist_map(&self) -> bool {
        !self.state.borrow().map_memory_only
    }

    /// When [`Self::tick`] next has something to do.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.state.borrow().timers.iter().map(|t| t.due).min()
    }

    /// Takes the actions requested since the last call, in order.
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.state.borrow_mut().actions)
    }

    /// Starts from a clean runtime and runs the loaded files again.
    fn reload(&mut self) {
        let mut fresh = match Engine::new(Rc::clone(&self.map)) {
            Ok(fresh) => fresh,
            Err(error) => return self.error(format!("cannot restart scripts: {error}")),
        };
        fresh.push_all(self.take_actions());
        for file in std::mem::take(&mut self.files) {
            fresh.load_file(&file);
        }
        fresh.push(Action::Echo("Scripts reloaded.".into()));
        *self = fresh;
    }

    fn hits(&self, text: &str, list: impl Fn(&State) -> &Vec<Matcher>, limit: usize) -> Vec<Hit> {
        let state = self.state.borrow();
        let enabled = |matcher: &&Matcher| {
            (matcher.group.as_ref()).is_none_or(|group| !state.disabled_groups.contains(group))
        };
        let matches = list(&state).iter().filter(enabled).filter_map(|matcher| {
            let captures = matcher.regex.captures(text)?;
            let group = |m: regex::Match| m.as_str().to_owned();
            Some(Hit {
                id: matcher.id,
                once: matcher.once,
                range: captures.get(0).map_or((0, 0), |m| (m.start(), m.end())),
                callback: matcher.callback.clone(),
                groups: captures.iter().map(|m| m.map(group)).collect(),
                named: (matcher.regex.capture_names().flatten())
                    .filter_map(|name| Some((name.to_owned(), group(captures.name(name)?))))
                    .collect(),
            })
        });
        matches.take(limit).collect()
    }

    fn fire(&mut self, hit: &Hit, text: &str) {
        self.state.borrow_mut().current_match = Some(hit.range);
        if hit.once {
            let mut state = self.state.borrow_mut();
            state.triggers.retain(|trigger| trigger.id != hit.id);
            state.aliases.retain(|alias| alias.id != hit.id);
        }
        self.guarded(|| {
            let m = self.lua.create_table()?;
            for (index, group) in hit.groups.iter().enumerate() {
                m.set(index, group.as_deref())?;
            }
            for (name, value) in &hit.named {
                m.set(name.as_str(), value.as_str())?;
            }
            hit.callback.call::<()>((m, text))
        });
    }

    /// Runs script code under the time budget, reporting failure as an
    /// [`Action::Error`]. Returns whether it succeeded.
    fn guarded(&self, run: impl FnOnce() -> mlua::Result<()>) -> bool {
        self.state.borrow_mut().deadline = Some(Instant::now() + TIME_BUDGET);
        let result = run();
        self.state.borrow_mut().deadline = None;
        match result {
            Ok(()) => true,
            Err(error) => {
                self.error(describe(&error));
                false
            }
        }
    }

    fn error(&self, message: String) {
        self.push(Action::Error(message));
    }

    fn push(&self, action: Action) {
        self.state.borrow_mut().actions.push(action);
    }

    fn push_all(&self, actions: Vec<Action>) {
        self.state.borrow_mut().actions.extend(actions);
    }
}

/// Splits typed input into commands at `separator`; a doubled separator
/// stands for the separator itself. Without a separator the input is one
/// command.
fn split_commands(input: &str, separator: Option<&str>) -> Vec<String> {
    let Some(separator) = separator.filter(|separator| !separator.is_empty()) else {
        return vec![input.to_owned()];
    };
    let mut commands = vec![String::new()];
    let mut rest = input;
    while let Some(at) = rest.find(separator) {
        let current = commands.last_mut().expect("never empty");
        current.push_str(&rest[..at]);
        rest = &rest[at + separator.len()..];
        if let Some(after) = rest.strip_prefix(separator) {
            current.push_str(separator);
            rest = after;
        } else {
            commands.push(String::new());
        }
    }
    commands.last_mut().expect("never empty").push_str(rest);
    commands
        .iter()
        .map(|command| command.trim().to_owned())
        .collect()
}

/// Whether `name` is safe to look up as a module: a relative path of plain
/// names, so it cannot reach outside the script folders.
fn valid_module_name(name: &str) -> bool {
    let plain = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    };
    name.split('/').all(plain)
}

/// The name `bind` and [`Engine::on_key`] know a key by: lower case, with
/// its modifiers in the order ctrl, alt, shift, as in `ctrl+shift+f5`.
pub fn key_name(key: &str) -> String {
    let lower = key.to_lowercase();
    let mut parts: Vec<&str> = lower.split('+').map(str::trim).collect();
    let key = parts.pop().unwrap_or_default();
    let mut name = String::new();
    for modifier in ["ctrl", "alt", "shift"] {
        if parts.contains(&modifier) {
            name.push_str(modifier);
            name.push('+');
        }
    }
    name + key
}

/// Reads a colour given by a script: a palette number, `"#rrggbb"`, or a
/// name such as `"red"` or `"bright blue"`. `nil` gives `None`.
fn color_from(value: &Value) -> mlua::Result<Option<Color>> {
    const NAMES: [&str; 8] = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];
    let unknown =
        || mlua::Error::runtime("a colour is a name, \"#rrggbb\" or a number from 0 to 255");
    match value {
        Value::Nil => Ok(None),
        Value::Integer(index) => Ok(Some(Color::Indexed(
            u8::try_from(*index).map_err(|_| unknown())?,
        ))),
        Value::Number(index) if index.fract() == 0.0 && (0.0..=255.0).contains(index) => {
            Ok(Some(Color::Indexed(*index as u8)))
        }
        Value::String(text) => {
            let text = text.to_string_lossy().trim().to_lowercase();
            if let Some(hex) = text.strip_prefix('#') {
                let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
                return match (hex.len(), channel(0), channel(2), channel(4)) {
                    (6, Some(r), Some(g), Some(b)) => Ok(Some(Color::Rgb(r, g, b))),
                    _ => Err(unknown()),
                };
            }
            let (bright, name) = match text.strip_prefix("bright ") {
                Some(name) => (8, name),
                None => (0, text.as_str()),
            };
            let index = NAMES
                .iter()
                .position(|known| *known == name)
                .ok_or_else(unknown)?;
            Ok(Some(Color::Indexed(bright + index as u8)))
        }
        _ => Err(unknown()),
    }
}

/// A colour as scripts see it: a palette number, `"#rrggbb"`, or nil.
fn color_value(lua: &Lua, color: Color) -> mlua::Result<Value> {
    match color {
        Color::Default => Ok(Value::Nil),
        Color::Indexed(index) => Ok(Value::Integer(index.into())),
        Color::Rgb(r, g, b) => Ok(Value::String(
            lua.create_string(format!("#{r:02x}{g:02x}{b:02x}"))?,
        )),
    }
}

/// Reads a room id argument: a number, or text (see [`room_id`]).
fn id_of(value: &Value) -> mlua::Result<RoomId> {
    match value {
        Value::Integer(id) => Ok(*id),
        Value::Number(id) if id.fract() == 0.0 => Ok(*id as RoomId),
        Value::String(text) => Ok(room_id(&text.to_string_lossy())),
        _ => Err(mlua::Error::runtime(
            "a room id must be a whole number or text",
        )),
    }
}

/// Reads a room description given by a script.
fn room_info(room: &Table) -> mlua::Result<RoomInfo> {
    let mut exits = std::collections::BTreeMap::new();
    if let Some(table) = room.get::<Option<Table>>("exits")? {
        for pair in table.pairs::<String, Value>() {
            let (direction, to) = pair?;
            exits.insert(muddie_mapper::canonical_direction(&direction), id_of(&to)?);
        }
    }
    let coordinate = |name: &str| room.get::<Option<i32>>(name);
    let pos = match (coordinate("x")?, coordinate("y")?) {
        (Some(x), Some(y)) => Some(Pos {
            x,
            y,
            z: coordinate("z")?.unwrap_or(0),
        }),
        _ => None,
    };
    Ok(RoomInfo {
        id: id_of(&room.get::<Value>("id")?)?,
        name: room.get::<Option<String>>("name")?.unwrap_or_default(),
        area: room.get::<Option<String>>("area")?.unwrap_or_default(),
        exits,
        pos,
    })
}

/// Applies the `cost`, `locked` and `command` a script gave to an exit.
fn apply_exit_options(exit: &mut Exit, options: &ExitOptions) {
    if let Some(cost) = options.cost {
        exit.cost = cost;
    }
    if let Some(locked) = options.locked {
        exit.locked = locked;
    }
    if let Some(command) = &options.command {
        exit.command = (!command.is_empty()).then(|| command.clone());
    }
}

/// The optional fields of a script's exit options table.
#[derive(Default)]
struct ExitOptions {
    cost: Option<u32>,
    locked: Option<bool>,
    command: Option<String>,
}

impl ExitOptions {
    fn read(options: Option<Table>) -> mlua::Result<Self> {
        let Some(options) = options else {
            return Ok(Self::default());
        };
        Ok(Self {
            cost: options.get("cost")?,
            locked: options.get("locked")?,
            command: options.get("command")?,
        })
    }
}

/// Describes a room to a script, in the shape [`room_info`] reads.
fn room_table(lua: &Lua, room: &Room) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("id", room.id)?;
    table.set("name", room.name.as_str())?;
    table.set("area", room.area.as_str())?;
    table.set("x", room.pos.x)?;
    table.set("y", room.pos.y)?;
    table.set("z", room.pos.z)?;
    table.set("weight", room.weight)?;
    table.set("avoid", room.avoid)?;
    let exits = lua.create_table()?;
    for (direction, exit) in &room.exits {
        exits.set(direction.as_str(), exit.to)?;
    }
    table.set("exits", exits)?;
    Ok(table)
}

/// Defines the `map` table.
fn install_map_api(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
) -> mlua::Result<()> {
    let api = lua.create_table()?;
    // The map is updated even when saving it fails; the script still gets
    // the failure as an error, so the user hears about it.
    let saved = |result: Result<(), muddie_mapper::Error>| {
        result.map_err(|error| mlua::Error::runtime(format!("could not save the map: {error}")))
    };

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "visit",
        lua.create_function(move |_, room: Table| {
            let info = room_info(&room)?;
            let id = info.id;
            let result = world.borrow_mut().visit(info);
            shared.borrow_mut().actions.push(Action::Entered(id));
            saved(result)?;
            Ok(id)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "set_room",
        lua.create_function(move |_, room: Table| {
            let info = room_info(&room)?;
            let id = info.id;
            saved(world.borrow_mut().set_room(info))?;
            Ok(id)
        })?,
    )?;

    let shared = Rc::clone(state);
    api.set(
        "native",
        lua.create_function(move |_, enabled: bool| {
            shared.borrow_mut().map_native_off = !enabled;
            Ok(())
        })?,
    )?;

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "persist",
        lua.create_function(move |_, enabled: bool| {
            shared.borrow_mut().map_memory_only = !enabled;
            if !enabled {
                world.borrow_mut().stop_storing();
            }
            Ok(())
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "clear",
        lua.create_function(move |_, ()| saved(world.borrow_mut().clear()))?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "room",
        lua.create_function(move |lua, id: Value| {
            let world = world.borrow();
            let room = match id {
                Value::Nil => world.current(),
                id => world.room(id_of(&id)?),
            };
            room.map(|room| room_table(lua, room)).transpose()
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "find",
        lua.create_function(move |lua, text: String| {
            let world = world.borrow();
            let rooms = world.find(&text).into_iter();
            rooms
                .map(|room| room_table(lua, room))
                .collect::<mlua::Result<Vec<Table>>>()
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "path",
        lua.create_function(move |_, (to, from): (Value, Value)| {
            let world = world.borrow();
            let from = match from {
                Value::Nil => world.current().map(|room| room.id),
                from => Some(id_of(&from)?),
            };
            let route = from.and_then(|from| world.route(from, id_of(&to).ok()?));
            Ok(route.map(|steps| {
                (steps.into_iter())
                    .map(|step| step.command)
                    .collect::<Vec<String>>()
            }))
        })?,
    )?;

    let shared = Rc::clone(state);
    api.set(
        "walk",
        lua.create_function(move |_, to: Value| {
            (shared.borrow_mut().actions).push(Action::Walk(id_of(&to)?));
            Ok(())
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "set_exit",
        lua.create_function(
            move |_, (from, direction, to, options): (Value, String, Value, Option<Table>)| {
                let (from, to) = (id_of(&from)?, id_of(&to)?);
                let options = ExitOptions::read(options)?;
                let mut world = world.borrow_mut();
                let result = world.set_exit(from, &direction, to).and_then(|found| {
                    world.edit_exit(from, &direction, |exit| apply_exit_options(exit, &options))?;
                    Ok(found)
                });
                let found = *result.as_ref().unwrap_or(&true);
                saved(result.map(drop))?;
                Ok(found)
            },
        )?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "edit_exit",
        lua.create_function(
            move |_, (from, direction, options): (Value, String, Option<Table>)| {
                let options = ExitOptions::read(options)?;
                let result = (world.borrow_mut()).edit_exit(id_of(&from)?, &direction, |exit| {
                    apply_exit_options(exit, &options)
                });
                let found = *result.as_ref().unwrap_or(&true);
                saved(result.map(drop))?;
                Ok(found)
            },
        )?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "exit",
        lua.create_function(move |lua, (from, direction): (Value, String)| {
            let world = world.borrow();
            let direction = muddie_mapper::canonical_direction(&direction);
            let exit = world
                .room(id_of(&from)?)
                .and_then(|room| room.exits.get(&direction));
            let Some(exit) = exit else {
                return Ok(None);
            };
            let table = lua.create_table()?;
            table.set("to", exit.to)?;
            table.set("cost", exit.cost)?;
            table.set("locked", exit.locked)?;
            table.set("command", exit.command.as_deref())?;
            table.set("custom", exit.custom)?;
            Ok(Some(table))
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "relayout",
        lua.create_function(move |_, area: Option<String>| {
            let mut world = world.borrow_mut();
            let area = area.or_else(|| Some(world.current()?.area.clone()));
            let Some(area) = area else {
                return Ok(0);
            };
            world
                .relayout(&area)
                .map_err(|error| mlua::Error::runtime(format!("could not save the map: {error}")))
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "remove_exit",
        lua.create_function(move |_, (from, direction): (Value, String)| {
            let result = world.borrow_mut().remove_exit(id_of(&from)?, &direction);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "remove_room",
        lua.create_function(move |_, id: Value| {
            let result = world.borrow_mut().remove_room(id_of(&id)?);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "move",
        lua.create_function(move |_, (id, x, y, z): (Value, i32, i32, Option<i32>)| {
            let pos = Pos {
                x,
                y,
                z: z.unwrap_or(0),
            };
            let result = world.borrow_mut().move_room(id_of(&id)?, pos);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "set_weight",
        lua.create_function(move |_, (id, weight): (Value, u32)| {
            let result = world.borrow_mut().set_weight(id_of(&id)?, weight);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "avoid",
        lua.create_function(move |_, (id, avoid): (Value, bool)| {
            let result = world.borrow_mut().set_avoid(id_of(&id)?, avoid);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "areas",
        lua.create_function(move |_, ()| {
            let areas: Vec<String> = (world.borrow().areas().into_iter())
                .map(str::to_owned)
                .collect();
            Ok(areas)
        })?,
    )?;

    api.set(
        "id",
        lua.create_function(|_, text: String| Ok(room_id(&text)))?,
    )?;
    lua.globals().set("map", api)
}

/// The error as a script author wants to read it: the message with its
/// `file:line`, without mlua's wrapping or the stack traceback.
fn describe(error: &mlua::Error) -> String {
    match error {
        mlua::Error::CallbackError { cause, .. } => describe(cause),
        mlua::Error::RuntimeError(message) => {
            let message = message.split("\nstack traceback:").next();
            message.unwrap_or_default().to_owned()
        }
        other => other.to_string(),
    }
}

/// Defines the global functions scripts call.
fn install_api(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);

    let shared = Rc::clone(state);
    add(
        "send",
        lua.create_function(move |_, text: String| {
            shared.borrow_mut().actions.push(Action::Send(text));
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "echo",
        lua.create_function(move |_, text: String| {
            shared.borrow_mut().actions.push(Action::Echo(text));
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "speak",
        lua.create_function(move |_, (text, options): (String, Option<Table>)| {
            let interrupt = match options {
                Some(options) => options.get::<Option<bool>>("interrupt")?.unwrap_or(false),
                None => false,
            };
            (shared.borrow_mut().actions).push(Action::Speak { text, interrupt });
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "gag",
        lua.create_function(move |_, ()| {
            shared.borrow_mut().gag = true;
            Ok(())
        })?,
    )?;

    for (name, is_alias) in [("trigger", false), ("alias", true)] {
        let shared = Rc::clone(state);
        add(
            name,
            lua.create_function(
                move |_, (pattern, callback, options): (String, Function, Option<Table>)| {
                    let regex = Regex::new(&pattern).map_err(mlua::Error::runtime)?;
                    let (group, once) = match options {
                        Some(options) => (
                            options.get::<Option<String>>("group")?,
                            options.get::<Option<bool>>("once")?.unwrap_or(false),
                        ),
                        None => (None, false),
                    };
                    let mut state = shared.borrow_mut();
                    let id = state.next_id();
                    let list = if is_alias {
                        &mut state.aliases
                    } else {
                        &mut state.triggers
                    };
                    list.push(Matcher {
                        id,
                        regex,
                        callback,
                        group,
                        once,
                    });
                    Ok(id)
                },
            )?,
        )?;
    }

    let shared = Rc::clone(state);
    add(
        "enable",
        lua.create_function(move |_, (group, enabled): (String, bool)| {
            let groups = &mut shared.borrow_mut().disabled_groups;
            if enabled {
                groups.remove(&group);
            } else {
                groups.insert(group);
            }
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "separator",
        lua.create_function(move |_, separator: Option<String>| {
            shared.borrow_mut().separator = Some(separator);
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "line_spans",
        lua.create_function(move |lua, ()| {
            let spans = lua.create_table()?;
            let state = shared.borrow();
            for span in state.current_line.iter().flat_map(|line| &line.spans) {
                let entry = lua.create_table()?;
                entry.set("text", span.text.as_str())?;
                entry.set("fg", color_value(lua, span.style.fg)?)?;
                entry.set("bg", color_value(lua, span.style.bg)?)?;
                entry.set("bold", span.style.bold)?;
                entry.set("italic", span.style.italic)?;
                entry.set("underline", span.style.underline)?;
                entry.set("inverse", span.style.inverse)?;
                spans.push(entry)?;
            }
            Ok(spans)
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "highlight",
        lua.create_function(move |_, (fg, bg): (Value, Value)| {
            let (fg, bg) = (color_from(&fg)?, color_from(&bg)?);
            let mut state = shared.borrow_mut();
            let Some((start, end)) = state.current_match else {
                return Ok(());
            };
            if let Some(line) = &mut state.current_line {
                line.restyle(start..end, |style| {
                    style.fg = fg.unwrap_or(style.fg);
                    style.bg = bg.unwrap_or(style.bg);
                });
            }
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "bind",
        lua.create_function(move |_, (key, action): (String, Value)| {
            let binding = match action {
                Value::String(command) => Binding::Command(command.to_string_lossy()),
                Value::Function(function) => Binding::Function(function),
                _ => return Err(mlua::Error::runtime("bind needs a command or a function")),
            };
            shared.borrow_mut().bindings.insert(key_name(&key), binding);
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "unbind",
        lua.create_function(move |_, key: String| {
            Ok(shared
                .borrow_mut()
                .bindings
                .remove(&key_name(&key))
                .is_some())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "remember",
        lua.create_function(move |_, (key, value): (String, Value)| {
            let mut state = shared.borrow_mut();
            match value {
                Value::Nil => {
                    state.memory.remove(&key);
                }
                value => {
                    let json = serde_json::to_value(&value).map_err(mlua::Error::runtime)?;
                    state.memory.insert(key, json);
                }
            }
            let Some(file) = &state.memory_file else {
                return Ok(());
            };
            // Written to a temporary file first, so a crash cannot leave
            // the real one half written.
            let text = serde_json::to_string_pretty(&state.memory).map_err(mlua::Error::runtime)?;
            let temporary = file.with_extension("json.tmp");
            std::fs::write(&temporary, text)
                .and_then(|()| std::fs::rename(&temporary, file))
                .map_err(|error| {
                    mlua::Error::runtime(format!("could not save {}: {error}", file.display()))
                })
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "recall",
        lua.create_function(move |lua, key: String| {
            let options = SerializeOptions::new()
                .serialize_none_to_null(false)
                .serialize_unit_to_null(false)
                .set_array_metatable(false);
            match shared.borrow().memory.get(&key) {
                Some(value) => lua.to_value_with(value, options),
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "play",
        lua.create_function(move |_, (file, options): (String, Option<Table>)| {
            let (volume, looped) = match options {
                Some(options) => (
                    options.get::<Option<f32>>("volume")?.unwrap_or(1.0),
                    options.get::<Option<bool>>("loop")?.unwrap_or(false),
                ),
                None => (1.0, false),
            };
            (shared.borrow_mut().actions).push(Action::PlaySound {
                file,
                volume: volume.clamp(0.0, 1.0),
                looped,
            });
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "stop_sounds",
        lua.create_function(move |_, ()| {
            shared.borrow_mut().actions.push(Action::StopSounds);
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "require",
        lua.create_function(move |lua, name: String| {
            if !valid_module_name(&name) {
                return Err(mlua::Error::runtime(format!("invalid module name: {name}")));
            }
            let (cached, dirs) = {
                let state = shared.borrow();
                if state.loading.contains(&name) {
                    return Err(mlua::Error::runtime(format!("circular require of {name}")));
                }
                (state.modules.get(&name).cloned(), state.module_dirs.clone())
            };
            if let Some(module) = cached {
                return Ok(module);
            }
            let candidates = dirs.iter().flat_map(|dir| {
                [
                    dir.join(format!("{name}.luau")),
                    dir.join(&name).join("init.luau"),
                ]
            });
            let Some(code) = candidates
                .into_iter()
                .find_map(|path| std::fs::read_to_string(path).ok())
            else {
                return Err(mlua::Error::runtime(format!("module not found: {name}")));
            };
            shared.borrow_mut().loading.push(name.clone());
            let loaded = lua.load(code).set_name(format!("@{name}")).eval::<Value>();
            let mut state = shared.borrow_mut();
            state.loading.retain(|loading| *loading != name);
            let module = loaded?;
            state.modules.insert(name, module.clone());
            Ok(module)
        })?,
    )?;

    for (name, repeats) in [("after", false), ("every", true)] {
        let shared = Rc::clone(state);
        add(
            name,
            lua.create_function(move |_, (seconds, callback): (f64, Function)| {
                let delay = Duration::try_from_secs_f64(seconds)
                    .map_err(|_| mlua::Error::runtime("the time must be a positive number"))?
                    .max(MIN_TIMER_INTERVAL);
                let mut state = shared.borrow_mut();
                let id = state.next_id();
                state.timers.push(Timer {
                    id,
                    due: Instant::now() + delay,
                    period: repeats.then_some(delay),
                    callback,
                });
                Ok(id)
            })?,
        )?;
    }

    // GMCP packages and MSDP variables share one store; the key function
    // keeps their names apart.
    type KeyFn = fn(&str) -> String;
    let gmcp_key: KeyFn = |package| package.to_lowercase();
    for (name, key) in [("on_gmcp", gmcp_key), ("on_msdp", msdp_key as KeyFn)] {
        let shared = Rc::clone(state);
        add(
            name,
            lua.create_function(move |_, (source, callback): (String, Function)| {
                let mut state = shared.borrow_mut();
                let id = state.next_id();
                state.data_handlers.push(DataHandler {
                    id,
                    key: key(&source),
                    callback,
                });
                Ok(id)
            })?,
        )?;
    }
    for (name, key) in [("gmcp", gmcp_key), ("msdp", msdp_key as KeyFn)] {
        let shared = Rc::clone(state);
        add(
            name,
            lua.create_function(move |_, source: String| {
                Ok(shared.borrow().data.get(&key(&source)).cloned())
            })?,
        )?;
    }

    let shared = Rc::clone(state);
    add(
        "send_msdp",
        lua.create_function(move |_, (variable, values): (String, Variadic<String>)| {
            (shared.borrow_mut().actions).push(Action::Msdp {
                variable,
                values: values.into_iter().collect(),
            });
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "mssp",
        lua.create_function(move |lua, ()| {
            let status = lua.create_table()?;
            for (name, value) in &shared.borrow().mssp {
                status.set(name.as_str(), value.as_str())?;
            }
            Ok(status)
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "send_gmcp",
        lua.create_function(move |_, (package, data): (String, Value)| {
            let data = match data {
                Value::Nil => String::new(),
                data => serde_json::to_string(&data).map_err(mlua::Error::runtime)?,
            };
            (shared.borrow_mut().actions).push(Action::Gmcp { package, data });
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "on_event",
        lua.create_function(move |_, (name, callback): (String, Function)| {
            let mut state = shared.borrow_mut();
            let id = state.next_id();
            state.data_handlers.push(DataHandler {
                id,
                key: event_key(&name),
                callback,
            });
            Ok(id)
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "supports",
        lua.create_function(move |_, modules: mlua::Variadic<String>| {
            (shared.borrow_mut().actions).push(Action::Supports(modules.into_iter().collect()));
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "completions",
        lua.create_function(move |_, (name, words): (String, Option<Vec<String>>)| {
            let words = words.unwrap_or_default();
            (shared.borrow_mut().actions).push(Action::Completions { name, words });
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "gauge",
        lua.create_function(
            move |_,
                  (name, value, max, options): (
                String,
                Option<f64>,
                Option<f64>,
                Option<mlua::Table>,
            )| {
                let level = match (value, max) {
                    (Some(value), Some(max)) => Some((value, max)),
                    (None, None) => None,
                    _ => return Err(mlua::Error::runtime("gauge needs both a value and a max")),
                };
                let mut look = GaugeLook::default();
                if let Some(options) = options {
                    let colour: Option<String> = options.get("colour")?;
                    let color: Option<String> = options.get("color")?;
                    look.colour = colour.or(color).unwrap_or_default();
                    let style: Option<String> = options.get("style")?;
                    look.segments = style.as_deref() == Some("segments");
                    look.low = options.get::<Option<f64>>("low")?.unwrap_or(0.0);
                }
                (shared.borrow_mut().actions).push(Action::Gauge { name, level, look });
                Ok(())
            },
        )?,
    )?;

    let shared = Rc::clone(state);
    add(
        "panel",
        lua.create_function(move |_, (name, text): (String, Option<String>)| {
            (shared.borrow_mut().actions).push(Action::Panel { name, text });
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "chat",
        lua.create_function(move |_, text: String| {
            shared.borrow_mut().actions.push(Action::Chat(text));
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "cancel",
        lua.create_function(move |_, id: u64| {
            let mut state = shared.borrow_mut();
            let count = |state: &State| {
                state.triggers.len()
                    + state.aliases.len()
                    + state.timers.len()
                    + state.data_handlers.len()
            };
            let before = count(&state);
            state.triggers.retain(|trigger| trigger.id != id);
            state.aliases.retain(|alias| alias.id != id);
            state.timers.retain(|timer| timer.id != id);
            state.data_handlers.retain(|handler| handler.id != id);
            Ok(count(&state) < before)
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(code: &str) -> Engine {
        let mut engine = Engine::new(Rc::default()).unwrap();
        engine.exec("test", code);
        assert_eq!(engine.take_actions(), [], "setup script failed");
        engine
    }

    fn send(text: &str) -> Action {
        Action::Send(text.into())
    }

    fn error_text(actions: &[Action]) -> &str {
        match actions {
            [Action::Error(message)] => message,
            other => panic!("expected one error, got {other:?}"),
        }
    }

    #[test]
    fn trigger_gets_groups_and_the_line() {
        let mut engine = engine(
            r#"trigger("^(?<who>\\w+) tells you: (.*)$", function(m, line)
                send(`reply {m.who} got {m[2]}`)
                echo(m[0] == line and "whole" or "partial")
            end)"#,
        );
        assert!(engine.on_line("Ana tells you: hola"));
        assert_eq!(
            engine.take_actions(),
            [send("reply Ana got hola"), Action::Echo("whole".into())]
        );
        assert!(engine.on_line("Nothing here"));
        assert_eq!(engine.take_actions(), []);
    }

    #[test]
    fn gag_hides_only_the_line_that_fired() {
        let mut engine = engine(r#"trigger("spam", function() gag() end)"#);
        assert!(!engine.on_line("some spam here"));
        assert!(engine.on_line("a normal line"));
    }

    #[test]
    fn alias_replaces_input_and_its_sends_are_not_expanded() {
        let mut engine = engine(
            r#"alias("^k (.+)$", function(m) send("k " .. m[1]) send("kill " .. m[1]) end)"#,
        );
        engine.on_input("k orc");
        assert_eq!(engine.take_actions(), [send("k orc"), send("kill orc")]);
        engine.on_input("look");
        assert_eq!(engine.take_actions(), [send("look")]);
    }

    #[test]
    fn timers_fire_repeat_and_cancel() {
        let mut engine = engine(
            r#"after(1, function() send("once") end)
            ticker = every(2, function() send("tick") end)
            alias("^stop$", function() echo(tostring(cancel(ticker))) end)"#,
        );
        let start = Instant::now();
        let at = |seconds: f64| start + Duration::from_secs_f64(seconds);
        engine.tick(at(0.5));
        assert_eq!(engine.take_actions(), []);
        engine.tick(at(1.5));
        assert_eq!(engine.take_actions(), [send("once")]);
        engine.tick(at(2.5));
        engine.tick(at(3.0));
        assert_eq!(engine.take_actions(), [send("tick")]);
        engine.tick(at(5.0));
        assert_eq!(engine.take_actions(), [send("tick")]);
        assert!(engine.next_deadline().is_some());
        engine.on_input("stop");
        assert_eq!(engine.take_actions(), [Action::Echo("true".into())]);
        assert_eq!(engine.next_deadline(), None);
    }

    #[test]
    fn callbacks_may_change_triggers_while_a_line_is_dispatched() {
        let mut engine = engine(
            r#"local second
            trigger("go", function() cancel(second) trigger("go", function() send("late") end) end)
            second = trigger("go", function() send("never") end)"#,
        );
        engine.on_line("go");
        assert_eq!(engine.take_actions(), []);
        engine.on_line("go");
        assert_eq!(engine.take_actions(), [send("late")]);
    }

    #[test]
    fn sandbox_has_no_file_or_process_access() {
        let mut engine = engine("");
        engine.exec(
            "probe",
            r#"echo(`{io} {package} {debug} {os.execute} {os.remove} {loadfile} {dofile}`)"#,
        );
        assert_eq!(
            engine.take_actions(),
            [Action::Echo("nil nil nil nil nil nil nil".into())]
        );
        engine.exec("probe", "send = nil; string.rep = nil");
        assert!(error_text(&engine.take_actions()).contains("readonly"));
    }

    #[test]
    fn runaway_script_is_stopped() {
        let mut engine = engine(r#"trigger("loop", function() while true do end end)"#);
        let started = Instant::now();
        engine.on_line("loop");
        assert!(error_text(&engine.take_actions()).contains("ran too long"));
        assert!(started.elapsed() < TIME_BUDGET * 4);
        // The engine keeps working afterwards.
        engine.on_input("/lua send('alive')");
        assert_eq!(engine.take_actions(), [send("alive")]);
    }

    #[test]
    fn errors_are_reported_and_globals_persist_between_chunks() {
        let mut engine = engine("counter = 41");
        engine.on_input("/lua counter += 1; send(tostring(counter))");
        assert_eq!(engine.take_actions(), [send("42")]);
        engine.on_input("/lua trigger('(', print)");
        assert!(error_text(&engine.take_actions()).contains("regex parse error"));
        engine.on_input("/lua error('boom')");
        assert_eq!(error_text(&engine.take_actions()), "input:1: boom");
    }

    #[test]
    fn gmcp_data_reaches_handlers_and_is_remembered() {
        let mut engine = engine(
            r#"on_gmcp("char", function(data, package) echo(`{package}: {data.hp}/{data.maxhp}`) end)
            on_gmcp("Char.Vitals", function(data) gauge("HP", data.hp, data.maxhp) end)
            on_gmcp("Room.Info", function(data) echo(`{data.name} {#data.exits} {data.area}`) end)"#,
        );
        engine.on_gmcp("Char.Vitals", r#"{"hp": 80, "maxhp": 100}"#);
        assert_eq!(
            engine.take_actions(),
            [
                Action::Echo("Char.Vitals: 80/100".into()),
                Action::Gauge {
                    name: "HP".into(),
                    level: Some((80.0, 100.0)),
                    look: GaugeLook::default(),
                },
            ]
        );
        engine.on_gmcp(
            "Room.Info",
            r#"{"name": "Plaza", "exits": ["n", "s"], "area": null}"#,
        );
        assert_eq!(engine.take_actions(), [Action::Echo("Plaza 2 nil".into())]);
        // Packages that only share a prefix are not "under" the handler's.
        engine.on_gmcp("Charms.List", "[]");
        engine.on_gmcp("Core.Ping", "");
        assert_eq!(engine.take_actions(), []);

        engine.on_input("/lua echo(`{gmcp('char.vitals').hp} {gmcp('Nope')} {gmcp('Core.Ping')}`)");
        assert_eq!(engine.take_actions(), [Action::Echo("80 nil nil".into())]);
        engine.on_gmcp("Char.Vitals", "{not json");
        assert!(error_text(&engine.take_actions()).contains("bad data for Char.Vitals"));
    }

    #[test]
    fn scripts_can_send_gmcp_and_drive_panels() {
        let mut engine = engine("");
        engine.exec(
            "test",
            r#"send_gmcp("Core.Ping")
            send_gmcp("Core.Supports.Add", {"Char.Items 1"})
            send_gmcp("Char.Login", {name = "ana"})
            gauge("HP")
            chat("Ana: hola")
            map.walk(42)"#,
        );
        let gmcp = |package: &str, data: &str| Action::Gmcp {
            package: package.into(),
            data: data.into(),
        };
        assert_eq!(
            engine.take_actions(),
            [
                gmcp("Core.Ping", ""),
                gmcp("Core.Supports.Add", r#"["Char.Items 1"]"#),
                gmcp("Char.Login", r#"{"name":"ana"}"#),
                Action::Gauge {
                    name: "HP".into(),
                    level: None,
                    look: GaugeLook::default(),
                },
                Action::Chat("Ana: hola".into()),
                Action::Walk(42),
            ]
        );
        engine.exec("test", r#"gauge("HP", 5)"#);
        assert!(error_text(&engine.take_actions()).contains("both a value and a max"));
    }

    #[test]
    fn msdp_and_mssp_are_available_to_scripts() {
        let mut engine = engine(
            r#"on_msdp("HEALTH", function(value, name) echo(`{name}={value}`) end)
            on_gmcp("health", function() echo("wrong kind") end)"#,
        );
        engine.on_msdp("HEALTH", r#""80""#);
        engine.on_msdp("ROOM", r#"{"VNUM":"7","EXITS":{"n":"8"}}"#);
        engine.on_mssp(&[("NAME".into(), "FakeMUD".into())]);
        assert_eq!(engine.take_actions(), [Action::Echo("HEALTH=80".into())]);
        engine.exec(
            "test",
            r#"echo(`{msdp("room").EXITS.n} {gmcp("room")} {mssp().NAME} {mssp().PLAYERS}`)
            send_msdp("REPORT", "HEALTH", "MANA")"#,
        );
        assert_eq!(
            engine.take_actions(),
            [
                Action::Echo("8 nil FakeMUD nil".into()),
                Action::Msdp {
                    variable: "REPORT".into(),
                    values: vec!["HEALTH".into(), "MANA".into()]
                },
            ]
        );
    }

    #[test]
    fn script_can_load_the_map_from_text() {
        // A MUD with no GMCP: the room is read from its title and exits
        // lines, and identified by the title since there is no number.
        let mut engine = engine(
            r#"map.native(false)
            local title
            trigger("^\\[ (.+) \\]$", function(m) title = m[1] end)
            trigger("^Exits: (.+)$", function(m)
                local exits = {}
                for direction, to in m[1]:gmatch("(%a+)=(%a+)") do exits[direction] = to end
                map.visit({id = title, name = title, area = "town", exits = exits})
            end)"#,
        );
        assert!(!engine.native_map());
        for line in [
            "[ Gate ]",
            "Exits: north=Square",
            "[ Square ]",
            "Exits: south=Gate east=Tower",
        ] {
            engine.on_line(line);
        }
        let (gate, square) = (room_id("Gate"), room_id("Square"));
        assert_eq!(
            engine.take_actions(),
            [Action::Entered(gate), Action::Entered(square)]
        );
        {
            let map = engine.map.borrow();
            assert_eq!(map.current().unwrap().name, "Square");
            assert_eq!(map.room(gate).unwrap().exits["n"].to, square);
            let (from, to) = (map.room(gate).unwrap().pos, map.room(square).unwrap().pos);
            assert_eq!((to.x - from.x, to.y - from.y), (0, 1));
        }
        engine.exec(
            "test",
            r#"local here = map.room()
            echo(`{here.name} {here.exits.s == map.id("Gate")} {map.room("Tower")}`)
            echo(table.concat(map.path("Gate"), ","))
            map.walk("Gate")"#,
        );
        assert_eq!(
            engine.take_actions(),
            [
                Action::Echo("Square true nil".into()),
                Action::Echo("s".into()),
                Action::Walk(gate),
            ]
        );
    }

    #[test]
    fn script_can_take_the_whole_map_from_the_server() {
        // The server sends each area complete: nothing is saved, and the
        // previous area is dropped when a new one arrives.
        let mut engine = engine(
            r#"map.native(false)
            map.persist(false)
            on_gmcp("Area.Map", function(area)
                map.clear()
                for _, room in area.rooms do
                    map.set_room({id = room.id, name = room.name, area = area.name,
                                  x = room.x, y = room.y, exits = room.exits})
                end
            end)
            on_gmcp("Area.Position", function(id) map.visit(map.room(id)) end)"#,
        );
        assert!(!engine.persist_map());
        engine.on_gmcp(
            "Area.Map",
            r#"{"name": "Docks", "rooms": [
                {"id": 1, "name": "Pier", "x": 0, "y": 0, "exits": {"e": 2}},
                {"id": 2, "name": "Wharf", "x": 1, "y": 0, "exits": {"w": 1}}]}"#,
        );
        engine.on_gmcp("Area.Position", "2");
        assert_eq!(engine.take_actions(), [Action::Entered(2)]);
        {
            let map = engine.map.borrow();
            assert_eq!(map.rooms().count(), 2);
            assert_eq!(map.current().unwrap().name, "Wharf");
            assert_eq!(map.room(2).unwrap().area, "Docks");
        }
        engine.on_gmcp(
            "Area.Map",
            r#"{"name": "Sea", "rooms": [{"id": 9, "name": "Raft", "x": 0, "y": 0}]}"#,
        );
        let map = engine.map.borrow();
        assert_eq!(map.rooms().count(), 1);
        assert!(map.current().is_none());
    }

    #[test]
    fn script_can_edit_and_query_the_map() {
        let mut engine = engine(
            r#"map.set_room({id = 1, name = "Gate", exits = {n = 2}})
            map.set_room({id = 2, name = "Square", x = 5, y = 5, exits = {south = 1}})
            map.set_room({id = 3, name = "Old Square"})"#,
        );
        assert!(engine.native_map());
        engine.exec(
            "test",
            r#"echo(`{map.room()} {map.path(2)} {#map.find("square")} {map.find("square")[2].id}`)
            echo(table.concat(map.path(2, 1), ",") .. " " .. tostring(map.path(3, 1)))
            echo(`{map.set_exit(2, "enter portal", 3)} {map.set_exit(9, "n", 1)}`)
            echo(table.concat(map.path(3, 1), ";"))
            echo(`{map.move(3, 7, 8)} {map.room(3).x},{map.room(3).y},{map.room(3).z}`)
            echo(`{map.remove_exit(2, "enter portal")} {map.remove_room(3)} {map.room(3)}`)
            map.set_room({id = 4, name = "Detour", area = "wilds", exits = {n = 2}})
            map.set_exit(1, "e", 4)
            echo(`{map.avoid(2, true)} {map.room(2).avoid} {map.path(2, 1)[1]} {map.path(4, 1)[1]}`)
            echo(`{map.set_weight(4, 9)} {map.room(4).weight} {table.concat(map.areas(), "+")}`)
            map.avoid(2, false)
            map.set_exit(1, "gate", 2, {command = "open gate;n", cost = 0})
            map.edit_exit(1, "n", {locked = true})
            local gate = map.exit(1, "gate")
            echo(`{map.path(2, 1)[1]} {gate.custom} {gate.locked} {map.exit(1, "n").locked} {map.exit(1, "x")}`)
            echo(`{map.edit_exit(1, "gate", {command = ""})} {map.exit(1, "gate").command} {map.edit_exit(1, "x", {})}`)
            echo(`{map.relayout("nowhere")} {map.relayout()}`)
            panel("Minimap", "+-+\n| |")
            panel("Minimap")
            map.visit({name = "No id"})"#,
        );
        let actions = engine.take_actions();
        let echoes: Vec<&str> = (actions.iter())
            .filter_map(|action| match action {
                Action::Echo(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            echoes,
            [
                "nil nil 2 2",
                "n nil",
                "true false",
                "n;enter portal",
                "true 7,8,0",
                "true true nil",
                "true true n e",
                "true 9 +wilds",
                "open gate;n true false true nil",
                "true nil false",
                "0 0",
            ]
        );
        assert_eq!(
            actions[11..13],
            [
                Action::Panel {
                    name: "Minimap".into(),
                    text: Some("+-+\n| |".into())
                },
                Action::Panel {
                    name: "Minimap".into(),
                    text: None
                },
            ]
        );
        assert!(error_text(&actions[13..]).contains("a room id must be"));
    }

    #[test]
    fn several_commands_on_one_line_each_go_through_aliases() {
        let mut engine = engine(r#"alias("^k (.+)$", function(m) send("kill " .. m[1]) end)"#);
        engine.on_input("n; k orc ;say hi;; bye;");
        assert_eq!(
            engine.take_actions(),
            [send("n"), send("kill orc"), send("say hi; bye"), send("")]
        );
        engine.on_input("");
        assert_eq!(engine.take_actions(), [send("")]);
        engine.on_input("/lua separator('|')");
        engine.on_input("n|s;e");
        assert_eq!(engine.take_actions(), [send("n"), send("s;e")]);
        engine.on_input("/lua separator(nil)");
        engine.on_input("n|s;e");
        assert_eq!(engine.take_actions(), [send("n|s;e")]);
    }

    #[test]
    fn groups_switch_triggers_off_and_once_fires_a_single_time() {
        let mut engine = engine(
            r#"trigger("hit", function() send("flee") end, {group = "combat"})
            trigger("hit", function() send("first blood") end, {once = true})
            alias("^x$", function() send("special") end, {group = "combat"})"#,
        );
        engine.on_line("you are hit");
        assert_eq!(engine.take_actions(), [send("flee"), send("first blood")]);
        engine.on_input("/lua enable('combat', false)");
        engine.on_line("you are hit");
        engine.on_input("x");
        assert_eq!(engine.take_actions(), [send("x")]);
        engine.on_input("/lua enable('combat', true)");
        engine.on_line("you are hit");
        engine.on_input("x");
        assert_eq!(engine.take_actions(), [send("flee"), send("special")]);
    }

    #[test]
    fn trigger_can_read_the_colours_of_its_line() {
        use muddie_core::{Span, Style};
        let mut engine = engine(
            r#"trigger("orc", function()
                local parts = {}
                for _, span in line_spans() do
                    table.insert(parts, `{span.text}:{span.fg}:{span.bold}`)
                end
                echo(table.concat(parts, "|") .. " " .. #line_spans())
            end)"#,
        );
        let span = |text: &str, style: Style| Span {
            text: text.into(),
            style,
        };
        let line = Line {
            spans: vec![
                span("An ", Style::default()),
                span(
                    "orc",
                    Style {
                        fg: Color::Indexed(1),
                        bold: true,
                        ..Style::default()
                    },
                ),
                span(
                    "!",
                    Style {
                        fg: Color::Rgb(255, 128, 0),
                        ..Style::default()
                    },
                ),
            ],
            links: Vec::new(),
        };
        assert_eq!(engine.on_styled_line(&line), Some(line.clone()));
        assert_eq!(
            engine.take_actions(),
            [Action::Echo(
                "An :nil:false|orc:1:true|!:#ff8000:false 3".into()
            )]
        );
        // Outside a trigger there is no line to describe.
        engine.on_input("/lua echo(tostring(#line_spans()))");
        assert_eq!(engine.take_actions(), [Action::Echo("0".into())]);
    }

    #[test]
    fn require_loads_modules_once_from_the_script_folder_only() {
        let dir = std::env::temp_dir().join(format!("muddie-require-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("combat")).unwrap();
        std::fs::write(
            dir.join("util.luau"),
            "loads = (loads or 0) + 1\nreturn {double = function(n) return n * 2 end}",
        )
        .unwrap();
        std::fs::write(
            dir.join("combat/init.luau"),
            "return require('util').double(21)",
        )
        .unwrap();
        std::fs::write(dir.join("loop.luau"), "return require('loop')").unwrap();
        std::fs::write(
            dir.join("main.luau"),
            "echo(`{require('util').double(4)} {require('combat')} {require('util') == require('util')} {loads}`)",
        )
        .unwrap();
        let mut engine = Engine::new(Rc::default()).unwrap();
        engine.load_file(&dir.join("main.luau"));
        assert_eq!(engine.take_actions(), [Action::Echo("8 42 true 1".into())]);
        for (name, problem) in [
            ("../util", "invalid module name"),
            ("/etc/passwd", "invalid module name"),
            ("missing", "module not found"),
            ("loop", "circular require"),
        ] {
            engine.on_input(&format!("/lua require('{name}')"));
            assert!(
                error_text(&engine.take_actions()).contains(problem),
                "{name}"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn trigger_can_recolour_its_match_or_gag_the_line() {
        use muddie_core::{Span, Style};
        let mut engine = engine(
            r##"trigger("An (\\w+)", function() highlight("bright yellow", "#102030") end)
            trigger("^secret", function() gag() end)"##,
        );
        let plain = |text: &str| Line {
            spans: vec![Span {
                text: text.into(),
                style: Style::default(),
            }],
            links: Vec::new(),
        };
        let shown = engine.on_styled_line(&plain("An orc!")).unwrap();
        let styles: Vec<(&str, Color, Color)> = (shown.spans.iter())
            .map(|span| (span.text.as_str(), span.style.fg, span.style.bg))
            .collect();
        assert_eq!(
            styles,
            [
                ("An orc", Color::Indexed(11), Color::Rgb(0x10, 0x20, 0x30)),
                ("!", Color::Default, Color::Default),
            ]
        );
        assert_eq!(engine.on_styled_line(&plain("secret plans")), None);
        engine.on_input("/lua highlight('plaid')");
        assert!(error_text(&engine.take_actions()).contains("a colour is"));
    }

    #[test]
    fn keys_run_their_binding() {
        let mut engine = engine(
            r#"bind("F5", "look;inventory")
            bind("Shift + CTRL+k", function() send("kick") end)"#,
        );
        assert!(engine.on_key("f5"));
        assert!(engine.on_key("ctrl+shift+k"));
        assert!(!engine.on_key("f6"));
        assert_eq!(
            engine.take_actions(),
            [send("look"), send("inventory"), send("kick")]
        );
        engine.on_input("/lua echo(`{unbind('f5')} {unbind('f5')}`)");
        assert!(!engine.on_key("f5"));
        assert_eq!(engine.take_actions(), [Action::Echo("true false".into())]);
        assert_eq!(key_name("Shift+Alt+Ctrl+Enter"), "ctrl+alt+shift+enter");
    }

    #[test]
    fn remembered_values_survive_a_new_engine() {
        let file =
            std::env::temp_dir().join(format!("muddie-memory-test-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let mut engine = engine("");
        engine.use_memory_file(file.clone());
        engine.exec(
            "test",
            r#"remember("kills", 41)
            remember("friends", {"ana", "luis"})
            remember("config", {speed = "fast"})
            remember("gone", 1)
            remember("gone", nil)"#,
        );
        assert_eq!(engine.take_actions(), []);
        let mut engine = self::engine("");
        engine.use_memory_file(file.clone());
        engine.exec(
            "test",
            r#"echo(`{recall("kills") + 1} {recall("friends")[2]} {recall("config").speed} {recall("gone")}`)"#,
        );
        assert_eq!(
            engine.take_actions(),
            [Action::Echo("42 luis fast nil".into())]
        );

        std::fs::write(&file, "not json").unwrap();
        let mut engine = self::engine("");
        engine.use_memory_file(file.clone());
        assert!(error_text(&engine.take_actions()).contains("not loaded"));
        // The damaged file is left alone rather than overwritten.
        engine.exec("test", r#"remember("x", 1)"#);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "not json");
        std::fs::remove_file(&file).unwrap();
    }

    #[test]
    fn sounds_are_requested_with_their_options() {
        let mut engine = engine("");
        engine.exec(
            "test",
            r#"play("alert.ogg") play("rain.ogg", {volume = 4, loop = true}) stop_sounds()"#,
        );
        let sound = |file: &str, volume, looped| Action::PlaySound {
            file: file.into(),
            volume,
            looped,
        };
        assert_eq!(
            engine.take_actions(),
            [
                sound("alert.ogg", 1.0, false),
                sound("rain.ogg", 1.0, true),
                Action::StopSounds
            ]
        );
    }

    /// The scripts shipped as examples must at least load without error.
    #[test]
    fn example_scripts_load() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/scripts");
        let mut files: Vec<PathBuf> = (std::fs::read_dir(&folder).unwrap())
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "luau")
            })
            .collect();
        files.sort();
        assert!(
            files.len() >= 6,
            "examples not found in {}",
            folder.display()
        );
        for file in files {
            let mut engine = Engine::new(Rc::default()).unwrap();
            engine.load_file(&file);
            // Announcing a module is all an example may do on loading:
            // anything else would be an error it reported.
            let mut actions = engine.take_actions();
            actions.retain(|action| !matches!(action, Action::Supports(_)));
            assert_eq!(actions, [], "{}", file.display());
        }
    }

    /// The text mapper example maps a room given in its expected format.
    #[test]
    fn text_mapper_example_maps_rooms() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/scripts");
        let mut engine = Engine::new(Rc::default()).unwrap();
        engine.load_file(&folder.join("text-mapper.luau"));
        engine.on_line("[ Market Square ]");
        engine.on_line("Exits: north=Gate east=Alley");
        let map = engine.map.borrow();
        let room = map.current().unwrap();
        assert_eq!((room.name.as_str(), room.exits.len()), ("Market Square", 2));
    }

    #[test]
    fn client_events_modules_and_completions() {
        let mut engine = Engine::new(Rc::default()).unwrap();
        engine.exec(
            "test",
            r#"
            supports("Map.Render 1", "Dev 1")
            on_event("panel", function(size) echo(size.columns .. "x" .. size.rows) end)
            on_event("connect", function() echo("in") end)
            completions("cmds", {"north", "look"})
            completions("cmds")
            "#,
        );
        engine.on_event("panel", r#"{"columns": 40, "rows": 30}"#);
        engine.on_event("connect", "");
        assert_eq!(
            engine.take_actions(),
            [
                Action::Supports(vec!["Map.Render 1".into(), "Dev 1".into()]),
                Action::Completions {
                    name: "cmds".into(),
                    words: vec!["north".into(), "look".into()]
                },
                Action::Completions {
                    name: "cmds".into(),
                    words: Vec::new()
                },
                Action::Echo("40x30".into()),
                Action::Echo("in".into()),
            ]
        );
        // A package of the same name from the server is not the event.
        engine.on_gmcp("panel", r#"{"columns": 1, "rows": 1}"#);
        assert_eq!(engine.take_actions(), []);
    }

    #[test]
    fn failing_repeating_timer_is_dropped() {
        let mut engine = engine(r#"every(1, function() error("bad") end)"#);
        engine.tick(Instant::now() + Duration::from_secs(2));
        assert!(error_text(&engine.take_actions()).contains("bad"));
        assert_eq!(engine.next_deadline(), None);
    }

    #[test]
    fn reload_runs_files_again_from_a_clean_state() {
        let dir = std::env::temp_dir().join(format!("muddie-script-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("init.luau");
        std::fs::write(&file, r#"trigger("hi", function() send("v1") end)"#).unwrap();
        let mut engine = Engine::new(Rc::default()).unwrap();
        engine.load_file(&file);
        engine.on_input("/lua stray = 1");
        std::fs::write(
            &file,
            r#"trigger("hi", function() send("v2 " .. tostring(stray)) end)"#,
        )
        .unwrap();
        engine.on_input("/reload");
        assert_eq!(
            engine.take_actions(),
            [Action::Echo("Scripts reloaded.".into())]
        );
        engine.on_line("hi");
        assert_eq!(engine.take_actions(), [send("v2 nil")]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
