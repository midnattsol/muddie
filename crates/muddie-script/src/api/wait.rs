//! `wait` and `lines`: stopping in the middle of a script until something
//! comes. Each function of a script is run in a coroutine, which is what
//! stops; what waits is made as any other thing a script makes, and goes
//! with it.

use super::*;
use mlua::{Function, MultiValue};

/// What a script that stopped to wait says it did.
pub(crate) struct Stopped(Table);

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<Stopped> {
    let shared = Rc::clone(state);
    let next_line = lua.create_function(
        move |_, (pattern, prompt, callback): (String, Option<bool>, Function)| {
            let pattern = Pattern::new(&pattern).map_err(mlua::Error::runtime)?;
            let mut state = shared.borrow_mut();
            let id = state.next_id();
            let owner = state.owner.clone();
            state.waiting.push(Matcher {
                id,
                owner,
                pattern,
                callback,
                belongs: Belongs::default(),
                once: true,
                prompt,
            });
            drop(state);
            Ok(Made::new(id, &shared))
        },
    )?;
    let matcher = lua.create_function(|lua, pattern: String| {
        let pattern = Pattern::new(&pattern).map_err(mlua::Error::runtime)?;
        lua.create_function(move |_, text: String| {
            Ok(pattern.regex().is_some_and(|regex| regex.is_match(&text)))
        })
    })?;
    let stopped = lua.create_table()?;
    let (wait, lines): (Function, Function) = lua
        .load(include_str!("wait.luau"))
        .set_name("@wait")
        .call((next_line, matcher, &stopped))?;
    lua.globals().set("wait", wait)?;
    lua.globals().set("lines", lines)?;
    Ok(Stopped(stopped))
}

impl Engine {
    /// Calls a function of a script in a coroutine of its own, which may
    /// stop to wait.
    pub(crate) fn call(
        &self,
        function: &Function,
        args: impl mlua::IntoLuaMulti,
    ) -> mlua::Result<()> {
        let thread = self.lua.create_thread(function.clone())?;
        let said = thread.resume::<MultiValue>(args)?;
        let stopped =
            || matches!(said.front(), Some(Value::Table(said)) if *said == self.stopped.0);
        match thread.status() == mlua::thread::ThreadStatus::Resumable && !stopped() {
            true => Err(mlua::Error::runtime(
                "coroutine.yield here stops for good: wait(...) stops for a while",
            )),
            false => Ok(()),
        }
    }
}
