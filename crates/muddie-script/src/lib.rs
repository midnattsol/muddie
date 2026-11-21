//! Triggers, aliases, timers and the Luau runtime that scripts them.
//!
//! The [`Engine`] has no I/O of its own: the frontend feeds it output lines,
//! typed input and the clock, and carries out the [`Action`]s it returns.
//!
//! Functions available to scripts:
//!
//! - `send(text)`: send a command to the MUD (aliases are not applied).
//! - `echo(text)`: show a line in the output.
//! - `log(text)`: show something on the standard output of a run started
//!   with `--debug`, to see what a script made of what it was given;
//!   nothing otherwise.
//! - `speak(text, {interrupt = true})`: say something through speech.
//! - `trigger(pattern, fn, options)`: call `fn(m, line)` for each output
//!   line matching the regular expression. `m[0]` is the whole match,
//!   `m[1]`... the groups, and named groups are also available as `m.name`.
//!   The optional `options` table takes `name` (its place, as
//!   `Folder/Name`, to switch it by), `tags` (a list of tags, to switch it
//!   with everything else that carries one of them) and `once` (remove it
//!   after it fires).
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
//! - `triggers`, `aliases`, `macros`, `timers` and `scripts`, each with
//!   `enable(place)`, `disable(place)`, `toggle(place)`, `enabled(place)`
//!   and `list()`: switch one thing of that kind by its place, or a folder
//!   with all it holds, for the session. `tags` has the same, for all that
//!   carries a tag, whatever its kind. `bind`, `after` and `every` take
//!   `name` and `tags` as triggers and aliases do.
//! - `separator(text)`: the text that separates several commands typed on
//!   one line (`;` by default, doubled to type it literally); `nil` turns
//!   splitting off.
//! - `bind(key, action, options)`: run `action` (a command to send, or a
//!   function) when `key` is pressed, e.g. `bind("f5", "look")` or
//!   `bind("ctrl+shift+k", function() ... end)`; `unbind(key)` removes it.
//!   A key bound again under another `name` does that while it is on and
//!   the first is not: the first bound that is on is the one that runs.
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
//! - `vars`: the remembered values as a table: `vars.target = "orc"`
//!   remembers, `vars.target` recalls, and `for name, value in vars`
//!   goes through them. `expand(text)` puts them in place of each
//!   `$name` in `text`, as the commands of rules and key bindings do.
//! - `on_event(name, fn)`: call `fn` when something happens in the client:
//!   `"connect"`, `"disconnect"`, or `"panel"` with the size in characters
//!   of the panel beside the output.
//! - `panel_size(name)`: how many characters fit across and down a panel
//!   that shows a drawing, as `{columns, rows}`, or `nil` if it is not on
//!   the screen yet; the map's panel if none is named.
//! - `supports(module, ...)`: announce GMCP modules to the server while
//!   the script is loaded.
//! - `completions(name, words)`: words for Tab to offer.
//! - `gauge(name, value, max, options)`: show or update a bar in the status
//!   strip; `gauge(name)` removes it. `options` may give a `colour`, a
//!   `style` (`"bar"`, `"cells"`, `"number"`, `"light"` or `"text"`), the
//!   `text` of that last kind, the colour `off` of a light, and `steps`,
//!   the colour it takes under each percentage.
//! - `chat(text)`: add a line to the chat panel.
//! - `channel(name, text, {panel = "..."})`: add a line said on a channel
//!   of the MUD's, which has a tab of its own in the panel named, made if
//!   there is none of that name, or in the panel of channels if none is
//!   named. `channel(name)` makes the tab before anyone speaks.
//! - `panel_add(name, text)`: add `text` at the end of a panel, which keeps
//!   what it had, as a log does. In a trigger, `to_panel(name)` adds the
//!   line being looked at, with its colours.
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
//! - `map.arrive{name = ..., exits = {...}, came = "n"}`: the player is
//!   now in a room the server showed by its name and exits, having taken
//!   the exit `came`, for servers that do not say which room it is: the
//!   map works out which, following each room it could be along the exits
//!   taken until they tell them apart. Each part is optional; without
//!   `came`, the move sent last that no room has answered yet is taken
//!   for it. Returns the room's number, or `nil` while unsure, which the
//!   user is told.
//! - `map.here(id)`: the player is in that room; false if there is none.
//! - `map.directions{n = {"norte", "n"}, w = {"oeste", "o"}, ...}`: the
//!   server's words for each direction, the first of each being what a
//!   walk sends. The map understands them wherever a direction is given.
//! - `map.import(address)`: take the map made with Mudlet at `address`, a
//!   `.dat` file or a zip holding one, on the web or on disk, in place of
//!   the map there is. A map taken from there already is taken again only
//!   if the file has changed, so a plugin may call it on every connection
//!   to keep it up to date. The user is told how it goes.
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
//! - `map.walk(to)`: walk there. `map.stop()`: the server said a move
//!   failed: the moves sent did not happen, and a walk stops.
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
use muddie_core::{Color, Line};
use muddie_mapper::{Exit, Located, Map, Pos, Room, RoomId, RoomInfo, Sighting, room_id};
use regex::Regex;

