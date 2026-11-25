//! Tests of the map: placing, editing, routes and keeping it.

use super::*;

mod adding;
mod basics;
mod following;
mod learning;

pub(crate) fn info(id: RoomId, name: &str, exits: &[(&str, RoomId)]) -> RoomInfo {
    RoomInfo {
        id,
        name: name.into(),
        area: "town".into(),
        exits: (exits.iter())
            .map(|(direction, to)| ((*direction).to_owned(), *to))
            .collect(),
        pos: None,
        colour: None,
    }
}

fn pos(x: i32, y: i32, z: i32) -> Pos {
    Pos { x, y, z }
}

/// A room as a map taken from elsewhere has it: in `area`, `at`, with its
/// exits.
fn room_at(area: &str, id: RoomId, name: &str, at: Pos, exits: &[(&str, RoomId)]) -> Room {
    Room {
        id,
        name: name.to_owned(),
        area: area.to_owned(),
        pos: at,
        exits: (exits.iter())
            .map(|(way, to)| {
                let exit = Exit {
                    to: *to,
                    ..Exit::default()
                };
                ((*way).to_owned(), exit)
            })
            .collect(),
        cost: 0,
        avoid: false,
        colour: None,
        note: String::new(),
        renamed: false,
        added: false,
        shape: None,
        symbol: String::new(),
        styled: false,
    }
}

/// A room by its number, name and exits.
type Laid<'a> = (RoomId, &'a str, &'a [(&'a str, RoomId)]);

/// A map of those rooms in `area`, taken whole, each in a row east by its
/// number.
fn map_of(area: &str, rooms: &[Laid<'_>]) -> Map {
    let mut map = Map::in_memory();
    let rooms = (rooms.iter())
        .map(|(id, name, exits)| room_at(area, *id, name, pos(*id as i32, 0, 0), exits))
        .collect();
    map.replace_all(rooms, None).unwrap();
    map
}

/// 1 -n-> 2 -e-> 3 -u-> 4, plus a one-way shortcut 1 -portal-> 4.
fn town() -> Map {
    let mut map = Map::in_memory();
    for room in [
        info(1, "Gate", &[("n", 2), ("portal", 4)]),
        info(2, "Square", &[("s", 1), ("e", 3)]),
        info(3, "Tower", &[("w", 2), ("u", 4)]),
        info(4, "Tower Top", &[("d", 3)]),
    ] {
        map.visit(room).unwrap();
    }
    map
}

/// A desert of rooms alike, as maps taken from elsewhere have them: a row
/// of three "Dune" rooms, 10 - 11 - 12, joined east and west, with an
/// "Oasis" north of 12 and a "Dune [e,w]" written with its exits.
fn desert() -> Map {
    let mut map = map_of(
        "desert",
        &[
            (10, "Dune", &[("e", 11)]),
            (11, "Dune [e,w]", &[("e", 12), ("w", 10)]),
            (12, "Dune", &[("w", 11), ("n", 13)]),
            (13, "Oasis", &[("s", 12)]),
        ],
    );
    // As maps taken from elsewhere are, the exits after a name.
    map.set_naming(Naming {
        exits_between: Some(("[".into(), "]".into())),
        ..Naming::default()
    });
    map
}
