//! Where the player is on the map, and walking to a room.

use super::*;

/// How long a walk waits for a move to take effect before giving up, as
/// happens at a closed door.
pub(super) const WALK_TIMEOUT: Duration = Duration::from_secs(6);

/// Most rooms listed when a search matches several.
pub(super) const MAX_LISTED_ROOMS: usize = 10;

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
        if let Some((walk, _)) = &mut self.walk {
            let progress = walk.entered(room);
            self.follow(progress);
        }
    }

    pub(super) fn walk_to(&mut self, room: RoomId) {
        let route = {
            let map = self.map.borrow();
            let Some(from) = map.current().map(|current| current.id) else {
                drop(map);
                return self.note(tr("The map does not know where you are yet."));
            };
            map.route(from, room).map(|route| (from, route))
        };
        let Some((from, route)) = route else {
            return self.note(tr("No known route to that room."));
        };
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
            self.offer(&format!("#{id}: {name}"), area, &format!("/map walk {id}"));
        }
        !rooms.is_empty()
    }

    /// Says where the player is, by the map.
    pub(super) fn say_where(&mut self) {
        let place = self.map.borrow().current().map(|room| {
            let exits: Vec<&str> = room.exits.keys().map(String::as_str).collect();
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
        match place {
            Some(place) => self.note(&place),
            None => self.refuse(tr("The map does not know where you are yet.")),
        }
    }

    /// Lists the rooms whose name has the text, to walk to.
    pub(super) fn find_rooms(&mut self, text: &str) {
        if !self.offer_rooms(text) {
            self.refuse(tr("No room matches."));
        }
    }

    /// Walks to a room, by number, or by name if only one has it; several
    /// are listed, to choose the one meant.
    pub(super) fn go_to(&mut self, room: &str) {
        if let Ok(id) = room.parse::<RoomId>() {
            return self.walk_to(id);
        }
        let found: Vec<RoomId> = (self.map.borrow().find(room).iter())
            .map(|room| room.id)
            .collect();
        match found[..] {
            [] => self.refuse(tr("No room matches.")),
            [id] => self.walk_to(id),
            _ => {
                self.offer_rooms(room);
            }
        }
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
            let direction = muddie_mapper::canonical_direction(direction);
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
            None => self.refuse(tr("The map does not know where you are yet.")),
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
