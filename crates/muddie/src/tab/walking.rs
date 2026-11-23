//! Where the player is on the map, and walking to a room.

use super::*;
use crate::widgets;
use muddie_mapper::Tidied;

/// How long a walk waits for a move to take effect before giving up, as
/// happens at a closed door.
pub(super) const WALK_TIMEOUT: Duration = Duration::from_secs(6);

/// Most rooms listed when a search matches several.
pub(super) const MAX_LISTED_ROOMS: usize = 10;

/// The most rooms an area is tidied with, unless the user says to: a map
/// with all in one area, as some MUDs' are, would be drawn all again.
const MAX_TIDIED: usize = 2000;

/// A map being taken from elsewhere, and whether the user knows the map
/// lost track of where they are.
#[derive(Default)]
pub(super) struct MapSource {
    /// A map being taken: where its result comes, where from, and why.
    pub taking: Option<(crate::mapfetch::Taking, String, super::places::Taking)>,
    /// The user was told the map lost track of where they are.
    pub lost: bool,
    /// A map a script asked for before the session's map was open, to be
    /// taken once it is.
    pub waiting: Option<String>,
    /// Whether maps the map was taken from have changed: where each answer
    /// comes, and the script's to tell if one has.
    pub checking: Vec<(crate::mapfetch::Checking, u64)>,
}

impl Tab {
    /// The server reported the room the player is in.
    pub(super) fn entered_room(&mut self, json: &str) {
        let Some(info) = RoomInfo::from_json(json) else {
            return;
        };
        let room = info.id;
        let saved = self.map.borrow_mut().visit(info);
        self.map_saved(saved);
        self.arrived_in(room);
    }

    /// The map now has the player in `room`: continue a walk, if any.
    pub(super) fn arrived_in(&mut self, room: RoomId) {
        if std::mem::take(&mut self.map_source.lost) {
            let name = {
                let map = self.map.borrow();
                map.room(room).map(|room| places::place_words(&map, room))
            };
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
            self.note(tr("Stopped: the map lost track of where you are."));
        }
        if !std::mem::replace(&mut self.map_source.lost, true) {
            let said = self.unknown_whereabouts();
            self.note(&said);
        }
    }

