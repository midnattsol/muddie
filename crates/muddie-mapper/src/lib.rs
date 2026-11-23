//! The automapper's data: a graph of rooms built from what the server
//! reports, where to draw each room, routes between rooms, and storage.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque};
use std::path::Path;

mod adding;
mod directions;
mod editing;
mod ghosts;
mod healing;
mod info;
mod journal;
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

pub use adding::{Added, Arrival, Refused};
pub use directions::{
    DIRECTIONS, Direction, canonical_direction, direction_offset, direction_rank, known_direction,
};
pub use editing::Replaced;
pub use healing::UNSHOWN_AFTER;
pub use info::RoomInfo;
pub use journal::Changed;
pub use locating::{Located, Sighting};
pub use mudlet::{MUDLET_READER, MudletMap, read_mudlet_map};
pub use naming::{Found, FoundBy, Naming};
pub use relayout::{Drawing, Tidied};
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
        self.steps_to(other, step).is_some()
    }

    /// How many whole `step`s away from here `other` lies, if one or more.
    fn steps_to(self, other: Pos, step: (i32, i32, i32)) -> Option<i32> {
        let gaps = [other.x - self.x, other.y - self.y, other.z - self.z];
        let steps = [step.0, step.1, step.2];
        let mut distance = None;
        for (gap, step) in gaps.into_iter().zip(steps) {
            if step == 0 {
                if gap != 0 {
                    return None;
                }
            } else if gap % step != 0 || *distance.get_or_insert(gap / step) != gap / step {
                return None;
            }
        }
        distance.filter(|distance| *distance >= 1)
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
    /// The name is one the user gave it, or a script on their behalf, in
    /// place of the one it had: kept when the server reports the room, or
    /// a map taken again names it otherwise.
    pub renamed: bool,
    /// Made by the map, from what the player saw on going where it had no
    /// room, rather than reported by the server or taken with a map from
    /// elsewhere: numbered from [`ADDED_FROM`], kept when a map is taken
    /// again, and joined with the room it has if it has it by then.
    pub added: bool,
    /// The shape it is drawn in, if not the map's (see [`Appearance`]),
    /// and a character or two drawn in it, if any.
    pub shape: Option<Shape>,
    pub symbol: String,
    /// Its colour, shape and symbol are the user's, or a script's on their
    /// behalf: kept when the server reports the room or a map is taken
    /// again.
    pub styled: bool,
}

/// The shape a room is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    #[default]
    Square,
    Circle,
    Diamond,
    Hexagon,
}

impl Shape {
    pub const ALL: [Shape; 4] = [Shape::Square, Shape::Circle, Shape::Diamond, Shape::Hexagon];

    /// Its name, as kept and as scripts say it.
    pub fn key(self) -> &'static str {
        match self {
            Shape::Square => "square",
            Shape::Circle => "circle",
            Shape::Diamond => "diamond",
            Shape::Hexagon => "hexagon",
        }
    }

    pub fn from_key(key: &str) -> Option<Shape> {
        Shape::ALL.into_iter().find(|shape| shape.key() == key)
    }
}

/// How the map draws a room with no colour or shape of its own: the
/// user's choice for the whole map, kept with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Appearance {
    /// `None` for the colour of the interface's theme.
    pub colour: Option<Colour>,
    pub shape: Shape,
}

/// A door on an exit, as a map taken from elsewhere marks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    Open,
    Closed,
    Locked,
}

/// The first number of the rooms the map adds of itself: far above any
/// number a server or a map from elsewhere gives a room, and within 52
/// bits, so scripts hold it exactly.
pub const ADDED_FROM: RoomId = 1 << 40;

/// The room an exit leads to while it is not known which: no room has it.
pub const UNEXPLORED: RoomId = -1;

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
    /// The door on it, if it has one.
    pub door: Option<Door>,
}

/// One move of a route: what to send and the room it leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// One or more commands, separated by `;`.
    pub command: String,
    pub to: RoomId,
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
    /// How scripts set the map up for the MUD.
    settings: Settings,
    /// The rooms by name as compared, made again when the rooms change.
    names: std::cell::RefCell<Option<(u64, Names)>>,
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
    /// What the map changed of itself, with the rooms as they were before
    /// each, the last last: to be undone.
    changes: Vec<journal::Entry>,
    /// What it changed of itself that has not been told yet.
    told: Vec<Changed>,
    /// The rooms the player was followed through, one move after another,
    /// by the way they went and the exits, though the server named them
    /// otherwise, each with that name: put right once a move is to a room
    /// as the map has it. The last is where the player is.
    pending: Vec<(RoomId, String)>,
    /// Whether the map knows for certain the player is where it has them:
    /// the room as last shown, by its own name and with all its exits.
    sure: bool,
    /// An exit that led to a room beside, to be put right if borne out.
    reroute: Option<healing::Reroute>,
    /// For each exit of a room the player came to without the server
    /// showing it, how many times in a row.
    unshown: HashMap<(RoomId, String), u32>,
}

/// How scripts set a map up for the MUD, all of which a map opened in
/// place of another for the session takes from it.
#[derive(Default, Clone)]
pub(crate) struct Settings {
    /// The words of the MUD's language for directions, and those taught
    /// over them.
    language: directions::Words,
    taught: directions::Words,
    /// How the MUD names its rooms.
    naming: Naming,
    /// Whether the map learns as the player goes: what it has wrong put
    /// right, and the rooms it has not added.
    learning: bool,
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
        self.changes.clear();
        self.forget_healing();
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
