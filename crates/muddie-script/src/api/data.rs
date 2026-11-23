//! What the MUD sends besides text, and what is sent back to it.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_data_handlers(lua, state)?;
    install_send_msdp(lua, state)?;
    install_mssp(lua, state)?;
    install_send_gmcp(lua, state)?;
    install_on_event(lua, state)?;
    install_supports(lua, state)?;
    install_completions(lua, state)?;
    Ok(())
}

fn install_data_handlers(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    // GMCP packages and MSDP variables share one store; the key function
    // keeps their names apart.
    type KeyFn = fn(&str) -> String;
    let gmcp_key: KeyFn = |package| package.to_lowercase();
    let handlers = [
        ("on_gmcp", gmcp_key),
        ("on_msdp", msdp_key as KeyFn),
        ("on_var", var_key as KeyFn),
    ];
    for (name, key) in handlers {
        let shared = Rc::clone(state);
        add(
            name,
            lua.create_function(move |_, (source, callback): (String, Function)| {
                let mut state = shared.borrow_mut();
                let id = state.next_id();
                let owner = state.owner.clone();
                state.data_handlers.push(DataHandler {
                    id,
                    owner,
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
    Ok(())
}

fn install_send_msdp(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
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
    Ok(())
}

fn install_mssp(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
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
    Ok(())
}

fn install_send_gmcp(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
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
    Ok(())
}

fn install_on_event(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "on_event",
        lua.create_function(move |_, (name, callback): (String, Function)| {
            let mut state = shared.borrow_mut();
            let id = state.next_id();
            let owner = state.owner.clone();
            state.data_handlers.push(DataHandler {
                id,
                owner,
                key: event_key(&name),
                callback,
            });
            Ok(id)
        })?,
    )?;
    Ok(())
}

fn install_supports(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "merge_gmcp",
        lua.create_function(move |_, packages: mlua::Variadic<String>| {
            let mut state = shared.borrow_mut();
            let owner = state.owner.clone();
            for package in packages {
                let asked = (owner.clone(), package.to_lowercase());
                if !state.merged.contains(&asked) {
                    state.merged.push(asked);
                }
            }
            Ok(())
        })?,
    )?;
    let shared = Rc::clone(state);
    add(
        "support_gmcp",
        lua.create_function(move |_, modules: mlua::Variadic<String>| {
            let mut state = shared.borrow_mut();
            let owner = state.owner.clone();
            for module in modules {
                let asked = (owner.clone(), module);
                if !state.supports.contains(&asked) {
                    state.supports.push(asked);
                    state.changed = true;
                }
            }
            Ok(())
        })?,
    )?;
    Ok(())
}

fn install_completions(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    let shared = Rc::clone(state);
    add(
        "completions",
        lua.create_function(move |_, (name, words): (String, Option<Vec<String>>)| {
            let words = words.unwrap_or_default();
            (shared.borrow_mut().actions).push(Action::Completions { name, words });
            Ok(())
        })?,
    )?;
    Ok(())
}
