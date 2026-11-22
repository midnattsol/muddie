//! The map in words: where the player is, what is around, the way to a
//! place, the names given to rooms and the areas; and maps taken from
//! elsewhere. All is said, so the map serves whoever does not see it.

use super::*;
use crate::mapfetch::{self, Taken};
use muddie_mapper::Step;
use std::collections::HashSet;
use std::sync::mpsc::TryRecvError;
use walking::MAX_LISTED_ROOMS;

/// How many moves away `/map near` looks when not told, and at most.
const NEAR: usize = 3;
const FARTHEST: usize = 10;

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
                    None => Err(tr("No known route to that place.").to_owned()),
                    Some((to, steps)) => {
                        let name = map
                            .room(to)
                            .map_or_else(String::new, |room| place_words(&map, room));
                        Ok(if steps.is_empty() {
                            trf("You are already in {0}.", &[&name])
                        } else {
                            let moves = trn("{0} move", "{0} moves", steps.len() as u64);
                            let mut said = format!("{name}, {moves}: {}.", in_runs(&steps));
                            let regions = map.regions_along(from, &steps);
                            if !regions.is_empty() {
                                said.push(' ');
                                said.push_str(&through(&regions));
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
                            None => tr("somewhere not yet mapped").to_owned(),
                        };
                        if exit.locked {
                            leads = trf("{0} (routes do not use it)", &[&leads]);
                        }
                        let word = direction_words(&map, way);
                        (word, leads, format!("/map go {}", exit.to))
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

    /// Lists what is a few moves away: each name once, at the nearest
    /// room of it, with the way there.
    pub(super) fn say_near(&mut self, argument: &str) {
        let moves = argument.parse::<usize>().unwrap_or(NEAR).clamp(1, FARTHEST);
        let found: Option<Vec<(String, String, String)>> = {
            let map = self.map.borrow();
            map.current().map(|here| {
                let mut named = HashSet::from([place_words(&map, here).to_lowercase()]);
                (map.around(here.id, moves).into_iter())
                    .filter_map(|(id, steps)| {
                        let room = map.room(id)?;
                        let place = place_words(&map, room);
                        named.insert(place.to_lowercase()).then(|| {
                            let moves = trn("{0} move", "{0} moves", steps.len() as u64);
                            let way = format!("{moves}: {}", in_runs(&steps));
                            (place, way, format!("/map go {id}"))
                        })
                    })
                    .take(MAX_LISTED_ROOMS)
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
                for (name, way, command) in found {
                    self.offer(&name, &way, &command);
                }
            }
        }
    }

    /// Says all the map knows of a room: the one given, or the player's.
    pub(super) fn say_info(&mut self, argument: &str) {
        let said = self.room_meant(argument).map(|id| {
            let map = self.map.borrow();
            map.room(id)
                .map(|room| info_of(&map, room))
                .unwrap_or_default()
        });
        match said {
            Ok(said) => self.note(&said),
            Err(why) => self.refuse(&why),
        }
    }

    /// The room a command means: by number, mark, name, or area and name,
    /// the nearest of several; with nothing given, the player's. Else why
    /// there is none.
    pub(super) fn room_meant(&self, argument: &str) -> Result<RoomId, String> {
        let map = self.map.borrow();
        let found = match argument.trim() {
            "" => map.current().map(|room| room.id),
            place => map.rooms_meant(place).first().copied(),
        };
        drop(map);
        match found {
            Some(id) => Ok(id),
            None if argument.trim().is_empty() => Err(self.unknown_whereabouts()),
            None => Err(tr("No room matches.").to_owned()),
        }
    }

    /// Says aloud all the map knows of a room, as it is explored.
    pub(super) fn say_info_aloud(&mut self, room: RoomId) {
        let said = {
            let map = self.map.borrow();
            map.room(room).map(|room| info_of(&map, room))
        };
        if let Some(said) = said {
            self.voice.say_now(&said);
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
        if let Some(said) = self.write_note(here, note) {
            self.note(&said);
        }
    }

    /// Writes a note on a room, or, empty, takes it off. Returns what to
    /// say of it, or None if it could not be saved, which is said.
    pub(super) fn write_note(&mut self, room: RoomId, note: &str) -> Option<String> {
        let saved = self.map.borrow_mut().set_note(room, note);
        self.map_saved(saved)?;
        Some(match note.trim() {
            "" => tr("The note was taken off.").to_owned(),
            note => trf("Noted: {0}", &[note]),
        })
    }

    /// Names the room the player is in; with no name, lists the names
    /// given, each to go to by choosing it.
    pub(super) fn mark_here(&mut self, name: &str) {
        if name.is_empty() {
            let marks: Vec<(String, String)> = {
                let map = self.map.borrow();
                (map.marks().iter())
                    .map(|(name, id)| {
                        let room = (map.room(*id))
                            .map_or_else(|| format!("#{id}"), |room| place_words(&map, room));
                        (name.clone(), room)
                    })
                    .collect()
            };
            if marks.is_empty() {
                return self.refuse(tr(
                    "No room is marked yet: Mark…, in the palette, marks the room you are in.",
                ));
            }
            for (name, room) in marks {
                self.offer(&name, &room, &format!("/map go {name}"));
            }
            return;
        }
        let here = self.map.borrow().current().map(|room| room.id);
        let Some(here) = here else {
            let said = self.unknown_whereabouts();
            return self.refuse(&said);
        };
        if self.mark_room(here, name) {
            self.note(&trf(
                "This room is now marked {0}: typed in the palette, {0} goes to it.",
                &[name],
            ));
        }
    }

    /// Names a room. Returns whether it was saved; if not, says so.
    pub(super) fn mark_room(&mut self, room: RoomId, name: &str) -> bool {
        let saved = self.map.borrow_mut().mark(name, room);
        self.map_saved(saved).is_some()
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
        let name = {
            let map = self.map.borrow();
            map.room(room).map(|room| place_words(&map, room))
        };
        let name = name.unwrap_or_else(|| format!("#{room}"));
        match avoid {
            true => self.note(&trf("Routes will avoid {0}.", &[&name])),
            false => self.note(&trf("Routes may pass through {0} again.", &[&name])),
        }
    }

    /// Forgets a name given to a room.
    pub(super) fn unmark(&mut self, name: &str) {
        let forgotten = self.map.borrow_mut().unmark(name);
        match self.map_saved(forgotten) {
            Some(true) => self.note(&trf("No room is marked {0} now.", &[name])),
            Some(false) => self.refuse(&trf("No room is marked {0}.", &[name])),
            None => {}
        }
    }

    /// Lists the areas, with how many rooms each has, to go to one.
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
            self.offer(&area, &rooms, &format!("/map go {area}"));
        }
    }

    /// A script takes the map at `address`: at once, or, if this map came
    /// from there, only if it has changed since.
    pub(super) fn import_for_script(&mut self, address: String) {
        // Before connecting the session has no map of its own yet: one
        // only for now, which connecting replaces. It waits till then.
        if self.connection.is_none() {
            self.map_source.waiting = Some(address);
            return;
        }
        // A map read by an older reader is taken whole again, for what
        // the reader reads now that it did not.
        let known = {
            let map = self.map.borrow();
            let read_now = map.source_reader() >= Some(muddie_mapper::MUDLET_READER);
            (map.source())
                .filter(|(from, _)| *from == address && read_now)
                .map(|(_, version)| version.map(str::to_owned))
        };
        match known {
            Some(version) => self.take_map(address, version, Taking::Update),
            None => self.take_map(address, None, Taking::Script),
        }
    }

    /// Takes a map made with Mudlet in place of this one, from the file or
    /// address given.
    pub(super) fn import_map(&mut self, source: &str) {
        if source.is_empty() {
            return self.refuse(tr(
                "Say which map made with Mudlet to take: a file or an address.",
            ));
        }
        self.take_map(source.to_owned(), None, Taking::Asked);
    }

    /// Starts taking a map, away from the screen; from the web, only if
    /// newer than the version `known`.
    fn take_map(&mut self, source: String, known: Option<String>, why: Taking) {
        if self.map_source.taking.is_some() {
            if why == Taking::Asked {
                self.refuse(tr("A map is being taken already."));
            }
            return;
        }
        match why {
            Taking::Update => self.say(&trf("Looking for a newer map at {0}…", &[&source]), false),
            _ => self.say(&trf("Taking the map from {0}…", &[&source]), false),
        }
        let result = mapfetch::take(source.clone(), known);
        self.map_source.taking = Some((result, source, why));
    }

    /// Puts in place a map that has been taken, once it is here.
    pub(super) fn check_map_taken(&mut self, ctx: &egui::Context) {
        let Some((result, _, _)) = &self.map_source.taking else {
            return;
        };
        let taken = match result.try_recv() {
            Err(TryRecvError::Empty) => {
                return ctx.request_repaint_after(Duration::from_millis(250));
            }
            Err(TryRecvError::Disconnected) => Err(tr("it stopped half way").to_owned()),
            Ok(taken) => taken,
        };
        let Some((_, source, why)) = self.map_source.taking.take() else {
            return;
        };
        match taken {
            Ok(Taken::Unchanged) => self.say(tr("The map is up to date already."), false),
            Ok(Taken::Map {
                map: taken,
                version,
            }) => {
                let saved = {
                    let mut map = self.map.borrow_mut();
                    // Where the player is stays, if the room is still there.
                    let here = map.current().map(|room| room.id);
                    let from_web = mapfetch::on_the_web(&source).then_some(source.as_str());
                    let reader = from_web.map(|_| muddie_mapper::MUDLET_READER);
                    (map.replace_all(taken.rooms, here)).and_then(|replaced| {
                        map.set_source(from_web.map(|from| (from, version.as_deref())))?;
                        map.set_source_reader(reader)?;
                        Ok(replaced)
                    })
                };
                match self.map_saved(saved) {
                    None => {}
                    Some(replaced) if why == Taking::Update => {
                        self.say(&brought_up_to_date(replaced), false)
                    }
                    Some(replaced) => {
                        let rooms = trn("{0} room", "{0} rooms", replaced.rooms as u64);
                        let mut said = vec![trf("The map was taken: {0}.", &[&rooms])];
                        said.extend(names_kept(replaced));
                        self.say(&sentences(said), false);
                        self.set_map_shown(true);
                    }
                }
            }
            Err(error) if why == Taking::Update => self.say(
                &trf(
                    "Could not bring the server's map up to date: {0}",
                    &[&error],
                ),
                true,
            ),
            Err(error) => self.say(&trf("Could not take the map: {0}", &[&error]), true),
        }
    }
}

impl Tab {
    /// Finds out whether the map at `address` has changed since the map
    /// was taken from it, to answer the script that asked if it has; if
    /// the map was not taken from there, there is nothing to answer.
    pub(super) fn check_newer_map(&mut self, address: String, answer: u64) {
        let known = {
            let map = self.map.borrow();
            let read_now = map.source_reader() >= Some(muddie_mapper::MUDLET_READER);
            (map.source())
                .filter(|(from, _)| *from == address)
                .map(|(_, version)| (version.map(str::to_owned), read_now))
        };
        match known {
            None => {}
            // Muddie reads more of it now than when it was taken.
            Some((_, false)) => self.answer_script(answer),
            Some((version, true)) => {
                (self.map_source.checking).push((mapfetch::changed(address, version), answer))
            }
        }
    }

    /// Answers the scripts whose maps were found to have changed, as the
    /// answers come.
    pub(super) fn check_newer_maps(&mut self, ctx: &egui::Context) {
        let mut changed = Vec::new();
        self.map_source
            .checking
            .retain(|(result, answer)| match result.try_recv() {
                Err(TryRecvError::Empty) => true,
                Ok(Ok(true)) => {
                    changed.push(*answer);
                    false
                }
                Ok(Err(error)) => {
                    crate::debug::show("map", "CHECK", &error);
                    false
                }
                _ => false,
            });
        if !self.map_source.checking.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        for answer in changed {
            self.answer_script(answer);
        }
    }

    fn answer_script(&mut self, answer: u64) {
        self.scripts.answer(answer);
        self.run_actions();
    }

    /// Does what a script offered to do with the map.
    pub(super) fn use_map_tool(&mut self, label: &str) {
        if self.scripts.use_map_tool(label) {
            self.run_actions();
        }
    }
}

/// What bringing the map up to date came to, said: how many rooms it has,
/// how many are new, and the names the user gave that were kept.
fn brought_up_to_date(replaced: muddie_mapper::Replaced) -> String {
    let rooms = trn("{0} room", "{0} rooms", replaced.rooms as u64);
    let new = trn("{0} new", "{0} new", replaced.new as u64);
    let mut said = vec![trf(
        "The map was brought up to date: {0}, {1}.",
        &[&rooms, &new],
    )];
    said.extend(names_kept(replaced));
    sentences(said)
}

/// That the names the user gave rooms were kept, if any were, and what
/// became of the rooms the map had added.
fn names_kept(replaced: muddie_mapper::Replaced) -> Vec<String> {
    let mut said = Vec::new();
    if replaced.renamed > 0 {
        said.push(trn(
            "The name you gave {0} room was kept.",
            "The names you gave {0} rooms were kept.",
            replaced.renamed as u64,
        ));
    }
    if replaced.joined > 0 {
        said.push(trn(
            "{0} room the map had added is the map's now.",
            "{0} rooms the map had added are the map's now.",
            replaced.joined as u64,
        ));
    }
    if replaced.added > 0 {
        said.push(trn(
            "{0} room the map added, which the map taken does not have, was kept.",
            "{0} rooms the map added, which the map taken does not have, were kept.",
            replaced.added as u64,
        ));
    }
    said
}

/// Why a map is being taken: the user or a script asked for it, or one
/// taken before is brought up to date.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Taking {
    Asked,
    Script,
    Update,
}

/// How many places the palette lists for what is typed, at most, besides
/// the rooms named.
const PLACES: usize = 8;

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
        (marks.into_iter().chain(rest.into_iter().take(PLACES)))
            .map(|found| {
                let (title, about) = crate::mapview::found_words(&map, here, &found);
                // A note says little of how to go there: the room's number does.
                let place = match found.by {
                    muddie_mapper::FoundBy::Note => found.room.to_string(),
                    _ => title.clone(),
                };
                commands::Item {
                    insert: String::new(),
                    usage: title,
                    group: tr("Places").to_owned(),
                    about,
                    more: false,
                    act: Some(commands::Act::Place(place)),
                }
            })
            .collect()
    }
}

pub(super) use crate::mapview::{
    direction_words, numbered, ordered_exits, place_words, room_facts, sentences,
};

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
    if room.weight > 1 {
        said.push(trf("Costs {0} to enter.", &[&room.weight.to_string()]));
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
            count => format!("{count} {command}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
