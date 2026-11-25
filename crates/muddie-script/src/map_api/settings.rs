//! What a script sets of the map, as fields of `map`: for as long as it is
//! loaded.

use super::*;
use crate::state::Set;
use mlua::IntoLua;

/// What there is to set.
const FIELDS: [&str; 6] = [
    "native",
    "persist",
    "learn",
    "language",
    "directions",
    "names",
];

/// What scripts set of the map, as it is asked for.
#[derive(Default)]
pub(crate) struct MapSettings {
    pub(crate) learn: bool,
    pub(crate) language: Option<String>,
    pub(crate) directions: Vec<(String, Vec<String>)>,
    pub(crate) names: Naming,
}

pub(super) fn install_settings(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    api: &Table,
) -> mlua::Result<()> {
    let meta = lua.create_table()?;
    let shared = Rc::clone(state);
    meta.set(
        "__index",
        lua.create_function(move |lua, (_, field): (Table, String)| {
            get(lua, &shared.borrow(), &field)
        })?,
    )?;
    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    meta.set(
        "__newindex",
        lua.create_function(move |_, (_, field, value): (Table, String, Value)| {
            set(&shared, &world, &field, value)
        })?,
    )?;
    api.set_metatable(Some(meta))
}

fn get(lua: &Lua, state: &State, field: &str) -> mlua::Result<Value> {
    let settings = &state.map_settings;
    match field {
        "native" => Ok(Value::Boolean(state.map_native_off.is_none())),
        "persist" => Ok(Value::Boolean(state.map_memory_only.is_none())),
        "learn" => Ok(Value::Boolean(settings.learn)),
        "language" => settings.language.as_deref().into_lua(lua),
        "directions" => {
            let directions = lua.create_table()?;
            for (direction, words) in &settings.directions {
                directions.set(direction.as_str(), words.clone())?;
            }
            directions.into_lua(lua)
        }
        "names" => {
            let names = lua.create_table()?;
            names.set("region", settings.names.region_before.as_deref())?;
            let exits = settings.names.exits_between.as_ref();
            names.set("exits", exits.map(|(open, close)| format!("{open}{close}")))?;
            names.into_lua(lua)
        }
        other => Err(mlua::Error::runtime(format!("there is no map.{other}"))),
    }
}

fn set(
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    field: &str,
    value: Value,
) -> mlua::Result<()> {
    match field {
        "native" => {
            let on = switch(field, &value)?.unwrap_or(true);
            let state = &mut *state.borrow_mut();
            state.map_native_off = (!on).then(|| state.owner.clone());
        }
        "persist" => {
            let on = switch(field, &value)?.unwrap_or(true);
            let mut state = state.borrow_mut();
            state.map_memory_only = (!on).then(|| state.owner.clone());
            drop(state);
            if !on {
                map.borrow_mut().stop_storing();
            }
        }
        "learn" => {
            let on = switch(field, &value)?.unwrap_or(false);
            state.borrow_mut().set(Set::Learning);
            state.borrow_mut().map_settings.learn = on;
            map.borrow_mut().set_learning(on);
        }
        "language" => {
            let language = match value {
                Value::Nil => None,
                Value::String(text) => Some(text.to_str()?.to_owned()),
                _ => return Err(mlua::Error::runtime("map.language is a text, as \"es\"")),
            };
            let mut state = state.borrow_mut();
            state.set(Set::Language);
            (state.actions).push(Action::Language(language.clone().unwrap_or_default()));
            state.map_settings.language = language;
        }
        "directions" => {
            let directions = directions_of(table(field, value)?)?;
            state.borrow_mut().set(Set::Directions);
            let mut map = map.borrow_mut();
            map.forget_taught_directions();
            map.teach_directions(&directions);
            state.borrow_mut().map_settings.directions = directions;
        }
        "names" => {
            let naming = naming_of(table(field, value)?)?;
            state.borrow_mut().set(Set::Naming);
            map.borrow_mut().set_naming(naming.clone());
            state.borrow_mut().map_settings.names = naming;
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "there is no map.{other} to set; there are {}",
                FIELDS.join(", ")
            )));
        }
    }
    Ok(())
}

/// Whether something is switched on, or put back as it is if nobody says.
fn switch(field: &str, value: &Value) -> mlua::Result<Option<bool>> {
    match value {
        Value::Nil => Ok(None),
        Value::Boolean(on) => Ok(Some(*on)),
        _ => Err(mlua::Error::runtime(format!(
            "map.{field} is true or false"
        ))),
    }
}

/// The table a field is set to, or nothing, to put it back.
fn table(field: &str, value: Value) -> mlua::Result<Option<Table>> {
    match value {
        Value::Nil => Ok(None),
        Value::Table(table) => Ok(Some(table)),
        _ => Err(mlua::Error::runtime(format!("map.{field} is a table"))),
    }
}

/// Words for directions: each with one, or a list of them.
fn directions_of(words: Option<Table>) -> mlua::Result<Vec<(String, Vec<String>)>> {
    let mut taught = Vec::new();
    let Some(words) = words else {
        return Ok(taught);
    };
    for pair in words.pairs::<String, Value>() {
        let (direction, said) = pair?;
        let said: Vec<String> = match said {
            Value::String(word) => vec![word.to_str()?.to_owned()],
            Value::Table(list) => list.sequence_values().collect::<mlua::Result<_>>()?,
            other => {
                return Err(mlua::Error::runtime(format!(
                    "the words for {direction} must be a string or a list, not {}",
                    other.type_name()
                )));
            }
        };
        taught.push((direction, said));
    }
    Ok(taught)
}

/// How the MUD writes the names of rooms.
fn naming_of(ways: Option<Table>) -> mlua::Result<Naming> {
    only(ways.as_ref(), &["region", "exits"])?;
    let Some(ways) = ways else {
        return Ok(Naming::default());
    };
    let exits: Option<String> = ways.get("exits")?;
    let pair: Option<Vec<char>> = exits.map(|pair| pair.chars().collect());
    let exits_between = match pair.as_deref() {
        None => None,
        Some([open, close]) => Some((open.to_string(), close.to_string())),
        Some(_) => {
            return Err(mlua::Error::runtime(
                "exits must be the two characters the exits are written between, such as \"[]\"",
            ));
        }
    };
    Ok(Naming {
        region_before: ways.get("region")?,
        exits_between,
    })
}
