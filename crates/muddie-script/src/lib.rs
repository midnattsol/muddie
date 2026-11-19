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
//! - `gauge(name, value, max)`: show or update a bar in the status strip;
//!   `gauge(name)` removes it.
//! - `chat(text)`: add a line to the chat panel.
//! - `cancel(id)`: remove the trigger, alias, timer or GMCP handler with the
//!   id that the functions above returned.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mlua::serde::SerializeOptions;
use mlua::{Function, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value, VmState};
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

struct GmcpHandler {
    id: u64,
    /// Lower-cased package name; also matches packages under it.
    package: String,
    callback: Function,
}

impl GmcpHandler {
    fn covers(&self, package: &str) -> bool {
        match package.strip_prefix(self.package.as_str()) {
            Some(rest) => rest.is_empty() || rest.starts_with('.'),
            None => false,
        }
    }
}

/// State shared between the engine and the functions scripts call.
#[derive(Default)]
struct State {
    last_id: u64,
    triggers: Vec<Matcher>,
    aliases: Vec<Matcher>,
    timers: Vec<Timer>,
    gmcp_handlers: Vec<GmcpHandler>,
    /// Data last received per GMCP package, keyed by lower-cased name.
    gmcp_data: HashMap<String, Value>,
    actions: Vec<Action>,
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
    /// Script files loaded so far, in order, for `/reload`.
    files: Vec<PathBuf>,
}

impl Engine {
    pub fn new() -> mlua::Result<Self> {
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
                Err(error) => return self.error(format!("bad GMCP data for {package}: {error}")),
            }
        };
        let key = package.to_lowercase();
        let callbacks: Vec<(u64, Function)> = {
            let mut state = self.state.borrow_mut();
            state.gmcp_data.insert(key.clone(), data.clone());
            (state.gmcp_handlers.iter())
                .filter(|handler| handler.covers(&key))
                .map(|handler| (handler.id, handler.callback.clone()))
                .collect()
        };
        for (id, callback) in callbacks {
            let still_there = (self.state.borrow().gmcp_handlers.iter()).any(|h| h.id == id);
            if still_there {
                self.guarded(|| callback.call::<()>((data.clone(), package)));
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
        let mut fresh = match Engine::new() {
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

    let shared = Rc::clone(state);
    add(
        "on_gmcp",
        lua.create_function(move |_, (package, callback): (String, Function)| {
            let mut state = shared.borrow_mut();
            let id = state.next_id();
            state.gmcp_handlers.push(GmcpHandler {
                id,
                package: package.to_lowercase(),
                callback,
            });
            Ok(id)
        })?,
    )?;

    let shared = Rc::clone(state);
    add(
        "gmcp",
        lua.create_function(move |_, package: String| {
            let state = shared.borrow();
            Ok(state.gmcp_data.get(&package.to_lowercase()).cloned())
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
                    + state.gmcp_handlers.len()
            };
            let before = count(&state);
            state.triggers.retain(|trigger| trigger.id != id);
            state.aliases.retain(|alias| alias.id != id);
            state.timers.retain(|timer| timer.id != id);
            state.gmcp_handlers.retain(|handler| handler.id != id);
            Ok(count(&state) < before)
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(code: &str) -> Engine {
        let mut engine = Engine::new().unwrap();
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
        assert!(error_text(&engine.take_actions()).contains("bad GMCP data for Char.Vitals"));
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
            chat("Ana: hola")"#,
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
            ]
        );
        engine.exec("test", r#"gauge("HP", 5)"#);
        assert!(error_text(&engine.take_actions()).contains("both a value and a max"));
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
        let mut engine = Engine::new().unwrap();
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
