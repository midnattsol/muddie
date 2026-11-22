//! Looking over what the map added of itself, and what may be wrong in
//! it: the rooms added, to go to or take off, undoing the last, and the
//! rooms that look wrong. All is said, and chosen from a list.

use super::*;
use muddie_mapper::Added;
use std::collections::HashSet;
use walking::MAX_LISTED_ROOMS;

impl Tab {
    /// Undoes the last room the map added, and says what was undone.
    pub(super) fn undo_added(&mut self) {
        let undone = self.map.borrow_mut().undo_added();
        match self.map_saved(undone) {
            None => {}
            Some(None) => self.refuse(tr("The map has added no room to undo.")),
            Some(Some((Added::Made(_), name))) => {
                self.note(&trf("Undone: {0} is no longer on the map.", &[&name]))
            }
            Some(Some((Added::Found(_), name))) => self.note(&trf(
                "Undone: the way to {0} is no longer on the map.",
                &[&name],
            )),
        }
    }

    /// Lists the rooms the map added of itself, each to go to.
    pub(super) fn list_added(&mut self) {
        let added: Vec<(String, String, RoomId)> = {
            let map = self.map.borrow();
            (map.added().into_iter())
                .map(|room| {
                    (
                        crate::mapview::numbered(&map, room),
                        room.area.clone(),
                        room.id,
                    )
                })
                .collect()
        };
        if added.is_empty() {
            return self.refuse(tr("The map has added no room."));
        }
        let more = added.len().saturating_sub(MAX_LISTED_ROOMS);
        for (name, area, id) in added.into_iter().take(MAX_LISTED_ROOMS) {
            self.offer(&name, &area, &format!("/map travel {id}"));
        }
        self.say_more(more);
    }

    /// Lists the rooms that look wrong: those with no name, and those no
    /// exit leads to or from, which nothing can reach. Each is chosen to
    /// hear all the map knows of it.
    pub(super) fn check_map(&mut self) {
        let found: Vec<(String, String, RoomId)> = {
            let map = self.map.borrow();
            let reached: HashSet<RoomId> = (map.rooms())
                .flat_map(|room| room.exits.values().map(|exit| exit.to))
                .collect();
            let mut found: Vec<(String, String, RoomId)> = (map.rooms())
                .filter_map(|room| {
                    let why = if room.name.trim().is_empty() {
                        tr("no name")
                    } else if room.exits.is_empty() && !reached.contains(&room.id) {
                        tr("no exit leads to it or from it")
                    } else {
                        return None;
                    };
                    let said = format!("{}, {}", crate::mapview::numbered(&map, room), room.area);
                    Some((said, why.to_owned(), room.id))
                })
                .collect();
            found.sort_by(|a, b| (&a.1, &a.0).cmp(&(&b.1, &b.0)));
            found
        };
        if found.is_empty() {
            return self.note(tr("Nothing on the map looks wrong."));
        }
        let more = found.len().saturating_sub(MAX_LISTED_ROOMS);
        for (room, why, id) in found.into_iter().take(MAX_LISTED_ROOMS) {
            self.offer(&room, &why, &format!("/map info {id}"));
        }
        self.say_more(more);
    }

    /// Takes a room off the map, by number.
    pub(super) fn remove_room_command(&mut self, argument: &str) {
        let Ok(id) = argument.trim().parse::<RoomId>() else {
            return self.refuse(tr("Give the number of the room: /map remove <number>."));
        };
        let name = {
            let map = self.map.borrow();
            map.room(id)
                .map(|room| crate::mapview::numbered(&map, room).trim().to_owned())
        };
        let Some(name) = name else {
            return self.refuse(tr("There is no room of that number."));
        };
        let removed = self.map.borrow_mut().remove_room(id);
        if self.map_saved(removed).is_some() {
            self.note(&trf("{0} is no longer on the map.", &[&name]));
        }
    }

    /// Says how many more there were than were listed, if any.
    fn say_more(&mut self, more: usize) {
        if more > 0 {
            self.note(&trn("And {0} more.", "And {0} more.", more as u64));
        }
    }
}
