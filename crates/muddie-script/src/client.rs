//! What the client tells the engine of itself, besides what the game
//! sends: the room the player is in, what was chosen from a list, what
//! was copied, and whether the game is read aloud.

use super::*;

impl Engine {
    /// Tells the scripts listening with `on("room")` that the player is in
    /// another room, with that room and the one before; nothing while the
    /// map does not know where the player is.
    pub fn watch_room(&mut self) {
        let now = self.map.borrow().current().map(|room| room.id);
        let before = {
            let mut state = self.state.borrow_mut();
            if now.is_none() || state.room_now == now {
                return;
            }
            std::mem::replace(&mut state.room_now, now)
        };
        let room = {
            let map = self.map.borrow();
            let Some(current) = map.current() else {
                return;
            };
            match crate::map_api::room_table(&self.lua, current) {
                Ok(room) => room,
                Err(error) => return self.error(error.to_string()),
            }
        };
        let callbacks = self.state.borrow().listeners(&event_key("room"));
        for (id, owner, callback) in callbacks {
            let still_on = self.state.borrow().runs(id);
            if still_on {
                self.as_owner(&owner, || self.call(&callback, (room.clone(), before)));
            }
        }
    }

    /// What was chosen from what [`Action::Choose`] offered under `id`:
    /// the place of the one chosen, from 0, and what it says; none if the
    /// list was closed without choosing.
    pub fn chosen(&mut self, id: u64, chosen: Option<(usize, &str)>) {
        let awaited = {
            let mut state = self.state.borrow_mut();
            let at = state.awaited.iter().position(|awaited| awaited.id == id);
            at.map(|at| state.awaited.remove(at))
        };
        let Some(awaited) = awaited else {
            return;
        };
        let (index, text) = match chosen {
            Some((index, text)) => (Some(index + 1), Some(text.to_owned())),
            None => (None, None),
        };
        self.as_owner(&awaited.owner, || {
            self.call(&awaited.callback, (index, text))
        });
    }

    /// Tells scripts that Muddie put `text` on the clipboard, for
    /// `clipboard.get()`.
    pub fn copied(&mut self, text: &str) {
        self.state.borrow_mut().copied = Some(text.to_owned());
    }

    /// Tells scripts whether the game is read aloud, as the player set it,
    /// for `session.reading`.
    pub fn set_reading(&mut self, reading: bool) {
        let mut state = self.state.borrow_mut();
        state.not_reading = !reading;
        state.reading_set_by = None;
    }
}
