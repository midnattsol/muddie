//! Triggers, aliases, timers and the Luau runtime that scripts them.
//!
//! The [`Engine`] has no I/O of its own: the frontend feeds it output lines,
//! typed input and the clock, and carries out the [`Action`]s it returns.
//!
//! What scripts are given is described for their authors, once, in
//! `docs/scripting.md`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::serde::SerializeOptions;
use mlua::{Function, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, Variadic, VmState};
use muddie_core::{Color, Line};
use muddie_mapper::{
    Added, Arrival, Colour, Exit, Located, Map, Naming, Pos, Refused, Room, RoomId, RoomInfo,
    Sighting, room_id,
};
use regex::Regex;

mod action;
mod api;
mod convert;
mod events;
mod map_api;
mod memory;
mod state;
mod switches;

pub use action::{Action, Effect, GaugeKind, GaugeLook, PanelOf};
pub use convert::{COLOUR_NAMES, colour_named, split_commands};

/// Something a script offers to do with the map, as the user is shown it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferedTool {
    /// What the user chooses it by.
    pub label: String,
    /// The script that offers it, by its name: a plugin's is the plugin's.
    pub by: String,
    /// For one that switches something on or off, whether it is on.
    pub checked: Option<bool>,
}
pub use api::set_interface_language;
use api::{in_interface_language, install_api, texts_of};
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
    /// Notes that a variable changed, for its handlers to be told.
    fn var_changed(&mut self, name: &str) {
        if !self.vars_changed.iter().any(|known| known == name) {
            self.vars_changed.push(name.to_owned());
        }
    }

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

pub struct Engine {
    lua: Lua,
    state: Rc<RefCell<State>>,
    /// The automapper's map, shared with the frontend.
    map: Rc<RefCell<Map>>,
    /// Script files loaded so far, in order.
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

