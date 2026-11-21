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
    /// Puts the player in a room the user names by number; with none,
    /// lists the rooms the map is unsure between, to choose, or says
    /// where the player is.
    pub(super) fn here(&mut self, argument: &str) {
        if let Ok(id) = argument.parse::<RoomId>() {
            if !self.map.borrow_mut().set_current(id) {
                return self.refuse(tr("There is no room of that number."));
            }
            self.arrived_in(id);
            return self.say_where();
        }
        if !argument.is_empty() {
            return self.refuse(tr(
                "Give the number of the room you are in: /map here <number>.",
            ));
        }
        let rooms: Vec<(String, String, String)> = {
            let map = self.map.borrow();
            (map.candidates().iter().take(MAX_LISTED_ROOMS))
                .filter_map(|id| map.room(*id))
                .map(|room| {
                    (
                        format!("#{}: {}", room.id, room.name),
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
                            .map_or_else(String::new, |room| room.name.clone());
                        Ok(if steps.is_empty() {
                            trf("You are already in {0}.", &[&name])
                        } else {
                            let moves = trn("{0} move", "{0} moves", steps.len() as u64);
                            format!("{name}, {moves}: {}.", in_runs(&steps))
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
                (room.exits.iter())
                    .map(|(way, exit)| {
                        let mut leads = match map.room(exit.to) {
                            Some(to) => to.name.clone(),
                            None => tr("somewhere not yet mapped").to_owned(),
                        };
                        if exit.locked {
                            leads = trf("{0} (routes do not use it)", &[&leads]);
                        }
                        let word = exit
                            .command
                            .as_deref()
                            .unwrap_or_else(|| map.command_for(way));
                        (word.to_owned(), leads, format!("/map travel {}", exit.to))
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
                let mut named = HashSet::from([here.name.to_lowercase()]);
                (map.around(here.id, moves).into_iter())
                    .filter_map(|(id, steps)| {
                        let room = map.room(id)?;
                        named.insert(room.name.to_lowercase()).then(|| {
                            let moves = trn("{0} move", "{0} moves", steps.len() as u64);
                            let way = format!("{moves}: {}", in_runs(&steps));
                            (room.name.clone(), way, format!("/map travel {id}"))
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
        let said = {
            let map = self.map.borrow();
            let room = match argument {
                "" => map.current(),
                place => map.rooms_meant(place).first().and_then(|id| map.room(*id)),
            };
            room.map(|room| {
                let mut said = format!("{} (#{}). {}", room.name, room.id, describe(&map, room));
                let marks: Vec<&str> = (map.marks().iter())
                    .filter(|(_, id)| **id == room.id)
                    .map(|(name, _)| name.as_str())
                    .collect();
                if !marks.is_empty() {
                    said.push(' ');
                    said.push_str(&trf("Called {0}.", &[&marks.join(", ")]));
                }
                if room.avoid {
                    said.push(' ');
                    said.push_str(tr("Routes avoid it."));
                }
                if room.weight > 1 {
                    said.push(' ');
                    said.push_str(&trf("Costs {0} to enter.", &[&room.weight.to_string()]));
                }
                said
            })
        };
        match said {
            Some(said) => self.note(&said),
            None if argument.is_empty() => {
                let said = self.unknown_whereabouts();
                self.refuse(&said);
            }
            None => self.refuse(tr("No room matches.")),
        }
    }

    /// Names the room the player is in; with no name, lists the names
    /// given, each to go to by choosing it.
    pub(super) fn mark_here(&mut self, name: &str) {
        if name.is_empty() {
            let marks: Vec<(String, String)> = {
                let map = self.map.borrow();
                (map.marks().iter())
                    .map(|(name, id)| {
                        let room = map
                            .room(*id)
                            .map_or_else(|| format!("#{id}"), |room| room.name.clone());
                        (name.clone(), room)
                    })
                    .collect()
            };
            if marks.is_empty() {
                return self.refuse(tr(
                    "No room has a name yet: /map mark <name> names this one.",
                ));
            }
            for (name, room) in marks {
                self.offer(&name, &room, &format!("/map travel {name}"));
            }
            return;
        }
        let here = self.map.borrow().current().map(|room| room.id);
        let Some(here) = here else {
            let said = self.unknown_whereabouts();
            return self.refuse(&said);
        };
        let saved = self.map.borrow_mut().mark(name, here);
        match saved {
            Ok(()) => self.note(&trf(
                "This room is now called {0}: /map travel {0} goes to it.",
                &[name],
            )),
            Err(error) => self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()])),
        }
    }

    /// Forgets a name given to a room.
    pub(super) fn unmark(&mut self, name: &str) {
        let forgotten = self.map.borrow_mut().unmark(name);
        match forgotten {
            Ok(true) => self.note(&trf("No room is called {0} now.", &[name])),
            Ok(false) => self.refuse(&trf("No room is called {0}.", &[name])),
            Err(error) => self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()])),
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
            self.offer(&area, &rooms, &format!("/map travel {area}"));
        }
    }

    /// A script takes the map at `address`: at once, or, if this map came
    /// from there, only if it has changed since.
    pub(super) fn import_for_script(&mut self, address: String) {
        let known = {
            let map = self.map.borrow();
            (map.source())
                .filter(|(from, _)| *from == address)
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
                "Give a map made with Mudlet: /map import <file or address>.",
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
        if why != Taking::Update {
            self.toast(&trf("Taking the map from {0}…", &[&source]), false);
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
            Ok(Taken::Unchanged) => {}
            Ok(Taken::Map {
                map: taken,
                version,
            }) => {
                let rooms = taken.rooms.len();
                let saved = {
                    let mut map = self.map.borrow_mut();
                    // Where the player is stays, if the room is still there.
                    let here = map.current().map(|room| room.id);
                    let from_web = mapfetch::on_the_web(&source).then_some(source.as_str());
                    (map.replace_all(taken.rooms, here)).and_then(|()| {
                        map.set_source(from_web.map(|from| (from, version.as_deref())))
                    })
                };
                let rooms = trn("{0} room", "{0} rooms", rooms as u64);
                match saved {
                    Err(error) => self.toast(
                        &trf("Could not save the map: {0}", &[&error.to_string()]),
                        true,
                    ),
                    Ok(()) if why == Taking::Update => self.toast(
                        &trf("The server's map was brought up to date: {0}.", &[&rooms]),
                        false,
                    ),
                    Ok(()) => {
                        self.toast(&trf("The map was taken: {0}.", &[&rooms]), false);
                        self.set_map_shown(true);
                    }
                }
            }
            Err(error) if why == Taking::Update => self.toast(
                &trf(
                    "Could not bring the server's map up to date: {0}",
                    &[&error],
                ),
                true,
            ),
            Err(error) => self.toast(&trf("Could not take the map: {0}", &[&error]), true),
        }
    }
}

/// Why a map is being taken: the user or a script asked for it, or one
/// taken before is brought up to date.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Taking {
    Asked,
    Script,
    Update,
}

/// What tells a room from others of its name: where it is and its exits.
fn describe(map: &muddie_mapper::Map, room: &muddie_mapper::Room) -> String {
    let exits: Vec<&str> = room.exits.keys().map(|way| map.command_for(way)).collect();
    let place = format!("{}, {}, {}", room.pos.x, room.pos.y, room.pos.z);
    let mut said = trf("In {0}, at {1}.", &[&room.area, &place]);
    said.push(' ');
    said.push_str(&match exits.is_empty() {
        true => tr("No exits.").to_owned(),
        false => trf("Exits: {0}.", &[&exits.join(", ")]),
    });
    said
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
