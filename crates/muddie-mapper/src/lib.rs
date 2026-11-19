//! The automapper's data: a graph of rooms built from what the server
//! reports, where to draw each room, routes between rooms, and storage.

mod info;
mod store;
mod walk;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;

pub use info::RoomInfo;
pub use rusqlite::Error;
pub use walk::{Progress, Walk};

use store::Store;

pub type RoomId = i64;

/// A room's place on the map grid: x grows east, y north and z up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Pos {
    fn offset(self, (dx, dy, dz): (i32, i32, i32), sign: i32) -> Self {
        Self {
            x: self.x + dx * sign,
            y: self.y + dy * sign,
            z: self.z + dz * sign,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub id: RoomId,
    pub name: String,
    pub area: String,
    pub pos: Pos,
    /// Exit direction (see [`canonical_direction`]) to destination room.
    pub exits: BTreeMap<String, RoomId>,
}

/// One move of a route: the command to send and the room it leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub direction: String,
    pub to: RoomId,
}

/// Lower-cases a direction and shortens the compass ones (`north` to `n`),
/// so exits compare equal however the server spells them.
pub fn canonical_direction(direction: &str) -> String {
    let lower = direction.trim().to_lowercase();
    let short = match lower.as_str() {
        "north" => "n",
        "south" => "s",
        "east" => "e",
        "west" => "w",
        "northeast" => "ne",
        "northwest" => "nw",
        "southeast" => "se",
        "southwest" => "sw",
        "up" => "u",
        "down" => "d",
        _ => return lower,
    };
    short.to_owned()
}

/// The grid step a compass direction stands for, if it is one.
pub fn direction_offset(direction: &str) -> Option<(i32, i32, i32)> {
    Some(match direction {
        "n" => (0, 1, 0),
        "s" => (0, -1, 0),
        "e" => (1, 0, 0),
        "w" => (-1, 0, 0),
        "ne" => (1, 1, 0),
        "nw" => (-1, 1, 0),
        "se" => (1, -1, 0),
        "sw" => (-1, -1, 0),
        "u" => (0, 0, 1),
        "d" => (0, 0, -1),
        _ => return None,
    })
}

#[derive(Default)]
pub struct Map {
    rooms: HashMap<RoomId, Room>,
    current: Option<RoomId>,
    /// `None` for a map that only lives in memory.
    store: Option<Store>,
}

