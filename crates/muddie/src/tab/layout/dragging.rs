//! Moving panels: dragged to another place, or a step along their side.

use super::*;

/// Where a panel dragged over another is put.
#[derive(Clone, Copy, PartialEq)]
pub(in crate::tab) enum Drop {
    Before,
    After,
    /// In its place, as a tab beside it.
    Into,
}

impl Tab {
    /// While a panel is dragged by its handle, shows where it would go;
    /// once it is let go, puts it there: before or after the panel it is
    /// over, or at the end of the side it is nearest to.
    pub(in crate::tab) fn finish_drag(&mut self, ui: &egui::Ui) {
        let Some(id) = self.panels.dragged else {
            return;
        };
        let area = ui.ctx().content_rect();
        let Some(pointer) = ui.ctx().pointer_latest_pos() else {
            self.panels.dragged = None;
            return;
        };
        // Over another panel: its side, and by where on it, before it,
        // after it, or in its place as a tab.
        let over = (self.panels.rects.iter())
            .find(|(other, rect)| *other != id && rect.contains(pointer))
            .copied();
        let onto = over.and_then(|(other, rect)| {
            let side = self.layout.panel(other)?.side;
            let (start, length, at) = if side.is_beside() {
                (rect.top(), rect.height(), pointer.y)
            } else {
                (rect.left(), rect.width(), pointer.x)
            };
            let part = (at - start) / length.max(1.0);
            let (drop, from, to) = match part {
                part if part < 0.3 => (Drop::Before, 0.0, 0.3),
                part if part > 0.7 => (Drop::After, 0.7, 1.0),
                _ => (Drop::Into, 0.3, 0.7),
            };
            let band = if side.is_beside() {
                egui::Rect::from_min_max(
                    egui::pos2(rect.left(), start + length * from),
                    egui::pos2(rect.right(), start + length * to),
                )
            } else {
                egui::Rect::from_min_max(
                    egui::pos2(start + length * from, rect.top()),
                    egui::pos2(start + length * to, rect.bottom()),
                )
            };
            Some((other, side, drop, band))
        });
        // Over nothing: the side the pointer is nearest to, each way in
        // its own measure.
        let reach = [
            (Side::Left, (pointer.x - area.left()) / area.width()),
            (Side::Right, (area.right() - pointer.x) / area.width()),
            (Side::Top, (pointer.y - area.top()) / area.height()),
            (Side::Bottom, (area.bottom() - pointer.y) / area.height()),
        ];
        let nearest = reach
            .into_iter()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(Side::Right, |(side, _)| side);
        if ui.input(|input| input.pointer.any_down()) {
            let band = match (onto, nearest) {
                (Some((.., half)), _) => half,
                (None, Side::Left) => area.with_max_x(area.left() + area.width() * 0.28),
                (None, Side::Right) => area.with_min_x(area.right() - area.width() * 0.28),
                (None, Side::Top) => area.with_max_y(area.top() + area.height() * 0.28),
                (None, Side::Bottom) => area.with_min_y(area.bottom() - area.height() * 0.28),
            };
            let theme = theme::current();
            let layer =
                egui::LayerId::new(egui::Order::Foreground, ui.make_persistent_id("panel drop"));
            let painter = ui.ctx().layer_painter(layer);
            let band = band.shrink(6.0);
            painter.rect_filled(
                band,
                egui::CornerRadius::same(12),
                theme.accent.gamma_multiply(0.14),
            );
            let edge = egui::Stroke::new(1.5, theme.accent.gamma_multiply(0.7));
            painter.rect_stroke(
                band,
                egui::CornerRadius::same(12),
                edge,
                egui::StrokeKind::Inside,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            return;
        }
        self.panels.dragged = None;
        let Some(from) = self.layout.panels.iter().position(|panel| panel.id == id) else {
            return;
        };
        let mut moved = self.layout.panels.remove(from);
        let side = onto.map_or(nearest, |(_, side, ..)| side);
        // It takes the size of the side it joins, or one that suits it.
        let there = self
            .layout
            .panels
            .iter()
            .find(|panel| panel.visible && panel.side == side);
        moved.size = match there {
            Some(panel) => panel.size,
            None if moved.side.is_beside() == side.is_beside() => moved.size,
            None if side.is_beside() => 300.0,
            None => 120.0,
        };
        moved.side = side;
        moved.weight = 1.0;
        moved.stack = 0;
        let Some((other, _, drop, _)) = onto else {
            return self.layout.panels.push(moved);
        };
        let Some(place) = self
            .layout
            .panels
            .iter()
            .position(|panel| panel.id == other)
        else {
            return self.layout.panels.push(moved);
        };
        // The panels the one dropped on shares its place with, if any.
        let stack = self.layout.panels[place].stack;
        let shares = |panel: &PanelSpec| stack != 0 && panel.stack == stack && panel.side == side;
        let first = self.layout.panels.iter().position(&shares).unwrap_or(place);
        let last = self.layout.panels.iter().rposition(shares).unwrap_or(place);
        match drop {
            Drop::Before => self.layout.panels.insert(first, moved),
            Drop::After => self.layout.panels.insert(last + 1, moved),
            Drop::Into => {
                // Joined as a tab, and shown, so that it is seen to be there.
                let joined = if stack == 0 {
                    self.layout.next_stack()
                } else {
                    stack
                };
                self.layout.panels[place].stack = joined;
                moved.stack = joined;
                moved.weight = self.layout.panels[place].weight;
                self.panels.shown_tabs.retain(|(known, _)| *known != joined);
                self.panels.shown_tabs.push((joined, moved.id));
                self.layout.panels.insert(last + 1, moved);
            }
        }
    }

    /// Makes a panel share the place of the one before it on its side.
    pub(in crate::tab) fn join_before(&mut self, index: usize) {
        let side = self.layout.panels[index].side;
        let Some(before) = (0..index).rev().find(|&other| {
            self.layout.panels[other].visible && self.layout.panels[other].side == side
        }) else {
            return;
        };
        let stack = match self.layout.panels[before].stack {
            0 => self.layout.next_stack(),
            stack => stack,
        };
        self.layout.panels[before].stack = stack;
        self.layout.panels[index].stack = stack;
        let id = self.layout.panels[index].id;
        self.panels.shown_tabs.retain(|(known, _)| *known != stack);
        self.panels.shown_tabs.push((stack, id));
    }

    /// Moves a panel one place among those of its side: `-1` earlier,
    /// `1` later.
    pub(in crate::tab) fn shift_panel(&mut self, index: usize, by: isize) {
        let side = self.layout.panels[index].side;
        let same: Vec<usize> = (0..self.layout.panels.len())
            .filter(|&other| self.layout.panels[other].side == side)
            .collect();
        let place = same.iter().position(|&other| other == index).unwrap_or(0);
        if let Some(&other) = place
            .checked_add_signed(by)
            .and_then(|wanted| same.get(wanted))
        {
            self.layout.panels.swap(index, other);
        }
    }
}
