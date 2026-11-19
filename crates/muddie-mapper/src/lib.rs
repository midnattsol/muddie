//! The automapper's data: a graph of rooms built from what the server
//! reports, where to draw each room, routes between rooms, and storage.

mod info;
mod store;
mod walk;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque};
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
    /// Whether `other` lies one or more whole `step`s away from here.
    fn in_line_with(self, other: Pos, step: (i32, i32, i32)) -> bool {
        let gaps = [other.x - self.x, other.y - self.y, other.z - self.z];
        let steps = [step.0, step.1, step.2];
        let mut distance = None;
        for (gap, step) in gaps.into_iter().zip(steps) {
            if step == 0 {
                if gap != 0 {
                    return false;
                }
            } else if gap % step != 0 || *distance.get_or_insert(gap / step) != gap / step {
                return false;
            }
        }
        distance.is_some_and(|distance| distance >= 1)
    }

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
    /// Exits by direction (see [`canonical_direction`]).
    pub exits: BTreeMap<String, Exit>,
    /// Cost of entering the room when finding routes; 1 for a normal room.
    /// Set by the user, so what the server reports never changes it.
    pub weight: u32,
    /// Routes never pass through the room, though they may end in it.
    /// Also set by the user.
    pub avoid: bool,
}

/// A way out of a room. Everything but `to` is set by the user, and kept
/// when the server reports the room again.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Exit {
    pub to: RoomId,
    /// Extra cost of using the exit when finding routes; 0 for a normal one.
    pub cost: u32,
    /// Routes never use the exit, as for a door that cannot be opened.
    pub locked: bool,
    /// What a walk sends to use the exit when the direction alone is not
    /// enough; several commands are separated by `;` ("open door;n").
    pub command: Option<String>,
    /// Added by the user rather than reported by the server, so it stays
    /// when the server lists the room's exits without it.
    pub custom: bool,
}

/// One move of a route: what to send and the room it leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// One or more commands, separated by `;`.
    pub command: String,
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

