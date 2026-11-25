//! Finding rooms, changing them, and their exits.

use super::*;

/// A room by its number or the text it is known by, rooms by name, and the areas.
pub(super) fn install_finding(lua: &Lua, map: &Rc<RefCell<Map>>, api: &Table) -> mlua::Result<()> {
    let world = Rc::clone(map);
    api.set(
        "room",
        lua.create_function(move |lua, id: Value| {
            let world = world.borrow();
            let room = match id {
                Value::Nil => world.current(),
                id => world.room(place_of(&world, &id)?),
            };
            room.map(|room| room_table(lua, room)).transpose()
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "find",
        // The rooms found by the text, as the client finds them: by mark,
        // note or name, the nearest first; or, exact, those called just so
        // as the MUD names them, by number.
        lua.create_function(move |_, (text, options): (String, Option<Table>)| {
            only(options.as_ref(), &["exact"])?;
            let exact = match options {
                Some(options) => options.get::<Option<bool>>("exact")?.unwrap_or(false),
                None => false,
            };
            let world = world.borrow();
            Ok(match exact {
                true => world.called(&text),
                false => {
                    let here = world.current().map(|room| room.id);
                    let mut rooms: Vec<RoomId> = Vec::new();
                    for found in world.search(&text, here) {
                        if !rooms.contains(&found.room) {
                            rooms.push(found.room);
                        }
                    }
                    rooms
                }
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
pub(super) fn install_changing(lua: &Lua, map: &Rc<RefCell<Map>>, api: &Table) -> mlua::Result<()> {
    let world = Rc::clone(map);
    api.set(
        "remove_room",
        lua.create_function(move |_, id: Value| {
            let id = place_of(&world.borrow(), &id)?;
            let result = world.borrow_mut().remove_room(id);
            let found = *result.as_ref().unwrap_or(&true);
            saved(result.map(drop))?;
            Ok(found)
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "marks",
        lua.create_function(move |_, ()| Ok(world.borrow().marks().clone()))?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "mark",
        lua.create_function(move |_, (name, room): (String, Value)| {
            let mut world = world.borrow_mut();
            match room {
                Value::Nil => saved(world.unmark(&name)),
                room => {
                    let id = place_of(&world, &room)?;
                    if world.room(id).is_none() {
                        return Ok(false);
                    }
                    saved(world.mark(&name, id)).map(|()| true)
                }
            }
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "edit_room",
        lua.create_function(move |_, (id, options): (Value, Table)| {
            let id = place_of(&world.borrow(), &id)?;
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
/// gives back the map's own colour or shape, no cost, or no symbol.
struct RoomEdit {
    name: Option<String>,
    note: Option<String>,
    cost: Option<u32>,
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
        only(
            Some(options),
            &[
                "name", "note", "cost", "avoid", "colour", "shape", "symbol", "x", "y", "z",
            ],
        )?;
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
            note: options.get("note")?,
            cost: match given("cost")? {
                Some(Value::Boolean(false)) => Some(0),
                Some(_) => options.get("cost")?,
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
        if let Some(note) = &self.note {
            map.set_note(id, note)?;
        }
        if let Some(cost) = self.cost {
            map.set_cost(id, cost)?;
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
/// The exits of rooms, and laying an area out again.
pub(super) fn install_exits(lua: &Lua, map: &Rc<RefCell<Map>>, api: &Table) -> mlua::Result<()> {
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

    install_tidying(lua, map, api)
}

/// Tidying an area, undoing a change, and removing exits.
fn install_tidying(lua: &Lua, map: &Rc<RefCell<Map>>, api: &Table) -> mlua::Result<()> {
    let world = Rc::clone(map);
    api.set(
        "tidy",
        lua.create_function(move |lua, area: Option<String>| {
            let mut world = world.borrow_mut();
            let area = match area {
                Some(area) => Some(world.area_named(&area).map_or(area, str::to_owned)),
                None => world.current().map(|room| room.area.clone()),
            };
            let Some(area) = area else {
                return Ok((0, None));
            };
            let tidied = saved(world.tidy(&area))?;
            // Left as it was, how it is drawn and how it would be.
            let drawing = |drawing: muddie_mapper::Drawing| -> mlua::Result<Table> {
                let table = lua.create_table()?;
                table.set("wrong_exits", drawing.wrong_exits)?;
                table.set("stacked", drawing.stacked)?;
                Ok(table)
            };
            Ok(match tidied {
                muddie_mapper::Tidied::Moved(moved) => (moved, None),
                muddie_mapper::Tidied::Kept { now, then } => {
                    let kept = lua.create_table()?;
                    kept.set("now", drawing(now)?)?;
                    kept.set("then", drawing(then)?)?;
                    (0, Some(kept))
                }
            })
        })?,
    )?;

    let world = Rc::clone(map);
    api.set(
        "undo",
        lua.create_function(move |_, ()| {
            let undone = saved(world.borrow_mut().undo_change())?;
            Ok(undone.is_some())
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
