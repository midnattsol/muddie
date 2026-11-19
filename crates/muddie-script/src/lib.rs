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
//! - `trigger(pattern, fn)`: call `fn(m, line)` for each output line matching
//!   the regular expression. `m[0]` is the whole match, `m[1]`... the groups,
//!   and named groups are also available as `m.name`.
//! - `gag()`: inside a trigger, hide the line that fired it.
//! - `alias(pattern, fn)`: call `fn(m, input)` instead of sending typed input
//!   that matches.
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
//! - `gauge(name, value, max)`: show or update a bar in the status strip;
//!   `gauge(name)` removes it.
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
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::serde::SerializeOptions;
use mlua::{Function, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, Variadic, VmState};
use muddie_mapper::{Exit, Map, Pos, Room, RoomId, RoomInfo, room_id};
use regex::Regex;

/// Longest a single script run or callback may take before it is stopped,
/// so a faulty script cannot freeze the client.
const TIME_BUDGET: Duration = Duration::from_millis(500);
const MEMORY_LIMIT: usize = 256 << 20;
const MIN_TIMER_INTERVAL: Duration = Duration::from_millis(10);

/// Something a script asked the client to do.
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
    },
    /// Add a line to the chat panel.
    Chat(String),
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
    /// A script failed; the message is for the user.
    Error(String),
}

struct Matcher {
    id: u64,
    regex: Regex,
    callback: Function,
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
        self.state.borrow_mut().gag = false;
        for hit in self.hits(line, |state| &state.triggers, usize::MAX) {
            let still_there = (self.state.borrow().triggers.iter()).any(|t| t.id == hit.id);
            if still_there {
                self.fire(&hit, line);
            }
        }
        !std::mem::take(&mut self.state.borrow_mut().gag)
    }

    /// Handles a line typed by the user: a client command, an alias, or
    /// otherwise a command to send as it is.
    pub fn on_input(&mut self, input: &str) {
        if let Some(code) = input.strip_prefix("/lua ") {
            self.exec("input", code);
        } else if input == "/reload" {
            self.reload();
        } else if let Some(hit) = self.hits(input, |state| &state.aliases, 1).pop() {
            self.fire(&hit, input);
        } else {
            self.push(Action::Send(input.to_owned()));
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
        let matches = list(&state).iter().filter_map(|matcher| {
            let captures = matcher.regex.captures(text)?;
            let group = |m: regex::Match| m.as_str().to_owned();
            Some(Hit {
                id: matcher.id,
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
            lua.create_function(move |_, (pattern, callback): (String, Function)| {
                let regex = Regex::new(&pattern).map_err(mlua::Error::runtime)?;
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
                });
                Ok(id)
            })?,
        )?;
    }

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
        "gauge",
        lua.create_function(
            move |_, (name, value, max): (String, Option<f64>, Option<f64>)| {
                let level = match (value, max) {
                    (Some(value), Some(max)) => Some((value, max)),
                    (None, None) => None,
                    _ => return Err(mlua::Error::runtime("gauge needs both a value and a max")),
                };
                (shared.borrow_mut().actions).push(Action::Gauge { name, level });
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
            r#"echo(`{io} {package} {require} {debug} {os.execute} {os.remove} {loadfile} {dofile}`)"#,
        );
        assert_eq!(
            engine.take_actions(),
            [Action::Echo("nil nil nil nil nil nil nil nil".into())]
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
                    level: Some((80.0, 100.0))
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
                    level: None
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