mod action;
mod api;
mod convert;
mod events;
mod map_api;
mod memory;
mod state;
mod switches;

pub use action::{Action, GaugeLook};
use api::install_api;
use convert::*;
use map_api::*;
use state::*;
pub use switches::{Belongs, Kind, Switches, places_up, tag_key};

/// Longest a single script run or callback may take before it is stopped,
/// so a faulty script cannot freeze the client.
const TIME_BUDGET: Duration = Duration::from_millis(500);
const MEMORY_LIMIT: usize = 256 << 20;
const MIN_TIMER_INTERVAL: Duration = Duration::from_millis(10);

impl DataHandler {
    fn covers(&self, key: &str) -> bool {
        match key.strip_prefix(self.key.as_str()) {
            Some(rest) => rest.is_empty() || rest.starts_with('.'),
            None => false,
        }
    }
}

impl State {
    fn next_id(&mut self) -> u64 {
        self.last_id += 1;
        self.last_id
    }

    /// Writes the remembered values to their file, if they have one.
    fn save_memory(&self) -> Result<(), String> {
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
    fn expand(&self, text: &str) -> String {
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
            Kind::Macro => (self.bindings.values().flatten())
                .map(|bound| &bound.belongs)
                .collect(),
            Kind::Timer => self.timers.iter().map(|timer| &timer.belongs).collect(),
            Kind::Script => self.scripts.iter().collect(),
        }
    }

