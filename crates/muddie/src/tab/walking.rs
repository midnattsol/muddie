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
}