impl Map {
    /// A map that is not saved anywhere.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Opens the map stored at `path`, creating the file if needed.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let (store, rooms) = Store::open(path)?;
        Ok(Self {
            rooms: rooms.into_iter().map(|room| (room.id, room)).collect(),
            current: None,
            store: Some(store),
        })
    }

    /// Records that the player is now in the room described by `info`,
    /// adding or updating it. The map is updated even if saving it fails.
    pub fn visit(&mut self, info: RoomInfo) -> Result<(), Error> {
        let pos = self.place(&info);
        let room = Room {
            id: info.id,
            name: info.name,
            area: info.area,
            pos,
            exits: info.exits,
        };
        self.current = Some(room.id);
        let unchanged = self.rooms.get(&room.id) == Some(&room);
        let saved = match &mut self.store {
            Some(store) if !unchanged => store.save(&room),
            _ => Ok(()),
        };
        self.rooms.insert(room.id, room);
        saved
    }

    /// Chooses where a room goes: where the server says, else where it
    /// already is, else next to a room it is known to connect to.
    fn place(&self, info: &RoomInfo) -> Pos {
        let known = || Some(self.rooms.get(&info.id)?.pos);
        let after_previous = || {
            let previous = self.current()?;
            let (direction, _) = previous.exits.iter().find(|(_, to)| **to == info.id)?;
            Some(previous.pos.offset(direction_offset(direction)?, 1))
        };
        let beside_neighbour = || {
            info.exits.iter().find_map(|(direction, to)| {
                let neighbour = self.rooms.get(to)?;
                Some(neighbour.pos.offset(direction_offset(direction)?, -1))
            })
        };
        info.pos
            .or_else(known)
            .or_else(after_previous)
            .or_else(beside_neighbour)
            .unwrap_or_default()
    }

    /// The room the player is in, once the server has reported one.
    pub fn current(&self) -> Option<&Room> {
        self.rooms.get(&self.current?)
    }

    pub fn room(&self, id: RoomId) -> Option<&Room> {
        self.rooms.get(&id)
    }

    pub fn rooms(&self) -> impl Iterator<Item = &Room> {
        self.rooms.values()
    }

    /// Rooms whose name contains `query`, ignoring case, ordered by name.
    pub fn find(&self, query: &str) -> Vec<&Room> {
        let query = query.to_lowercase();
        let mut found: Vec<&Room> = (self.rooms.values())
            .filter(|room| room.name.to_lowercase().contains(&query))
            .collect();
        found.sort_by(|a, b| (&a.name, a.id).cmp(&(&b.name, b.id)));
        found
    }

    /// The shortest known route between two rooms, counting each exit as
    /// one step. Empty if they are the same room; `None` if there is none.
    pub fn route(&self, from: RoomId, to: RoomId) -> Option<Vec<Step>> {
        // Breadth-first search, remembering how each room was reached.
        let mut reached_by: HashMap<RoomId, (RoomId, &str)> = HashMap::new();
        let mut queue = VecDeque::from([from]);
        while let Some(id) = queue.pop_front() {
            if id == to {
                break;
            }
            let Some(room) = self.rooms.get(&id) else {
                continue;
            };
            for (direction, &next) in &room.exits {
                if next != from && !reached_by.contains_key(&next) {
                    reached_by.insert(next, (id, direction));
                    queue.push_back(next);
                }
            }
        }
        let mut steps = Vec::new();
        let mut at = to;
        while at != from {
            let &(previous, direction) = reached_by.get(&at)?;
            steps.push(Step {
                direction: direction.to_owned(),
                to: at,
            });
            at = previous;
        }
        steps.reverse();
        Some(steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn info(id: RoomId, name: &str, exits: &[(&str, RoomId)]) -> RoomInfo {
        RoomInfo {
            id,
            name: name.into(),
            area: "town".into(),
            exits: (exits.iter())
                .map(|(direction, to)| ((*direction).to_owned(), *to))
                .collect(),
            pos: None,
        }
    }

    fn pos(x: i32, y: i32, z: i32) -> Pos {
        Pos { x, y, z }
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

    #[test]
    fn rooms_are_placed_by_the_direction_walked() {
        let map = town();
        let at = |id| map.room(id).unwrap().pos;
        assert_eq!(
            [at(1), at(2), at(3), at(4)],
            [pos(0, 0, 0), pos(0, 1, 0), pos(1, 1, 0), pos(1, 1, 1)]
        );
        assert_eq!(map.current().unwrap().name, "Tower Top");
    }

    #[test]
    fn room_reached_by_an_odd_exit_is_placed_beside_a_neighbour() {
        let mut map = Map::in_memory();
        map.visit(info(1, "Gate", &[("portal", 9)])).unwrap();
        map.visit(info(2, "Square", &[("s", 1)])).unwrap();
        assert_eq!(map.room(2).unwrap().pos, pos(0, 1, 0));
    }

    #[test]
    fn server_coordinates_win_and_revisits_keep_the_position() {
        let mut map = town();
        map.visit(info(2, "Square (night)", &[("s", 1)])).unwrap();
        assert_eq!(map.room(2).unwrap().pos, pos(0, 1, 0));
        assert_eq!(map.room(2).unwrap().name, "Square (night)");
        let mut moved = info(2, "Square", &[]);
        moved.pos = Some(pos(5, 5, 0));
        map.visit(moved).unwrap();
        assert_eq!(map.room(2).unwrap().pos, pos(5, 5, 0));
    }

    #[test]
    fn route_is_the_shortest_and_respects_one_way_exits() {
        let map = town();
        let directions = |from, to| -> Option<Vec<String>> {
            Some(
                map.route(from, to)?
                    .into_iter()
                    .map(|step| step.direction)
                    .collect(),
            )
        };
        assert_eq!(directions(1, 4).unwrap(), ["portal"]);
        assert_eq!(directions(4, 1).unwrap(), ["d", "w", "s"]);
        assert_eq!(directions(2, 2).unwrap(), Vec::<String>::new());
        assert_eq!(directions(1, 99), None);
        assert_eq!(
            map.route(4, 1).unwrap()[0],
            Step {
                direction: "d".into(),
                to: 3
            }
        );
    }

    #[test]
    fn find_ignores_case_and_sorts() {
        let map = town();
        let names: Vec<&str> = map
            .find("tower")
            .iter()
            .map(|room| room.name.as_str())
            .collect();
        assert_eq!(names, ["Tower", "Tower Top"]);
        assert!(map.find("dungeon").is_empty());
    }

    #[test]
    fn map_survives_reopening() {
        let path =
            std::env::temp_dir().join(format!("muddie-map-test-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let mut map = Map::open(&path).unwrap();
            for room in [
                info(1, "Gate", &[("n", 2)]),
                info(2, "Square", &[("s", 1), ("e", 3)]),
            ] {
                map.visit(room).unwrap();
            }
            // An update must replace the stored exits, not add to them.
            map.visit(info(2, "Square", &[("s", 1)])).unwrap();
        }
        let map = Map::open(&path).unwrap();
        assert!(map.current().is_none());
        assert_eq!(map.rooms().count(), 2);
        let square = map.room(2).unwrap();
        assert_eq!(
            (square.name.as_str(), square.area.as_str()),
            ("Square", "town")
        );
        assert_eq!(square.pos, pos(0, 1, 0));
        assert_eq!(square.exits, BTreeMap::from([("s".to_owned(), 1)]));
        std::fs::remove_file(&path).unwrap();
    }
}
