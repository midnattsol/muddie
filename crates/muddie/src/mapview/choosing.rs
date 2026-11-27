//! Choosing a room with the keys or a click, and the words said of a room.

use super::*;

impl MapView {
    /// Moves the room chosen along its exit `way`, or says there is none.
    pub(super) fn step(&mut self, map: &Map, from: RoomId, way: &'static str) -> MapAsk {
        let to = (map.room(from))
            .and_then(|room| room.exits.get(way))
            .map(|exit| exit.to)
            .filter(|to| map.room(*to).is_some());
        match to {
            Some(to) => {
                self.choose(to);
                MapAsk::Selected(to, Some(from))
            }
            None => MapAsk::NoExit(way),
        }
    }

    /// The room chosen, if one is.
    #[cfg(test)]
    pub fn chosen_room(&self) -> Option<RoomId> {
        self.selected
    }

    /// Chooses a room with the keys, the one chosen before kept to go
    /// back to, and brings it into sight.
    pub(super) fn choose(&mut self, room: RoomId) {
        if let Some(before) = self.selected.filter(|before| *before != room) {
            self.trail.push(before);
        }
        self.selected = Some(room);
        self.reveal = Some(room);
    }

    /// Leaves the map for the command line, choosing no room; the view
    /// goes back to following the player.
    pub(super) fn leave(&mut self, ui: &Ui, response: &Response, typed: String) -> MapAsk {
        match self.browsing.take() {
            Some(_) => (self.pan, self.glide) = (Vec2::ZERO, None),
            None => self.shift(ui, -self.pan),
        }
        self.selected = None;
        self.had_focus = false;
        self.trail.clear();
        self.prompt = None;
        ui.memory_mut(|memory| memory.surrender_focus(response.id));
        MapAsk::Leave(typed)
    }

    /// Starts exploring the map from `from`: it is chosen, and the map
    /// takes the keyboard when next drawn. Returns what is said on coming
    /// to it.
    pub fn explore(&mut self, map: &Map, from: &Room) -> String {
        self.selected = Some(from.id);
        self.reveal = Some(from.id);
        self.focus_wanted = true;
        self.explore_started = true;
        self.trail.clear();
        self.prompt = None;
        self.said_region = map.region(from).map(str::to_owned);
        let here = place_words(map, from);
        let start = match map.current() {
            Some(_) => trf("Exploring the map from {0}.", &[&here]),
            // Not sure where the player is: from one of the rooms alike.
            None => crate::i18n::trn(
                "Not sure where you are: exploring the map from one of {0} rooms alike, {1}.",
                "Not sure where you are: exploring the map from one of {0} rooms alike, {1}.",
                map.candidates().len() as u64,
            )
            .replace("{1}", &here),
        };
        sentences([start, exits_words(map, from), keys_hint().to_owned()])
    }

    /// What is said of a room the keys or a click chose, `came` from the
    /// one before: its name, and its region on coming to another; how far
    /// it is and which way; its exits; and, only if so, that it has none
    /// back, that some are closed, that routes avoid it, its names and its
    /// note.
    pub fn said_of(&mut self, map: &Map, room: &Room, came: Option<RoomId>) -> String {
        let name = map.short_name(room);
        let region = map.region(room).map(str::to_owned);
        let said = match &region {
            Some(region) if self.said_region.as_ref() != Some(region) && region != name => {
                trf("Into {0}: {1}", &[region, name])
            }
            _ => name.to_owned(),
        };
        self.said_region = region;
        // How far, from where the player is: nothing, while the map is
        // not sure, as was said on starting.
        let here = map.current().map(|here| here.id);
        let far = here.map(|_| how_far(map, here, room));
        let mut parts: Vec<String> = [Some(said), far, Some(exits_words(map, room))]
            .into_iter()
            .flatten()
            .collect();
        if let Some(came) = came.filter(|came| *came != room.id)
            && !room.exits.values().any(|exit| exit.to == came)
        {
            parts.push(tr("No way straight back").to_owned());
        }
        parts.extend(room_facts(map, room));
        sentences(parts)
    }
}
