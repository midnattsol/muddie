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

use mlua::chunk::Compiler;
use mlua::serde::SerializeOptions;
use mlua::{Function, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, Variadic, VmState};
use muddie_core::{Color, Line, Span, Style};
use muddie_mapper::{
    Added, Arrival, Colour, Exit, Located, Map, Naming, Pos, Refused, Room, RoomId, RoomInfo,
    Sighting, room_id,
};
use regex::Regex;

mod action;
mod api;
mod convert;
mod events;
mod explain;
mod gauges;
mod line;
mod made;
mod map_api;
mod memory;
mod state;
mod switches;

pub use action::{Action, Aloud, Effect, GaugeKind, GaugeLook, How, Playing, Saying};
pub use convert::{bindable, colour_named, split_commands};
pub use line::Heard;

/// Something a script offers to do with the map, as the user is shown it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferedTool {
    /// What the user chooses it by.
    pub label: String,
    /// The script that offers it.
    pub owner: String,
    /// For one that switches something on or off, whether it is on.
    pub checked: Option<bool>,
}
use api::{AfterText, Stopped, aloud_of, install_api, only, texts_of};
pub use api::{CheckKeys, KnownShortcut, in_interface_language, set_interface_language};
use convert::*;
use line::{Given, Looked, Text};
use made::Made;
use map_api::*;
pub use state::Session;
use state::*;
pub use switches::{Belongs, Kind, Switches, places_up, tag_key};

/// Longest a single script run or callback may take before it is stopped,
/// so a faulty script cannot freeze the client.
const TIME_BUDGET: Duration = Duration::from_millis(500);
const MEMORY_LIMIT: usize = 256 << 20;
/// The global tables whose fields are asked for as they are now.
const CHANGING: [&str; 4] = ["vars", "session", "map", "shortcuts"];
const MIN_TIMER_INTERVAL: Duration = Duration::from_millis(10);

pub struct Engine {
    lua: Lua,
    state: Rc<RefCell<State>>,
    /// The automapper's map, shared with the frontend.
    map: Rc<RefCell<Map>>,
    /// Script files loaded so far, in order.
    files: Vec<PathBuf>,
    /// What a script that stopped to wait says.
    stopped: Stopped,
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
        // What a script reads of these changes: not to be found once, as it
        // is loaded, and kept, as Luau does with what is in a global table.
        lua.set_compiler(Compiler::new().set_mutable_globals(CHANGING));
        // mlua always installs a `require` that reads any file on disk.
        lua.globals().raw_remove("require")?;
        lua.set_memory_limit(MEMORY_LIMIT)?;
        let state = Rc::new(RefCell::new(State::default()));
        let stopped = install_api(&lua, &state)?;
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
            stopped,
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

    /// Offers the code of a module that has no file, as that of a plugin run
    /// as it comes: to the scripts beside `owner`, until that is unloaded.
    pub fn offer_module(&mut self, owner: &str, code: &str) {
        let offered = &mut self.state.borrow_mut().offered;
        offered.insert(crate::owner_of(Path::new(owner)), code.to_owned());
    }

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

    /// Runs a chunk of Luau code as its own owner; `name` identifies it
    /// in error messages.
    pub fn exec(&mut self, name: &str, code: &str) {
        self.exec_owned(name, name, code);
    }