    /// Runs a script file and remembers it, to run it again on restarting
    /// the scripts.
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
    /// macros and handlers, and what it set for all: the packages it
    /// merges, the separator, the built-in map loader turned off. What the other scripts made and what they
    /// keep is left as it is, so that one script can be run again without
    /// the rest starting over.
    pub fn unload(&mut self, owner: &str) {
        let mut state = self.state.borrow_mut();
        state.triggers.retain(|matcher| matcher.owner != owner);
        state.aliases.retain(|matcher| matcher.owner != owner);
        state.timers.retain(|timer| timer.owner != owner);
        state.data_handlers.retain(|handler| handler.owner != owner);
        state.awaited.retain(|awaited| awaited.owner != owner);
        state.gauges.retain(|shown| shown.owner != owner);
        let offered = state.map_actions.len();
        state.map_actions.retain(|tool| tool.owner != owner);
        if state.map_actions.len() != offered {
            state.actions.push(Action::MapActions);
        }
        let asked = state.supports.len();
        state.supports.retain(|(by, _)| by != owner);
        state.changed |= state.supports.len() != asked;
        state.merged.retain(|(by, _)| by != owner);
        state.reports.retain(|(by, _)| by != owner);
        state.completions.retain(|(by, _, _)| by != owner);
        if state.separator.as_ref().is_some_and(|(by, _)| by == owner) {
            state.separator = None;
        }
        if state.map_native_off.as_deref() == Some(owner) {
            state.map_native_off = None;
        }
        if state.map_memory_only.as_deref() == Some(owner) {
            state.map_memory_only = None;
        }
        // What it set of the map and the session goes back as it was
        // before any script set it.
        let (undone, kept) = std::mem::take(&mut state.set_by)
            .into_iter()
            .partition(|(_, by)| by == owner);
        state.set_by = kept;
        for (what, _) in undone {
            let mut map = self.map.borrow_mut();
            match what {
                crate::state::Set::Learning => map.set_learning(false),
                crate::state::Set::Directions => map.forget_taught_directions(),
                crate::state::Set::Naming => map.set_naming(Default::default()),
                crate::state::Set::Language => {
                    state.actions.push(Action::Language(String::new()));
                }
                crate::state::Set::Panel(name) => {
                    state.actions.push(Action::Panel { name, text: None });
                }
                crate::state::Set::Channel(name) => {
                    state.actions.push(Action::ChannelClosed(name));
                }
            }
        }
        state.macros.retain(|_, bound| {
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

    /// Forgets a script file, taking away what it made, what it shows
    /// too.
    pub fn unload_file(&mut self, path: &Path) {
        let owner = owner_of(path);
        self.unload(&owner);
        (self.state.borrow_mut().actions).push(Action::Unloaded(owner));
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

    /// The words the scripts loaded give to complete, list after list.
    pub fn completions(&self) -> Vec<String> {
        let state = self.state.borrow();
        (state.completions.iter())
            .flat_map(|(_, _, words)| words.iter().cloned())
            .collect()
    }

    /// The MSDP variables the scripts loaded ask to be told of.
    pub fn msdp_reports(&self) -> Vec<String> {
        let mut variables: Vec<String> = Vec::new();
        for (_, variable) in &self.state.borrow().reports {
            if !variables.contains(variable) {
                variables.push(variable.clone());
            }
        }
        variables
    }

    /// What the scripts offer to do with the map, in the order offered.
    pub fn map_actions(&self) -> Vec<OfferedTool> {
        (self.state.borrow().map_actions.iter())
            .map(|tool| OfferedTool {
                label: in_interface_language(&tool.label).unwrap_or_default(),
                by: (Path::new(&tool.owner).file_stem()).map_or_else(
                    || tool.owner.clone(),
                    |stem| stem.to_string_lossy().into_owned(),
                ),
                checked: tool.checked,
            })
            .collect()
    }

    /// Does what a script offered to do with the map, chosen by `label`:
    /// one that switches something is switched, and told to what. Returns
    /// false if none offers it.
    pub fn use_map_action(&mut self, label: &str) -> bool {
        let tool = {
            let mut state = self.state.borrow_mut();
            let tool = (state.map_actions.iter_mut())
                .find(|tool| in_interface_language(&tool.label).as_deref() == Some(label));
            tool.map(|tool| {
                tool.checked = tool.checked.map(|on| !on);
                (tool.owner.clone(), tool.callback.clone(), tool.checked)
            })
        };
        let Some((owner, callback, checked)) = tool else {
            return false;
        };
        self.as_owner(&owner, || callback.call::<()>(checked));
        true
    }

    /// Calls, once, the function a script gave to be told what the client
    /// went to find out, asked for by an action with this id.
    pub fn answer(&mut self, id: u64) {
        let awaited = {
            let mut state = self.state.borrow_mut();
            let at = state.awaited.iter().position(|awaited| awaited.id == id);
            at.map(|at| state.awaited.remove(at))
        };
        if let Some(awaited) = awaited {
            self.as_owner(&awaited.owner, || awaited.callback.call::<()>(()));
        }
    }

    /// The map the scripts work on, shared with the frontend.
    pub fn map(&self) -> &Rc<RefCell<Map>> {
        &self.map
    }

    /// Takes the actions requested since the last call, in order.
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.state.borrow_mut().actions)
    }

    /// Runs script code as `owner`, whose what it makes then is, and
    /// under the time budget. Returns whether it succeeded.
    fn as_owner(&self, owner: &str, run: impl FnOnce() -> mlua::Result<()>) -> bool {
        let before = {
            let mut state = self.state.borrow_mut();
            state.running += 1;
            std::mem::replace(&mut state.owner, owner.to_owned())
        };
        let ok = self.guarded(run);
        let out = {
            let mut state = self.state.borrow_mut();
            state.owner = before;
            state.running -= 1;
            state.running == 0
        };
        // Out of script code: what it changed is told.
        if out {
            self.tell_vars_changed();
        }
        ok
    }

    /// Calls the handlers of each variable changed with its value and its
    /// name. A handler that changes a variable has it told in turn, a few
    /// rounds at most, so two that change each other do not go on forever.
    fn tell_vars_changed(&self) {
        if std::mem::replace(&mut self.state.borrow_mut().telling, true) {
            return;
        }
        for _ in 0..8 {
            let changed = std::mem::take(&mut self.state.borrow_mut().vars_changed);
            if changed.is_empty() {
                break;
            }
            for name in changed {
                let key = var_key(&name);
                let state = self.state.borrow();
                let callbacks: Vec<(u64, String, Function)> = (state.data_handlers.iter())
                    .filter(|handler| handler.covers(&key) && handler.on(&state.switches))
                    .map(|handler| (handler.id, handler.owner.clone(), handler.callback.clone()))
                    .collect();
                drop(state);
                if callbacks.is_empty() {
                    continue;
                }
                let vars: mlua::Result<Table> = self.lua.globals().get("vars");
                let value: Value = vars
                    .and_then(|vars| vars.get(name.as_str()))
                    .unwrap_or(Value::Nil);
                for (id, owner, callback) in callbacks {
                    let still_there =
                        (self.state.borrow().data_handlers.iter()).any(|handler| handler.id == id);
                    if still_there {
                        self.as_owner(&owner, || {
                            callback.call::<()>((value.clone(), name.as_str()))
                        });
                    }
                }
            }
        }
        let mut state = self.state.borrow_mut();
        state.vars_changed.clear();
        state.telling = false;
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
}

impl ExitOptions {
    fn read(options: Option<Table>) -> mlua::Result<Self> {
        let Some(options) = options else {
            return Ok(Self::default());
        };
        Ok(Self {
            cost: options.get("cost")?,
            blocked: options.get("blocked")?,
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
/// What a script file is known as, as the owner of what it makes.
pub fn owner_of(path: &Path) -> String {
    path.components().collect::<PathBuf>().display().to_string()
}
