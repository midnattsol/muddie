//! Where the player is on the map, and walking to a room.

use super::*;

/// How long a walk waits for a move to take effect before giving up, as
/// happens at a closed door.
pub(super) const WALK_TIMEOUT: Duration = Duration::from_secs(6);

/// Most rooms listed when a search matches several.
pub(super) const MAX_LISTED_ROOMS: usize = 10;

/// A map being taken from elsewhere, and whether the user knows the map
/// lost track of where they are.
#[derive(Default)]
pub(super) struct MapSource {
    /// A map being taken: where its result comes, where from, and why.
    pub taking: Option<(crate::mapfetch::Taking, String, super::places::Taking)>,
    /// The user was told the map lost track of where they are.
    pub lost: bool,
}

impl Tab {
    /// The server reported the room the player is in.
    pub(super) fn entered_room(&mut self, json: &str) {
        let Some(info) = RoomInfo::from_json(json) else {
            return;
        };
        let room = info.id;
        let saved = self.map.borrow_mut().visit(info);
        if let Err(error) = saved {
            self.note(&trf("Could not save the map: {0}", &[&error.to_string()]));
        }
        self.arrived_in(room);
    }

    /// The map now has the player in `room`: continue a walk, if any.
    pub(super) fn arrived_in(&mut self, room: RoomId) {
        if std::mem::take(&mut self.map_source.lost) {
            let name = self.map.borrow().room(room).map(|room| room.name.clone());
            let name = name.unwrap_or_else(|| format!("#{room}"));
            self.note(&trf("The map knows where you are again: {0}.", &[&name]));
        }
        if let Some((walk, _)) = &mut self.walk {
            let progress = walk.entered(room);
            self.follow(progress);
        }
    }

    /// The map lost track of where the player is: a walk cannot go on,
    /// and the user is told once, until it knows again.
    pub(super) fn lost_track(&mut self) {
        if self.walk.take().is_some() {
            self.note(tr("Walk stopped: the map lost track of where you are."));
        }
        if !std::mem::replace(&mut self.map_source.lost, true) {
            let said = self.unknown_whereabouts();
            self.note(&said);
        }
    }

    /// Why the map cannot say where the player is, and what to do.
    pub(super) fn unknown_whereabouts(&self) -> String {
        crate::mapview::unknown_whereabouts(&self.map.borrow())
    }

    pub(super) fn walk_to(&mut self, room: RoomId) {
        self.walk_to_nearest(&[room]);
    }

    /// Walks to whichever of `rooms` is nearest, saying where and how far.
    pub(super) fn walk_to_nearest(&mut self, rooms: &[RoomId]) {
        let found = {
            let map = self.map.borrow();
            let from = map.current().map(|current| current.id);
            let targets = rooms.iter().copied().collect();
            from.map(|from| {
                let route = map.route_to_nearest(from, &targets);
                let to = route.as_ref().and_then(|(to, _)| map.room(*to));
                let place = to.map(|room| (room.name.clone(), room.area.clone()));
                (from, route, place)
            })
        };
        let Some((from, route, place)) = found else {
            let said = self.unknown_whereabouts();
            return self.refuse(&said);
        };
        let Some(((_, route), (name, area))) = route.zip(place) else {
            return self.refuse(tr("No known route to that place."));
        };
        if route.is_empty() {
            return self.note(&trf("You are already in {0}.", &[&name]));
        }
        let moves = trn("{0} move", "{0} moves", route.len() as u64);
        let mut said = trf("Going to {0}, in {1}: {2}.", &[&name, &area, &moves]);
        if rooms.len() > 1 {
            said.push(' ');
            said.push_str(&trn(
                "The nearest of {0} rooms of that name.",
                "The nearest of {0} rooms of that name.",
                rooms.len() as u64,
            ));
        }
        self.note(&said);
        let walk = Walk::new(from, route);
        let progress = walk.start();
        self.walk = Some((walk, Instant::now()));
        self.follow(progress);
    }

    pub(super) fn follow(&mut self, progress: Progress) {
        match progress {
            Progress::Go(commands) => {
                if let Some((_, moved_at)) = &mut self.walk {
                    *moved_at = Instant::now();
                }
                // An exit may need several commands, as in "open door;n".
                for command in commands.split(';') {
                    self.send(command.trim().to_owned());
                }
            }
            Progress::Waiting => {}
            Progress::Arrived => {
                self.walk = None;
                self.note(tr("Arrived."));
            }
            Progress::Lost => {
                self.walk = None;
                self.note(tr("Walk stopped: this is not the room the route expected."));
            }
        }
    }

    /// Gives up a walk whose last move had no effect for too long.
    pub(super) fn check_walk(&mut self, ctx: &egui::Context) {
        let Some((_, moved_at)) = &self.walk else {
            return;
        };
        let waited = moved_at.elapsed();
        if waited >= WALK_TIMEOUT {
            self.walk = None;
            self.note(tr("Walk stopped: the last move had no effect."));
        } else {
            ctx.request_repaint_after(WALK_TIMEOUT - waited);
        }
    }

