//! The map in words: where the player is, what is around, the way to a
//! place, the names given to rooms and the areas; and maps imported from
//! elsewhere. All is said, so the map serves whoever does not see it.

use super::*;
use crate::mapfetch::{self, Imported};
use muddie_mapper::Step;
use std::collections::HashSet;
use std::sync::mpsc::TryRecvError;
use walking::MAX_LISTED_ROOMS;

mod imports;

pub(super) use imports::ImportWhy;

/// How many moves away `/map near` looks at most.
const FARTHEST: usize = 20;

impl Tab {
    /// Puts the player in a room the user names: by number, mark or name;
    /// with none, lists the rooms the map is unsure between, to choose, or
    /// says where the player is.
    pub(super) fn here(&mut self, argument: &str) {
        if !argument.trim().is_empty() {
            let id = match self.room_meant(argument) {
                Ok(id) => id,
                Err(why) => return self.refuse(&why),
            };
            self.map.borrow_mut().set_current(id);
            self.arrived_in(id);
            return self.say_where();
        }
        let rooms: Vec<(String, String, String)> = {
            let map = self.map.borrow();
            (map.candidates().iter().take(MAX_LISTED_ROOMS))
                .filter_map(|id| map.room(*id))
                .map(|room| {
                    (
                        numbered(&map, room),
                        describe(&map, room),
                        format!("/map here {}", room.id),
                    )
                })
                .collect()
        };
        if rooms.is_empty() {
            return self.say_where();
        }
        let several = self.map.borrow().candidates().len();
        self.note(&trn(
            "You are in one of {0} rooms alike:",
            "You are in one of {0} rooms alike:",
            several as u64,
        ));
        for (text, about, command) in rooms {
            self.offer(&text, &about, &command);
        }
    }

    /// Says the way to a place, without going.
    pub(super) fn say_route(&mut self, place: &str) {
        let said = {
            let map = self.map.borrow();
            let rooms: HashSet<RoomId> = map.rooms_meant(place).into_iter().collect();
            let from = map.current().map(|room| room.id);
            match (rooms.is_empty(), from) {
                (true, _) => Err(tr("No room matches.").to_owned()),
                (false, None) => Err(String::new()),
                (false, Some(from)) => match map.route_to_nearest(from, &rooms) {
                    None => Err(tr("No known way to that place.").to_owned()),
                    Some((to, steps)) => {
                        let name = map
                            .room(to)
                            .map_or_else(String::new, |room| place_words(&map, room));
                        Ok(if steps.is_empty() {
                            trf("You are already in {0}.", &[&name])
                        } else {
                            let moves = trn("{0} move", "{0} moves", steps.len() as u64);
                            let mut said =
                                trf("{0}, {1}: {2}.", &[&name, &moves, &in_runs(&steps)]);
                            let regions = map.regions_along(from, &steps);
                            if !regions.is_empty() {
                                said.push(' ');
                                said.push_str(&through(&regions));
                            }
                            if rooms.len() > 1 {
                                said.push(' ');
                                said.push_str(&nearest_of(rooms.len()));
                            }
                            said
                        })
                    }
                },
            }
        };
        match said {
            Ok(said) => self.note(&said),
            Err(why) if why.is_empty() => {
                let said = self.unknown_whereabouts();
                self.refuse(&said);
            }
            Err(why) => self.refuse(&why),
        }
    }

    /// Says where each exit of the room the player is in leads, each to
    /// be gone through by choosing it.
    pub(super) fn say_exits(&mut self) {
        let exits: Option<Vec<(String, String, String)>> = {
            let map = self.map.borrow();
            map.current().map(|room| {
                (ordered_exits(room).into_iter())
                    .map(|way| {
                        let exit = &room.exits[way];
                        let mut leads = match map.room(exit.to) {
                            Some(to) => place_words(&map, to),
                            None => tr("not explored").to_owned(),
                        };
                        if exit.blocked {
                            leads = trf("{0} (kept off routes)", &[&leads]);
                        }
                        if let Some(door) = exit.door {
                            let door = crate::mapview::door_words(&map, way, door);
                            leads = trf("{0}; door: {1}", &[&leads, &door]);
                        }
                        let word = direction_words(&map, way);
                        (word, leads, format!("/map through {way}"))
                    })
                    .collect()
            })
        };
        match exits {
            None => {
                let said = self.unknown_whereabouts();
                self.refuse(&said);
            }
            Some(exits) if exits.is_empty() => self.note(tr("This room has no exits on the map.")),
            Some(exits) => {
                for (way, leads, command) in exits {
                    self.offer(&way, &leads, &command);
                }
            }
        }
    }

