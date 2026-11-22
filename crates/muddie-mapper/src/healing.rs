//! Putting right what the map has wrong, as the player goes, by what the
//! server shows: the names of rooms, exits it lacks, and exits that lead
//! elsewhere than it has them. Nothing is changed on one sighting alone:
//! only where the map knew for certain where the player was, and, for what
//! it followed the player by though the map had it otherwise, once the
//! next move is to a room as the map has it. Each is a change of the map's
//! own, told and undone as the rooms it adds are (see the journal). What
//! it could only take away, an exit the server does not show, it leaves,
//! as a door shut or an exit hidden is not shown either: it is counted, for
//! the user to look at.

use super::*;

/// What [`Map::arrive`] worked out, for what it shows to be put right.
pub(crate) struct Arrived {
    /// The room the map had the player in, or those it could be.
    pub(crate) before: Vec<RoomId>,
    /// Whether the map knew for certain the player was there: the room as
    /// shown, by its own name and with every exit it has.
    pub(crate) was_sure: bool,
    /// Whether the server said a move was made.
    pub(crate) moved: bool,
    /// The exit taken, as the map keeps it.
    pub(crate) way: Option<String>,
    /// The name shown, and as the map compares names.
    pub(crate) name: Option<String>,
    pub(crate) key: Option<String>,
    /// The exits shown, as the map keeps them.
    pub(crate) shown: Option<BTreeSet<String>>,
    /// The room the player was followed to by the way and the exits,
    /// though the map names it otherwise.
    pub(crate) followed: Option<RoomId>,
    /// Whether the player was found in a room next to where they were,
    /// other than the one the map has the exit taken lead to.
    pub(crate) beside: bool,
}

/// An exit that leads elsewhere than the map has it, as one move showed,
/// to be put right once the next bears it out.
#[derive(Debug, Clone)]
pub(crate) struct Reroute {
    from: RoomId,
    way: String,
    was: RoomId,
    to: RoomId,
}

/// How many times a room is gone to without the server showing an exit
/// the map has, before the exit is said to look wrong.
pub const UNSHOWN_AFTER: u32 = 3;

impl Map {
    /// Whether the map learns as the player goes, from what the server
    /// shows: it puts right names, exits it lacks and exits that lead
    /// elsewhere, and adds the rooms it has not where the player goes by
    /// the compass from a room it knew they were in. Off unless asked for.
    pub fn set_learning(&mut self, on: bool) {
        self.learning = on;
    }

    /// The room shown, gone to by the compass from a room the map knew for
    /// certain the player was in, by an exit it has not or that leads to no
    /// room it has: added, as [`Map::add`] does, if the map is learning and
    /// what is shown is a room, named, and with exits all directions.
    pub(crate) fn add_if_new(&mut self, seen: &Sighting<'_>, way: Option<&str>) -> Option<Located> {
        let (Some(name), Some(exits), Some(way), Some(here)) =
            (seen.name, seen.exits, way, self.current)
        else {
            return None;
        };
        let directions = |words: &[String]| {
            !words.is_empty()
                && (words.iter()).all(|word| known_direction(&self.direction(word)).is_some())
        };
        let leads = (self.rooms.get(&here))
            .and_then(|room| room.exits.get(way))
            .is_some_and(|exit| self.rooms.contains_key(&exit.to));
        let new = self.learning
            && self.sure
            && seen.came.is_some()
            && known_direction(way).is_some()
            && !name.trim().is_empty()
            && directions(exits)
            && !leads;
        if !new {
            return None;
        }
        let added = self
            .add_shown(Arrival {
                name,
                exits,
                came: way,
            })
            .ok()?;
        // Made as shown, or found so: the player is there for certain.
        self.sure = true;
        Some(Located::Here(added.room()))
    }

    /// Forgets what waited to be borne out, as the player is put somewhere
    /// else or the rooms change.
    pub(crate) fn forget_healing(&mut self) {
        self.pending.clear();
        self.reroute = None;
        self.sure = false;
    }

    /// Exits the map has that the server has not shown, as the player
    /// came to their rooms, at least `times` in a row: each with its room
    /// and how many times.
    pub fn unshown_exits(&self, times: u32) -> Vec<(RoomId, String, u32)> {
        let mut unshown: Vec<(RoomId, String, u32)> = (self.unshown.iter())
            .filter(|(_, count)| **count >= times)
            .filter(|((room, way), _)| {
                (self.rooms.get(room)).is_some_and(|room| room.exits.contains_key(way))
            })
            .map(|((room, way), count)| (*room, way.clone(), *count))
            .collect();
        unshown.sort();
        unshown
    }

