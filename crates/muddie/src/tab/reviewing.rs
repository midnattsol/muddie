//! Looking over what the map changed of itself, and what may be wrong in
//! it: what it changed, told as it goes and undone the last first; the
//! rooms it added, to go to or take off; and the rooms that look wrong.
//! All is said, and chosen from a list.

use super::*;
use crate::mapview::{direction_words, room_words};
use muddie_mapper::{Changed, Map, UNSHOWN_AFTER};
use std::collections::HashSet;
use std::time::Duration;

use walking::MAX_LISTED_ROOMS;

/// How long after the map changes something of itself it is told, with
/// whatever else it changed meanwhile.
const TELL_CHANGES_AFTER: Duration = Duration::from_secs(3);

/// Exits, in the words they are walked by.
fn ways_said(map: &Map, ways: &[String]) -> String {
    let said: Vec<String> = ways.iter().map(|way| direction_words(map, way)).collect();
    said.join(", ")
}

/// Several changes of the map's own, counted by kind.
fn counted(changes: &[Changed]) -> String {
    let count = |kind: fn(&Changed) -> u64| changes.iter().map(kind).sum::<u64>();
    let made = count(|change| u64::from(matches!(change, Changed::Made(..))));
    let found = count(|change| u64::from(matches!(change, Changed::Found(..))));
    let renamed = count(|change| u64::from(matches!(change, Changed::Renamed { .. })));
    let exits = count(|change| match change {
        Changed::ExitsAdded { exits, .. } => exits.len() as u64,
        _ => 0,
    });
    let rerouted = count(|change| u64::from(matches!(change, Changed::Rerouted { .. })));
    let parts: Vec<String> = [
        (made, trn("{0} room added", "{0} rooms added", made)),
        (
            found,
            trn("{0} exit connected", "{0} exits connected", found),
        ),
        (
            renamed,
            trn("{0} name put right", "{0} names put right", renamed),
        ),
        (exits, trn("{0} exit added", "{0} exits added", exits)),
        (
            rerouted,
            trn("{0} exit put right", "{0} exits put right", rerouted),
        ),
    ]
    .into_iter()
    .filter(|(number, _)| *number > 0)
    .map(|(_, part)| part)
    .collect();
    parts.join(", ")
}

impl Tab {
    /// Undoes the last change the map made of itself, and says what was
    /// undone.
    pub(super) fn undo_change(&mut self) {
        let undone = self.map.borrow_mut().undo_change();
        let Some(undone) = self.map_saved(undone) else {
            return;
        };
        let Some(undone) = undone else {
            return self.refuse(tr("The map has changed nothing of itself to undo."));
        };
        let said = {
            let map = self.map.borrow();
            let room = |id| room_words(&map, id);
            match undone {
                Changed::Made(_, name) => trf("Undone: {0} is no longer on the map.", &[&name]),
                Changed::Found(_, name) => {
                    trf("Undone: the exit to {0} is no longer on the map.", &[&name])
                }
                Changed::Renamed { was, now, .. } if was.is_empty() => {
                    trf("Undone: “{0}” has no name again.", &[&now])
                }
                Changed::Renamed { was, now, .. } => {
                    trf("Undone: “{0}” is called “{1}” again.", &[&now, &was])
                }
                Changed::ExitsAdded { room: id, exits } => trn(
                    "Undone: “{1}” no longer has the exit {2}.",
                    "Undone: “{1}” no longer has the exits {2}.",
                    exits.len() as u64,
                )
                .replace("{1}", &room(id))
                .replace("{2}", &ways_said(&map, &exits)),
                Changed::Rerouted {
                    room: id, way, was, ..
                } => trf(
                    "Undone: {0} from “{1}” leads to “{2}” again.",
                    &[&direction_words(&map, &way), &room(id), &room(was)],
                ),
            }
        };
        self.note(&said);
    }

    /// Tells what the map changed of itself, a moment after the first of
    /// it, all together: as the player walks, one message and not many.
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
        let said = {
            let map = self.map.borrow();
            let room = |id| room_words(&map, id);
            match changes.as_slice() {
                [Changed::Made(_, name)] => trf("Map: added the room “{0}”.", &[name]),
                [Changed::Found(_, name)] => trf("Map: connected an exit to “{0}”.", &[name]),
                [Changed::Renamed { was, now, .. }] if was.is_empty() => trf(
                    "Map put right: a room with no name is now called “{0}”.",
                    &[now],
                ),
                [Changed::Renamed { was, now, .. }] => {
                    trf("Map put right: “{0}” is now called “{1}”.", &[was, now])
                }
                [Changed::ExitsAdded { room: id, exits }] => trn(
                    "Map put right: “{1}” has the exit {2} now.",
                    "Map put right: “{1}” has the exits {2} now.",
                    exits.len() as u64,
                )
                .replace("{1}", &room(*id))
                .replace("{2}", &ways_said(&map, exits)),
                [
                    Changed::Rerouted {
                        room: id, way, now, ..
                    },
                ] => trf(
                    "Map put right: {0} from “{1}” leads to “{2}” now.",
                    &[&direction_words(&map, way), &room(*id), &room(*now)],
                ),
                _ => trf("Map: {0}.", &[&counted(&changes)]),
            }
        };
        self.note(&trf(
            "{0} To undo the last: Undo the map's last change, in the palette.",
            &[&said],
        ));
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
            self.offer(&name, &area, &format!("/map go {id}"));
        }
        self.say_more(more);
    }

    /// Lists the rooms that look wrong: those with no name, those no exit
    /// leads to or from, which nothing can reach, and those with an exit
    /// the game has not shown the last times the player came. Each is chosen to
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
            // Exits the game has not shown, time after time: a door shut,
            // an exit hidden, or one the map has wrong; for the user to say.
            for (id, way, times) in map.unshown_exits(UNSHOWN_AFTER) {
                let Some(room) = map.room(id) else {
                    continue;
                };
                let said = format!("{}, {}", crate::mapview::numbered(&map, room), room.area);
                let why = trf(
                    "the game has not shown its exit {0} the last {1} times",
                    &[&direction_words(&map, &way), &times.to_string()],
                );
                found.push((said, why, id));
            }
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

    /// Takes a room off the map: by number, mark or name, which the
    /// command is not run without.
    pub(super) fn remove_room_command(&mut self, argument: &str) {
        let id = match self.room_meant(argument) {
            Ok(id) => id,
            Err(why) => return self.refuse(&why),
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
    pub(super) fn say_more(&mut self, more: usize) {
        if more > 0 {
            self.note(&trn("And {0} more.", "And {0} more.", more as u64));
        }
    }
}