    /// Lists what is near, as exploring the map says it, each with the
    /// way there.
    pub(super) fn say_near(&mut self, argument: &str) {
        let moves = (argument.parse::<usize>())
            .unwrap_or(crate::mapview::NEAR)
            .clamp(1, FARTHEST);
        let found: Option<Vec<(String, String, String)>> = {
            let map = self.map.borrow();
            map.current().map(|here| {
                (crate::mapview::near(&map, here.id, moves).into_iter())
                    .map(|(id, what, steps)| {
                        let moves = trn("{0} move", "{0} moves", steps.len() as u64);
                        let way = trf("{0}: {1}", &[&moves, &in_runs(&steps)]);
                        (what, way, format!("/map go {id}"))
                    })
                    .collect()
            })
        };
        match found {
            None => {
                let said = self.unknown_whereabouts();
                self.refuse(&said);
            }
            Some(found) if found.is_empty() => {
                self.note(tr("Nothing else is that near on the map."))
            }
            Some(found) => {
                let more = found.len().saturating_sub(MAX_LISTED_ROOMS);
                for (name, way, command) in found.into_iter().take(MAX_LISTED_ROOMS) {
                    self.offer(&name, &way, &command);
                }
                self.say_more(more);
            }
        }
    }

    /// Says all the map knows of a room: the one given, or the player's.
    pub(super) fn say_info(&mut self, argument: &str) {
        match self.room_meant(argument) {
            Ok(room) => self.say_room_info(room),
            Err(why) => self.refuse(&why),
        }
    }

    /// Says all the map knows of a room.
    pub(super) fn say_room_info(&mut self, room: RoomId) {
        let said = {
            let map = self.map.borrow();
            map.room(room).map(|room| info_of(&map, room))
        };
        match said {
            Some(said) => self.note(&said),
            None => self.refuse(tr("There is no room of that number.")),
        }
    }

    /// The room a command means: by number, mark, name, or area and name,
    /// the nearest of several; with nothing given, the player's. Else why
    /// there is none.
    pub(super) fn room_meant(&self, argument: &str) -> Result<RoomId, String> {
        let map = self.map.borrow();
        let found = match argument.trim() {
            "" => map.current().map(|room| room.id),
            place => map.room_meant(place),
        };
        drop(map);
        match found {
            Some(id) => Ok(id),
            None if argument.trim().is_empty() => Err(self.unknown_whereabouts()),
            None => Err(tr("No room matches.").to_owned()),
        }
    }

    /// Writes a note on the room the player is in, or, with none, takes
    /// it off.
    pub(super) fn note_here(&mut self, note: &str) {
        let here = self.map.borrow().current().map(|room| room.id);
        let Some(here) = here else {
            let said = self.unknown_whereabouts();
            return self.refuse(&said);
        };
        self.write_note(here, note);
    }

    /// Writes a note on a room, or, empty, takes it off, and says so.
    pub(super) fn write_note(&mut self, room: RoomId, note: &str) {
        let saved = self.map.borrow_mut().set_note(room, note);
        if self.map_saved(saved).is_some() {
            self.note(&match note.trim() {
                "" => tr("The note was taken off.").to_owned(),
                note => trf("Noted: {0}", &[note]),
            });
        }
    }

    /// Marks the room the player is in, to go to it by its mark.
    pub(super) fn mark_here(&mut self, name: &str) {
        let here = self.map.borrow().current().map(|room| room.id);
        let Some(here) = here else {
            let said = self.unknown_whereabouts();
            return self.refuse(&said);
        };
        self.mark_room(here, name);
    }

    /// Marks a room, and says so.
    pub(super) fn mark_room(&mut self, room: RoomId, name: &str) {
        let saved = self.map.borrow_mut().mark(name, room);
        if self.map_saved(saved).is_some() {
            self.note(&trf(
                "This room is now marked {0}: typed in the palette, {0} goes to it.",
                &[name],
            ));
        }
    }

