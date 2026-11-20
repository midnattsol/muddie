//! Triggers, aliases, groups of them, keys and timers.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_triggers_and_aliases(lua, state)?;
    install_enable(lua, state)?;
    install_group_table(lua, state)?;
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
                    let (group, once) = match options {
                        Some(options) => (
                            options.get::<Option<String>>("group")?,
                            options.get::<Option<bool>>("once")?.unwrap_or(false),
                        ),
                        None => (None, false),
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
                        group,
                        once,
                    });
                    Ok(id)
                },
            )?,
        )?;
    }
    Ok(())
}

fn install_enable(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
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
    Ok(())
}

fn install_group_table(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    // The same switch, and what goes with it, under one name.
    let groups = lua.create_table()?;
    let shared = Rc::clone(state);
    groups.set(
        "enabled",
        lua.create_function(move |_, group: String| {
            Ok(!shared.borrow().disabled_groups.contains(&group))
        })?,
    )?;
    for (name, wanted) in [
        ("enable", Some(true)),
        ("disable", Some(false)),
        ("toggle", None),
    ] {
        let shared = Rc::clone(state);
        groups.set(
            name,
            lua.create_function(move |_, group: String| {
                let disabled = &mut shared.borrow_mut().disabled_groups;
                let on = wanted.unwrap_or(disabled.contains(&group));
                if on {
                    disabled.remove(&group);
                } else {
                    disabled.insert(group);
                }
                Ok(on)
            })?,
        )?;
    }
    let shared = Rc::clone(state);
    groups.set(
        "list",
        lua.create_function(move |_, ()| Ok(shared.borrow().group_names()))?,
    )?;
    globals.set("group", groups)?;
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
                let group: Option<String> = match options {
                    Some(options) => options.get("group")?,
                    None => None,
                };
                let mut state = shared.borrow_mut();
                let (key, owner) = (key_name(&key), state.owner.clone());
                state
                    .bound_by
                    .retain(|(_, known, of)| *known != key || *of != group);
                state.bound_by.push((owner, key.clone(), group.clone()));
                let bound = state.bindings.entry(key).or_default();
                // Binding a key again replaces what it did in that group.
                bound.retain(|(known, _)| *known != group);
                bound.push((group, binding));
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
                    let group: Option<String> = match options {
                        Some(options) => options.get("group")?,
                        None => None,
                    };
                    let delay = Duration::try_from_secs_f64(seconds)
                        .map_err(|_| mlua::Error::runtime("the time must be a positive number"))?
                        .max(MIN_TIMER_INTERVAL);
                    let mut state = shared.borrow_mut();
                    let id = state.next_id();
                    let owner = state.owner.clone();
                    state.timers.push(Timer {
                        id,
                        owner,
                        group,
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
