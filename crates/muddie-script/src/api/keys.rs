//! `shortcuts`: the keys of every shortcut of the client, to read and to
//! choose. What a script chooses goes first while it is loaded, and goes
//! with it: so that a set of keys can be written, shared and changed as a
//! script.

use super::*;

/// One of the client's shortcuts, as the frontend tells of it.
#[derive(Debug, Clone, PartialEq)]
pub struct KnownShortcut {
    /// Its name: `explore`, `map_north`.
    pub name: String,
    /// Where it works: `everywhere`, `reading`, `line`, `map`, `editor`,
    /// `layout`.
    pub place: String,
    /// What it does, in the language of the interface.
    pub about: String,
    /// Its keys without the scripts, as they are written to be kept.
    pub keys: Vec<String>,
}

/// Reads keys a script gives for a shortcut, `(shortcut, keys)`: the keys
/// as they are kept, or what is wrong with them.
pub type CheckKeys = fn(&str, &str) -> Result<String, String>;

pub(crate) fn install(lua: &Lua, state: &Rc<RefCell<State>>) -> mlua::Result<()> {
    let shortcuts = super::group(lua, "shortcuts")?;
    let shared = Rc::clone(state);
    shortcuts.set(
        "all",
        lua.create_function(move |lua, ()| {
            let state = shared.borrow();
            let list = lua.create_table()?;
            for known in &state.known_shortcuts {
                let entry = lua.create_table()?;
                entry.set("name", known.name.as_str())?;
                entry.set("place", known.place.as_str())?;
                entry.set("about", known.about.as_str())?;
                entry.set("keys", keys_now(&state, known))?;
                list.push(entry)?;
            }
            Ok(list)
        })?,
    )?;
    let meta = lua.create_table()?;
    let shared = Rc::clone(state);
    meta.set(
        "__index",
        lua.create_function(move |_, (_, name): (Table, String)| {
            let state = shared.borrow();
            let known = known(&state, &name)?;
            Ok(keys_now(&state, known))
        })?,
    )?;
    let shared = Rc::clone(state);
    meta.set(
        "__newindex",
        lua.create_function(move |_, (_, name, value): (Table, String, Value)| {
            choose(&shared, &name, value)
        })?,
    )?;
    shortcuts.set_metatable(Some(meta))
}

/// The shortcut named so, or an error that says which there are.
fn known<'a>(state: &'a State, name: &str) -> mlua::Result<&'a KnownShortcut> {
    (state.known_shortcuts.iter())
        .find(|known| known.name == name)
        .ok_or_else(|| {
            let names: Vec<&str> = (state.known_shortcuts.iter())
                .map(|known| known.name.as_str())
                .collect();
            mlua::Error::runtime(format!(
                "there is no shortcut {name}; there are {}",
                names.join(", ")
            ))
        })
}

/// The keys a shortcut has now: those a script chose, else the client's.
fn keys_now(state: &State, known: &KnownShortcut) -> Vec<String> {
    (state.shortcuts.iter())
        .rev()
        .find(|(_, name, _)| *name == known.name)
        .map_or_else(|| known.keys.clone(), |(_, _, keys)| keys.clone())
}

/// Chooses a shortcut's keys: one, a list, `false` for none, or `nil` to
/// give it back those it had.
fn choose(state: &Rc<RefCell<State>>, name: &str, value: Value) -> mlua::Result<()> {
    let mut state = state.borrow_mut();
    known(&state, name)?;
    let given: Option<Vec<String>> = match value {
        Value::Nil => None,
        Value::Boolean(false) => Some(Vec::new()),
        Value::String(key) => Some(vec![key.to_str()?.to_owned()]),
        Value::Table(keys) => Some(
            (keys.sequence_values::<String>())
                .collect::<mlua::Result<Vec<String>>>()
                .map_err(|_| {
                    mlua::Error::runtime(format!("shortcuts.{name} is given keys as texts"))
                })?,
        ),
        other => {
            let kind = match other {
                Value::Integer(_) | Value::Number(_) => "number",
                other => other.type_name(),
            };
            return Err(mlua::Error::runtime(format!(
                "shortcuts.{name} is a key, a list of keys, false for none or nil, not {kind}"
            )));
        }
    };
    state.shortcuts.retain(|(_, known, _)| known != name);
    if let Some(given) = given {
        let check = state
            .check_shortcut
            .ok_or_else(|| mlua::Error::runtime("no shortcuts here"))?;
        let mut keys = Vec::new();
        for key in given {
            let kept = check(name, &key).map_err(|wrong| {
                mlua::Error::runtime(format!("shortcuts.{name}: {key} {wrong}"))
            })?;
            if !keys.contains(&kept) {
                keys.push(kept);
            }
        }
        let owner = state.owner.clone();
        state.shortcuts.push((owner, name.to_owned(), keys));
    }
    state.shortcuts_changed += 1;
    Ok(())
}
