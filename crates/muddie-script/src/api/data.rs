//! What the MUD sends besides text, and what is sent back to it.

use super::*;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_on(lua, state)?;
    install_data(lua, state)?;
    install_send_msdp(lua, state)?;
    install_mssp(lua, state)?;
    install_screen_reader(lua, state)?;
    install_send_gmcp(lua, state)?;
    install_supports(lua, state)?;
    install_completions(lua, state)?;
    Ok(())
}

/// What there is to listen to by a name of its own, and what that name is.
const NAMED: [(&str, &str); 3] = [
    ("gmcp", "a package"),
    ("msdp", "a variable"),
    ("var", "a variable"),
];

/// What happens in the client that there is to listen to.
const EVENTS: [&str; 5] = ["connect", "disconnect", "reader", "panel", "mxp"];

/// The key of what `what` and `name` say to listen to, as its data is kept.
fn listened(what: &str, name: Option<&str>) -> mlua::Result<String> {
    let named = NAMED.iter().find(|(known, _)| *known == what);
    match (named, name) {
        (Some(_), Some(name)) => Ok(match what {
            "gmcp" => name.to_lowercase(),
            "msdp" => msdp_key(name),
            _ => var_key(name),
        }),
        (Some((_, of)), None) => Err(mlua::Error::runtime(format!(
            "on(\"{what}\", ...) needs the name of {of} before the function"
        ))),
        (None, None) if EVENTS.contains(&what) => Ok(event_key(what)),
        (None, Some(_)) if EVENTS.contains(&what) => Err(mlua::Error::runtime(format!(
            "on(\"{what}\", ...) takes the function next, with no name"
        ))),
        (None, _) => {
            let named = NAMED.iter().map(|(known, _)| *known);
            let all: Vec<&str> = named.chain(EVENTS).collect();
            Err(mlua::Error::runtime(format!(
                "there is no {what} to listen to; there are {}",
                all.join(", ")
            )))
        }
    }
}

/// `on(what, [name], function, [options])`: calls the function with what
/// comes.
fn install_on(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let shared = Rc::clone(state);
    lua.globals().set(
        "on",
        lua.create_function(
            move |_, (what, second, third, fourth): (String, Value, Value, Value)| {
                let (name, callback, options) = match second {
                    Value::Function(callback) => (None, callback, third),
                    Value::String(name) => match third {
                        Value::Function(callback) => {
                            (Some(name.to_str()?.to_owned()), callback, fourth)
                        }
                        _ => return Err(no_function()),
                    },
                    _ => return Err(no_function()),
                };
                let options = match options {
                    Value::Nil => None,
                    Value::Table(options) => Some(options),
                    _ => return Err(mlua::Error::runtime("the options of on are a table")),
                };
                super::rules::only(options.as_ref(), &["place", "tags"])?;
                let belongs = super::rules::belongs(options.as_ref())?;
                let key = listened(&what, name.as_deref())?;
                let mut state = shared.borrow_mut();
                let id = state.next_id();
                let owner = state.owner.clone();
                state.data_handlers.push(DataHandler {
                    id,
                    owner,
                    key,
                    callback,
                    belongs,
                });
                drop(state);
                Ok(Made::new(id, &shared))
            },
        )?,
    )
}

fn no_function() -> mlua::Error {
    mlua::Error::runtime("on needs a function: on(what, function) or on(what, name, function)")
}

fn install_data(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let globals = lua.globals();
    let add = |name: &str, function: Function| globals.set(name, function);
    // GMCP packages and MSDP variables share one store; the key function
    // keeps their names apart.
    type KeyFn = fn(&str) -> String;
    let gmcp_key: KeyFn = |package| package.to_lowercase();
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

fn install_screen_reader(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let shared = Rc::clone(state);
    lua.globals().set(
        "screen_reader",
        lua.create_function(move |_, ()| Ok(shared.borrow().screen_reader))?,
    )
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
        "report_msdp",
        lua.create_function(move |_, variables: mlua::Variadic<String>| {
            let mut state = shared.borrow_mut();
            let owner = state.owner.clone();
            let mut new = Vec::new();
            for variable in variables {
                let asked = (owner.clone(), variable.clone());
                if !state.reports.contains(&asked) {
                    state.reports.push(asked);
                    new.push(variable);
                }
            }
            // Told now, if MSDP is on; and on connecting, as asked.
            if !new.is_empty() {
                (state.actions).push(Action::Msdp {
                    variable: "REPORT".into(),
                    values: new,
                });
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
            let mut state = shared.borrow_mut();
            let owner = state.owner.clone();
            // A list keeps its place when a newer one replaces it.
            let known = state
                .completions
                .iter()
                .position(|(_, list, _)| *list == name);
            match (known, words.is_empty()) {
                (Some(index), true) => drop(state.completions.remove(index)),
                (Some(index), false) => state.completions[index] = (owner, name, words),
                (None, false) => state.completions.push((owner, name, words)),
                (None, true) => {}
            }
            Ok(())
        })?,
    )?;
    Ok(())
}
