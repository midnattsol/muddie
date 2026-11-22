//! Where the player is, for servers that do not say which room it is: from
//! the name and exits of the room shown, and the exit taken to it. While
//! several rooms could be the one, each is followed along the exits taken
//! until the rooms they lead to tell them apart.

use super::*;

/// What the map keeps of the room the player was last known to be in.
const LAST_ROOM: &str = "last room";

/// How long a move sent is waited for: one the server has not answered by
/// then did not happen.
const MOVE_ANSWERED_IN: std::time::Duration = std::time::Duration::from_secs(5);

/// What the server showed of the room the player is now in. Each part is
/// optional; the more there is, the surer the map can be.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sighting<'a> {
    pub name: Option<&'a str>,
    pub exits: Option<&'a [String]>,
    /// The exit taken to get there, in any of the words for it.
    pub came: Option<&'a str>,
}

/// Where the map now has the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located {
    Here(RoomId),
    /// In one of these rooms, which the map cannot yet tell apart.
    Unsure(Vec<RoomId>),
    /// In no room the map knows.
    Nowhere,
}

impl Map {
    /// A command was sent to the server: if it is a move, the next room
    /// shown is where it led, unless the server says otherwise.
    pub fn sent(&mut self, command: &str) {
        let way = self.direction(command);
        let here = self.current.and_then(|id| self.rooms.get(&id));
        let a_move = direction_offset(&way).is_some()
            || matches!(way.as_str(), "in" | "out")
            || here.is_some_and(|room| room.exits.contains_key(&way));
        if a_move {
            self.moves.push_back((way, std::time::Instant::now()));
        }
    }

    /// The moves sent did not happen, as the server said.
    pub fn moves_failed(&mut self) {
        self.moves.clear();
    }

    /// The oldest move sent that is still awaited.
    pub(crate) fn next_move(&mut self) -> Option<String> {
        while let Some((way, at)) = self.moves.pop_front() {
            if at.elapsed() < MOVE_ANSWERED_IN {
                return Some(way);
            }
        }
        None
    }

    /// Puts the player in a room, as the user or a script says. Returns
    /// false if there is no such room.
    pub fn set_current(&mut self, id: RoomId) -> bool {
        if !self.rooms.contains_key(&id) {
            return false;
        }
        self.current = Some(id);
        self.candidates.clear();
        self.pending.clear();
        self.keep_whereabouts();
        true
    }

    /// Whether the map puts right the name of a room the player is
    /// followed to by the way they went and its exits, though the server
    /// names it otherwise (see [`Map::arrive`]). Off unless asked for.
    pub fn set_fix_names(&mut self, on: bool) {
        self.fix_names = on;
    }

    /// The rooms the player may be in while the map cannot tell which, in
    /// order of number; empty when it can, or when it has no idea.
    pub fn candidates(&self) -> &[RoomId] {
        &self.candidates
    }

    /// The rooms of that name and exits; of that name if none has just
    /// those exits. With no room of that name, those of the names most
    /// like it (see [`Map::called_alike`]). In order of number.
    pub fn locate(&self, name: &str, exits: Option<&[String]>) -> Vec<RoomId> {
        let mut called = self.called(name);
        if called.is_empty() {
            called = self.called_alike(name);
        }
        let Some(exits) = exits.map(|exits| self.exit_set(exits)) else {
            return called;
        };
        let exact: Vec<RoomId> = (called.iter().copied())
            .filter(|id| self.shows_exits(*id, &exits))
            .collect();
        if exact.is_empty() { called } else { exact }
    }

