//! Moving panels: dragged to another side, another place on theirs, or
//! into another's place as a tab.

use super::*;

/// Where a panel dragged over another is put.
#[derive(Clone, Copy, PartialEq)]
pub(in crate::tab) enum Drop {
    Before,
    After,
    /// In its place, as a tab beside it.
    Into,
}

/// Where a dragged panel would go over another: that panel, its side,
/// how it would join it, and the part of it that says so.
pub(in crate::tab) struct DropTarget {
    other: u32,
    side: Side,
    drop: Drop,
    band: egui::Rect,
}

/// Over nothing: the side the pointer is nearest to, each way in its own
/// measure.
fn nearest_side(area: egui::Rect, pointer: egui::Pos2) -> Side {
    let reach = [
        (Side::Left, (pointer.x - area.left()) / area.width()),
        (Side::Right, (area.right() - pointer.x) / area.width()),
        (Side::Top, (pointer.y - area.top()) / area.height()),
        (Side::Bottom, (area.bottom() - pointer.y) / area.height()),
    ];
    reach
        .into_iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(Side::Right, |(side, _)| side)
}

/// The part of the window a panel would take on a side.
fn side_band(area: egui::Rect, side: Side) -> egui::Rect {
    match side {
        Side::Left => area.with_max_x(area.left() + area.width() * 0.28),
        Side::Right => area.with_min_x(area.right() - area.width() * 0.28),
        Side::Top => area.with_max_y(area.top() + area.height() * 0.28),
        Side::Bottom => area.with_min_y(area.bottom() - area.height() * 0.28),
    }
}

/// Marks where a panel would go, over everything, while it is dragged.
fn show_drop(ui: &egui::Ui, band: egui::Rect) {
    let theme = theme::current();
    let layer = egui::LayerId::new(egui::Order::Foreground, ui.make_persistent_id("panel drop"));
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
        let onto = self.drop_target(id, pointer);
        let nearest = nearest_side(area, pointer);
        if ui.input(|input| input.pointer.any_down()) {
            let band = match onto {
                Some(target) => target.band,
                None => side_band(area, nearest),
            };
            show_drop(ui, band);
            return;
        }
        self.panels.dragged = None;
        self.drop_panel(id, onto, nearest);
    }

    /// The panel the pointer is over, but the one dragged: its side, and
    /// by where on it, before it, after it, or in its place as a tab.
    fn drop_target(&self, id: u32, pointer: egui::Pos2) -> Option<DropTarget> {
        let (other, rect) = (self.panels.rects.iter())
            .find(|(other, rect)| *other != id && rect.contains(pointer))
            .copied()?;
        let side = self.layout.panel(other)?.side;
        let (start, length, at) = match side.is_beside() {
            true => (rect.top(), rect.height(), pointer.y),
            false => (rect.left(), rect.width(), pointer.x),
        };
        let part = (at - start) / length.max(1.0);
        let (drop, from, to) = match part {
            part if part < 0.3 => (Drop::Before, 0.0, 0.3),
            part if part > 0.7 => (Drop::After, 0.7, 1.0),
            _ => (Drop::Into, 0.3, 0.7),
        };
        let band = match side.is_beside() {
            true => egui::Rect::from_min_max(
                egui::pos2(rect.left(), start + length * from),
                egui::pos2(rect.right(), start + length * to),
            ),
            false => egui::Rect::from_min_max(
                egui::pos2(start + length * from, rect.top()),
                egui::pos2(start + length * to, rect.bottom()),
            ),
        };
        Some(DropTarget {
            other,
            side,
            drop,
            band,
        })
    }

    /// Puts the panel dropped where it was let go: on the side of the one
    /// it is over, or the side it is nearest to, taking a size that suits.
    pub(in crate::tab) fn drop_panel(&mut self, id: u32, onto: Option<DropTarget>, nearest: Side) {
        let Some(from) = self.layout.panels.iter().position(|panel| panel.id == id) else {
            return;
        };
        let mut moved = self.layout.panels.remove(from);
        let side = onto.as_ref().map_or(nearest, |target| target.side);
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
        self.layout.reveal(side);
        moved.weight = 1.0;
        moved.stack = 0;
        let place = onto.as_ref().and_then(|target| {
            self.layout
                .panels
                .iter()
                .position(|panel| panel.id == target.other)
        });
        match (onto, place) {
            (Some(target), Some(place)) => self.drop_beside(moved, place, target.drop),
            _ => self.layout.panels.push(moved),
        }
    }

    /// Puts a panel before, after, or in the place of the one at `place`,
    /// as a tab with it and those it shares its place with. Joined as a
    /// tab, it is shown, so that it is seen to be there.
    pub(in crate::tab) fn drop_beside(&mut self, mut moved: PanelSpec, place: usize, drop: Drop) {
        let (side, stack) = (
            self.layout.panels[place].side,
            self.layout.panels[place].stack,
        );
        let shares = |panel: &PanelSpec| stack != 0 && panel.stack == stack && panel.side == side;
        let first = self.layout.panels.iter().position(&shares).unwrap_or(place);
        let last = self.layout.panels.iter().rposition(shares).unwrap_or(place);
        match drop {
            Drop::Before => self.layout.panels.insert(first, moved),
            Drop::After => self.layout.panels.insert(last + 1, moved),
            Drop::Into => {
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
}