/// A room's neighbour for layout: the neighbour, the grid step of the
/// compass exit joining them, and which way along it the neighbour lies.
type Link = (RoomId, (i32, i32, i32), i32);

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
        let came_from = self.current;
        // Without coordinates from the server, the layout is ours to fix.
        let free_layout = info.pos.is_none();
        let saved = self.upsert(info, came_from);
        self.current = Some(id);
        let joined = if free_layout {
            self.join_islands(came_from, id)
        } else {
            Ok(())
        };
        saved.and(joined)
    }

    /// Lines up separately drawn parts of the map that the room just
    /// entered shows to be connected: the part walked into, and any part an
    /// exit of this room leads to. A part is only moved when it has no other
    /// link to the rest, so consistent geometry is never disturbed.
    fn join_islands(&mut self, came_from: Option<RoomId>, here: RoomId) -> Result<(), Error> {
        let mut links = Vec::new();
        if let Some(previous) = came_from.and_then(|id| self.rooms.get(&id)) {
            let exit = previous.exits.iter().find(|(_, exit)| exit.to == here);
            links.extend(exit.map(|(direction, _)| (previous.id, direction.clone(), here)));
        }
        if let Some(room) = self.rooms.get(&here) {
            let exits = room.exits.iter();
            links.extend(exits.map(|(direction, exit)| (here, direction.clone(), exit.to)));
        }
        let mut moved = Vec::new();
        for (anchor, direction, other) in links {
            moved.extend(self.align(anchor, &direction, other));
        }
        let rooms = moved.iter().filter_map(|id| self.rooms.get(id));
        match &mut self.store {
            Some(store) => store.save(rooms),
            None => Ok(()),
        }
    }

    /// If `other` sits on a separate island from `anchor`, moves that island
    /// so `other` is `direction` of `anchor`. Returns the rooms it moved.
    fn align(&mut self, anchor: RoomId, direction: &str, other: RoomId) -> Vec<RoomId> {
        let (Some(from), Some(to)) = (self.rooms.get(&anchor), self.rooms.get(&other)) else {
            return Vec::new();
        };
        let Some(step) = direction_offset(direction) else {
            return Vec::new();
        };
        if anchor == other || from.area != to.area || from.pos.in_line_with(to.pos, step) {
            return Vec::new();
        }
        let Some(island) = self.island(other, anchor) else {
            return Vec::new();
        };
        let (start, origin) = (from.pos, to.pos);
        let elsewhere: HashSet<Pos> = (self.rooms.values())
            .filter(|room| room.area == to.area && !island.contains(&room.id))
            .map(|room| room.pos)
            .collect();
        let shifted = |pos: Pos, target: Pos| Pos {
            x: pos.x + target.x - origin.x,
            y: pos.y + target.y - origin.y,
            z: pos.z + target.z - origin.z,
        };
        // As when placing one room: lengthen the link until nothing overlaps.
        let fits = |target: &Pos| {
            (island.iter()).all(|id| !elsewhere.contains(&shifted(self.rooms[id].pos, *target)))
        };
        let target = (1..=MAX_STRETCH)
            .map(|distance| start.offset(step, distance))
            .find(fits)
            .unwrap_or(start.offset(step, 1));
        for id in &island {
            let room = self.rooms.get_mut(id).expect("island rooms exist");
            room.pos = shifted(room.pos, target);
        }
        island.into_iter().collect()
    }

    /// The rooms of `start`'s area joined to it by exits in either
    /// direction, ignoring any direct link to `apart`. `None` if `apart` is
    /// among them, meaning the two are not on separate islands.
    fn island(&self, start: RoomId, apart: RoomId) -> Option<HashSet<RoomId>> {
        let area = &self.rooms.get(&start)?.area;
        let mut neighbours: HashMap<RoomId, Vec<RoomId>> = HashMap::new();
        for room in self.rooms.values().filter(|room| &room.area == area) {
            for to in room.exits.values().map(|exit| exit.to) {
                let direct = (room.id, to) == (start, apart) || (room.id, to) == (apart, start);
                if !direct && self.rooms.get(&to).is_some_and(|other| &other.area == area) {
                    neighbours.entry(room.id).or_default().push(to);
                    neighbours.entry(to).or_default().push(room.id);
                }
            }
        }
        let mut island = HashSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(id) = queue.pop_front() {
            for next in neighbours.get(&id).into_iter().flatten() {
                if *next == apart {
                    return None;
                }
                if island.insert(*next) {
                    queue.push_back(*next);
                }
            }
        }
        Some(island)
    }

    /// Adds or updates a room without moving the player there, for servers
    /// or scripts that describe rooms other than the current one.
    pub fn set_room(&mut self, info: RoomInfo) -> Result<(), Error> {
        self.upsert(info, None)
    }

    fn upsert(&mut self, info: RoomInfo, came_from: Option<RoomId>) -> Result<(), Error> {
        let pos = self.place(&info, came_from);
        let existing = self.rooms.get(&info.id);
        let old_exits = existing.map(|room| &room.exits);
        // The server decides where its exits lead; what the user added to
        // them, and the exits the user added, carry over.
        let mut exits: BTreeMap<String, Exit> = (info.exits.into_iter())
            .map(|(direction, to)| {
                let old = old_exits.and_then(|exits| exits.get(&direction));
                let exit = Exit {
                    to,
                    custom: false,
                    ..old.cloned().unwrap_or_default()
                };
                (direction, exit)
            })
            .collect();
        for (direction, exit) in old_exits.into_iter().flatten() {
            if exit.custom && !exits.contains_key(direction) {
                exits.insert(direction.clone(), exit.clone());
            }
        }
        let room = Room {
            id: info.id,
            name: info.name,
            area: info.area,
            pos,
            exits,
            weight: existing.map_or(1, |room| room.weight),
            avoid: existing.is_some_and(|room| room.avoid),
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
            Some(store) => store.save([room]),
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
            let (direction, _) = (previous.exits.iter()).find(|(_, exit)| exit.to == info.id)?;
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

    /// Sets the cost of entering a room when finding routes (at least 1).
    pub fn set_weight(&mut self, id: RoomId, weight: u32) -> Result<bool, Error> {
        self.edit_room(id, |room| room.weight = weight.max(1))
    }

    /// Sets whether routes must keep out of a room.
    pub fn set_avoid(&mut self, id: RoomId, avoid: bool) -> Result<bool, Error> {
        self.edit_room(id, |room| room.avoid = avoid)
    }

    /// Adds an exit of the user's own, or changes where an exit leads.
    /// `direction` is what a walk sends to use it unless the exit is given
    /// a command, so it need not be a compass direction.
    pub fn set_exit(&mut self, from: RoomId, direction: &str, to: RoomId) -> Result<bool, Error> {
        let direction = canonical_direction(direction);
        self.edit_room(from, |room| {
            let exit = room.exits.entry(direction).or_insert(Exit {
                custom: true,
                ..Exit::default()
            });
            exit.to = to;
        })
    }

    /// Changes an exit's cost, lock or command. Returns false if there is
    /// no such room or exit.
    pub fn edit_exit(
        &mut self,
        from: RoomId,
        direction: &str,
        edit: impl FnOnce(&mut Exit),
    ) -> Result<bool, Error> {
        let direction = canonical_direction(direction);
        let has_exit =
            (self.rooms.get(&from)).is_some_and(|room| room.exits.contains_key(&direction));
        if !has_exit {
            return Ok(false);
        }
        self.edit_room(from, |room| {
            room.exits.entry(direction).and_modify(edit);
        })
    }

    pub fn remove_exit(&mut self, from: RoomId, direction: &str) -> Result<bool, Error> {
        let direction = canonical_direction(direction);
        self.edit_room(from, |room| {
            room.exits.remove(&direction);
        })
    }

    /// Draws an area again from scratch: starting from the player's room
    /// if it is there, each room is placed by the compass direction of the
    /// exits that reach it. For an area whose drawing has become tangled.
    /// Returns how many rooms moved.
    pub fn relayout(&mut self, area: &str) -> Result<usize, Error> {
        let mut members: Vec<RoomId> = (self.rooms.values())
            .filter(|room| room.area == area)
            .map(|room| room.id)
            .collect();
        members.sort_unstable();
        // Every compass exit links two rooms both ways: a neighbour, the
        // grid step, and which way along it the neighbour lies.
        let mut links: HashMap<RoomId, Vec<Link>> = HashMap::new();
        for id in &members {
            for (direction, exit) in &self.rooms[id].exits {
                let inside = (self.rooms.get(&exit.to)).is_some_and(|room| room.area == area);
                if let (Some(step), true) = (direction_offset(direction), inside) {
                    links.entry(*id).or_default().push((exit.to, step, 1));
                    links.entry(exit.to).or_default().push((*id, step, -1));
                }
            }
        }
        let player = self.current.filter(|id| members.contains(id));
        let mut placed: HashMap<RoomId, Pos> = HashMap::new();
        let mut taken: HashSet<Pos> = HashSet::new();
        for seed in player.into_iter().chain(members.iter().copied()) {
            if placed.contains_key(&seed) {
                continue;
            }
            // The first room stays where it is; a part no compass exit
            // reaches starts its own island east of everything placed.
            let old = self.rooms[&seed].pos;
            let east_edge = placed.values().map(|pos| pos.x).max();
            let start = match east_edge {
                Some(x) => Pos {
                    x: x + 2,
                    y: 0,
                    z: old.z,
                },
                None => old,
            };
            placed.insert(seed, start);
            taken.insert(start);
            let mut queue = VecDeque::from([seed]);
            while let Some(id) = queue.pop_front() {
                for &(next, step, sign) in links.get(&id).into_iter().flatten() {
                    if placed.contains_key(&next) {
                        continue;
                    }
                    let from = placed[&id];
                    let pos = (1..=MAX_STRETCH)
                        .map(|distance| from.offset(step, sign * distance))
                        .find(|pos| !taken.contains(pos))
                        .unwrap_or(from.offset(step, sign));
                    placed.insert(next, pos);
                    taken.insert(pos);
                    queue.push_back(next);
                }
            }
        }
        let mut moved = Vec::new();
        for (id, pos) in placed {
            let room = self.rooms.get_mut(&id).expect("members exist");
            if room.pos != pos {
                room.pos = pos;
                moved.push(id);
            }
        }
        let rooms = moved.iter().filter_map(|id| self.rooms.get(id));
        if let Some(store) = &mut self.store {
            store.save(rooms)?;
        }
        Ok(moved.len())
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

    /// The names of all areas, in alphabetical order.
    pub fn areas(&self) -> Vec<&str> {
        let areas: BTreeSet<&str> = self.rooms.values().map(|room| room.area.as_str()).collect();
        areas.into_iter().collect()
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

    /// The cheapest known route between two rooms: each move costs the
    /// weight of the room entered plus the exit's own cost, locked exits
    /// are never used, and rooms marked to avoid are only entered as the
    /// destination. Empty if both are the same room; `None`
    /// if there is no route.
    pub fn route(&self, from: RoomId, to: RoomId) -> Option<Vec<Step>> {
        // Dijkstra's algorithm, remembering how each room was best reached.
        let mut cost_to: HashMap<RoomId, u64> = HashMap::from([(from, 0)]);
        let mut reached_by: HashMap<RoomId, (RoomId, &str)> = HashMap::new();
        let mut queue = BinaryHeap::from([Reverse((0_u64, from))]);
        while let Some(Reverse((cost, id))) = queue.pop() {
            if id == to {
                break;
            }
            if cost > cost_to[&id] {
                continue;
            }
            let Some(room) = self.rooms.get(&id) else {
                continue;
            };
            for (direction, exit) in &room.exits {
                if exit.locked {
                    continue;
                }
                let next = exit.to;
                // An unexplored room can be walked into, but not through.
                let (weight, avoid) = match self.rooms.get(&next) {
                    Some(room) => (room.weight, room.avoid),
                    None => (1, false),
                };
                if avoid && next != to {
                    continue;
                }
                let next_cost = cost + u64::from(weight) + u64::from(exit.cost);
                if cost_to.get(&next).is_none_or(|known| next_cost < *known) {
                    cost_to.insert(next, next_cost);
                    let command = exit.command.as_deref().unwrap_or(direction);
                    reached_by.insert(next, (id, command));
                    queue.push(Reverse((next_cost, next)));
                }
            }
        }
        let mut steps = Vec::new();
        let mut at = to;
        while at != from {
            let &(previous, command) = reached_by.get(&at)?;
            steps.push(Step {
                command: command.to_owned(),
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
            .map(|step| step.command)
            .collect();
        assert_eq!(directions, ["d", "w", "d"]);

        assert!(map.remove_exit(2, "d").unwrap());
        assert_eq!(map.route(4, 5), None);
        assert!(map.move_room(5, pos(9, 9, 9)).unwrap());
        assert_eq!(map.room(5).unwrap().pos, pos(9, 9, 9));

        assert!(map.remove_room(4).unwrap());
        assert!(!map.remove_room(4).unwrap());
        assert!(map.current().is_none());
        assert_eq!(map.room(3).unwrap().exits["u"].to, 4);
    }

    #[test]
    fn route_prefers_cheap_rooms_and_keeps_out_of_avoided_ones() {
        // Two ways from 1 to 4: short through 2, long through 5 and 6.
        let mut map = Map::in_memory();
        for room in [
            info(1, "Start", &[("n", 2), ("e", 5)]),
            info(2, "Swamp", &[("n", 4)]),
            info(5, "Road", &[("n", 6)]),
            info(6, "Bridge", &[("w", 4)]),
            info(4, "End", &[]),
        ] {
            map.set_room(room).unwrap();
        }
        let via = |map: &Map, to| -> Option<Vec<RoomId>> {
            Some(map.route(1, to)?.into_iter().map(|step| step.to).collect())
        };
        assert_eq!(via(&map, 4).unwrap(), [2, 4]);
        assert!(map.set_weight(2, 5).unwrap());
        assert_eq!(via(&map, 4).unwrap(), [5, 6, 4]);
        assert!(map.set_weight(2, 0).unwrap());
        assert_eq!(map.room(2).unwrap().weight, 1);
        assert_eq!(via(&map, 4).unwrap(), [2, 4]);

        assert!(map.set_avoid(2, true).unwrap());
        assert_eq!(via(&map, 4).unwrap(), [5, 6, 4]);
        assert_eq!(
            via(&map, 2).unwrap(),
            [2],
            "an avoided room can still be the destination"
        );
        map.set_avoid(6, true).unwrap();
        assert_eq!(via(&map, 4), None);

        // What the server reports later does not undo the user's marks.
        map.visit(info(2, "Swamp (dry)", &[("n", 4)])).unwrap();
        assert!(map.room(2).unwrap().avoid);
    }

    #[test]
    fn island_is_lined_up_when_its_connection_is_found() {
        let mut map = Map::in_memory();
        map.visit(info(1, "Gate", &[("n", 2)])).unwrap();
        // Teleported: rooms 10 and 11 are drawn apart from the gate.
        map.visit(info(10, "Dock", &[("e", 11)])).unwrap();
        map.visit(info(11, "Pier", &[("w", 10)])).unwrap();
        assert_eq!(map.room(10).unwrap().pos, pos(2, 0, 0));
        // Back at the gate, then north: room 2 turns out to lead east to
        // the dock, so the dock's island belongs beside it.
        map.visit(info(1, "Gate", &[("n", 2)])).unwrap();
        map.visit(info(2, "Square", &[("s", 1), ("e", 10)]))
            .unwrap();
        assert_eq!(map.room(2).unwrap().pos, pos(0, 1, 0));
        assert_eq!(map.room(10).unwrap().pos, pos(1, 1, 0));
        assert_eq!(map.room(11).unwrap().pos, pos(2, 1, 0));
        assert_eq!(map.room(1).unwrap().pos, pos(0, 0, 0));
    }

    #[test]
    fn walking_into_an_island_moves_it_but_a_loop_is_left_alone() {
        let mut map = Map::in_memory();
        map.visit(info(1, "Gate", &[("n", 2)])).unwrap();
        map.visit(info(20, "Cave", &[("s", 21)])).unwrap();
        map.visit(info(21, "Pit", &[("n", 20)])).unwrap();
        // The gate gains an exit west into the cave: walk through it.
        map.visit(info(1, "Gate", &[("n", 2), ("w", 20)])).unwrap();
        map.visit(info(20, "Cave", &[("s", 21), ("e", 1)])).unwrap();
        assert_eq!(map.room(20).unwrap().pos, pos(-1, 0, 0));
        assert_eq!(map.room(21).unwrap().pos, pos(-1, -1, 0));

        // A second link between rooms already joined does not move them,
        // even though it does not fit the grid.
        map.visit(info(21, "Pit", &[("n", 20), ("e", 1)])).unwrap();
        map.visit(info(1, "Gate", &[("n", 2), ("w", 20)])).unwrap();
        assert_eq!(map.room(21).unwrap().pos, pos(-1, -1, 0));
        assert_eq!(map.room(1).unwrap().pos, pos(0, 0, 0));
    }

    #[test]
    fn exit_cost_lock_and_command_shape_the_route() {
        let mut map = Map::in_memory();
        for room in [
            info(1, "Hall", &[("n", 2), ("e", 3)]),
            info(2, "Vault", &[]),
            info(3, "Passage", &[("n", 4)]),
            info(4, "Loft", &[("w", 2)]),
        ] {
            map.set_room(room).unwrap();
        }
        let commands = |map: &Map| -> Option<Vec<String>> {
            Some(
                map.route(1, 2)?
                    .into_iter()
                    .map(|step| step.command)
                    .collect(),
            )
        };
        assert_eq!(commands(&map).unwrap(), ["n"]);
        // A door: the walk must open it first.
        assert!(
            map.edit_exit(1, "North", |exit| exit.command = Some("open door;n".into()))
                .unwrap()
        );
        assert_eq!(commands(&map).unwrap(), ["open door;n"]);
        // Costly enough, the long way round wins.
        map.edit_exit(1, "n", |exit| exit.cost = 5).unwrap();
        assert_eq!(commands(&map).unwrap(), ["e", "n", "w"]);
        map.edit_exit(1, "n", |exit| exit.cost = 0).unwrap();
        map.edit_exit(1, "n", |exit| exit.locked = true).unwrap();
        assert_eq!(commands(&map).unwrap(), ["e", "n", "w"]);
        map.edit_exit(3, "n", |exit| exit.locked = true).unwrap();
        assert_eq!(commands(&map), None);
        assert!(!map.edit_exit(1, "up", |exit| exit.locked = true).unwrap());
        assert!(!map.edit_exit(99, "n", |exit| exit.locked = true).unwrap());
    }

    #[test]
    fn server_reports_keep_what_the_user_set_on_exits() {
        let mut map = Map::in_memory();
        map.visit(info(1, "Hall", &[("n", 2), ("e", 3)])).unwrap();
        map.edit_exit(1, "n", |exit| exit.locked = true).unwrap();
        map.set_exit(1, "enter portal", 9).unwrap();
        // The server now says north leads elsewhere and east is gone.
        map.visit(info(1, "Hall", &[("n", 5)])).unwrap();
        let hall = map.room(1).unwrap();
        assert_eq!((hall.exits["n"].to, hall.exits["n"].locked), (5, true));
        assert!(!hall.exits.contains_key("e"));
        assert_eq!(
            (
                hall.exits["enter portal"].to,
                hall.exits["enter portal"].custom
            ),
            (9, true)
        );
        // Once the server lists the user's exit itself, it is no longer
        // custom, so it goes away when the server later drops it.
        map.visit(info(1, "Hall", &[("enter portal", 9)])).unwrap();
        map.visit(info(1, "Hall", &[])).unwrap();
        assert!(map.room(1).unwrap().exits.is_empty());
    }

    #[test]
    fn relayout_redraws_a_tangled_area_around_the_player() {
        let mut map = town();
        let mut dock = info(30, "Dock", &[("e", 31)]);
        dock.area = "town".into();
        map.set_room(dock).unwrap();
        map.set_room(info(31, "Pier", &[])).unwrap();
        for (id, at) in [
            (1, pos(7, -3, 2)),
            (2, pos(7, -3, 2)),
            (3, pos(0, 9, 0)),
            (31, pos(-4, 0, 0)),
        ] {
            map.move_room(id, at).unwrap();
        }
        // The player is on the tower top, which keeps its place.
        assert_eq!(map.relayout("town").unwrap(), 4);
        let at = |id| map.room(id).unwrap().pos;
        assert_eq!(
            [at(4), at(3), at(2), at(1)],
            [pos(1, 1, 1), pos(1, 1, 0), pos(0, 1, 0), pos(0, 0, 0)]
        );
        // The dock and pier connect to nothing else: a separate island.
        assert_eq!([at(30), at(31)], [pos(3, 0, 0), pos(4, 0, 0)]);
        assert_eq!(map.relayout("town").unwrap(), 0);
        assert_eq!(map.relayout("nowhere").unwrap(), 0);
    }

    #[test]
    fn areas_are_listed_once_in_order() {
        let mut map = town();
        let mut other = info(9, "Raft", &[]);
        other.area = "sea".into();
        map.set_room(other).unwrap();
        assert_eq!(map.areas(), ["sea", "town"]);
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
                    .map(|step| step.command)
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
                command: "d".into(),
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
            assert!(map.set_weight(1, 3).unwrap());
            assert!(map.set_avoid(2, true).unwrap());
            let door = |exit: &mut Exit| {
                *exit = Exit {
                    cost: 4,
                    locked: true,
                    command: Some("open door;n".into()),
                    ..exit.clone()
                }
            };
            assert!(map.edit_exit(1, "n", door).unwrap());
            assert!(map.set_exit(1, "leap", 2).unwrap());
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
        let exits: Vec<(&str, RoomId)> = (square.exits.iter())
            .map(|(direction, exit)| (direction.as_str(), exit.to))
            .collect();
        assert_eq!(exits, [("s", 1)]);
        let gate = map.room(1).unwrap();
        let expected = Exit {
            to: 2,
            cost: 4,
            locked: true,
            command: Some("open door;n".into()),
            custom: false,
        };
        assert_eq!(gate.exits["n"], expected);
        assert!(gate.exits["leap"].custom);
        assert_eq!(map.room(1).unwrap().pos, pos(4, 4, 0));
        assert_eq!(
            (map.room(1).unwrap().weight, map.room(2).unwrap().avoid),
            (3, true)
        );

        let mut map = map;
        map.clear().unwrap();
        drop(map);
        assert_eq!(Map::open(&path).unwrap().rooms().count(), 0);
        std::fs::remove_file(&path).unwrap();
    }
}