    /// The player is now in a room the server showed: works out which, from
    /// where the player was, the exit taken, and what the room is like.
    ///
    /// By the exit taken, from the room the player was in or each that
    /// could have been it, to a room of that name. With no exit, the room
    /// the player was in if it is so, as after looking, or one next to it.
    /// Failing those, any room of that name and exits.
    ///
    /// A room is known by where it is as much as by its name: if the exit
    /// the server says was taken leads to a room with just the exits shown,
    /// and no room around has the name shown with them, the player is
    /// followed there, though the map names it otherwise, as maps taken
    /// from elsewhere name rooms with what was said in them, cut short, or
    /// not at all. If the next move is to a room as the map has it, name
    /// and exits, the room between is the one: with [`Map::set_fix_names`],
    /// it is given the name shown, as a change that can be undone.
    pub fn arrive(&mut self, seen: Sighting<'_>) -> Located {
        // The exit taken: as the server says, or else the move sent that
        // this room answers. Either way, that move is answered.
        let sent = self.next_move();
        let came = seen.came.map(|came| self.direction(came)).or(sent);
        let key = seen.name.map(|name| self.name_key(name));
        let exits = seen.exits.map(|exits| self.exit_set(exits));
        // A room the player was followed to though named otherwise goes by
        // the name shown there until it is put right, or the player is lost.
        let pending = std::mem::take(&mut self.pending);
        let last_pending = pending.last().map(|(room, _)| *room);
        let named = |id: RoomId| {
            (key.as_ref()).is_none_or(|key| {
                let called = |name: &str| self.name_key(name) == *key;
                (self.rooms.get(&id)).is_some_and(|room| called(&room.name))
                    || (pending.iter()).any(|(room, name)| *room == id && called(name))
            })
        };
        let like = |id: RoomId| {
            named(id)
                && exits
                    .as_ref()
                    .is_none_or(|exits| self.shows_exits(id, exits))
        };
        let before: Vec<RoomId> = match self.current {
            Some(id) => vec![id],
            None => self.candidates.clone(),
        };
        // Where the player was, to take the nearest of rooms alike.
        let was = self
            .current
            .and_then(|id| self.rooms.get(&id))
            .map(|room| (room.area.clone(), room.pos));
        let led_by = |way: &str| -> Vec<RoomId> {
            let mut led: Vec<RoomId> = (before.iter())
                .filter_map(|id| Some(self.rooms.get(id)?.exits.get(way)?.to))
                .filter(|id| self.rooms.contains_key(id))
                .collect();
            led.sort_unstable();
            led.dedup();
            led
        };
        // A room of that name the exit leads to, though its exits are not
        // those shown, is taken only if no room of that name has them: else
        // the map is not trusted there, and the player is looked for.
        let fits_elsewhere = || {
            exits.is_some()
                && (seen.name).is_some_and(|name| self.called(name).into_iter().any(like))
        };
        let moved: Vec<RoomId> = match &came {
            Some(way) => {
                let led_to: Vec<RoomId> = led_by(way).into_iter().filter(|id| named(*id)).collect();
                let fitting: Vec<RoomId> = led_to.iter().copied().filter(|id| like(*id)).collect();
                if !fitting.is_empty() {
                    fitting
                } else if fits_elsewhere() {
                    Vec::new()
                } else {
                    led_to
                }
            }
            None => Vec::new(),
        };
        // The rooms up to two moves from where the player was that are as
        // the server shows: where a map that is wrong in a place, or a move
        // it did not follow, most likely left the player.
        let around_within = |moves: usize| {
            let mut near: Vec<RoomId> = Vec::new();
            let mut seen: HashSet<RoomId> = before.iter().copied().collect();
            let mut ring = before.clone();
            for _ in 0..moves {
                let next: Vec<RoomId> = (ring.iter())
                    .filter_map(|id| self.rooms.get(id))
                    .flat_map(|room| room.exits.values().map(|exit| exit.to))
                    .filter(|id| seen.insert(*id))
                    .collect();
                near.extend(next.iter().copied().filter(|id| like(*id)));
                if !near.is_empty() {
                    break;
                }
                ring = next;
            }
            near
        };
        let around = || around_within(2);
        // By the way the server says was taken and the exits shown, though
        // not the name: the one room it leads to with just those exits.
        let shaped = || -> Option<RoomId> {
            let (Some(way), Some(exits), Some(_)) = (seen.came.and(came.as_ref()), &exits, &key)
            else {
                return None;
            };
            match led_by(way).as_slice() {
                [one] if self.shows_exits(*one, exits) && !exits.is_empty() => Some(*one),
                _ => None,
            }
        };
        let mut followed = None;
        let mut found = if !moved.is_empty() {
            moved
        } else if key.is_none() && exits.is_none() {
            self.pending = pending;
            return self.whereabouts();
        } else if seen.came.is_some() {
            // A move the server says was made, which the map's exits do not
            // lead to such a room by: one next to where the player was, as
            // the map has the exit wrong; else the one the way and the
            // exits lead to, by whatever name; else one further.
            let next_to = around_within(1);
            match shaped() {
                Some(room) if next_to.is_empty() => {
                    followed = Some(room);
                    vec![room]
                }
                _ if !next_to.is_empty() => next_to,
                _ => around(),
            }
        } else {
            // No move, or only one sent, which may not have been made:
            // still there, as after looking; else one around.
            let staying: Vec<RoomId> = before.iter().copied().filter(|id| like(*id)).collect();
            if staying.is_empty() {
                around()
            } else {
                staying
            }
        };
        found.sort_unstable();
        found.dedup();
        if found.is_empty()
            && let Some(name) = seen.name
        {
            found = self.locate(name, seen.exits);
        }
        // Of several, the nearest to where the player was comes first: it
        // is where the map is drawn while it is unsure.
        if let Some((area, pos)) = was {
            let far = |id: &RoomId| {
                let room = &self.rooms[id];
                let gap = |a: i32, b: i32| i64::from(a - b).pow(2);
                let elsewhere = i64::from(room.area != area || room.pos.z != pos.z);
                (elsewhere, gap(room.pos.x, pos.x) + gap(room.pos.y, pos.y))
            };
            found.sort_by_key(far);
        }
        // The rooms named otherwise, left by a move to a room as the map has
        // it, name and exits: they are the ones, and their names those shown.
        // Only from the last of them, where the map had the player.
        let from_pending =
            matches!((before.as_slice(), last_pending), ([was], Some(last)) if *was == last);
        let confirmed = match found.as_slice() {
            [next] => {
                from_pending
                    && Some(*next) != last_pending
                    && seen.came.is_some()
                    && exits.is_some()
                    && (self.rooms.get(next))
                        .is_some_and(|known| Some(self.name_key(&known.name)) == key)
                    && like(*next)
            }
            _ => false,
        };
        let staying = from_pending && found.as_slice() == before.as_slice();
        let written = seen.name.map(|name| name.trim().to_owned());
        let located = self.settle(found);
        if confirmed {
            for (room, name) in pending {
                self.put_right(room, &name);
            }
        } else if staying {
            self.pending = pending;
        } else if let (Some(room), Some(name)) = (followed, written) {
            // On from the last, the way goes on; else it starts here.
            if from_pending {
                self.pending = pending;
                self.pending.retain(|(known, _)| *known != room);
            }
            self.pending.push((room, name));
        }
        located
    }

