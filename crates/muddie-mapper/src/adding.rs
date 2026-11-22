//! Rooms the map adds of itself, where the player went and it had none:
//! looked for among the rooms it has before one is made, so that no room
//! is had twice, refused while the map does not know where the player
//! was, and undone if wrong (see the journal). Taking a map again keeps them, or joins each
//! with the room the map taken has, once it has it.

use super::*;

/// What the player saw on going where the map has no room.
#[derive(Debug, Clone, Copy)]
pub struct Arrival<'a> {
    /// The room's name, as the server shows it.
    pub name: &'a str,
    /// Its exits, in any of the words for them.
    pub exits: &'a [String],
    /// The exit taken from the room the map has the player in: a
    /// direction, in any of its words, or a command.
    pub came: &'a str,
}

/// The room the player went to, which the exit taken now leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Added {
    /// One the map had, there and alike.
    Found(RoomId),
    /// One made.
    Made(RoomId),
}

impl Added {
    pub fn room(self) -> RoomId {
        match self {
            Added::Found(id) | Added::Made(id) => id,
        }
    }
}

/// Why the map added no room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// It does not know for certain which room the player was in: what is
    /// added would hang from the wrong one.
    Lost,
    /// Neither the room's name nor the exit taken was given.
    Unnamed,
    /// The exit already leads to a room the map has: the player is in it,
    /// or the map is wrong there, which is not for adding to put right.
    Known(RoomId),
    /// Rooms the map has could be the one, and it cannot tell which:
    /// rather than have the room twice, none is made.
    Alike(Vec<RoomId>),
}

/// What became of the rooms the map had added on taking a map.
#[derive(Default)]
pub(crate) struct Carried {
    /// Rooms kept, which the map taken does not have.
    pub(crate) kept: usize,
    /// Rooms added that the map taken has, each with the number it has
    /// there.
    pub(crate) joined: HashMap<RoomId, RoomId>,
}

impl Map {
    /// The player went by `came` from the room the map has them in, and
    /// is in a room the exit does not lead to on the map: finds it among
    /// the rooms the map has, or makes it, and has the exit lead there and
    /// the player be there.
    ///
    /// By a compass exit, the room is found that way, on the same level of
    /// the same area, within the stretch of a link, with that name and
    /// those exits, and not reached back from another room. By any other,
    /// as through a portal, which could lead anywhere, a room with that
    /// name and exits anywhere refuses it: the map cannot tell. A room
    /// made is drawn the way of the exit or, for any other, beside the
    /// room left; its exits lead to unexplored rooms until taken, and the
    /// way back is not assumed.
    pub fn add(&mut self, seen: Arrival<'_>) -> Result<Added, Refused> {
        // The move sent that this answers, if one was, is answered.
        let _ = self.next_move();
        let name = seen.name.trim();
        let came = self.direction(seen.came);
        if name.is_empty() || came.is_empty() {
            return Err(Refused::Unnamed);
        }
        let Some(here) = self.current().cloned() else {
            return Err(Refused::Lost);
        };
        if let Some(exit) = here.exits.get(&came)
            && self.rooms.contains_key(&exit.to)
        {
            return Err(Refused::Known(exit.to));
        }
        let exits = self.exit_set(seen.exits);
        let key = self.name_key(name);
        let alike = |room: &Room| {
            room.id != here.id
                && self.name_key(&room.name) == key
                && self.has_exits(room.id, &exits)
        };
        let step = direction_offset(&came);
        let mut found: Vec<RoomId> = match step {
            Some(step) => {
                let back = (DIRECTIONS.iter())
                    .find(|known| known.step == Some((-step.0, -step.1, -step.2)))
                    .map(|known| known.key);
                // Its way back, if it has one, leads here or nowhere known.
                let unclaimed = |room: &Room| {
                    let back = back.and_then(|back| room.exits.get(back));
                    back.is_none_or(|exit| exit.to == here.id || !self.rooms.contains_key(&exit.to))
                };
                let that_way = |room: &Room| {
                    room.area == here.area
                        && (here.pos.steps_to(room.pos, step))
                            .is_some_and(|steps| steps <= placing::MAX_STRETCH)
                };
                (self.rooms.values())
                    .filter(|room| that_way(room) && alike(room) && unclaimed(room))
                    .map(|room| room.id)
                    .collect()
            }
            None => {
                let anywhere: Vec<RoomId> = (self.rooms.values())
                    .filter(|room| alike(room))
                    .map(|room| room.id)
                    .collect();
                if !anywhere.is_empty() {
                    let mut anywhere = anywhere;
                    anywhere.sort_unstable();
                    return Err(Refused::Alike(anywhere));
                }
                anywhere
            }
        };
        found.sort_unstable();
        let (added, made) = match found.as_slice() {
            [] => {
                let id = self.next_added_id();
                let pos = match step {
                    Some(step) => self.free_along(&here.area, here.pos, step, 1),
                    None => self.free_beside(&here.area, here.pos),
                };
                let unexplored = Exit {
                    to: UNEXPLORED,
                    ..Exit::default()
                };
                let room = Room {
                    id,
                    name: name.to_owned(),
                    area: here.area.clone(),
                    pos,
                    exits: (exits.into_iter())
                        .map(|direction| (direction, unexplored.clone()))
                        .collect(),
                    weight: 1,
                    avoid: false,
                    colour: None,
                    note: String::new(),
                    renamed: false,
                    added: true,
                };
                (Added::Made(id), Some(room))
            }
            [one] => (Added::Found(*one), None),
            _ => return Err(Refused::Alike(found)),
        };
        let to = added.room();
        let mut before = vec![(here.id, Some(here.clone()))];
        if let Some(room) = made {
            before.push((to, None));
            self.rooms.insert(to, room);
        }
        // Of the user's, as one the map taken again keeps, and with what
        // was set on it if it was there, unexplored.
        let link = Exit {
            to,
            custom: true,
            ..here.exits.get(&came).cloned().unwrap_or_default()
        };
        if let Some(room) = self.rooms.get_mut(&here.id) {
            room.exits.insert(came, link);
        }
        self.keep([here.id, to]);
        self.current = Some(to);
        self.candidates.clear();
        self.keep_whereabouts();
        self.pending.clear();
        let changed = match added {
            Added::Made(id) => Changed::Made(id, name.to_owned()),
            Added::Found(id) => Changed::Found(id, name.to_owned()),
        };
        self.told.push(changed.clone());
        self.record(changed, before);
        Ok(added)
    }

