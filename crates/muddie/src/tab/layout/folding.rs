//! Putting away the panels of a whole side, from a tab on the edge between
//! them and the output, and bringing them back from a thin rail.

use super::*;

impl Tab {
    /// The rail a side put away leaves at the edge of the window, with
    /// the tab that brings it back; nothing for a side with no panels.
    pub(in crate::tab) fn side_rail(&mut self, ui: &mut egui::Ui, side: Side) {
        if !self.layout.sides().contains(&(side, false)) {
            return;
        }
        let id = ui.make_persistent_id(("rail", side as u8));
        let rail = match side {
            Side::Left => Panel::left(id),
            Side::Right => Panel::right(id),
            Side::Top => Panel::top(id),
            Side::Bottom => Panel::bottom(id),
        };
        let shown = rail
            .frame(egui::Frame::new())
            .show_separator_line(false)
            .resizable(false)
            .exact_size(RAIL)
            .show(ui, |ui| {
                let label = side.toggle_label(false);
                widgets::fold_tab(ui, ui.max_rect().center(), side.edge(), false, label)
            });
        if shown.inner.clicked() {
            self.layout.reveal(side);
        }
    }

    /// The tab on the edge of each side out, which puts it away: drawn
    /// over the output and the panels, once both are, so that it is the
    /// one that takes a click there rather than the edge.
    pub(in crate::tab) fn fold_tabs(&mut self, ui: &mut egui::Ui) {
        for (side, rect) in std::mem::take(&mut self.panels.sides) {
            let edge = match side {
                Side::Left => rect.right_center(),
                Side::Right => rect.left_center(),
                Side::Top => rect.center_bottom(),
                Side::Bottom => rect.center_top(),
            };
            let label = side.toggle_label(true);
            if widgets::fold_tab(ui, edge, side.edge(), true, label).clicked() {
                self.toggle_side(side);
            }
        }
    }
}

/// How wide the rail of a side put away is.
const RAIL: f32 = 20.0;
