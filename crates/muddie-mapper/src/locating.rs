//! Where the player is, for servers that do not say which room it is: from
//! the name and exits of the room shown, and the exit taken to it. While
//! several rooms could be the one, each is followed along the exits taken
//! until the rooms they lead to tell them apart.

use super::*;
use naming::name_key;

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
    /// Puts the player in a room, as the user or a script says. Returns
    /// false if there is no such room.
    pub fn set_current(&mut self, id: RoomId) -> bool {
        if !self.rooms.contains_key(&id) {
            return false;
        }
        self.current = Some(id);
        self.candidates.clear();
        true
    }

    /// The rooms the player may be in while the map cannot tell which, in
    /// order of number; empty when it can, or when it has no idea.
    pub fn candidates(&self) -> &[RoomId] {
        &self.candidates
    }

    /// The rooms of that name and exits; of that name if none has just
    /// those exits. In order of number.
    pub fn locate(&self, name: &str, exits: Option<&[String]>) -> Vec<RoomId> {
        let called = self.called(name);
        let Some(exits) = exits.map(|exits| self.exit_set(exits)) else {
            return called;
        };
        let exact: Vec<RoomId> = (called.iter().copied())
            .filter(|id| self.has_exits(*id, &exits))
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
    pub fn arrive(&mut self, seen: Sighting<'_>) -> Located {
        let key = seen.name.map(name_key);
        let exits = seen.exits.map(|exits| self.exit_set(exits));
        let named = |id: RoomId| {
            (key.as_ref()).is_none_or(|key| {
                self.rooms
                    .get(&id)
                    .is_some_and(|room| name_key(&room.name) == *key)
            })
        };
        let like =
            |id: RoomId| named(id) && exits.as_ref().is_none_or(|exits| self.has_exits(id, exits));
        let before: Vec<RoomId> = match self.current {
            Some(id) => vec![id],
            None => self.candidates.clone(),
        };
        let mut found: Vec<RoomId> = match seen.came {
            Some(came) => {
                let way = self.direction(came);
                let led_to = (before.iter())
                    .filter_map(|id| Some(self.rooms.get(id)?.exits.get(&way)?.to))
                    .filter(|id| self.rooms.contains_key(id) && named(*id));
                preferring(led_to.collect(), like)
            }
            None if key.is_none() && exits.is_none() => return self.whereabouts(),
            None => {
                let staying: Vec<RoomId> = before.iter().copied().filter(|id| like(*id)).collect();
                if staying.is_empty() {
                    let next = (before.iter())
                        .filter_map(|id| self.rooms.get(id))
                        .flat_map(|room| room.exits.values().map(|exit| exit.to))
                        .filter(|id| like(*id));
                    next.collect()
                } else {
                    staying
                }
            }
        };
        found.sort_unstable();
        found.dedup();
        if found.is_empty()
            && let Some(name) = seen.name
        {
            found = self.locate(name, seen.exits);
        }
        self.settle(found)
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
        self.whereabouts()
    }

    /// Exits as the map keeps them, however they were said.
    fn exit_set(&self, exits: &[String]) -> BTreeSet<String> {
        (exits.iter())
            .map(|exit| self.direction(exit))
            .filter(|exit| !exit.is_empty())
            .collect()
    }

    /// Whether a room has just these exits.
    fn has_exits(&self, id: RoomId, exits: &BTreeSet<String>) -> bool {
        self.rooms.get(&id).is_some_and(|room| {
            room.exits.len() == exits.len()
                && exits.iter().all(|exit| room.exits.contains_key(exit))
        })
    }
}

/// Those of `ids` that are `better`, if any are; else all of them.
fn preferring(ids: Vec<RoomId>, better: impl Fn(RoomId) -> bool) -> Vec<RoomId> {
    let best: Vec<RoomId> = ids.iter().copied().filter(|id| better(*id)).collect();
    if best.is_empty() { ids } else { best }
}
