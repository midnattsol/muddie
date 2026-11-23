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
        colour: room_colour(&room.get::<Value>("colour")?)?,
    })
}

/// Reads a room's colour given by a script, as [`color_from`] does; a
/// number of the palette is taken in its standard colour.
pub(super) fn room_colour(value: &Value) -> mlua::Result<Option<Colour>> {
    Ok(color_from(value)?.and_then(|colour| match colour {
        Color::Rgb(red, green, blue) => Some([red, green, blue]),
        Color::Indexed(index) => Some(indexed_rgb(index)),
        Color::Default => None,
    }))
}

/// A room's colour as scripts see it: `"#rrggbb"`, or nil for none.
fn colour_text([red, green, blue]: Colour) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// Applies the `cost`, `blocked` and `command` a script gave to an exit.
pub(super) fn apply_exit_options(exit: &mut Exit, options: &ExitOptions) {
    if let Some(cost) = options.cost {
        exit.cost = cost;
    }
    if let Some(blocked) = options.blocked {
        exit.blocked = blocked;
    }
    if let Some(command) = &options.command {
        exit.command = (!command.is_empty()).then(|| command.clone());
    }
}

/// The optional fields of a script's exit options table.
#[derive(Default)]
pub(super) struct ExitOptions {
    pub(crate) cost: Option<u32>,
    pub(crate) blocked: Option<bool>,
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
    table.set("colour", room.colour.map(colour_text))?;
    table.set("note", room.note.as_str())?;
    table.set("renamed", room.renamed)?;
    table.set("added", room.added)?;
    table.set("shape", room.shape.map(muddie_mapper::Shape::key))?;
    table.set("symbol", room.symbol.as_str())?;
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
    install_tools(lua, state, map, &api)?;
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

