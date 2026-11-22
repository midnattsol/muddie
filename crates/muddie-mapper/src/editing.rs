//! Changing rooms and exits by hand: moving, weighing, avoiding, naming,
//! linking and removing them.

use super::*;

/// What taking a map in place of the one there was came to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Replaced {
    /// How many rooms the map has now.
    pub rooms: usize,
    /// How many of them it did not have; none if it had no rooms at all.
    pub new: usize,
    /// How many kept the name the user gave them.
    pub renamed: usize,
}

impl Map {
    /// Takes `rooms` in place of every room there is, all at once and kept
    /// in one go: a map brought from elsewhere, with the room the player
    /// was in, if it is among them.
    ///
    /// What the user set on rooms that are in both, the rooms to avoid,
    /// the names they gave, the exits of their own, and a colour and a
    /// note where the new rooms have none, carries over, and so do the
    /// marks: the same map brought again, newer, keeps them.
    pub fn replace_all(
        &mut self,
        rooms: Vec<Room>,
        current: Option<RoomId>,
    ) -> Result<Replaced, Error> {
        let mut rooms: HashMap<RoomId, Room> =
            rooms.into_iter().map(|room| (room.id, room)).collect();
        let mut replaced = Replaced {
            rooms: rooms.len(),
            ..Replaced::default()
        };
        replaced.new = match self.rooms.is_empty() {
            true => 0,
            false => (rooms.keys())
                .filter(|id| !self.rooms.contains_key(id))
                .count(),
        };
        for (id, old) in &self.rooms {
            let Some(room) = rooms.get_mut(id) else {
                continue;
            };
            room.avoid |= old.avoid;
            room.colour = room.colour.or(old.colour);
            if room.note.is_empty() {
                room.note.clone_from(&old.note);
            }
            if old.renamed {
                room.name.clone_from(&old.name);
                room.renamed = true;
                replaced.renamed += 1;
            }
            for (direction, exit) in old.exits.iter().filter(|(_, exit)| exit.custom) {
                room.exits
                    .entry(direction.clone())
                    .or_insert_with(|| exit.clone());
            }
        }
        self.rooms = rooms;
        self.current = current.filter(|id| self.rooms.contains_key(id));
        self.candidates.clear();
        self.unsaved.clear();
        self.revision += 1;
        // In order of number, as the file keeps them, to be written faster.
        let mut ordered: Vec<&Room> = self.rooms.values().collect();
        ordered.sort_unstable_by_key(|room| room.id);
        match &mut self.store {
            Some(store) => store.replace(ordered).map(|()| replaced),
            None => Ok(replaced),
        }
    }

    /// Gives a room a name of the user's in place of the one it has, which
    /// the server reporting the room, or the map taken again, then leaves
    /// as it is. Returns false if there is no such room.
    pub fn rename(&mut self, id: RoomId, name: &str) -> Result<bool, Error> {
        self.edit_room(id, |room| {
            room.name = name.trim().to_owned();
            room.renamed = true;
        })
    }

    /// Moves a room on the map grid. Returns false if there is no such room.
    pub fn move_room(&mut self, id: RoomId, pos: Pos) -> Result<bool, Error> {
        self.edit_room(id, |room| room.pos = pos)
    }

    /// Sets the cost of entering a room when finding routes (at least 1).
    pub fn set_weight(&mut self, id: RoomId, weight: u32) -> Result<bool, Error> {
        self.edit_room(id, |room| room.weight = weight.max(1))
    }

    /// Sets the colour a room is drawn in, or gives it back the map's own.
    pub fn set_colour(&mut self, id: RoomId, colour: Option<Colour>) -> Result<bool, Error> {
        self.edit_room(id, |room| room.colour = colour)
    }

    /// Writes a note on a room, or, empty, takes it away.
    pub fn set_note(&mut self, id: RoomId, note: &str) -> Result<bool, Error> {
        self.edit_room(id, |room| room.note = note.trim().to_owned())
    }

    /// Sets whether routes must keep out of a room.
    pub fn set_avoid(&mut self, id: RoomId, avoid: bool) -> Result<bool, Error> {
        self.edit_room(id, |room| room.avoid = avoid)
    }

    /// Adds an exit of the user's own, or changes where an exit leads.
    /// `direction` is what a walk sends to use it unless the exit is given
    /// a command, so it need not be a compass direction.
    pub fn set_exit(&mut self, from: RoomId, direction: &str, to: RoomId) -> Result<bool, Error> {
        let direction = self.direction(direction);
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
        let direction = self.direction(direction);
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
        let direction = self.direction(direction);
        self.edit_room(from, |room| {
            room.exits.remove(&direction);
        })
    }

    pub(super) fn edit_room(
        &mut self,
        id: RoomId,
        edit: impl FnOnce(&mut Room),
    ) -> Result<bool, Error> {
        let Some(mut room) = self.rooms.get(&id).cloned() else {
            return Ok(false);
        };
        edit(&mut room);
        self.keep([id]);
        self.rooms.insert(id, room);
        Ok(true)
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
        self.unsaved.remove(&id);
        self.revision += 1;
        match &mut self.store {
            Some(store) => store.delete(id).map(|()| true),
            None => Ok(true),
        }
    }
}
