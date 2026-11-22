//! What the map changed of itself, as the player went: rooms added, and
//! what it had wrong put right (see healing). Each is kept as the rooms
//! were before it, to be undone, the last first.

use super::*;

/// Something the map changed of itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Changed {
    /// It made a room where the player went, of this name.
    Made(RoomId, String),
    /// It had the exit taken lead to a room it had, of this name.
    Found(RoomId, String),
    /// It gave a room the name the server shows, as the room was shown
    /// where the map has it, between rooms that are as it has them.
    Renamed {
        room: RoomId,
        was: String,
        now: String,
    },
    /// It gave a room exits the server shows that it lacked, leading to
    /// rooms not explored.
    ExitsAdded { room: RoomId, exits: Vec<String> },
    /// It had an exit lead to the room the player was found in by taking
    /// it, rather than the one it had.
    Rerouted {
        room: RoomId,
        way: String,
        was: RoomId,
        now: RoomId,
    },
}

impl Changed {
    /// The room it is about.
    pub fn room(&self) -> RoomId {
        match self {
            Changed::Made(id, _) | Changed::Found(id, _) => *id,
            Changed::Renamed { room, .. }
            | Changed::ExitsAdded { room, .. }
            | Changed::Rerouted { room, .. } => *room,
        }
    }
}

/// A change, with the rooms it changed as they were: `None` for one it
/// made.
pub(crate) struct Entry {
    changed: Changed,
    before: Vec<(RoomId, Option<Room>)>,
}

/// How many changes can be undone, the last first.
const KEPT: usize = 100;

impl Map {
    /// Keeps a change, with the rooms it changed as they were.
    pub(crate) fn record(&mut self, changed: Changed, before: Vec<(RoomId, Option<Room>)>) {
        if self.changes.len() == KEPT {
            self.changes.remove(0);
        }
        self.changes.push(Entry { changed, before });
    }

    /// The last change the map made of itself that can be undone.
    pub fn last_change(&self) -> Option<&Changed> {
        self.changes.last().map(|entry| &entry.changed)
    }

    /// Undoes the last change the map made of itself: the rooms it changed
    /// are as they were, what was changed of them since too, and a room it
    /// made is taken away. Returns what was undone, if anything was.
    pub fn undo_change(&mut self) -> Result<Option<Changed>, Error> {
        let Some(entry) = self.changes.pop() else {
            return Ok(None);
        };
        let mut removed = Vec::new();
        for (id, before) in entry.before {
            match before {
                Some(room) => {
                    self.rooms.insert(id, room);
                    self.keep([id]);
                }
                None => {
                    self.rooms.remove(&id);
                    self.unsaved.remove(&id);
                    removed.push(id);
                }
            }
        }
        // In a room no longer there, the map does not know where the
        // player is until the server shows a room.
        if self.current.is_some_and(|id| !self.rooms.contains_key(&id)) {
            self.current = None;
            self.candidates.clear();
        }
        self.revision += 1;
        if let Some(store) = &mut self.store {
            for id in removed {
                store.delete(id)?;
            }
        }
        Ok(Some(entry.changed))
    }

    /// What the map changed of itself since last asked, to be told.
    pub fn take_told(&mut self) -> Vec<Changed> {
        std::mem::take(&mut self.told)
    }
}