    let (shared, world) = (Rc::clone(state), Rc::clone(map));
    api.set(
        "arrive",
        lua.create_function(move |_, seen: Table| {
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

    let shared = Rc::clone(state);
    api.set(
        "language",
        lua.create_function(move |_, language: String| {
            (shared.borrow_mut().actions).push(Action::Language(language));
            Ok(())
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "learn",
        lua.create_function(move |_, on: bool| {
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
            let id = id_of(&id)?;
            let found = world.borrow_mut().set_current(id);
            if found {
                shared.borrow_mut().actions.push(Action::Entered(id));
            }
            Ok(found)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "directions",
        lua.create_function(move |_, words: Table| {
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

    let world = Rc::clone(map);
    api.set(
        "names",
        lua.create_function(move |_, ways: Option<Table>| {
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
        // Rooms whose name has the text in it, or, exact, called just so as
        // the MUD names them; by number.
        lua.create_function(move |_, (text, options): (String, Option<Table>)| {
            let exact = match options {
                Some(options) => options.get::<Option<bool>>("exact")?.unwrap_or(false),
                None => false,
            };
            let world = world.borrow();
            Ok(match exact {
                true => world.called(&text),
                false => world.find(&text).into_iter().map(|room| room.id).collect(),
            })
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

/// Removing, moving, weighing, colouring and avoiding rooms.
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
        "edit_room",
        lua.create_function(move |_, (id, options): (Value, Table)| {
            let id = id_of(&id)?;
            let edit = RoomEdit::read(&options)?;
            let mut world = world.borrow_mut();
            let Some(room) = world.room(id) else {
                return Ok(false);
            };
            let at = room.pos;
            saved(edit.apply(&mut world, id, at))?;
            Ok(true)
        })?,
    )?;
    Ok(())
}

/// What `map.edit_room` changes of a room: each only if given; `false`
/// gives back the map's own colour, shape or weight, or no symbol.
struct RoomEdit {
    name: Option<String>,
    weight: Option<u32>,
    avoid: Option<bool>,
    colour: Option<Option<muddie_mapper::Colour>>,
    shape: Option<Option<muddie_mapper::Shape>>,
    symbol: Option<String>,
    x: Option<i32>,
    y: Option<i32>,
    z: Option<i32>,
}

impl RoomEdit {
    fn read(options: &Table) -> mlua::Result<Self> {
        let given = |key: &str| -> mlua::Result<Option<Value>> {
            Ok(match options.get::<Value>(key)? {
                Value::Nil => None,
                value => Some(value),
            })
        };
        let shape = |value: Value| -> mlua::Result<Option<muddie_mapper::Shape>> {
            match value {
                Value::Boolean(false) => Ok(None),
                Value::String(key) => {
                    let key = key.to_str()?.trim().to_owned();
                    let shape = muddie_mapper::Shape::from_key(&key);
                    shape.map(Some).ok_or_else(|| {
                        mlua::Error::runtime(format!(
                            "a room's shape is square, circle, diamond or hexagon, not {key}"
                        ))
                    })
                }
                other => Err(mlua::Error::runtime(format!(
                    "a room's shape is a name, or false for the map's, not {}",
                    other.type_name()
                ))),
            }
        };
        let colour = |value: Value| match value {
            Value::Boolean(false) => Ok(None),
            value => room_colour(&value),
        };
        let symbol = |value: Value| match value {
            Value::Boolean(false) => Ok(String::new()),
            Value::String(text) => Ok(text.to_str()?.to_owned()),
            other => Err(mlua::Error::runtime(format!(
                "a room's symbol is a text, or false for none, not {}",
                other.type_name()
            ))),
        };
        Ok(Self {
            name: options.get("name")?,
            weight: match given("weight")? {
                Some(Value::Boolean(false)) => Some(1),
                Some(_) => options.get("weight")?,
                None => None,
            },
            avoid: options.get("avoid")?,
            colour: given("colour")?.map(colour).transpose()?,
            shape: given("shape")?.map(shape).transpose()?,
            symbol: given("symbol")?.map(symbol).transpose()?,
            x: options.get("x")?,
            y: options.get("y")?,
            z: options.get("z")?,
        })
    }

    fn apply(self, map: &mut Map, id: RoomId, at: Pos) -> Result<(), muddie_mapper::Error> {
        if let Some(name) = &self.name {
            map.rename(id, name)?;
        }
        if let Some(weight) = self.weight {
            map.set_weight(id, weight)?;
        }
        if let Some(avoid) = self.avoid {
            map.set_avoid(id, avoid)?;
        }
        if let Some(colour) = self.colour {
            map.set_colour(id, colour)?;
        }
        if let Some(shape) = self.shape {
            map.set_shape(id, shape)?;
        }
        if let Some(symbol) = &self.symbol {
            map.set_symbol(id, symbol)?;
        }
        if self.x.is_some() || self.y.is_some() || self.z.is_some() {
            let pos = Pos {
                x: self.x.unwrap_or(at.x),
                y: self.y.unwrap_or(at.y),
                z: self.z.unwrap_or(at.z),
            };
            map.move_room(id, pos)?;
        }
        Ok(())
    }
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
            let mut state = shared.borrow_mut();
            state.map_native_off = (!enabled).then(|| state.owner.clone());
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
        "go",
        lua.create_function(move |_, to: Value| {
            (shared.borrow_mut().actions).push(Action::Walk(id_of(&to)?));
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    api.set(
        "stop",
        lua.create_function(move |_, ()| {
            shared.borrow_mut().actions.push(Action::StopWalk);
            Ok(())
        })?,
    )?;

    let shared = Rc::clone(state);
    api.set(
        "import",
        lua.create_function(move |_, address: String| {
            shared.borrow_mut().actions.push(Action::ImportMap(address));
            Ok(())
        })?,
    )?;
    Ok(())
}

/// Naming rooms anew, how many there are, what the scripts offer to do
/// with the map, and whether the map taken from somewhere has changed.
fn install_tools(
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
                let at = state.map_tools.iter().position(|tool| tool.label == label);
                let tool = |callback| MapTool {
                    owner,
                    label,
                    checked,
                    callback,
                };
                match (at, callback) {
                    (Some(at), Some(callback)) => state.map_tools[at] = tool(callback),
                    (None, Some(callback)) => state.map_tools.push(tool(callback)),
                    (Some(at), None) => drop(state.map_tools.remove(at)),
                    (None, None) => return Ok(()),
                }
                state.actions.push(Action::MapTools);
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

/// The exits of rooms, and laying an area out again.
fn install_exits(lua: &Lua, map: &Rc<RefCell<Map>>, api: &Table) -> mlua::Result<()> {
    let world = Rc::clone(map);
    api.set(
        "set_exit",
        lua.create_function(
            move |_, (from, direction, to, options): (Value, String, Value, Option<Table>)| {
                // With no room to lead to, the exit there is changed only.
                let from = id_of(&from)?;
                let to = match to {
                    Value::Nil => None,
                    to => Some(id_of(&to)?),
                };
                let options = ExitOptions::read(options)?;
                let mut world = world.borrow_mut();
                let led = match to {
                    Some(to) => world.set_exit(from, &direction, to),
                    None => Ok(true),
                };
                let result = led.and_then(|found| {
                    let edited = world
                        .edit_exit(from, &direction, |exit| apply_exit_options(exit, &options))?;
                    Ok(found && edited)
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
            let direction = world.direction(&direction);
            let exit = world
                .room(id_of(&from)?)
                .and_then(|room| room.exits.get(&direction));
            let Some(exit) = exit else {
                return Ok(None);
            };
            let table = lua.create_table()?;
            table.set("to", exit.to)?;
            table.set("cost", exit.cost)?;
            table.set("blocked", exit.blocked)?;
            table.set("command", exit.command.as_deref())?;
            table.set("custom", exit.custom)?;
            Ok(Some(table))
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "direction",
        lua.create_function(move |_, word: String| {
            let direction = world.borrow().direction(&word);
            let known = muddie_mapper::known_direction(&direction).is_some();
            Ok(known.then_some(direction))
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "tidy",
        lua.create_function(move |_, area: Option<String>| {
            let mut world = world.borrow_mut();
            let area = area.or_else(|| Some(world.current()?.area.clone()));
            let Some(area) = area else {
                return Ok((0, None));
            };
            let tidied = (world.tidy(&area))
                .map_err(|error| mlua::Error::runtime(format!("could not save the map: {error}")))?;
            Ok(match tidied {
                muddie_mapper::Tidied::Moved(moved) => (moved, None),
                // Left as it was, and why.
                muddie_mapper::Tidied::Kept { now, then } => (
                    0,
                    Some(format!(
                        "drawn again, {} exits would point the wrong way and {} rooms would lie on another; now {} and {}",
                        then.wrong_exits, then.stacked, now.wrong_exits, now.stacked
                    )),
                ),
            })
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
