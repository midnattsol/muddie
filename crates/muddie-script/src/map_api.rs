//! The map, as scripts see it.

use super::*;

use keeping::{install_actions, install_keeping, install_routes};
use rooms::{install_changing, install_exits, install_finding};
pub(crate) use settings::MapSettings;
use settings::install_settings;
use visiting::install_visiting;

mod keeping;
mod rooms;
mod settings;
mod visiting;

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

/// Reads a place: a room's id, as [`id_of`] reads one, or, as the user
/// names places, its mark or name, the nearest of several. Wherever a
/// script gives a room, it may give it so.
fn place_of(world: &Map, value: &Value) -> mlua::Result<RoomId> {
    let id = id_of(value)?;
    let named = match value {
        Value::String(text) if world.room(id).is_none() => {
            world.room_meant(&text.to_string_lossy())
        }
        _ => None,
    };
    Ok(named.unwrap_or(id))
}

/// Reads a room description given by a script, as the MUD sends one in
/// GMCP `Room.Info` or MSDP `ROOM`, or made by the script: its `id` (or
/// `num`, `vnum`), `name`, `area`, `exits`, where it is (`x`, `y`, `z`, or
/// `coords`), and the `colour` to draw it in.
pub(super) fn room_info(room: &Table) -> mlua::Result<RoomInfo> {
    let value = serde_json::to_value(room).map_err(mlua::Error::runtime)?;
    let mut info = RoomInfo::from_value(&value)
        .ok_or_else(|| mlua::Error::runtime("a room needs its id, a number or text"))?;
    info.colour = room_colour(&room.get::<Value>("colour")?)?;
    Ok(info)
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

impl ExitOptions {
    fn read(options: Option<Table>) -> mlua::Result<Self> {
        only(options.as_ref(), &["cost", "blocked", "command"])?;
        let Some(options) = options else {
            return Ok(Self::default());
        };
        Ok(Self {
            cost: options.get("cost")?,
            blocked: options.get("blocked")?,
            command: options.get("command")?,
        })
    }
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
    table.set("cost", room.cost)?;
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
    install_keeping(lua, map, &api)?;
    install_routes(lua, state, map, &api)?;
    install_exits(lua, map, &api)?;
    install_actions(lua, state, map, &api)?;
    install_settings(lua, state, map, &api)?;
    lua.globals().set("map", api)
}

/// The map is updated even when saving it fails; the script still gets
/// the failure as an error, so the user hears about it.
fn saved<T>(result: Result<T, muddie_mapper::Error>) -> mlua::Result<T> {
    result.map_err(|error| mlua::Error::runtime(format!("could not save the map: {error}")))
}