    /// The rooms the map added of itself, in the order they were made.
    pub fn added(&self) -> Vec<&Room> {
        let mut added: Vec<&Room> = self.rooms.values().filter(|room| room.added).collect();
        added.sort_unstable_by_key(|room| room.id);
        added
    }

    /// The number for the next room the map adds: after those it added.
    fn next_added_id(&self) -> RoomId {
        (self.rooms.keys().copied())
            .filter(|id| *id >= ADDED_FROM)
            .max()
            .map_or(ADDED_FROM, |id| id + 1)
    }

    /// Carries the rooms the map added into `taken`, a map taken in place
    /// of this one: each that `taken` has, as reached from a room by the
    /// same exit, with the same name and exits of the compass, is joined
    /// with it, what was set on it going there; the rest are kept, each
    /// part of them moved as the room it hangs from was. Exits that led to
    /// a room joined lead to the one it was joined with.
    pub(crate) fn carry_added(&self, taken: &mut HashMap<RoomId, Room>) -> Carried {
        let mut own: BTreeMap<RoomId, Room> = (self.rooms.values())
            .filter(|room| room.added && !taken.contains_key(&room.id))
            .map(|room| (room.id, room.clone()))
            .collect();
        if own.is_empty() {
            return Carried::default();
        }
        // The rooms that led to each added, and by which exit.
        let mut led_from: HashMap<RoomId, Vec<(RoomId, String)>> = HashMap::new();
        for room in self.rooms.values() {
            for (direction, exit) in &room.exits {
                if own.contains_key(&exit.to) {
                    let from = (room.id, direction.clone());
                    led_from.entry(exit.to).or_default().push(from);
                }
            }
        }
        // The rooms taken that lead to each, by each exit.
        let mut leading: HashMap<(RoomId, &str), Vec<RoomId>> = HashMap::new();
        for room in taken.values() {
            for (direction, exit) in &room.exits {
                leading
                    .entry((exit.to, direction.as_str()))
                    .or_default()
                    .push(room.id);
            }
        }
        let compass = |room: &Room| -> BTreeSet<String> {
            (room.exits.keys())
                .filter(|direction| known_direction(direction).is_some())
                .cloned()
                .collect()
        };
        let same = |room: &Room, other: &Room| {
            self.name_key(&room.name) == self.name_key(&other.name)
                && compass(room) == compass(other)
        };
        let mut joined: HashMap<RoomId, RoomId> = HashMap::new();
        let mut claimed: HashSet<RoomId> = HashSet::new();
        // A room joined is a room taken, by which those next to it may be
        // found in turn.
        loop {
            let now = |id: RoomId| joined.get(&id).copied().unwrap_or(id);
            let mut found = Vec::new();
            for (id, room) in own.iter().filter(|(id, _)| !joined.contains_key(id)) {
                let mut candidates = BTreeSet::new();
                for (from, direction) in led_from.get(id).into_iter().flatten() {
                    let exit = taken
                        .get(&now(*from))
                        .and_then(|from| from.exits.get(direction));
                    candidates.extend(exit.map(|exit| exit.to));
                }
                for (direction, exit) in &room.exits {
                    let to = now(exit.to);
                    let from = leading.get(&(to, direction.as_str()));
                    candidates.extend(from.into_iter().flatten());
                }
                let candidates: Vec<RoomId> = (candidates.into_iter())
                    .filter(|id| !claimed.contains(id))
                    .filter(|id| taken.get(id).is_some_and(|other| same(room, other)))
                    .collect();
                if let [one] = candidates.as_slice() {
                    found.push((*id, *one));
                }
            }
            // Two found to be the same room are neither.
            let mut times: HashMap<RoomId, usize> = HashMap::new();
            for (_, into) in &found {
                *times.entry(*into).or_default() += 1;
            }
            found.retain(|(_, into)| times[into] == 1);
            if found.is_empty() {
                break;
            }
            for (id, into) in found {
                joined.insert(id, into);
                claimed.insert(into);
            }
        }
        let now = |id: RoomId| joined.get(&id).copied().unwrap_or(id);
        for (id, into) in &joined {
            let room = &own[id];
            let Some(other) = taken.get_mut(into) else {
                continue;
            };
            other.avoid |= room.avoid;
            other.colour = other.colour.or(room.colour);
            if other.weight == 1 {
                other.weight = room.weight;
            }
            if other.note.is_empty() {
                other.note.clone_from(&room.note);
            }
            if room.renamed {
                other.name.clone_from(&room.name);
                other.renamed = true;
            }
            for (direction, exit) in &room.exits {
                if !other.exits.contains_key(direction) && exit.to != UNEXPLORED {
                    let exit = Exit {
                        to: now(exit.to),
                        custom: true,
                        ..exit.clone()
                    };
                    other.exits.insert(direction.clone(), exit);
                }
            }
        }
        own.retain(|id, _| !joined.contains_key(id));
        let moved = self.moved_with_their_anchors(&own, taken, &now);
        for (id, mut room) in own {
            if let Some((shift, area)) = moved.get(&id) {
                room.pos = room.pos.offset(*shift, 1);
                room.area.clone_from(area);
            }
            taken.insert(id, room);
        }
        for room in taken.values_mut() {
            for exit in room.exits.values_mut() {
                exit.to = now(exit.to);
            }
        }
        Carried {
            kept: taken.values().filter(|room| room.added).count(),
            joined,
        }
    }

