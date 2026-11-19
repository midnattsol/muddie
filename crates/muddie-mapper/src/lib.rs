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

/// How many cells a link may be stretched to find a free spot for a new
/// room whose natural place is taken.
const MAX_STRETCH: i32 = 6;

/// The id for a room the server identifies by text. Numeric text keeps its
/// value; anything else (some servers use hashes or names) gets a stable
/// number of its own: negative, so it cannot clash with a room number, and
/// within 52 bits, so scripts can hold it exactly in a Luau number.
pub fn room_id(identifier: &str) -> RoomId {
    let identifier = identifier.trim();
    if let Ok(number) = identifier.parse() {
        return number;
    }
    // FNV-1a: stable across runs and platforms, unlike the std hasher.
    let hash = (identifier.bytes()).fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    // Offset past -1, which servers use for "unknown room".
    -((hash >> 12) as RoomId) - 2
}

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

    /// Whether changes are being saved to a file.
    pub fn is_stored(&self) -> bool {
        self.store.is_some()
    }

    /// Stops saving: from now on the map lives only in memory. The file,
    /// if there was one, is left as it is.
    pub fn stop_storing(&mut self) {
        self.store = None;
    }

    /// Forgets every room, in the file too if the map is stored.
    pub fn clear(&mut self) -> Result<(), Error> {
        self.rooms.clear();
        self.current = None;
        match &mut self.store {
            Some(store) => store.clear(),
            None => Ok(()),
        }
    }

    /// Records that the player is now in the room described by `info`,
    /// adding or updating it. The map is updated even if saving it fails.
    pub fn visit(&mut self, info: RoomInfo) -> Result<(), Error> {
        let id = info.id;
        let saved = self.upsert(info, self.current);
        self.current = Some(id);
        saved
    }

    /// Adds or updates a room without moving the player there, for servers
    /// or scripts that describe rooms other than the current one.
    pub fn set_room(&mut self, info: RoomInfo) -> Result<(), Error> {
        self.upsert(info, None)
    }

    fn upsert(&mut self, info: RoomInfo, came_from: Option<RoomId>) -> Result<(), Error> {
        let pos = self.place(&info, came_from);
        let room = Room {
            id: info.id,
            name: info.name,
            area: info.area,
            pos,
            exits: info.exits,
        };
        if self.rooms.get(&room.id) == Some(&room) {
            return Ok(());
        }
        let saved = self.save(&room);
        self.rooms.insert(room.id, room);
        saved
    }

    fn save(&mut self, room: &Room) -> Result<(), Error> {
        match &mut self.store {
            Some(store) => store.save(room),
            None => Ok(()),
        }
    }

    /// Chooses where a room goes: where the server says, else where it
    /// already is, else next to a room it connects to, else clear of the
    /// rest of its area.
    fn place(&self, info: &RoomInfo, came_from: Option<RoomId>) -> Pos {
        if let Some(pos) = info.pos {
            return pos;
        }
        if let Some(room) = self.rooms.get(&info.id) {
            return room.pos;
        }
        // An anchor room, the grid step of the exit joining it to the new
        // room, and which way along that step the new room lies.
        let after_previous = || {
            let previous = self.rooms.get(&came_from?)?;
            let (direction, _) = previous.exits.iter().find(|(_, to)| **to == info.id)?;
            Some((previous.pos, direction_offset(direction)?, 1))
        };
        let beside_neighbour = || {
            info.exits.iter().find_map(|(direction, to)| {
                let neighbour = self.rooms.get(to)?;
                Some((neighbour.pos, direction_offset(direction)?, -1))
            })
        };
        let taken =
            |pos: Pos| (self.rooms.values()).any(|room| room.area == info.area && room.pos == pos);
        match after_previous().or_else(beside_neighbour) {
            // MUD geography is rarely a true grid: when the natural cell is
            // taken, lengthen the link rather than stack two rooms.
            Some((anchor, step, sign)) => (1..=MAX_STRETCH)
                .map(|distance| anchor.offset(step, sign * distance))
                .find(|pos| !taken(*pos))
                .unwrap_or(anchor.offset(step, sign)),
            // Nothing to hang it from (the first room, or after a teleport):
            // start a new island to the east of what is already drawn.
            None => {
                let east_edge = (self.rooms.values())
                    .filter(|room| room.area == info.area)
                    .map(|room| room.pos.x)
                    .max();
                Pos {
                    x: east_edge.map_or(0, |x| x + 2),
                    y: 0,
                    z: 0,
                }
            }
        }
    }

    /// Moves a room on the map grid. Returns false if there is no such room.
    pub fn move_room(&mut self, id: RoomId, pos: Pos) -> Result<bool, Error> {
        self.edit_room(id, |room| room.pos = pos)
    }

    /// Adds an exit, or changes where it leads. `direction` is also the
    /// command a walk sends to use it, so it can be any command.
    pub fn set_exit(&mut self, from: RoomId, direction: &str, to: RoomId) -> Result<bool, Error> {
        let direction = canonical_direction(direction);
        self.edit_room(from, |room| {
            room.exits.insert(direction, to);
        })
    }

    pub fn remove_exit(&mut self, from: RoomId, direction: &str) -> Result<bool, Error> {
        let direction = canonical_direction(direction);
        self.edit_room(from, |room| {
            room.exits.remove(&direction);
        })
    }

    fn edit_room(&mut self, id: RoomId, edit: impl FnOnce(&mut Room)) -> Result<bool, Error> {
        let Some(mut room) = self.rooms.get(&id).cloned() else {
            return Ok(false);
        };
        edit(&mut room);
        let saved = self.save(&room);
        self.rooms.insert(id, room);
        saved.map(|()| true)
    }

    /// Forgets a room. Exits of other rooms that led to it remain, as
    /// exits to an unexplored room. Returns false if there is no such room.
    pub fn remove_room(&mut self, id: RoomId) -> Result<bool, Error> {
        if self.rooms.remove(&id).is_none() {
            return Ok(false);
        }
        if self.current == Some(id) {
            self.current = None;
        }
        match &mut self.store {
            Some(store) => store.delete(id).map(|()| true),
            None => Ok(true),
        }
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
    fn taken_cell_stretches_the_link_instead_of_stacking() {
        let mut map = Map::in_memory();
        // Going n, e, s, w should close a square, but here it does not:
        // room 5 is a different room from the start.
        for room in [
            info(1, "A", &[("n", 2)]),
            info(2, "B", &[("e", 3)]),
            info(3, "C", &[("s", 4)]),
            info(4, "D", &[("w", 5)]),
            info(5, "E", &[]),
        ] {
            map.visit(room).unwrap();
        }
        assert_eq!(map.room(1).unwrap().pos, pos(0, 0, 0));
        assert_eq!(map.room(5).unwrap().pos, pos(-1, 0, 0));
    }

    #[test]
    fn unanchored_room_starts_an_island_clear_of_the_area() {
        let mut map = town();
        map.visit(info(50, "Teleport Pad", &[])).unwrap();
        assert_eq!(map.room(50).unwrap().pos, pos(3, 0, 0));
        let mut elsewhere = info(60, "Other Land", &[]);
        elsewhere.area = "island".into();
        map.visit(elsewhere).unwrap();
        assert_eq!(map.room(60).unwrap().pos, pos(0, 0, 0));
    }

    #[test]
    fn rooms_can_be_added_edited_and_removed_without_visiting() {
        let mut map = town();
        map.set_room(info(5, "Cellar", &[("u", 2)])).unwrap();
        assert_eq!(map.current().unwrap().id, 4);
        assert_eq!(map.room(5).unwrap().pos, pos(0, 1, -1));

        assert!(map.set_exit(2, "Down", 5).unwrap());
        assert!(!map.set_exit(99, "n", 1).unwrap());
        let directions: Vec<String> = (map.route(4, 5).unwrap().into_iter())
            .map(|step| step.direction)
            .collect();
        assert_eq!(directions, ["d", "w", "d"]);

        assert!(map.remove_exit(2, "d").unwrap());
        assert_eq!(map.route(4, 5), None);
        assert!(map.move_room(5, pos(9, 9, 9)).unwrap());
        assert_eq!(map.room(5).unwrap().pos, pos(9, 9, 9));

        assert!(map.remove_room(4).unwrap());
        assert!(!map.remove_room(4).unwrap());
        assert!(map.current().is_none());
        assert_eq!(map.room(3).unwrap().exits.get("u"), Some(&4));
    }

    #[test]
    fn text_identifiers_get_stable_ids() {
        assert_eq!(room_id(" 42 "), 42);
        assert_eq!(room_id("-1"), -1);
        let hashed = room_id("a7f3c9");
        assert!((-(1 << 52) - 2..-1).contains(&hashed));
        assert_eq!(hashed, room_id("a7f3c9"));
        assert_ne!(hashed, room_id("a7f3ca"));
        // Pinned: stored maps depend on this value never changing.
        assert_eq!(room_id("plaza"), -4_460_106_186_057_796);
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
            map.visit(info(3, "Doomed", &[("w", 2)])).unwrap();
            assert!(map.remove_room(3).unwrap());
            assert!(map.move_room(1, pos(4, 4, 0)).unwrap());
            assert!(map.is_stored());
            // What happens after storing stops must not reach the file.
            map.stop_storing();
            assert!(!map.is_stored());
            map.visit(info(7, "Unsaved", &[])).unwrap();
            map.clear().unwrap();
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
        assert_eq!(map.room(1).unwrap().pos, pos(4, 4, 0));

        let mut map = map;
        map.clear().unwrap();
        drop(map);
        assert_eq!(Map::open(&path).unwrap().rooms().count(), 0);
        std::fs::remove_file(&path).unwrap();
    }
}