    /// Runs a chunk of code as part of `owner`: what it makes, at once or
    /// later from what it made, is taken away by [`Self::unload`] of that
    /// owner. `name` identifies the chunk in error messages.
    pub fn exec_owned(&mut self, owner: &str, name: &str, code: &str) {
        (self.state.borrow_mut().sources).keep(owner, name, code);
        // The "@" makes errors read `name:line: message`.
        let chunk = self.lua.load(code).set_name(format!("@{name}"));
        self.as_owner(owner, || self.call(&chunk.into_function()?, ()));
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
        state.waiting.retain(|matcher| matcher.owner != owner);
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
        let chosen = state.shortcuts.len();
        state.shortcuts.retain(|(by, _, _)| by != owner);
        if state.shortcuts.len() != chosen {
            state.shortcuts_changed += 1;
        }
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
                crate::state::Set::Learning => {
                    state.map_settings.learn = false;
                    map.set_learning(false);
                }
                crate::state::Set::Directions => {
                    state.map_settings.directions.clear();
                    map.forget_taught_directions();
                }
                crate::state::Set::Naming => {
                    state.map_settings.names = Naming::default();
                    map.set_naming(Naming::default());
                }
                crate::state::Set::Language => {
                    state.map_settings.language = None;
                    state.actions.push(Action::Language(String::new()));
                }
                crate::state::Set::Panel(name) => {
                    state.actions.push(Action::Panel { name, text: None });
                }
                crate::state::Set::Channel(name) => {
                    state.channel_closed(&name);
                    state.actions.push(Action::ChannelClosed(name));
                }
            }
        }
        state.macros.retain(|_, bound| {
            bound.retain(|bound| bound.owner != owner);
            !bound.is_empty()
        });
        state.tidy_paused();
        // A module it is asks for it afresh next time.
        let path = crate::owner_of(Path::new(owner));
        state.sources.forget(owner);
        state.modules.remove(&path);
        state.offered.remove(&path);
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

    /// `text` with each `$name` that is a variable replaced by its value,
    /// as what rules send is.
    pub fn expand(&self, text: &str) -> String {
        self.state.borrow().expand(text)
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
                owner: tool.owner.clone(),
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
        self.as_owner(&owner, || self.call(&callback, checked));
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
            self.as_owner(&awaited.owner, || self.call(&awaited.callback, ()));
        }
    }

    /// Tells scripts of the client's shortcuts, and how to read the keys
    /// they give them; again whenever their keys change.
    pub fn know_shortcuts(&mut self, known: Vec<KnownShortcut>, check: CheckKeys) {
        let mut state = self.state.borrow_mut();
        state.known_shortcuts = known;
        state.check_shortcut = Some(check);
    }

    /// The keys scripts chose for shortcuts, in the order they did: the
    /// script, the shortcut and its keys; with how many times they
    /// changed, to tell when they did.
    pub fn shortcuts(&self) -> (u64, Vec<(String, String, Vec<String>)>) {
        let state = self.state.borrow();
        (state.shortcuts_changed, state.shortcuts.clone())
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
            self.tell_changes();
        }
        ok
    }

    /// Tells who listens what changed: the variables, the gauges, what was
    /// said on the channels and by other scripts. A
    /// handler that changes one has it told in turn, a few rounds at most,
    /// so two that change each other do not go on forever.
    fn tell_changes(&self) {
        {
            let mut state = self.state.borrow_mut();
            if state.running > 0 || std::mem::replace(&mut state.telling, true) {
                return;
            }
        }
        for _ in 0..8 {
            let mut state = self.state.borrow_mut();
            let vars = std::mem::take(&mut state.vars_changed);
            let gauges = std::mem::take(&mut state.gauges_changed);
            let said = std::mem::take(&mut state.said);
            let emitted = std::mem::take(&mut state.emitted);
            drop(state);
            if vars.is_empty() && gauges.is_empty() && said.is_empty() && emitted.is_empty() {
                break;
            }
            self.tell_vars_changed(vars);
            self.tell_gauges_changed(gauges);
            self.tell_said(said);
            self.tell_emitted(emitted);
        }
        let mut state = self.state.borrow_mut();
        state.vars_changed.clear();
        state.gauges_changed.clear();
        state.said.clear();
        state.emitted.clear();
        state.telling = false;
    }

    /// Calls the handlers of each variable changed with its value and its
    /// name.
    fn tell_vars_changed(&self, changed: Vec<String>) {
        for name in changed {
            let callbacks = self.state.borrow().listeners(&var_key(&name));
            if callbacks.is_empty() {
                continue;
            }
            let vars: mlua::Result<Table> = self.lua.globals().get("vars");
            let value: Value = vars
                .and_then(|vars| vars.get(name.as_str()))
                .unwrap_or(Value::Nil);
            for (id, owner, callback) in callbacks {
                let still_on = self.state.borrow().runs(id);
                if still_on {
                    self.as_owner(&owner, || {
                        self.call(&callback, (value.clone(), name.as_str()))
                    });
                }
            }
        }
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
                self.error(self.explain(describe(&error)));
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

#[cfg(test)]
mod tests;

/// Who a script file is, as the owner of what it makes: its path with the
/// separators of the system throughout, so that the same file reached as
/// `scripts\New folder/hunt.luau` and as `scripts\New folder\hunt.luau`, as
/// Windows takes both, is one owner.
pub fn owner_of(path: &Path) -> String {
    path.components().collect::<PathBuf>().display().to_string()
}
