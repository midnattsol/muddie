//! Triggers, aliases, keys and timers, and switching them by their place
//! or their tags.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_triggers_and_aliases(lua, state)?;
    install_switches(lua, state)?;
    install_bind(lua, state)?;
    install_unbind(lua, state)?;
    install_timers(lua, state)?;
    install_cancel(lua, state)?;
    Ok(())
}

fn install_triggers_and_aliases(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    for (name, is_alias) in [("trigger", false), ("alias", true)] {
        let shared = Rc::clone(state);
        add(
            name,
            lua.create_function(
                move |_, (pattern, callback, options): (String, Function, Option<Table>)| {
                    let regex = Regex::new(&pattern).map_err(mlua::Error::runtime)?;
                    let belongs = belongs(options.as_ref())?;
                    let once = match &options {
                        Some(options) => options.get::<Option<bool>>("once")?.unwrap_or(false),
                        None => false,
                    };
                    let mut state = shared.borrow_mut();
                    let id = state.next_id();
                    let owner = state.owner.clone();
                    let list = if is_alias {
                        &mut state.aliases
                    } else {
                        &mut state.triggers
                    };
                    list.push(Matcher {
                        id,
                        owner,
                        regex,
                        callback,
                        belongs,
                        once,
                    });
                    Ok(id)
                },
            )?,
        )?;
    }
    Ok(())
}

/// What `options` say a thing is switched by: its `name`, which is its
/// place, and its `tags`, a list or one alone.
fn belongs(options: Option<&Table>) -> mlua::Result<Belongs> {
    let Some(options) = options else {
        return Ok(Belongs::default());
    };
    let place = options.get::<Option<String>>("name")?;
    let tags = match options.get::<Value>("tags")? {
        Value::Nil => Vec::new(),
        Value::String(tag) => vec![tag.to_string_lossy()],
        Value::Table(tags) => tags
            .sequence_values::<String>()
            .collect::<mlua::Result<_>>()?,
        _ => return Err(mlua::Error::runtime("tags must be a list of names")),
    };
    let place = place
        .map(|place| place.trim().to_owned())
        .filter(|place| !place.is_empty());
    Ok(Belongs { place, tags })
}

/// A table for each kind of thing, to switch one by its place or a folder
/// with all it holds, and one for tags: `enable`, `disable`, `toggle`,
/// `enabled` and `list`.
fn install_switches(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    for kind in Kind::ALL.map(Some).into_iter().chain([None]) {
        let table = lua.create_table()?;
        let shared = Rc::clone(state);
        table.set(
            "enabled",
            lua.create_function(move |_, name: String| {
                let switches = &shared.borrow().switches;
                Ok(match kind {
                    Some(kind) => switches.place_on(kind, &name),
                    None => switches.tag_on(&name),
                })
            })?,
        )?;
        for (verb, wanted) in [
            ("enable", Some(true)),
            ("disable", Some(false)),
            ("toggle", None),
        ] {
            let shared = Rc::clone(state);
            table.set(
                verb,
                lua.create_function(move |_, name: String| {
                    let state = &mut *shared.borrow_mut();
                    state.changed = true;
                    let switches = &mut state.switches;
                    let was = match kind {
                        Some(kind) => switches.place_on(kind, &name),
                        None => switches.tag_on(&name),
                    };
                    let on = wanted.unwrap_or(!was);
                    match kind {
                        Some(kind) => switches.set_place(kind, &name, on),
                        None => switches.set_tag(&name, on),
                    }
                    Ok(on)
                })?,
            )?;
        }
        let shared = Rc::clone(state);
        table.set(
            "list",
            lua.create_function(move |_, ()| {
                let state = shared.borrow();
                Ok(match kind {
                    Some(kind) => state.place_names(kind),
                    None => state.tag_names(),
                })
            })?,
        )?;
        globals.set(kind.map_or("tags", Kind::table), table)?;
    }
    Ok(())
}

fn install_bind(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "bind",
        lua.create_function(
            move |_, (key, action, options): (String, Value, Option<mlua::Table>)| {
                let binding = match action {
                    Value::String(command) => Binding::Command(command.to_string_lossy()),
                    Value::Function(function) => Binding::Function(function),
                    _ => return Err(mlua::Error::runtime("bind needs a command or a function")),
                };
                let belongs = belongs(options.as_ref())?;
                let mut state = shared.borrow_mut();
                let owner = state.owner.clone();
                let bound = state.bindings.entry(key_name(&key)).or_default();
                // Binding a key again under the same name replaces it.
                bound.retain(|known| known.belongs.place != belongs.place);
                bound.push(Bound {
                    owner,
                    belongs,
                    binding,
                });
                Ok(())
            },
        )?,
    )?;
    Ok(())
}

fn install_unbind(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
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
    Ok(())
}

fn install_timers(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    for (name, repeats) in [("after", false), ("every", true)] {
        let shared = Rc::clone(state);
        add(
            name,
            lua.create_function(
                move |_, (seconds, callback, options): (f64, Function, Option<mlua::Table>)| {
                    let belongs = belongs(options.as_ref())?;
                    let delay = Duration::try_from_secs_f64(seconds)
                        .map_err(|_| mlua::Error::runtime("the time must be a positive number"))?
                        .max(MIN_TIMER_INTERVAL);
                    let mut state = shared.borrow_mut();
                    let id = state.next_id();
                    let owner = state.owner.clone();
                    state.timers.push(Timer {
                        id,
                        owner,
                        belongs,
                        due: Instant::now() + delay,
                        period: repeats.then_some(delay),
                        callback,
                    });
                    Ok(id)
                },
            )?,
        )?;
    }
    Ok(())
}

fn install_cancel(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
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