    /// Shows where the map has the player, with `--debug`.
    pub(super) fn record_whereabouts(&self) {
        if !crate::debug::on() {
            return;
        }
        let map = self.map.borrow();
        let said = match map.whereabouts() {
            muddie_mapper::Located::Here(id) => {
                let name = map.room(id).map_or("", |room| room.name.as_str());
                format!("here #{id} {name}")
            }
            muddie_mapper::Located::Unsure(rooms) => {
                let first: Vec<String> = rooms.iter().take(10).map(|id| format!("#{id}")).collect();
                format!("unsure among {}: {}", rooms.len(), first.join(" "))
            }
            muddie_mapper::Located::Nowhere => "nowhere".to_owned(),
        };
        crate::debug::show(&self.title(), "MAP", &said);
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
                let place = to.map(|room| {
                    let through = route
                        .as_ref()
                        .map(|(_, steps)| map.regions_along(from, steps));
                    (places::place_words(&map, room), through.unwrap_or_default())
                });
                (from, route, place)
            })
        };
        let Some((from, route, place)) = found else {
            let said = self.unknown_whereabouts();
            return self.refuse(&said);
        };
        let Some(((_, route), (place, through))) = route.zip(place) else {
            return self.refuse(tr("No known route to that place."));
        };
        if route.is_empty() {
            return self.note(&trf("You are already in {0}.", &[&place]));
        }
        let moves = trn("{0} move", "{0} moves", route.len() as u64);
        let mut said = trf("Going to {0}: {1}.", &[&place, &moves]);
        if !through.is_empty() {
            said.push(' ');
            said.push_str(&places::through(&through));
        }
        if rooms.len() > 1 {
            said.push(' ');
            said.push_str(&trn(
                "The nearest of {0} rooms of that name.",
                "The nearest of {0} rooms of that name.",
                rooms.len() as u64,
            ));
        }
        said.push(' ');
        said.push_str(tr("Escape stops it."));
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
                self.note(tr("Stopped: this is not the room the way expected."));
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
            self.note(tr("Stopped: the last move had no effect."));
        } else {
            ctx.request_repaint_after(WALK_TIMEOUT - waited);
        }
    }

    /// How many rooms an area has, if more than are tidied unasked.
    fn too_many_to_tidy(&self, area: &str) -> Option<usize> {
        let rooms = (self.map.borrow().rooms())
            .filter(|room| room.area == area)
            .count();
        (rooms > MAX_TIDIED).then_some(rooms)
    }

    /// Tidies an area chosen on the map: a large one once the user says so,
    /// in a dialog.
    pub(super) fn tidy_chosen(&mut self, area: &str) {
        match self.too_many_to_tidy(area) {
            Some(_) => self.tidy_asked = Some(area.to_owned()),
            None => self.tidy(area, false),
        }
    }

    /// The dialog that asks whether to tidy a large area, while open.
    pub(super) fn tidy_window(&mut self, ctx: &egui::Context) {
        let Some(area) = self.tidy_asked.clone() else {
            return;
        };
        let rooms = self.too_many_to_tidy(&area).unwrap_or_default();
        let (mut open, mut tidy, mut cancel) = (true, false, false);
        let size = crate::theme::size::DIALOG_S;
        widgets::dialog(ctx, "tidy", tr("Tidy the area"), &mut open, size, |ui| {
            ui.label(trf(
                "{0} has {1} rooms: they are all drawn again, and what was placed by hand is lost.",
                &[&area, &rooms.to_string()],
            ));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                tidy = widgets::primary_button(ui, tr("Tidy")).clicked();
                cancel = widgets::quiet_button(ui, tr("Cancel")).clicked();
            });
        });
        if tidy {
            self.tidy(&area, true);
        }
        if !open || tidy || cancel {
            self.tidy_asked = None;
        }
    }

    /// Draws an area again from scratch; a large one only if `all`.
    pub(super) fn tidy(&mut self, area: &str, all: bool) {
        if let (Some(rooms), false) = (self.too_many_to_tidy(area), all) {
            return self.refuse(&trf(
                "{0} has {1} rooms: tidying it would draw them all again, and what was drawn by hand would be lost.",
                &[area, &rooms.to_string()],
            ));
        }
        let tidied = self.map.borrow_mut().relayout(area);
        match self.map_saved(tidied) {
            Some(Tidied::Moved(0)) => self.note(tr("The map is as tidy as it gets.")),
            Some(Tidied::Moved(moved)) => self.note(&trf(
                "Tidied the map: {0} rooms moved.",
                &[&moved.to_string()],
            )),
            // Drawn again it would look worse: it is left as it is.
            Some(Tidied::Kept { now, then }) => self.refuse(&trf(
                "Not tidied: drawn again, {0} exits would point the wrong way and {1} rooms would lie on another; now it is {2} and {3}.",
                &[
                    &then.wrong_exits.to_string(),
                    &then.stacked.to_string(),
                    &now.wrong_exits.to_string(),
                    &now.stacked.to_string(),
                ],
            )),
            None => {}
        }
    }

    /// Up to [`MAX_LISTED_ROOMS`] rooms matching `query`, one line each.
    /// Lists the rooms whose name contains `query`, each to be walked to
    /// by choosing it. Returns whether there were any.
    pub(super) fn offer_rooms(&mut self, query: &str) -> bool {
        let rooms: Vec<(RoomId, String, String)> = {
            let map = self.map.borrow();
            (map.find(query).into_iter())
                .take(MAX_LISTED_ROOMS)
                .map(|room| (room.id, places::numbered(&map, room), room.area.clone()))
                .collect()
        };
        for (id, name, area) in &rooms {
            self.offer(name, area, &format!("/map go {id}"));
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

    /// Explores the map with the keys, from where the player is, without
    /// moving: the map is shown, takes the keyboard, and says so.
    pub(super) fn explore_map(&mut self) {
        let said = {
            let map = self.map.borrow();
            let first = || map.candidates().first().and_then(|id| map.room(*id));
            map.current()
                .or_else(first)
                .map(|from| self.map_view.explore(&map, from))
        };
        let Some(said) = said else {
            let said = self.unknown_whereabouts();
            return self.refuse(&said);
        };
        self.set_view(0);
        self.set_map_shown(true);
        self.voice.say_now(&said);
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
            let exits = crate::mapview::exits_words(&map, room);
            places::sentences([places::place_words(&map, room), exits])
        });
        // On the way somewhere, where to and how far.
        let going = self.walk.as_ref().and_then(|(walk, _)| {
            let (left, to) = walk.left();
            let name = places::place_words(&map, map.room(to?)?);
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
        match self.room_meant(room) {
            Ok(id) => {
                let avoid = self.map.borrow().room(id).is_some_and(|room| !room.avoid);
                self.avoid_room(id, avoid);
            }
            Err(why) => self.refuse(&why),
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
        let Some((saved, locked)) = toggled else {
            return self.refuse(tr("This room has no such exit."));
        };
        match self.map_saved(saved) {
            Some(_) if locked => self.note(tr("Routes will not use that exit.")),
            Some(_) => self.note(tr("Routes may use that exit again.")),
            None => {}
        }
    }

    /// Draws the area the player is in again from scratch; a large one
    /// only if told "all".
    pub(super) fn tidy_here(&mut self, argument: &str) {
        let area = self.map.borrow().current().map(|room| room.area.clone());
        match area {
            Some(area) => self.tidy(&area, argument.trim().eq_ignore_ascii_case("all")),
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
            tr("Stopped.")
        } else {
            tr("Not going anywhere.")
        });
    }
}
