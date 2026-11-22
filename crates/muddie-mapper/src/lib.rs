//! The automapper's data: a graph of rooms built from what the server
//! reports, where to draw each room, routes between rooms, and storage.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque};
use std::path::Path;

mod editing;
mod ghosts;
mod info;
mod locating;
mod mudlet;
mod naming;
mod placing;
mod relayout;
mod routes;
mod store;
#[cfg(test)]
mod tests;
mod walk;

pub use info::RoomInfo;
pub use locating::{Located, Sighting};
pub use mudlet::{MUDLET_READER, MudletMap, read_mudlet_map};
pub use naming::{Found, FoundBy, Naming};
pub use rusqlite::Error;
pub use walk::{Progress, Walk};

use store::Store;

pub type RoomId = i64;

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
    /// The colour the room is drawn in, as red, green and blue; `None` for
    /// the map's own. Kept when the server reports the room without one.
    pub colour: Option<Colour>,
    /// What the user wrote of the room, to be told with it; empty for
    /// none. Kept, as what the user sets, when the server reports it.
    pub note: String,
}

/// A colour: red, green and blue.
pub type Colour = [u8; 3];

/// The sixteen standard colours, in the order of ANSI's palette, as Mudlet
/// draws rooms in them unless told otherwise.
pub const STANDARD_COLOURS: [Colour; 16] = [
    [0, 0, 0],
    [128, 0, 0],
    [0, 179, 0],
    [128, 128, 0],
    [0, 0, 128],
    [128, 0, 128],
    [0, 128, 128],
    [192, 192, 192],
    [128, 128, 128],
    [255, 0, 0],
    [0, 255, 0],
    [255, 255, 0],
    [0, 0, 255],
    [255, 0, 255],
    [0, 255, 255],
    [255, 255, 255],
];

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

/// The rooms by name as compared (see [`naming::name_key`]).
type Names = HashMap<String, Vec<RoomId>>;

/// How many moves away each room is, by the routes walks take.
pub type Distances = std::sync::Arc<HashMap<RoomId, usize>>;

/// A room's neighbour for layout: the neighbour, the grid step of the
/// compass exit joining them, and which way along it the neighbour lies.
type Link = (RoomId, (i32, i32, i32), i32);

#[derive(Default)]
pub struct Map {
    rooms: HashMap<RoomId, Room>,
    current: Option<RoomId>,
    /// `None` for a map that only lives in memory.
    store: Option<Store>,
    /// Rooms changed since the map was last written, to be written all in
    /// one go by [`Map::flush`] rather than each as it changes.
    unsaved: std::collections::BTreeSet<RoomId>,
    /// Counts the changes to its rooms, for what is worked out from them
    /// to be worked out again only when they change.
    revision: u64,
    /// Rooms named by the user, by the name as given.
    marks: BTreeMap<String, RoomId>,
    /// What is known about the map, by key: where it was taken from.
    about: BTreeMap<String, String>,
    /// The words of the server's language for directions, each with the
    /// direction it is, and the word a walk sends for each direction.
    words: HashMap<String, String>,
    sent: HashMap<String, String>,
    /// The rooms by name as compared, made again when the rooms change.
    names: std::cell::RefCell<Option<(u64, Names)>>,
    /// How the MUD names its rooms, as a script taught the map.
    naming: Naming,
    /// Whether the rooms are in more than one area, as of a revision.
    several_areas: std::cell::Cell<Option<(u64, bool)>>,
    /// The rooms left over, as of a revision.
    ghosts: std::cell::RefCell<Option<(u64, ghosts::Ghosts)>>,
    /// How many moves away each room is from one, as of a revision.
    distances: std::cell::RefCell<Option<(RoomId, u64, Distances)>>,
    /// The rooms the player may be in when the map cannot tell which.
    candidates: Vec<RoomId>,
    /// The moves sent that the server has not shown the end of yet, in
    /// order, each with when it was sent.
    moves: VecDeque<(String, std::time::Instant)>,
}

impl Drop for Map {
    fn drop(&mut self) {
        let _ = self.flush();
        self.keep_whereabouts();
    }
}

impl Map {
    /// A map that is not saved anywhere.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Opens the map stored at `path`, creating the file if needed.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let (store, stored) = Store::open(path)?;
        let mut map = Self::default();
        map.rooms = (stored.rooms.into_iter())
            .map(|room| (room.id, room))
            .collect();
        map.store = Some(store);
        map.marks = stored.marks;
        map.about = stored.about;
        map.current = map.last_room();
        Ok(map)
    }

    /// Whether changes are being saved to a file.
    pub fn is_stored(&self) -> bool {
        self.store.is_some()
    }

    /// Stops saving: from now on the map lives only in memory. The file,
    /// if there was one, is left as it is.
    pub fn stop_storing(&mut self) {
        // What changed until now is the file's; what changes after, not.
        let _ = self.flush();
        self.store = None;
    }

    /// Writes the rooms changed since it was last done, in one go.
    pub fn flush(&mut self) -> Result<(), Error> {
        if self.unsaved.is_empty() {
            return Ok(());
        }
        let ids = std::mem::take(&mut self.unsaved);
        match &mut self.store {
            Some(store) => store.save(ids.iter().filter_map(|id| self.rooms.get(id))),
            None => Ok(()),
        }
    }

    /// Notes that rooms changed: they are written with the next
    /// [`Map::flush`].
    pub(crate) fn keep(&mut self, ids: impl IntoIterator<Item = RoomId>) {
        // None changed, nothing to work out again.
        let mut ids = ids.into_iter().peekable();
        if ids.peek().is_none() {
            return;
        }
        self.revision += 1;
        if self.store.is_some() {
            self.unsaved.extend(ids);
        }
    }

    /// Which change of its rooms the map is at: the same number, the same
    /// rooms.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Forgets every room, in the file too if the map is stored.
    pub fn clear(&mut self) -> Result<(), Error> {
        self.rooms.clear();
        self.current = None;
        self.candidates.clear();
        self.unsaved.clear();
        self.revision += 1;
        match &mut self.store {
            Some(store) => store.clear(),
            None => Ok(()),
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
        let query = self.name_key(query);
        let mut found: Vec<&Room> = (self.with_names(|names| {
            (names.iter())
                .filter(|(name, _)| name.contains(&query))
                .flat_map(|(_, ids)| ids.clone())
                .collect::<Vec<_>>()
        }))
        .into_iter()
        .filter_map(|id| self.rooms.get(&id))
        .collect();
        found.sort_by(|a, b| (&a.name, a.id).cmp(&(&b.name, b.id)));
        found
    }
}