    /// Gives a room the name the server showed for it, as a change of the
    /// map's own, if names are put right.
    fn put_right(&mut self, id: RoomId, name: &str) {
        let Some(room) = self.rooms.get(&id).cloned() else {
            return;
        };
        if !self.fix_names || name.is_empty() || room.name == name {
            return;
        }
        let changed = Changed::Renamed {
            room: id,
            was: room.name.clone(),
            now: name.to_owned(),
        };
        if let Some(room) = self.rooms.get_mut(&id) {
            room.name = name.to_owned();
            room.renamed = true;
        }
        self.keep([id]);
        self.revision += 1;
        self.told.push(changed.clone());
        self.record(changed, vec![(id, Some(room))]);
    }

    /// Keeps the room the player is in with the map, as it changes, to
    /// start from there when it is opened again, however it was closed:
    /// the next room the server shows tells whether they are still there.
    pub(crate) fn keep_whereabouts(&mut self) {
        let Some(id) = self.current else {
            return;
        };
        let id = id.to_string();
        if self.about.get(LAST_ROOM) == Some(&id) {
            return;
        }
        if let Some(store) = &mut self.store {
            let _ = store.set_about(LAST_ROOM, Some(&id));
        }
        self.about.insert(LAST_ROOM.to_owned(), id);
    }

