//! Changing rooms and exits by hand: moving, weighing, avoiding, linking and
//! removing them.

use super::*;

impl Map {
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

    pub(super) fn edit_room(
        &mut self,
        id: RoomId,
        edit: impl FnOnce(&mut Room),
    ) -> Result<bool, Error> {
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
}
