//! The map, as scripts see it.

use super::*;

/// Reads a room id argument: a number, or text (see [`room_id`]).
pub(super) fn id_of(value: &Value) -> mlua::Result<RoomId> {
    match value {
        Value::Integer(id) => Ok(*id),
        Value::Number(id) if id.fract() == 0.0 => Ok(*id as RoomId),
        Value::String(text) => Ok(room_id(&text.to_string_lossy())),
        _ => Err(mlua::Error::runtime(
            "a room id must be a whole number or text",
        )),
    }
}

/// Reads a room description given by a script.
pub(super) fn room_info(room: &Table) -> mlua::Result<RoomInfo> {
    let mut exits = std::collections::BTreeMap::new();
    if let Some(table) = room.get::<Option<Table>>("exits")? {
        for pair in table.pairs::<String, Value>() {
            let (direction, to) = pair?;
            exits.insert(muddie_mapper::canonical_direction(&direction), id_of(&to)?);
        }
    }
    let coordinate = |name: &str| room.get::<Option<i32>>(name);
    let pos = match (coordinate("x")?, coordinate("y")?) {
        (Some(x), Some(y)) => Some(Pos {
            x,
            y,
            z: coordinate("z")?.unwrap_or(0),
        }),
        _ => None,
    };
    Ok(RoomInfo {
        id: id_of(&room.get::<Value>("id")?)?,
        name: room.get::<Option<String>>("name")?.unwrap_or_default(),
        area: room.get::<Option<String>>("area")?.unwrap_or_default(),
        exits,
        pos,
    })
}

/// Applies the `cost`, `locked` and `command` a script gave to an exit.
pub(super) fn apply_exit_options(exit: &mut Exit, options: &ExitOptions) {
    if let Some(cost) = options.cost {
        exit.cost = cost;
    }
    if let Some(locked) = options.locked {
        exit.locked = locked;
    }
    if let Some(command) = &options.command {
        exit.command = (!command.is_empty()).then(|| command.clone());
    }
}

/// The optional fields of a script's exit options table.
#[derive(Default)]
pub(super) struct ExitOptions {
    pub(crate) cost: Option<u32>,
    pub(crate) locked: Option<bool>,
    pub(crate) command: Option<String>,
}

/// Describes a room to a script, in the shape [`room_info`] reads.
pub(super) fn room_table(lua: &Lua, room: &Room) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("id", room.id)?;
    table.set("name", room.name.as_str())?;
    table.set("area", room.area.as_str())?;
    table.set("x", room.pos.x)?;
    table.set("y", room.pos.y)?;
    table.set("z", room.pos.z)?;
    table.set("weight", room.weight)?;
    table.set("avoid", room.avoid)?;
    let exits = lua.create_table()?;
    for (direction, exit) in &room.exits {
        exits.set(direction.as_str(), exit.to)?;
    }
    table.set("exits", exits)?;
    Ok(table)
}

/// Defines the `map` table.
pub(super) fn install_map_api(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
) -> mlua::Result<()> {
    let api = lua.create_table()?;
    install_visiting(lua, state, map, &api)?;
    install_finding(lua, map, &api)?;
    install_changing(lua, map, &api)?;
    install_keeping(lua, state, map, &api)?;
    install_routes(lua, state, map, &api)?;
    install_exits(lua, map, &api)?;
    lua.globals().set("map", api)
}

/// The map is updated even when saving it fails; the script still gets
/// the failure as an error, so the user hears about it.
fn saved(result: Result<(), muddie_mapper::Error>) -> mlua::Result<()> {
    result.map_err(|error| mlua::Error::runtime(format!("could not save the map: {error}")))
}

