//! Where the view is: the room chosen, the area and level on show, panning, zooming and gliding.

use super::*;

impl MapView {
    /// What is shown of the room chosen, but for its exits, worked out
    /// again only when it, where the player is or the map changes.
    pub(super) fn told_of_selected(&mut self, map: &Map) -> Option<String> {
        let room = map.room(self.selected?)?;
        let key = (room.id, map.current().map(|here| here.id), map.revision());
        if self.told.as_ref().is_none_or(|(known, _)| *known != key) {
            self.told = Some((key, selection_shown(map, key.1, room)));
        }
        self.told.as_ref().map(|(_, told)| told.clone())
    }

    /// Brings the room the keys chose into sight: to its area and level,
    /// and into the middle of the screen.
    pub(super) fn bring_into_sight(
        &mut self,
        ui: &Ui,
        response: &Response,
        map: &Map,
        grid: &Grid,
        (area, level): (&str, i32),
    ) {
        let Some(room) = self.reveal.and_then(|id| map.room(id)) else {
            return;
        };
        if room.area != area || room.pos.z != level {
            // Another area or level: shown first, brought onto the screen
            // when drawn.
            self.browsing = Some((room.area.clone(), room.pos.z));
            ui.ctx().request_repaint();
            return;
        }
        self.reveal = None;
        // In the middle, as the player's room is while they play.
        let at = grid.on_screen(room.pos);
        self.shift(ui, response.rect.center() - at);
    }

    /// Moves the view by `by`, gliding there as it does after the player,
    /// unless animations are off.
    pub(super) fn shift(&mut self, ui: &Ui, by: Vec2) {
        if by == Vec2::ZERO {
            return;
        }
        let gliding = self.glide_offset(ui);
        self.pan += by;
        self.glide = (ui.style().animation_time > 0.0)
            .then(|| ((gliding - by) / self.cell, ui.input(|input| input.time)));
        ui.ctx().request_repaint();
    }

    /// How far the view was moved from where it follows the player.
    #[cfg(test)]
    pub fn pan(&self) -> Vec2 {
        self.pan
    }

    /// Which area and level to show: the player's, unless the view was
    /// taken to another; and the player's.
    pub(super) fn place(
        &mut self,
        ui: &Ui,
        map: &Map,
        current: &Room,
        locate: bool,
    ) -> (String, i32, (String, i32)) {
        let home = (current.area.clone(), current.pos.z);
        if self.centred_on != Some(current.id) {
            // The player moved: go back to following them, gliding there
            // from where the view was if that was near.
            let before = self.centred_on.and_then(|id| map.room(id));
            let following = self.browsing.is_none();
            let set_out = self.pan + self.glide_offset(ui);
            self.glide = before
                .filter(|before| following && before.area == current.area)
                .map(|before| (before.pos.x - current.pos.x, before.pos.y - current.pos.y))
                .filter(|(dx, dy)| dx.abs().max(dy.abs()) <= GLIDE_REACH)
                .filter(|_| ui.style().animation_time > 0.0)
                .map(|(dx, dy)| {
                    let from = set_out / self.cell - vec2(dx as f32, -dy as f32);
                    (from, ui.input(|input| input.time))
                });
            self.centred_on = Some(current.id);
            self.pan = Vec2::ZERO;
            self.browsing = None;
        }
        if locate {
            // Back to the player, wherever the view had wandered.
            self.browsing = None;
            self.pan = Vec2::ZERO;
            self.glide = None;
        }
        let (area, level) = self.browsing.clone().unwrap_or_else(|| home.clone());
        (area, level, home)
    }

    /// How far the view still is from where it glides to, on screen; it
    /// is drawn again until it gets there.
    pub(super) fn glide_offset(&mut self, ui: &Ui) -> Vec2 {
        let Some((from, start)) = self.glide else {
            return Vec2::ZERO;
        };
        let done = ((ui.input(|input| input.time) - start) / GLIDE) as f32;
        if done >= 1.0 {
            self.glide = None;
            return Vec2::ZERO;
        }
        ui.ctx().request_repaint();
        // Fast at first, slowing down as it arrives.
        let left = (1.0 - done.max(0.0)).powi(3);
        from * left * self.cell
    }

    /// Zooms to cells of this size, as the scroll wheel does.
    #[cfg(test)]
    pub fn zoom_to(&mut self, cell: f32) {
        self.cell = cell.clamp(*CELL_RANGE.start(), *CELL_RANGE.end());
    }

    /// The levels an area has rooms on, from the lowest, worked out again
    /// only when the map or the area changes.
    pub(super) fn levels_of(&mut self, map: &Map, area: &str) -> Vec<i32> {
        let known = (self.levels.as_ref())
            .filter(|((revision, of), _)| *revision == map.revision() && of == area);
        if let Some((_, levels)) = known {
            return levels.clone();
        }
        let levels: std::collections::BTreeSet<i32> = (map.rooms())
            .filter(|room| room.area == area)
            .map(|room| room.pos.z)
            .collect();
        let levels: Vec<i32> = levels.into_iter().collect();
        self.levels = Some(((map.revision(), area.to_owned()), levels.clone()));
        levels
    }

    /// How many areas the map has.
    pub fn area_count(&self, map: &Map) -> usize {
        match self.areas.get() {
            Some((revision, count)) if revision == map.revision() => count,
            _ => {
                let count = map.areas().len();
                self.areas.set(Some((map.revision(), count)));
                count
            }
        }
    }

    /// Shows `area`, at the level the player is on if it has rooms there,
    /// else its lowest: to look at it, without going.
    pub fn show_area(&mut self, map: &Map, area: &str) {
        let levels = self.levels_of(map, area);
        let Some(lowest) = levels.first().copied() else {
            return;
        };
        let on = map
            .current()
            .map(|room| room.pos.z)
            .filter(|z| levels.contains(z));
        self.browsing = Some((area.to_owned(), on.unwrap_or(lowest)));
        self.pan = Vec2::ZERO;
    }
}