    /// What a change of the map gave, if it was saved; if it could not
    /// be, that is said, the one way it is for every change.
    pub(super) fn map_saved<T>(&mut self, result: Result<T, muddie_mapper::Error>) -> Option<T> {
        match result {
            Ok(done) => Some(done),
            Err(error) => {
                self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()]));
                None
            }
        }
    }

    /// Keeps routes out of a room, or lets them through it again, and
    /// says so.
    pub(super) fn avoid_room(&mut self, room: RoomId, avoid: bool) {
        let saved = self.map.borrow_mut().set_avoid(room, avoid);
        if self.map_saved(saved).is_none() {
            return;
        }
        let name = room_words(&self.map.borrow(), room);
        match avoid {
            true => self.note(&trf("{0} is kept off routes.", &[&name])),
            false => self.note(&trf("Routes may pass through {0} again.", &[&name])),
        }
    }

    /// Forgets a mark given to a room.
    pub(super) fn unmark(&mut self, name: &str) {
        let forgotten = self.map.borrow_mut().unmark(name);
        match self.map_saved(forgotten) {
            Some(true) => self.note(&trf("No room is marked {0} now.", &[name])),
            Some(false) => self.refuse(&trf("No room is marked {0}.", &[name])),
            None => {}
        }
    }

    /// Lists the areas, with how many rooms each has, to show one.
    pub(super) fn list_areas(&mut self) {
        let areas: Vec<(String, usize)> = {
            let map = self.map.borrow();
            let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
            for room in map.rooms() {
                *counts.entry(room.area.as_str()).or_default() += 1;
            }
            (counts.into_iter())
                .filter(|(area, _)| !area.is_empty())
                .map(|(area, count)| (area.to_owned(), count))
                .collect()
        };
        if areas.is_empty() {
            return self.refuse(tr("The map has no areas yet."));
        }
        for (area, count) in areas {
            let rooms = trn("{0} room", "{0} rooms", count as u64);
            self.offer(&area, &rooms, &format!("/map show-area {area}"));
        }
    }
}

impl Tab {
    /// The places of the map that have `query`, for the palette: the rooms
    /// named, then those whose note or name has it, each name once, at its
    /// nearest room, the nearest first, with where each is and how far.
    pub(super) fn palette_places(&self, query: &str) -> Vec<commands::Item> {
        if query.trim().chars().count() < 2 {
            return Vec::new();
        }
        let map = self.map.borrow();
        let here = map.current().map(|here| here.id);
        let found = map.search(query, here);
        let (marks, rest): (Vec<_>, Vec<_>) = (found.into_iter())
            .partition(|found| matches!(found.by, muddie_mapper::FoundBy::Mark(_)));
        (marks
            .into_iter()
            .chain(rest.into_iter().take(MAX_LISTED_ROOMS)))
        .map(|found| {
            let (title, about) = crate::mapview::found_words(&map, here, &found);
            // By its number, so that what is done is done to the room
            // shown, of several of a name.
            let place = found.room.to_string();
            commands::Item {
                insert: String::new(),
                usage: title,
                group: tr("Places").to_owned(),
                about,
                act: Some(commands::Act::Place(place)),
            }
        })
        .collect()
    }
}

pub(super) use crate::mapview::{
    direction_words, numbered, ordered_exits, place_words, room_facts, room_words, sentences,
};

/// That a room is the nearest of several that a place may mean.
pub(super) fn nearest_of(rooms: usize) -> String {
    trn(
        "The nearest of {0} rooms that match.",
        "The nearest of {0} rooms that match.",
        rooms as u64,
    )
}

/// The regions a route goes through, said.
pub(super) fn through(regions: &[String]) -> String {
    trf("Through {0}.", &[&regions.join(", ")])
}

/// All the map knows of a room: its name and number, where it is and its
/// exits, how far it is and which way, what is so of it, and whether it
/// is left over or costs more to enter.
fn info_of(map: &muddie_mapper::Map, room: &muddie_mapper::Room) -> String {
    let mut said = vec![numbered(map, room), describe(map, room)];
    // Another room than the player's: how far, and which way.
    let here = map.current().map(|here| here.id);
    if here.is_some_and(|here| here != room.id) {
        said.push(crate::mapview::how_far(map, here, room));
    }
    said.extend(room_facts(map, room));
    if let Some(copied) = map.ghost_of(room.id) {
        said.push(trf(
            "It is left over: a copy of #{0}, in its place, that no other room leads to. It is not drawn, nor found.",
            &[&copied.to_string()],
        ));
    }
    if room.cost > 0 {
        said.push(trf("Costs {0} more to enter.", &[&room.cost.to_string()]));
    }
    if !room.symbol.trim().is_empty() {
        said.push(trf("Its symbol: {0}", &[room.symbol.trim()]));
    }
    sentences(said)
}

/// What tells a room from others of its name: where it is and its exits.
fn describe(map: &muddie_mapper::Map, room: &muddie_mapper::Room) -> String {
    let place = format!("{}, {}, {}", room.pos.x, room.pos.y, room.pos.z);
    sentences([
        trf("In {0}, at {1}.", &[&room.area, &place]),
        crate::mapview::exits_words(map, room),
    ])
}

/// A route as it is said: each command, and one repeated as many times
/// as it is (`3 north, east`).
fn in_runs(steps: &[Step]) -> String {
    let mut runs: Vec<(usize, &str)> = Vec::new();
    for step in steps {
        match runs.last_mut() {
            Some((count, command)) if *command == step.command => *count += 1,
            _ => runs.push((1, &step.command)),
        }
    }
    (runs.into_iter())
        .map(|(count, command)| match count {
            1 => command.to_owned(),
            count => trf("{0} {1}", &[&count.to_string(), command]),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