/// Visiting a room, and setting one as the MUD describes it.
fn install_visiting(
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

/// A room by its number or the text it is known by, rooms by name, and the areas.
fn install_finding(lua: &Lua, map: &Rc<RefCell<Map>>, api: &Table) -> mlua::Result<()> {
    let world = Rc::clone(map);
    api.set(
        "room",
        lua.create_function(move |lua, id: Value| {
            let world = world.borrow();
            let room = match id {
                Value::Nil => world.current(),
                id => world.room(id_of(&id)?),
            };
            room.map(|room| room_table(lua, room)).transpose()
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "find",
        lua.create_function(move |lua, text: String| {
            let world = world.borrow();
            let rooms = world.find(&text).into_iter();
            rooms
                .map(|room| room_table(lua, room))
                .collect::<mlua::Result<Vec<Table>>>()
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "areas",
        lua.create_function(move |_, ()| {
            let areas: Vec<String> = (world.borrow().areas().into_iter())
                .map(str::to_owned)
                .collect();
            Ok(areas)
        })?,
    )?;

    api.set(
        "id",
        lua.create_function(|_, text: String| Ok(room_id(&text)))?,
    )?;
    Ok(())
}

/// Removing, moving, weighing and avoiding rooms.
fn install_changing(lua: &Lua, map: &Rc<RefCell<Map>>, api: &Table) -> mlua::Result<()> {
    let world = Rc::clone(map);
    api.set(
        "remove_room",
        lua.create_function(move |_, id: Value| {
            let result = world.borrow_mut().remove_room(id_of(&id)?);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "move",
        lua.create_function(move |_, (id, x, y, z): (Value, i32, i32, Option<i32>)| {
            let pos = Pos {
                x,
                y,
                z: z.unwrap_or(0),
            };
            let result = world.borrow_mut().move_room(id_of(&id)?, pos);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "set_weight",
        lua.create_function(move |_, (id, weight): (Value, u32)| {
            let result = world.borrow_mut().set_weight(id_of(&id)?, weight);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "avoid",
        lua.create_function(move |_, (id, avoid): (Value, bool)| {
            let result = world.borrow_mut().set_avoid(id_of(&id)?, avoid);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;
    Ok(())
}

/// Whether the client keeps the map itself, and in a file.
fn install_keeping(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    api: &Table,
) -> mlua::Result<()> {
    let shared = Rc::clone(state);
    api.set(
        "native",
        lua.create_function(move |_, enabled: bool| {
            shared.borrow_mut().map_native_off = !enabled;
            Ok(())
        })?,
    )?;

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "persist",
        lua.create_function(move |_, enabled: bool| {
            shared.borrow_mut().map_memory_only = !enabled;
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
fn install_routes(
    lua: &Lua,
    state: &Rc<RefCell<State>>,
    map: &Rc<RefCell<Map>>,
    api: &Table,
) -> mlua::Result<()> {
    let world = Rc::clone(map);
    api.set(
        "path",
        lua.create_function(move |_, (to, from): (Value, Value)| {
            let world = world.borrow();
            let from = match from {
                Value::Nil => world.current().map(|room| room.id),
                from => Some(id_of(&from)?),
            };
            let route = from.and_then(|from| world.route(from, id_of(&to).ok()?));
            Ok(route.map(|steps| {
                (steps.into_iter())
                    .map(|step| step.command)
                    .collect::<Vec<String>>()
            }))
        })?,
    )?;

    let shared = Rc::clone(state);
    api.set(
        "walk",
        lua.create_function(move |_, to: Value| {
            (shared.borrow_mut().actions).push(Action::Walk(id_of(&to)?));
            Ok(())
        })?,
    )?;
    Ok(())
}

/// The exits of rooms, and laying an area out again.
fn install_exits(lua: &Lua, map: &Rc<RefCell<Map>>, api: &Table) -> mlua::Result<()> {
    let world = Rc::clone(map);
    api.set(
        "set_exit",
        lua.create_function(
            move |_, (from, direction, to, options): (Value, String, Value, Option<Table>)| {
                let (from, to) = (id_of(&from)?, id_of(&to)?);
                let options = ExitOptions::read(options)?;
                let mut world = world.borrow_mut();
                let result = world.set_exit(from, &direction, to).and_then(|found| {
                    world.edit_exit(from, &direction, |exit| apply_exit_options(exit, &options))?;
                    Ok(found)
                });
                let found = *result.as_ref().unwrap_or(&true);
                saved(result.map(drop))?;
                Ok(found)
            },
        )?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "edit_exit",
        lua.create_function(
            move |_, (from, direction, options): (Value, String, Option<Table>)| {
                let options = ExitOptions::read(options)?;
                let result = (world.borrow_mut()).edit_exit(id_of(&from)?, &direction, |exit| {
                    apply_exit_options(exit, &options)
                });
                let found = *result.as_ref().unwrap_or(&true);
                saved(result.map(drop))?;
                Ok(found)
            },
        )?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "exit",
        lua.create_function(move |lua, (from, direction): (Value, String)| {
            let world = world.borrow();
            let direction = muddie_mapper::canonical_direction(&direction);
            let exit = world
                .room(id_of(&from)?)
                .and_then(|room| room.exits.get(&direction));
            let Some(exit) = exit else {
                return Ok(None);
            };
            let table = lua.create_table()?;
            table.set("to", exit.to)?;
            table.set("cost", exit.cost)?;
            table.set("locked", exit.locked)?;
            table.set("command", exit.command.as_deref())?;
            table.set("custom", exit.custom)?;
            Ok(Some(table))
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "relayout",
        lua.create_function(move |_, area: Option<String>| {
            let mut world = world.borrow_mut();
            let area = area.or_else(|| Some(world.current()?.area.clone()));
            let Some(area) = area else {
                return Ok(0);
            };
            world
                .relayout(&area)
                .map_err(|error| mlua::Error::runtime(format!("could not save the map: {error}")))
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "remove_exit",
        lua.create_function(move |_, (from, direction): (Value, String)| {
            let result = world.borrow_mut().remove_exit(id_of(&from)?, &direction);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;
    Ok(())
}
