//! Looking over what the map changed of itself, and what may be wrong in
//! it: what it changed, told as it goes and undone the last first; the
//! rooms it added, to go to or take off; and the rooms that look wrong.
//! All is said, and chosen from a list.

use super::*;
use muddie_mapper::Changed;
use std::collections::HashSet;
use std::time::Duration;

/// How long after the map changes something of itself it is told, with
/// whatever else it changed meanwhile.
const TELL_CHANGES_AFTER: Duration = Duration::from_secs(3);
use walking::MAX_LISTED_ROOMS;

impl Tab {
    /// Undoes the last change the map made of itself, and says what was
    /// undone.
    pub(super) fn undo_change(&mut self) {
        let undone = self.map.borrow_mut().undo_change();
        match self.map_saved(undone) {
            None => {}
            Some(None) => self.refuse(tr("The map has changed nothing of itself to undo.")),
            Some(Some(Changed::Made(_, name))) => {
                self.note(&trf("Undone: {0} is no longer on the map.", &[&name]))
            }
            Some(Some(Changed::Found(_, name))) => self.note(&trf(
                "Undone: the way to {0} is no longer on the map.",
                &[&name],
            )),
            Some(Some(Changed::Renamed { was, now, .. })) => self.note(&match was.is_empty() {
                true => trf("Undone: “{0}” has no name again.", &[&now]),
                false => trf("Undone: “{0}” is called “{1}” again.", &[&now, &was]),
            }),
        }
    }

    /// Tells what the map changed of itself, a moment after the first of
    /// it, all together: as the player walks, one notice and not many.
    pub(super) fn tell_map_changes(&mut self, ctx: &egui::Context, now: Instant) {
        let told = self.map.borrow_mut().take_told();
        if !told.is_empty() {
            self.map_told.1.get_or_insert(now);
            self.map_told.0.extend(told);
        }
        let Some(since) = self.map_told.1 else {
            return;
        };
        let wait = TELL_CHANGES_AFTER.saturating_sub(now.duration_since(since));
        if !wait.is_zero() {
            ctx.request_repaint_after(wait);
            return;
        }
        let changes = std::mem::take(&mut self.map_told).0;
        let said = match changes.as_slice() {
            [Changed::Made(_, name)] => trf("Map: added the room “{0}”.", &[name]),
            [Changed::Found(_, name)] => trf("Map: added the way to “{0}”.", &[name]),
            [Changed::Renamed { was, now, .. }] if was.is_empty() => trf(
                "Map put right: a room with no name is now called “{0}”.",
                &[now],
            ),
            [Changed::Renamed { was, now, .. }] => {
                trf("Map put right: “{0}” is now called “{1}”.", &[was, now])
            }
            _ => {
                let count =
                    |kind: fn(&Changed) -> bool| changes.iter().filter(|c| kind(c)).count() as u64;
                let made = count(|change| matches!(change, Changed::Made(..)));
                let found = count(|change| matches!(change, Changed::Found(..)));
                let renamed = count(|change| matches!(change, Changed::Renamed { .. }));
                let parts: Vec<String> = [
                    (made, trn("{0} room added", "{0} rooms added", made)),
                    (found, trn("{0} way added", "{0} ways added", found)),
                    (
                        renamed,
                        trn("{0} name put right", "{0} names put right", renamed),
                    ),
                ]
                .into_iter()
                .filter(|(number, _)| *number > 0)
                .map(|(_, part)| part)
                .collect();
                trf("Map: {0}.", &[&parts.join(", ")])
            }
        };
        self.note(&trf("{0} To undo the last: /map undo.", &[&said]));
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
