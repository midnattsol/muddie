//! Where the player is, for servers that do not say which room it is: from
//! the name and exits of the room shown, and the exit taken to it. While
//! several rooms could be the one, each is followed along the exits taken
//! until the rooms they lead to tell them apart.

use super::*;

mod search;
use search::{Found, Search};

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
        self.forget_healing();
        self.keep_whereabouts();
        true
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
    /// not at all. With [`Map::set_learning`], what the map has wrong on
    /// the way is put right once it is borne out, and a room it has not
    /// is added (see `healing`).
    pub fn arrive(&mut self, seen: Sighting<'_>) -> Located {
        // The exit taken: as the server says, or else the move sent that
        // this room answers. Either way, that move is answered.
        let sent = self.next_move();
        let came = seen.came.map(|came| self.direction(came)).or(sent);
        if let Some(added) = self.add_if_new(&seen, came.as_deref()) {
            return added;
        }
        let key = seen.name.map(|name| self.name_key(name));
        let exits = seen.exits.map(|exits| self.exit_set(exits));
        // A room the player was followed to though named otherwise goes by
        // the name shown there until it is put right, or the player is lost.
        let pending = std::mem::take(&mut self.pending);
        let was_sure = std::mem::take(&mut self.sure);
        let before: Vec<RoomId> = match self.current {
            Some(id) => vec![id],
            None => self.candidates.clone(),
        };
        // Where the player was, to take the nearest of rooms alike.
        let was = self
            .current
            .and_then(|id| self.rooms.get(&id))
            .map(|room| (room.area.clone(), room.pos));
        let search = Search {
            map: self,
            seen,
            came: came.as_deref(),
            key: &key,
            exits: &exits,
            pending: &pending,
            before: &before,
        };
        let Some(Found {
            rooms: mut found,
            followed,
            beside,
        }) = search.find()
        else {
            self.pending = pending;
            self.sure = was_sure;
            return self.whereabouts();
        };
        found.sort_unstable();
        found.dedup();
        // A name no room has, after a move the server says was made: the
        // player is in a room the map has not, not in one named like it.
        let unknown =
            seen.came.is_some() && seen.name.is_some_and(|name| self.called(name).is_empty());
        if found.is_empty()
            && !unknown
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
        let arrived = healing::Arrived {
            before,
            was_sure,
            moved: seen.came.is_some(),
            way: came,
            name: seen.name.map(|name| name.trim().to_owned()),
            key,
            shown: exits,
            followed,
            beside,
        };
        let located = self.settle(found);
        self.heal(arrived, pending);
        located
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
}
