//! What the MUD sends besides text, and what is sent back to it.

use super::*;
use crate::gauges::Levels;
use crate::state::GAUGES;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    install_on(lua, state)?;
    install_gmcp(lua, state)?;
    install_msdp(lua, state)?;
    install_mssp(lua, state)
}

/// What there is to listen to by a name of its own, and what that name is.
const NAMED: [(&str, &str); 4] = [
    ("gmcp", "a package"),
    ("msdp", "a variable"),
    ("var", "a variable"),
    ("gauge", "a gauge"),
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
            "gauge" => format!("{GAUGES}{name}"),
            _ => var_key(name),
        }),
        (Some(("gauge", _)), None) => Ok(GAUGES.to_owned()),
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
                let levels = levels_of(&what, options.as_ref())?;
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
                    levels,
                });
                drop(state);
                Ok(Made::new(id, &shared))
            },
        )?,
    )
}

/// The levels of a gauge its options ask to be told at, if they do: of
/// nothing else are there any.
fn levels_of(what: &str, options: Option<&Table>) -> mlua::Result<Option<Levels>> {
    if what != "gauge" {
        super::rules::only(options, &["place", "tags"])?;
        return Ok(None);
    }
    super::rules::only(options, &["under", "over", "place", "tags"])?;
    let level = |key: &str| match options {
        Some(options) => options.get::<Option<f64>>(key),
        None => Ok(None),
    };
    Ok(Levels::new(level("under")?, level("over")?))
}

fn no_function() -> mlua::Error {
    mlua::Error::runtime("on needs a function: on(what, function) or on(what, name, function)")
}

/// Tells who calls `name(...)`, which was once a function, what there is
/// now.
fn no_call(lua: &Lua, table: &Table, name: &str) -> mlua::Result<()> {
    let said = format!("{name}(...) is no longer: use {name}.get(...)");
    let meta = lua.create_table()?;
    meta.set(
        "__call",
        lua.create_function(move |_, _: mlua::MultiValue| -> mlua::Result<()> {
            Err(mlua::Error::runtime(said.clone()))
        })?,
    )?;
    table.set_metatable(Some(meta))
}

/// `gmcp`: what the MUD sent over it, and what it is sent and asked.
fn install_gmcp(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let gmcp = super::group(lua, "gmcp")?;
    no_call(lua, &gmcp, "gmcp")?;
    let shared = Rc::clone(state);
    gmcp.set(
        "get",
        lua.create_function(move |_, package: String| {
            Ok(shared.borrow().data.get(&package.to_lowercase()).cloned())
        })?,
    )?;
    let shared = Rc::clone(state);
    gmcp.set(
        "send",
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
    gmcp.set(
        "merge",
        lua.create_function(move |_, packages: Variadic<String>| {
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
    gmcp.set(
        "support",
        lua.create_function(move |_, modules: Variadic<String>| {
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
    )
}

/// `msdp`: the same.
fn install_msdp(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let msdp = super::group(lua, "msdp")?;
    no_call(lua, &msdp, "msdp")?;
    let shared = Rc::clone(state);
    msdp.set(
        "get",
        lua.create_function(move |_, variable: String| {
            Ok(shared.borrow().data.get(&msdp_key(&variable)).cloned())
        })?,
    )?;
    let shared = Rc::clone(state);
    msdp.set(
        "send",
        lua.create_function(move |_, (variable, values): (String, Variadic<String>)| {
            (shared.borrow_mut().actions).push(Action::Msdp {
                variable,
                values: values.into_iter().collect(),
            });
            Ok(())
        })?,
    )?;
    let shared = Rc::clone(state);
    msdp.set(
        "report",
        lua.create_function(move |_, variables: Variadic<String>| {
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
    )
}

/// `mssp`: the server's status.
fn install_mssp(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let mssp = super::group(lua, "mssp")?;
    no_call(lua, &mssp, "mssp")?;
    let shared = Rc::clone(state);
    mssp.set(
        "get",
        lua.create_function(move |lua, ()| {
            let status = lua.create_table()?;
            for (name, value) in &shared.borrow().mssp {
                status.set(name.as_str(), value.as_str())?;
            }
            Ok(status)
        })?,
    )
}
