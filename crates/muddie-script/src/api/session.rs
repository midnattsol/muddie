//! The session: where it is connected, who reads it, and how commands are
//! typed in it.

use super::*;

/// What there is to ask of the session.
const FIELDS: [&str; 8] = [
    "connected",
    "host",
    "profile",
    "reader",
    "focused",
    "separator",
    "input",
    "reading",
];

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let session = super::group(lua, "session")?;
    install_completions(lua, state, &session)?;
    let shared = Rc::clone(state);
    session.set(
        "feed",
        lua.create_function(move |_, text: Text| {
            shared.borrow_mut().actions.push(Action::Feed(text.0));
            Ok(())
        })?,
    )?;
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
                "focused" => Value::Boolean(state.focused.unwrap_or(false)),
                "reading" => Value::Boolean(!state.not_reading),
                "input" => match &state.input {
                    Some(text) => Value::String(lua.create_string(text)?),
                    None => Value::Nil,
                },
                "separator" => match &state.separator {
                    None => Value::String(lua.create_string(";")?),
                    Some((_, None)) => Value::Boolean(false),
                    Some((_, Some(text))) => Value::String(lua.create_string(text)?),
                },
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "there is no session.{other}; there are {} feed and completions",
                        FIELDS.join(", ")
                    )));
                }
            })
        })?,
    )?;
    let shared = Rc::clone(state);
    meta.set(
        "__newindex",
        lua.create_function(move |_, (_, field, value): (Table, String, Value)| {
            match field.as_str() {
                "separator" => set_separator(&shared, value),
                "input" => set_input(&shared, &value),
                "reading" => set_reading(&shared, &value),
                _ => Err(mlua::Error::runtime(format!(
                    "session.{field} is asked for, not set"
                ))),
            }
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

/// Puts a text in the command line, unless what is typed there is not to be
/// told.
fn set_input(state: &Rc<RefCell<State>>, text: &Value) -> mlua::Result<()> {
    let text = match text {
        Value::Nil => String::new(),
        Value::String(text) => text.to_str()?.to_owned(),
        _ => return Err(mlua::Error::runtime("session.input is a text")),
    };
    let mut state = state.borrow_mut();
    if state.input.is_some() {
        state.input = Some(text.clone());
        state.actions.push(Action::Input(text));
    }
    Ok(())
}

/// Has the game read aloud, or not: `nil` is read aloud again.
fn set_reading(state: &Rc<RefCell<State>>, value: &Value) -> mlua::Result<()> {
    let reading = match value {
        Value::Nil => true,
        Value::Boolean(reading) => *reading,
        other => {
            return Err(mlua::Error::runtime(format!(
                "session.reading is true or false, not {}",
                other.type_name()
            )));
        }
    };
    let mut state = state.borrow_mut();
    state.not_reading = !reading;
    state.reading_set_by = (!reading).then(|| state.owner.clone());
    state.actions.push(Action::Reading(reading));
    Ok(())
}

/// Sets what separates the commands typed on one line: a text; `false`, for
/// none; nothing, for `;` again.
fn set_separator(state: &Rc<RefCell<State>>, separator: Value) -> mlua::Result<()> {
    let state = &mut *state.borrow_mut();
    let owner = state.owner.clone();
    state.separator = match separator {
        Value::Nil => None,
        Value::Boolean(false) => Some((owner, None)),
        Value::String(text) => Some((owner, Some(text.to_str()?.to_owned()))),
        other => {
            return Err(mlua::Error::runtime(format!(
                "session.separator is a text, or false for none, not {}",
                other.type_name()
            )));
        }
    };
    Ok(())
}