    pub(super) fn tidy(&mut self, area: &str) {
        let moved = self.map.borrow_mut().relayout(area);
        match moved {
            Ok(moved) => self.note(&trf(
                "Tidied the map: {0} rooms moved.",
                &[&moved.to_string()],
            )),
            Err(error) => self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()])),
        }
    }

    /// Up to [`MAX_LISTED_ROOMS`] rooms matching `query`, one line each.
    /// Lists the rooms whose name contains `query`, each to be walked to
    /// by choosing it. Returns whether there were any.
    pub(super) fn offer_rooms(&mut self, query: &str) -> bool {
        let rooms: Vec<(RoomId, String, String)> = (self.map.borrow().find(query).iter())
            .take(MAX_LISTED_ROOMS)
            .map(|room| (room.id, room.name.to_string(), room.area.to_string()))
            .collect();
        for (id, name, area) in &rooms {
            self.offer(
                &format!("#{id}: {name}"),
                area,
                &format!("/map travel {id}"),
            );
        }
        !rooms.is_empty()
    }

    /// Says where the player is, by the map, as a line of the client's.
    pub(super) fn say_where(&mut self) {
        match self.where_now() {
            Some(place) => self.note(&place),
            None => {
                let said = self.unknown_whereabouts();
                self.refuse(&said);
            }
        }
    }

    /// Says where the player is aloud only, at once: for a key pressed
    /// often, which would fill the output.
    pub(super) fn say_where_aloud(&mut self) {
        let place = self.where_now();
        let text = place.unwrap_or_else(|| self.unknown_whereabouts());
        self.voice.say_now(&text);
    }

    /// Where the player is, by the map: the room, its area and its exits,
    /// and on the way somewhere, where to and how far.
    fn where_now(&self) -> Option<String> {
        let map = self.map.borrow();
        let place = map.current().map(|room| {
            let exits: Vec<&str> = (room.exits.keys())
                .map(|way| map.command_for(way))
                .collect();
            let fields = [
                &room.name,
                &room.id.to_string(),
                &room.area,
                &exits.join(", "),
            ];
            trf(
                "{0} (#{1}) in {2}. Exits: {3}.",
                &fields.map(String::as_str),
            )
        });
        // On the way somewhere, where to and how far.
        let going = self.walk.as_ref().and_then(|(walk, _)| {
            let (left, to) = walk.left();
            let name = self.map.borrow().room(to?)?.name.clone();
            Some(
                trn(
                    "Walking to {1}: {0} step left.",
                    "Walking to {1}: {0} steps left.",
                    left as u64,
                )
                .replace("{1}", &name),
            )
        });
        match going {
            Some(going) => place.map(|place| format!("{place} {going}")),
            None => place,
        }
    }

    /// Lists the rooms whose name has the text, to walk to.
    pub(super) fn find_rooms(&mut self, text: &str) {
        if !self.offer_rooms(text) {
            self.refuse(tr("No room matches."));
        }
    }

    /// Goes to a place: a room by number, a mark, an area, or a name, and
    /// of several rooms the nearest.
    pub(super) fn go_to(&mut self, place: &str) {
        let rooms = self.map.borrow().rooms_meant(place);
        if rooms.is_empty() {
            return self.refuse(tr("No room matches."));
        }
        self.walk_to_nearest(&rooms);
    }

    /// Lets routes pass through a room, or keeps them out of it: the one
    /// given by number, or the one the player is in.
    pub(super) fn toggle_avoid(&mut self, room: &str) {
        let toggled = {
            let mut map = self.map.borrow_mut();
            let found = match room.parse::<RoomId>() {
                Ok(id) => map.room(id),
                Err(_) if room.is_empty() => map.current(),
                Err(_) => None,
            };
            let target = found.map(|room| (room.id, !room.avoid, room.name.clone()));
            target.map(|(id, avoid, name)| (map.set_avoid(id, avoid), avoid, name))
        };
        match toggled {
            Some((Ok(_), true, name)) => self.note(&trf("Routes will avoid {0}.", &[&name])),
            Some((Ok(_), false, name)) => {
                self.note(&trf("Routes may pass through {0} again.", &[&name]))
            }
            Some((Err(error), ..)) => {
                self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()]))
            }
            None => self.refuse(tr("No such room; use /map avoid with a room number.")),
        }
    }

    /// Lets routes use an exit of the room the player is in, or keeps
    /// them from it.
    pub(super) fn toggle_lock(&mut self, direction: &str) {
        let toggled = {
            let mut map = self.map.borrow_mut();
            let here = map.current().map(|room| room.id);
            let direction = map.direction(direction);
            let exit = here.and_then(|id| map.room(id)?.exits.get(&direction).cloned());
            here.zip(exit).map(|(id, exit)| {
                let locked = !exit.locked;
                (
                    map.edit_exit(id, &direction, |exit| exit.locked = locked),
                    locked,
                )
            })
        };
        match toggled {
            Some((Ok(_), true)) => self.note(tr("Routes will not use that exit.")),
            Some((Ok(_), false)) => self.note(tr("Routes may use that exit again.")),
            Some((Err(error), _)) => {
                self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()]))
            }
            None => self.refuse(tr("This room has no such exit.")),
        }
    }

    /// Draws the area the player is in again from scratch.
    pub(super) fn tidy_here(&mut self) {
        let area = self.map.borrow().current().map(|room| room.area.clone());
        match area {
            Some(area) => self.tidy(&area),
            None => {
                let said = self.unknown_whereabouts();
                self.refuse(&said);
            }
        }
    }

    /// Stops walking, if walking.
    pub(super) fn stop_walking(&mut self) {
        let stopped = self.walk.take().is_some();
        self.note(if stopped {
            tr("Walk stopped.")
        } else {
            tr("Not walking.")
        });
    }
}