    /// The room the player was last known to be in, if it is still there.
    pub(crate) fn last_room(&self) -> Option<RoomId> {
        let id = self.about.get(LAST_ROOM)?.parse().ok()?;
        self.rooms.contains_key(&id).then_some(id)
    }

    /// Where the map has the player, as [`Map::arrive`] answers.
    pub fn whereabouts(&self) -> Located {
        match (self.current, self.candidates.is_empty()) {
            (Some(id), _) => Located::Here(id),
            (None, false) => Located::Unsure(self.candidates.clone()),
            (None, true) => Located::Nowhere,
        }
    }

    /// Takes the rooms the player may be in: one is where the player is.
    fn settle(&mut self, mut found: Vec<RoomId>) -> Located {
        self.current = None;
        self.candidates.clear();
        match found.len() {
            0 => {}
            1 => self.current = found.pop(),
            _ => self.candidates = found,
        }
        self.keep_whereabouts();
        self.whereabouts()
    }

    /// The rooms whose name is most like `name`, when none has it: those
    /// whose name has it in it or is in it, else those whose name shares
    /// the most of its words. None if no word is shared.
    pub(crate) fn called_alike(&self, name: &str) -> Vec<RoomId> {
        let key = self.name_key(name);
        if key.is_empty() {
            return Vec::new();
        }
        let words = |text: &str| -> BTreeSet<String> {
            (text.split(|c: char| !c.is_alphanumeric()))
                .filter(|word| word.chars().count() > 2)
                .map(str::to_owned)
                .collect()
        };
        let wanted = words(&key);
        let mut ids = self.with_names(|names| {
            let within: Vec<RoomId> = (names.iter())
                .filter(|(known, _)| {
                    !known.is_empty() && (known.contains(&key) || key.contains(known.as_str()))
                })
                .flat_map(|(_, ids)| ids.iter().copied())
                .collect();
            if !within.is_empty() {
                return within;
            }
            let shared = |known: &str| words(known).intersection(&wanted).count();
            let most = names.keys().map(|known| shared(known)).max().unwrap_or(0);
            if most == 0 {
                return Vec::new();
            }
            (names.iter())
                .filter(|(known, _)| shared(known) == most)
                .flat_map(|(_, ids)| ids.iter().copied())
                .collect()
        });
        ids.sort_unstable();
        ids
    }

    /// Exits as the map keeps them, however they were said.
    pub(crate) fn exit_set(&self, exits: &[String]) -> BTreeSet<String> {
        (exits.iter())
            .map(|exit| self.direction(exit))
            .filter(|exit| !exit.is_empty())
            .collect()
    }

    /// Whether a room has the exits the server shows, and no other of the
    /// compass: an exit by a command, which it does not show, may be there.
    pub(crate) fn shows_exits(&self, id: RoomId, exits: &BTreeSet<String>) -> bool {
        self.rooms.get(&id).is_some_and(|room| {
            exits.iter().all(|exit| room.exits.contains_key(exit))
                && (room.exits.keys())
                    .all(|exit| exits.contains(exit) || known_direction(exit).is_none())
        })
    }

    /// Whether a room has just these exits.
    pub(crate) fn has_exits(&self, id: RoomId, exits: &BTreeSet<String>) -> bool {
        self.rooms.get(&id).is_some_and(|room| {
            room.exits.len() == exits.len()
                && exits.iter().all(|exit| room.exits.contains_key(exit))
        })
    }
}
