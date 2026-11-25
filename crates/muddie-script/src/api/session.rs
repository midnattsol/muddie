//! The session: where it is connected, who reads it, and how commands are
//! typed in it.

use super::*;

/// What there is to ask of the session.
const FIELDS: [&str; 4] = ["connected", "host", "profile", "reader"];

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let session = super::group(lua, "session")?;
    install_completions(lua, state, &session)?;
    install_separator(lua, state, &session)?;
    let meta = lua.create_table()?;
    let shared = Rc::clone(state);
    meta.set(
        "__index",
        lua.create_function(move |lua, (_, field): (Table, String)| {
            let state = shared.borrow();
            Ok(match field.as_str() {
                "connected" => Value::Boolean(state.session.connected),
                "host" => Value::String(lua.create_string(&state.session.host)?),
                "profile" => match &state.session.profile {
                    Some(name) => Value::String(lua.create_string(name)?),
                    None => Value::Nil,
                },
                "reader" => Value::Boolean(state.screen_reader),
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "there is no session.{other}; there are {}, completions, separator",
                        FIELDS.join(", ")
                    )));
                }
            })
        })?,
    )?;
    meta.set(
        "__newindex",
        lua.create_function(|_, (_, field): (Table, String)| -> mlua::Result<()> {
            Err(mlua::Error::runtime(format!(
                "session.{field} is asked for, not set"
            )))
        })?,
    )?;
    session.set_metatable(Some(meta))
}

/// Words for Tab to offer.
fn install_completions(lua: &Lua, state: &Rc<RefCell<State>>, session: &Table) -> mlua::Result<()> {
    let shared = Rc::clone(state);
    session.set(
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
    )
}

/// What separates the commands typed on one line.
fn install_separator(lua: &Lua, state: &Rc<RefCell<State>>, session: &Table) -> mlua::Result<()> {
    let shared = Rc::clone(state);
    session.set(
        "separator",
        // A text to split by; `false` for none; nothing for `;` again.
        lua.create_function(move |_, separator: Value| {
            let owner = shared.borrow().owner.clone();
            shared.borrow_mut().separator = match separator {
                Value::Nil => None,
                Value::Boolean(false) => Some((owner, None)),
                Value::String(text) => Some((owner, Some(text.to_str()?.to_owned()))),
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "separator takes a text, or false for none, not {}",
                        other.type_name()
                    )));
                }
            };
            Ok(())
        })?,
    )
}
