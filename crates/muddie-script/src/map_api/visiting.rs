//! Visiting a room, and setting one as the MUD describes it.

use super::*;

/// Visiting a room, and setting one as the MUD describes it.
pub(super) fn install_visiting(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    api: &Table,
) -> mlua::Result<()> {
    install_entering(lua, state, map, api)?;
    install_following(lua, state, map, api)?;
    install_teaching(lua, state, map, api)
}

/// Entering a room: visited, arrived at as the server shows it, or added.
fn install_entering(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    api: &Table,
) -> mlua::Result<()> {
    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "visit",
        lua.create_function(move |_, room: Table| {
            let info = room_info(&room)?;
            let id = info.id;
            let result = world.borrow_mut().visit(info);
            shared.borrow_mut().actions.push(Action::Entered(id));
            saved(result)?;
            Ok(id)
        })?,
    )?;

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "arrive",
        lua.create_function(move |_, seen: Table| {
            only(Some(&seen), &["name", "exits", "came"])?;
            let name: Option<String> = seen.get("name")?;
            let exits: Option<Vec<String>> = seen.get("exits")?;
            let came: Option<String> = seen.get("came")?;
            let located = world.borrow_mut().arrive(Sighting {
                name: name.as_deref(),
                exits: exits.as_deref(),
                came: came.as_deref(),
            });
            let (action, here) = match located {
                Located::Here(id) => (Action::Entered(id), Some(id)),
                _ => (Action::LostTrack, None),
            };
            shared.borrow_mut().actions.push(action);
            Ok(here)
        })?,
    )?;

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "add",
        lua.create_function(move |_, seen: Table| {
            only(Some(&seen), &["name", "exits", "came"])?;
            let name: Option<String> = seen.get("name")?;
            let exits: Option<Vec<String>> = seen.get("exits")?;
            let came: Option<String> = seen.get("came")?;
            let added = world.borrow_mut().add(Arrival {
                name: name.as_deref().unwrap_or_default(),
                exits: exits.as_deref().unwrap_or_default(),
                came: came.as_deref().unwrap_or_default(),
            });
            let (id, said) = match added {
                Ok(Added::Made(id)) => (Some(id), "made"),
                Ok(Added::Found(id)) => (Some(id), "found"),
                Err(Refused::Lost) => (None, "lost"),
                Err(Refused::Unnamed) => (None, "unnamed"),
                Err(Refused::Known(_)) => (None, "known"),
                Err(Refused::Alike(_)) => (None, "alike"),
            };
            if let Some(id) = id {
                shared.borrow_mut().actions.push(Action::Entered(id));
            }
            Ok((id, said))
        })?,
    )?;
    Ok(())
}

/// Following the player: the language, learning, the rooms they may be in,
/// and putting them in one.
fn install_following(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    api: &Table,
) -> mlua::Result<()> {
    let shared = Rc::clone(state);
    api.set(
        "language",
        lua.create_function(move |_, language: String| {
            let mut state = shared.borrow_mut();
            state.set(crate::state::Set::Language);
            state.actions.push(Action::Language(language));
            Ok(())
        })?,
    )?;

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "learn",
        lua.create_function(move |_, on: bool| {
            shared.borrow_mut().set(crate::state::Set::Learning);
            world.borrow_mut().set_learning(on);
            Ok(())
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "candidates",
        lua.create_function(move |_, ()| Ok(world.borrow().candidates().to_vec()))?,
    )?;

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "here",
        lua.create_function(move |_, id: Value| {
            let id = place_of(&world.borrow(), &id)?;
            let found = world.borrow_mut().set_current(id);
            if found {
                shared.borrow_mut().actions.push(Action::Entered(id));
            }
            Ok(found)
        })?,
    )?;
    Ok(())
}

/// Teaching the map the words of the MUD, and setting a room.
fn install_teaching(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    api: &Table,
) -> mlua::Result<()> {
    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "directions",
        lua.create_function(move |_, words: Table| {
            shared.borrow_mut().set(crate::state::Set::Directions);
            let mut taught = Vec::new();
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
            world.borrow_mut().teach_directions(&taught);
            Ok(())
        })?,
    )?;

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "names",
        lua.create_function(move |_, ways: Option<Table>| {
            only(ways.as_ref(), &["region", "exits"])?;
            shared.borrow_mut().set(crate::state::Set::Naming);
            let naming = match ways {
                None => Naming::default(),
                Some(ways) => {
                    let exits: Option<String> = ways.get("exits")?;
                    let exits_between = match exits.as_deref().map(|pair| pair.chars().collect::<Vec<_>>()) {
                        None => None,
                        Some(pair) if pair.len() == 2 => Some((pair[0].to_string(), pair[1].to_string())),
                        Some(_) => {
                            return Err(mlua::Error::runtime(
                                "exits must be the two characters the exits are written between, such as \"[]\"",
                            ));
                        }
                    };
                    Naming {
                        region_before: ways.get("region")?,
                        exits_between,
                    }
                }
            };
            world.borrow_mut().set_naming(naming);
            Ok(())
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "set_room",
        lua.create_function(move |_, room: Table| {
            let info = room_info(&room)?;
            let id = info.id;
            saved(world.borrow_mut().set_room(info))?;
            Ok(id)
        })?,
    )?;
    Ok(())
}
