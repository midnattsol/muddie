//! Rooms left over in a map: copies of a room, in its very place and of
//! its name, that no room of the rest leads to, as maps made by hand gather
//! when a room is made again instead of fixed. They are kept, but neither
//! drawn nor taken for where the player is nor found: should a way to one
//! be made, it is one of the rest again.

use super::*;

/// The rooms left over, each with the room it is a copy of.
pub(crate) type Ghosts = HashMap<RoomId, RoomId>;

impl Map {
    /// The room `id` is a copy of, if it is one left over.
    pub fn ghost_of(&self, id: RoomId) -> Option<RoomId> {
        self.with_ghosts(|ghosts| ghosts.get(&id).copied())
    }

    /// Whether `id` is a room left over.
    pub fn is_ghost(&self, id: RoomId) -> bool {
        self.ghost_of(id).is_some()
    }

    /// Calls `use_them` with the rooms left over, worked out again when
    /// the rooms have changed since.
    pub(crate) fn with_ghosts<T>(&self, use_them: impl FnOnce(&Ghosts) -> T) -> T {
        let mut ghosts = self.ghosts.borrow_mut();
        if ghosts
            .as_ref()
            .is_none_or(|(revision, _)| *revision != self.revision)
        {
            *ghosts = Some((self.revision, self.find_ghosts()));
        }
        let (_, found) = ghosts.as_ref().expect("made above");
        use_them(found)
    }

    /// Works out the rooms left over. Those that share their place and
    /// name with another are suspect; of them, those any other room leads
    /// to are not left over, nor those that one of these leads to, and so
    /// on. Of a place where none is led to, the one with most exits stays.
    fn find_ghosts(&self) -> Ghosts {
        // By place first, which costs nothing to compare; then, of the
        // places shared, by name.
        let mut placed: HashMap<(&str, Pos), Vec<RoomId>> = HashMap::new();
        for room in self.rooms.values() {
            (placed.entry((room.area.as_str(), room.pos)).or_default()).push(room.id);
        }
        let mut alike: HashMap<(&str, Pos, String), Vec<RoomId>> = HashMap::new();
        for ((area, pos), ids) in placed.into_iter().filter(|(_, ids)| ids.len() > 1) {
            for id in ids {
                let name = self.rooms.get(&id).map(|room| self.name_key(&room.name));
                alike
                    .entry((area, pos, name.unwrap_or_default()))
                    .or_default()
                    .push(id);
            }
        }
        alike.retain(|_, ids| ids.len() > 1);
        if alike.is_empty() {
            return Ghosts::new();
        }
        let mut suspect: HashSet<RoomId> = alike.values().flatten().copied().collect();
        // Those led to from a room not suspect are not left over; then
        // those led to from these, and so on.
        let mut cleared: Vec<RoomId> = (self.rooms.values())
            .filter(|room| !suspect.contains(&room.id))
            .flat_map(|room| room.exits.values().map(|exit| exit.to))
            .filter(|id| suspect.contains(id))
            .collect();
        while let Some(id) = cleared.pop() {
            if suspect.remove(&id)
                && let Some(room) = self.rooms.get(&id)
            {
                cleared.extend(
                    room.exits
                        .values()
                        .map(|exit| exit.to)
                        .filter(|to| suspect.contains(to)),
                );
            }
        }
        let mut ghosts = Ghosts::new();
        for ids in alike.values_mut() {
            ids.sort_unstable();
            let exits = |id: &RoomId| self.rooms.get(id).map_or(0, |room| room.exits.len());
            // The one that stays in the place: one not left over, else
            // the one with most exits.
            let stays = (ids.iter().copied())
                .find(|id| !suspect.contains(id))
                .or_else(|| {
                    ids.iter()
                        .copied()
                        .max_by_key(|id| (exits(id), std::cmp::Reverse(*id)))
                });
            let Some(stays) = stays else {
                continue;
            };
            for id in ids
                .iter()
                .copied()
                .filter(|id| *id != stays && suspect.contains(id))
            {
                ghosts.insert(id, stays);
            }
        }
        ghosts
    }
}