    /// Puts right what the arrival shows the map to have wrong, as far as
    /// it is borne out, and keeps what waits to be.
    pub(crate) fn heal(&mut self, arrived: Arrived, pending: Vec<(RoomId, String)>) {
        let here = self.current;
        // The room as shown, by its own name, and with every exit of the
        // compass it has among those shown; and with just those.
        let as_shown = |map: &Map, id: RoomId| {
            let (Some(room), Some(key), Some(shown)) =
                (map.rooms.get(&id), &arrived.key, &arrived.shown)
            else {
                return false;
            };
            map.name_key(&room.name) == *key
                && (room.exits.keys())
                    .filter(|way| known_direction(way).is_some())
                    .all(|way| shown.contains(way))
        };
        let from = match arrived.before.as_slice() {
            [one] => Some(*one),
            _ => None,
        };
        let matched = here.is_some_and(|id| as_shown(self, id));
        // Rooms followed through by the way and exits, named otherwise.
        let last = pending.last().map(|(room, _)| *room);
        let from_pending = from.is_some() && from == last;
        let borne_out = arrived.moved && matched && here != from;
        if from_pending && borne_out {
            for (room, name) in &pending {
                self.rename_as_shown(*room, name);
            }
        } else if from_pending && here == from {
            self.pending = pending;
        } else if let (Some(room), Some(name)) = (arrived.followed, &arrived.name) {
            if from_pending {
                self.pending = pending;
                self.pending.retain(|(known, _)| *known != room);
            }
            self.pending.push((room, name.clone()));
        }
        // An exit that led to a room beside, borne out the same way.
        match self.reroute.take() {
            Some(reroute) if from == Some(reroute.to) && borne_out => {
                self.reroute_as_shown(&reroute)
            }
            Some(reroute) if from == Some(reroute.to) && here == from => {
                self.reroute = Some(reroute)
            }
            _ => {}
        }
        if let (true, true, Some(from), Some(way), Some(to)) =
            (arrived.beside, arrived.was_sure, from, &arrived.way, here)
        {
            let was = (self.rooms.get(&from))
                .and_then(|room| room.exits.get(way))
                .map(|exit| exit.to)
                .filter(|was| *was != to && self.rooms.contains_key(was));
            if let Some(was) = was {
                self.reroute = Some(Reroute {
                    from,
                    way: way.clone(),
                    was,
                    to,
                });
            }
        }
        // From a room known for certain, or one just borne out, to one by
        // its own name: the exits shown that it lacks are added, and those
        // it has that are not shown counted.
        let came_sure = arrived.was_sure || (from_pending && borne_out);
        if let (true, true, Some(id), Some(shown)) =
            (came_sure, arrived.moved, here, &arrived.shown)
            && (self.rooms.get(&id))
                .is_some_and(|room| Some(self.name_key(&room.name)) == arrived.key)
        {
            if matched {
                self.add_exits_shown(id, shown);
            }
            self.count_unshown(id, shown);
        }
        self.sure = here.is_some_and(|id| as_shown(self, id));
    }

    /// Gives a room the name the server showed for it, as a change of the
    /// map's own, if what it has wrong is put right.
    fn rename_as_shown(&mut self, id: RoomId, name: &str) {
        let Some(room) = self.rooms.get(&id).cloned() else {
            return;
        };
        if !self.learning || name.is_empty() || room.name == name {
            return;
        }
        if let Some(room) = self.rooms.get_mut(&id) {
            room.name = name.to_owned();
            room.renamed = true;
        }
        self.changed_itself(
            Changed::Renamed {
                room: id,
                was: room.name.clone(),
                now: name.to_owned(),
            },
            room,
        );
    }

    /// Has an exit lead where the player was found by taking it.
    fn reroute_as_shown(&mut self, reroute: &Reroute) {
        let Some(room) = self.rooms.get(&reroute.from).cloned() else {
            return;
        };
        let still = room.exits.get(&reroute.way).map(|exit| exit.to) == Some(reroute.was);
        if !self.learning || !still || !self.rooms.contains_key(&reroute.to) {
            return;
        }
        if let Some(exit) =
            (self.rooms.get_mut(&reroute.from)).and_then(|room| room.exits.get_mut(&reroute.way))
        {
            exit.to = reroute.to;
            exit.custom = true;
        }
        self.changed_itself(
            Changed::Rerouted {
                room: reroute.from,
                way: reroute.way.clone(),
                was: reroute.was,
                now: reroute.to,
            },
            room,
        );
    }

    /// Adds to a room the exits of the compass shown that it lacks, leading
    /// to rooms not explored until taken.
    fn add_exits_shown(&mut self, id: RoomId, shown: &BTreeSet<String>) {
        let Some(room) = self.rooms.get(&id).cloned() else {
            return;
        };
        let lacking: Vec<String> = (shown.iter())
            .filter(|way| known_direction(way).is_some() && !room.exits.contains_key(*way))
            .cloned()
            .collect();
        if !self.learning || lacking.is_empty() {
            return;
        }
        if let Some(room) = self.rooms.get_mut(&id) {
            for way in &lacking {
                let unexplored = Exit {
                    to: UNEXPLORED,
                    custom: true,
                    ..Exit::default()
                };
                room.exits.insert(way.clone(), unexplored);
            }
        }
        self.changed_itself(
            Changed::ExitsAdded {
                room: id,
                exits: lacking,
            },
            room,
        );
    }

    /// Counts, for a room gone to, the exits it has that were not shown;
    /// one shown starts again.
    fn count_unshown(&mut self, id: RoomId, shown: &BTreeSet<String>) {
        let Some(room) = self.rooms.get(&id) else {
            return;
        };
        for way in room
            .exits
            .keys()
            .filter(|way| known_direction(way).is_some())
        {
            let key = (id, way.clone());
            match shown.contains(way) {
                true => drop(self.unshown.remove(&key)),
                false => *self.unshown.entry(key).or_default() += 1,
            }
        }
    }

    /// Keeps a change the map made of itself to a room, as it was before,
    /// to be told and undone.
    fn changed_itself(&mut self, changed: Changed, before: Room) {
        let id = before.id;
        self.keep([id]);
        self.revision += 1;
        self.told.push(changed.clone());
        self.record(changed, vec![(id, Some(before))]);
    }
}
