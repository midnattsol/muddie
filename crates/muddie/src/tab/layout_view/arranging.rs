//! Arranging panels from the keyboard: moving one among those of its side or to
//! another side, sharing a place, and the arrangements to start from.

use super::*;

/// An arrangement to start from.
#[derive(Clone, Copy)]
pub(super) enum Arrangement {
    /// The map and the panels of text at the right, the gauges below.
    Classic,
    /// The map at the left, the panels of text at the right.
    Wide,
    /// No panel: the game alone.
    GameOnly,
}

impl Arrangement {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Classic => tr("Classic"),
            Self::Wide => tr("Wide screen"),
            Self::GameOnly => tr("The game alone"),
        }
    }

    pub(super) fn about(self) -> &'static str {
        match self {
            Self::Classic => tr("The map and the panels of text at the right, the gauges below."),
            Self::Wide => {
                tr("The map at the left, the panels of text at the right, the gauges below.")
            }
            Self::GameOnly => tr("Every panel hidden; they are kept, to show again."),
        }
    }
}

impl Tab {
    /// With the keyboard on the panel chosen: the arrows go to another,
    /// Home and End to the first and last; Alt and Up or Down move it among
    /// those of its side, and Alt and Left or Right to another side.
    pub(super) fn layout_keys(&mut self, ui: &mut egui::Ui, listed: &[u32]) {
        let Some(chosen) = self.layout_view.chosen else {
            return;
        };
        if ui.memory(|memory| memory.focused()) != Some(row_id(chosen)) {
            return;
        }
        let (move_up, move_down, move_back, move_on) = ui.input_mut(|input| {
            (
                crate::shortcuts::PANEL_UP.pressed(input),
                crate::shortcuts::PANEL_DOWN.pressed(input),
                crate::shortcuts::PANEL_SIDE_BACK.pressed(input),
                crate::shortcuts::PANEL_SIDE_ON.pressed(input),
            )
        });
        if move_up || move_down {
            return self.move_panel(chosen, move_up);
        }
        if move_back || move_on {
            let side = self
                .layout
                .panel(chosen)
                .map_or(Side::Right, |panel| panel.side);
            let at = SIDES.iter().position(|known| *known == side).unwrap_or(0);
            let to = match move_on {
                true => SIDES[(at + 1) % SIDES.len()],
                false => SIDES[(at + SIDES.len() - 1) % SIDES.len()],
            };
            self.put_on_side(chosen, to);
            self.voice.say_now(&trf("Moved: {0}.", &[side_name(to)]));
            return;
        }
        let pressed = |key: Key| ui.input_mut(|input| input.consume_key(Modifiers::NONE, key));
        let (up, down, home, end) = (
            pressed(Key::ArrowUp),
            pressed(Key::ArrowDown),
            pressed(Key::Home),
            pressed(Key::End),
        );
        let at = listed.iter().position(|id| *id == chosen).unwrap_or(0);
        let to = match (up, down, home, end) {
            (true, ..) => Some(at.saturating_sub(1)),
            (_, true, ..) => Some((at + 1).min(listed.len().saturating_sub(1))),
            (_, _, true, _) => Some(0),
            (.., true) => listed.len().checked_sub(1),
            _ => None,
        };
        if let Some(&id) = to.and_then(|to| listed.get(to)) {
            self.layout_view.chosen = Some(id);
            self.layout_view.reveal = true;
            ui.memory_mut(|memory| memory.request_focus(row_id(id)));
        }
    }

    /// Moves a panel before the one above it on its side, or after the one
    /// below, and says where it is now. Among those it shares a place with,
    /// it changes its place among the tabs.
    pub(super) fn move_panel(&mut self, id: u32, up: bool) {
        let Some(from) = self.layout.panels.iter().position(|panel| panel.id == id) else {
            return;
        };
        let side = self.layout.panels[from].side;
        let beside: Vec<usize> = (0..self.layout.panels.len())
            .filter(|&index| self.layout.panels[index].side == side)
            .collect();
        let at = beside.iter().position(|&index| index == from).unwrap_or(0);
        let next = match up {
            true => at.checked_sub(1).map(|at| beside[at]),
            false => beside.get(at + 1).copied(),
        };
        let Some(next) = next else {
            return self.voice.say_now(match up {
                true => tr("It is the first already."),
                false => tr("It is the last already."),
            });
        };
        let (stack, other) = (
            self.layout.panels[from].stack,
            self.layout.panels[next].stack,
        );
        if stack != 0 && stack == other {
            self.layout.panels.swap(from, next);
        } else {
            let mut moved = self.layout.panels.remove(from);
            moved.stack = 0;
            let next = if next > from { next - 1 } else { next };
            let drop = if up { Drop::Before } else { Drop::After };
            self.drop_beside(moved, next, drop);
        }
        let now: Vec<u32> = (self.layout.panels.iter())
            .filter(|panel| panel.side == side)
            .map(|panel| panel.id)
            .collect();
        let place = now.iter().position(|known| *known == id).unwrap_or(0);
        self.say_place(up, place, now.len());
    }

    /// Moves a panel to the end of another side, which takes it at its size.
    pub(super) fn put_on_side(&mut self, id: u32, side: Side) {
        if self
            .layout
            .panel(id)
            .is_some_and(|panel| panel.side != side)
        {
            self.drop_panel(id, None, side);
        }
    }

    /// Puts a panel in the place of another, as a tab beside it; or, with
    /// none, in a place of its own after the one it shared.
    pub(super) fn share_place(&mut self, id: u32, with: u32) {
        let Some(from) = self.layout.panels.iter().position(|panel| panel.id == id) else {
            return;
        };
        let mut moved = self.layout.panels.remove(from);
        moved.stack = 0;
        let place = match with {
            0 => None,
            other => self
                .layout
                .panels
                .iter()
                .position(|panel| panel.id == other),
        };
        match place {
            Some(place) => self.drop_beside(moved, place, Drop::Into),
            None => self.layout.panels.insert(from, moved),
        }
    }

    /// Puts the panels as an arrangement has them. Panels are kept: those
    /// of text go together, the map and the gauges where it says.
    pub(super) fn layout_as(&mut self, how: Arrangement) {
        self.layout.collapsed.clear();
        if let Arrangement::GameOnly = how {
            self.layout
                .panels
                .iter_mut()
                .for_each(|panel| panel.visible = false);
            return;
        }
        if !self.layout.has(layout::Kind::Map) {
            self.add_panel(layout::Kind::Map);
        }
        let map_side = match how {
            Arrangement::Wide => Side::Left,
            _ => Side::Right,
        };
        for panel in &mut self.layout.panels {
            panel.visible = true;
            panel.stack = 0;
            panel.weight = 1.0;
            (panel.side, panel.size) = match panel.kind {
                layout::Kind::Map => (map_side, layout::BESIDE),
                kind => kind.home(),
            };
        }
        let rank = |panel: &PanelSpec| match panel.kind {
            layout::Kind::Map => 0,
            layout::Kind::Text => 1,
            layout::Kind::Gauges => 2,
        };
        self.layout.panels.sort_by_key(rank);
    }
}