    /// The places there are of a kind, with the folders they are in, in
    /// alphabetical order.
    fn place_names(&self, kind: Kind) -> Vec<String> {
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
    fn tag_names(&self) -> Vec<String> {
        let carried = (Kind::ALL.iter())
            .flat_map(|&kind| self.belongings(kind))
            .flat_map(|belongs| belongs.tags.iter().map(|tag| tag_key(tag)))
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
        // The remembered values as a table, over `remember` and `recall`.
        lua.load(
            r#"vars = setmetatable({}, {
                __index = function(_, name) return recall(name) end,
                __newindex = function(_, name, value) remember(name, value) end,
                __iter = function() return next, remembered() end,
            })"#,
        )
        .set_name("=vars")
        .exec()?;

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
                let owner = owner_of(path);
                self.exec_owned(&owner, &name.to_string_lossy(), &code);
            }
            Err(error) => self.error(format!("cannot read {}: {error}", path.display())),
        }
    }

    /// Runs a chunk of Luau code; `name` identifies it in error messages.
    /// Says whether `code` is Luau that could be run, without running it:
    /// the line and the message of its first mistake if it is not.
    pub fn check(&self, code: &str) -> Option<(usize, String)> {
        let error = self
            .lua
            .load(code)
            .set_name("=code")
            .into_function()
            .err()?;
        let text = error.to_string();
        // The message reads `code:3: what is wrong`.
        let located = text.split_once("code:").and_then(|(_, rest)| {
            let (line, message) = rest.split_once(':')?;
            let message = message.trim().lines().next().unwrap_or_default();
            Some((line.trim().parse().ok()?, message.to_owned()))
        });
        Some(located.unwrap_or((1, text)))
    }

    pub fn exec(&mut self, name: &str, code: &str) {
        self.exec_owned(name, name, code);
    }

    /// Runs a chunk of code as part of `owner`: what it makes, at once or
    /// later from what it made, is taken away by [`Self::unload`] of that
    /// owner. `name` identifies the chunk in error messages.
    pub fn exec_owned(&mut self, owner: &str, name: &str, code: &str) {
        // The "@" makes errors read `name:line: message`.
        let chunk = self.lua.load(code).set_name(format!("@{name}"));
        self.as_owner(owner, || chunk.exec());
    }

    /// Takes away what `owner` made: its triggers, aliases, timers, key
    /// bindings and handlers. What the other scripts made and what they
    /// keep is left as it is, so that one script can be run again without
    /// the rest starting over.
    pub fn unload(&mut self, owner: &str) {
        let mut state = self.state.borrow_mut();
        state.triggers.retain(|matcher| matcher.owner != owner);
        state.aliases.retain(|matcher| matcher.owner != owner);
        state.timers.retain(|timer| timer.owner != owner);
        state.data_handlers.retain(|handler| handler.owner != owner);
        let asked = state.supports.len();
        state.supports.retain(|(by, _)| by != owner);
        state.changed |= state.supports.len() != asked;
        state.bindings.retain(|_, bound| {
            bound.retain(|bound| bound.owner != owner);
            !bound.is_empty()
        });
        // A module it is asks for it afresh next time.
        let stem = Path::new(owner)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned());
        if let Some(stem) = stem {
            state.modules.remove(&stem);
        }
    }

    /// Runs a script file again by itself: what it made before is taken
    /// away first, and the other scripts are not touched.
    pub fn reload_file(&mut self, path: &Path) {
        self.unload(&owner_of(path));
        self.load_file(path);
    }

    /// Whether a script file is loaded.
    pub fn is_loaded(&self, path: &Path) -> bool {
        self.files.iter().any(|loaded| loaded == path)
    }

    /// Forgets a script file, taking away what it made.
    pub fn unload_file(&mut self, path: &Path) {
        self.unload(&owner_of(path));
        self.files.retain(|loaded| loaded != path);
    }

    /// The GMCP modules the scripts loaded ask for, as `"Name version"`,
    /// each once, in the order first asked for.
    pub fn supports(&self) -> Vec<String> {
        let mut modules: Vec<String> = Vec::new();
        for (_, module) in &self.state.borrow().supports {
            if !modules.contains(module) {
                modules.push(module.clone());
            }
        }
        modules
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

    /// Runs script code as `owner`, whose what it makes then is, and
    /// under the time budget. Returns whether it succeeded.
    fn as_owner(&self, owner: &str, run: impl FnOnce() -> mlua::Result<()>) -> bool {
        let before = std::mem::replace(&mut self.state.borrow_mut().owner, owner.to_owned());
        let ok = self.guarded(run);
        self.state.borrow_mut().owner = before;
        ok
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

#[cfg(test)]
mod tests;

/// Who a script file is, as the owner of what it makes: its path with the
/// separators of the system throughout, so that the same file reached as
/// `scripts\New folder/hunt.luau` and as `scripts\New folder\hunt.luau`, as
/// Windows takes both, is one owner.
fn owner_of(path: &Path) -> String {
    path.components().collect::<PathBuf>().display().to_string()
}