    /// How far each part of the rooms `kept` moves, and to which area: as
    /// the room it hangs from did between this map and `taken`. A part is
    /// the rooms kept joined to each other by exits; it hangs from the
    /// first room of the map by number joined to it that both maps have.
    fn moved_with_their_anchors(
        &self,
        kept: &BTreeMap<RoomId, Room>,
        taken: &HashMap<RoomId, Room>,
        now: &dyn Fn(RoomId) -> RoomId,
    ) -> HashMap<RoomId, ((i32, i32, i32), String)> {
        let mut next: HashMap<RoomId, BTreeSet<RoomId>> = HashMap::new();
        for room in self.rooms.values() {
            for exit in room.exits.values() {
                let (from, to) = (room.id, exit.to);
                if kept.contains_key(&from) || kept.contains_key(&to) {
                    next.entry(from).or_default().insert(to);
                    next.entry(to).or_default().insert(from);
                }
            }
        }
        let mut moved = HashMap::new();
        let mut placed: HashSet<RoomId> = HashSet::new();
        for start in kept.keys() {
            if !placed.insert(*start) {
                continue;
            }
            let mut part = vec![*start];
            let mut anchors = BTreeSet::new();
            let mut queue = VecDeque::from([*start]);
            while let Some(id) = queue.pop_front() {
                for other in next.get(&id).into_iter().flatten() {
                    if kept.contains_key(other) {
                        if placed.insert(*other) {
                            part.push(*other);
                            queue.push_back(*other);
                        }
                    } else {
                        anchors.insert(*other);
                    }
                }
            }
            let anchor = anchors.into_iter().find_map(|id| {
                let was = self.rooms.get(&id)?;
                let is = taken.get(&now(id))?;
                Some((was, is))
            });
            let Some((was, is)) = anchor else {
                continue;
            };
            let shift = (
                is.pos.x - was.pos.x,
                is.pos.y - was.pos.y,
                is.pos.z - was.pos.z,
            );
            // Rooms drawn in another area than the room's are left where
            // they are.
            for id in part.into_iter().filter(|id| kept[id].area == was.area) {
                moved.insert(id, (shift, is.area.clone()));
            }
        }
        moved
    }
}
