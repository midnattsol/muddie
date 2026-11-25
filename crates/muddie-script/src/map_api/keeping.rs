//! Keeping the map in a file, routes, and what scripts offer to do with it.

use super::*;

/// Whether the client keeps the map itself, and in a file.
pub(super) fn install_keeping(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    api: &Table,
) -> mlua::Result<()> {
    let shared = Rc::clone(state);
    api.set(
        "native",
        // Given, whether the built-in loader is on; asked, whether it is.
        lua.create_function(move |_, enabled: Option<bool>| {
            let mut state = shared.borrow_mut();
            if let Some(enabled) = enabled {
                state.map_native_off = (!enabled).then(|| state.owner.clone());
            }
            Ok(state.map_native_off.is_none())
        })?,
    )?;

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "persist",
        lua.create_function(move |_, enabled: bool| {
            let mut state = shared.borrow_mut();
            state.map_memory_only = (!enabled).then(|| state.owner.clone());
            drop(state);
            if !enabled {
                world.borrow_mut().stop_storing();
            }
            Ok(())
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "clear",
        lua.create_function(move |_, ()| saved(world.borrow_mut().clear()))?,
    )?;
    Ok(())
}

/// Routes, and walking them.
pub(super) fn install_routes(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    api: &Table,
) -> mlua::Result<()> {
    let world = Rc::clone(map);
    api.set(
        "way",
        lua.create_function(move |_, (to, from): (Value, Value)| {
            let world = world.borrow();
            let from = match from {
                Value::Nil => world.current().map(|room| room.id),
                from => Some(place_of(&world, &from)?),
            };
            let to = place_of(&world, &to)?;
            let route = from.and_then(|from| world.route(from, to));
            Ok(route.map(|steps| {
                (steps.into_iter())
                    .map(|step| step.command)
                    .collect::<Vec<String>>()
            }))
        })?,
    )?;

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "go",
        lua.create_function(move |_, to: Value| {
            let to = place_of(&world.borrow(), &to)?;
            (shared.borrow_mut().actions).push(Action::Walk(to));
            Ok(())
        })?,
    )?;

    for (name, action) in [("stop", Action::StopWalk), ("failed", Action::MoveFailed)] {
        let shared = Rc::clone(state);
        api.set(
            name,
            lua.create_function(move |_, ()| {
                shared.borrow_mut().actions.push(action.clone());
                Ok(())
            })?,
        )?;
    }

    let shared = Rc::clone(state);
    api.set(
        "import",
        lua.create_function(move |_, address: String| {
            shared.borrow_mut().actions.push(Action::ImportMap(address));
            Ok(())
        })?,
    )?;
    let shared = Rc::clone(state);
    api.set(
        "show_area",
        lua.create_function(move |_, area: String| {
            shared.borrow_mut().actions.push(Action::ShowArea(area));
            Ok(())
        })?,
    )?;
    Ok(())
}

/// Naming rooms anew, how many there are, what the scripts offer to do
/// with the map, and whether the map taken from somewhere has changed.
pub(super) fn install_actions(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    api: &Table,
) -> mlua::Result<()> {
    let world = Rc::clone(map);
    api.set(
        "count",
        lua.create_function(move |_, ()| Ok(world.borrow().rooms().count()))?,
    )?;

    let shared = Rc::clone(state);
    api.set(
        "action",
        lua.create_function(
            move |_, (label, callback, options): (Value, Option<Function>, Option<Table>)| {
                let label = texts_of(label)?;
                if label.is_empty() {
                    return Err(mlua::Error::runtime(
                        "an action needs a name to be chosen by",
                    ));
                }
                let checked = match &options {
                    Some(options) => options.get::<Option<bool>>("checked")?,
                    None => None,
                };
                let mut state = shared.borrow_mut();
                let owner = state.owner.clone();
                let at = state
                    .map_actions
                    .iter()
                    .position(|tool| tool.label == label);
                let tool = |callback| MapAction {
                    owner,
                    label,
                    checked,
                    callback,
                };
                match (at, callback) {
                    (Some(at), Some(callback)) => state.map_actions[at] = tool(callback),
                    (None, Some(callback)) => state.map_actions.push(tool(callback)),
                    (Some(at), None) => drop(state.map_actions.remove(at)),
                    (None, None) => return Ok(()),
                }
                state.actions.push(Action::MapActions);
                Ok(())
            },
        )?,
    )?;

    let shared = Rc::clone(state);
    api.set(
        "newer",
        lua.create_function(move |_, (address, callback): (String, Function)| {
            let mut state = shared.borrow_mut();
            let id = state.next_id();
            let owner = state.owner.clone();
            state.awaited.push(Awaited {
                id,
                owner,
                callback,
            });
            state.actions.push(Action::MapNewer {
                address,
                answer: id,
            });
            Ok(())
        })?,
    )?;
    Ok(())
}
