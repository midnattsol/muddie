//! Putting away the panels of a whole side, from the header of its first
//! panel, and bringing them back from the strip of their names it leaves.

use super::*;

/// How thick the strip of a side put away is, card and all.
const STRIP: f32 = 28.0;

/// A panel named in the strip of a side put away.
struct StripName {
    id: u32,
    stack: u32,
    name: String,
    /// It is not there for who listens.
    unreached: bool,
}

impl Tab {
    /// The button that puts the side away, in the header of the first
    /// place of the side only: it is the whole side that goes.
    pub(in crate::tab) fn fold_button(&mut self, ui: &mut egui::Ui, place: &[usize]) {
        let side = self.layout.panels[place[0]].side;
        // With a screen reader, in the first that is there for it, if any is.
        let here = |index: &usize| {
            let panel = &self.layout.panels[*index];
            self.layout.on_screen(panel) && panel.side == side
        };
        let all = || (0..self.layout.panels.len()).filter(here);
        let first = (all().find(|&index| !self.unreached(index))).or_else(|| all().next());
        if !first.is_some_and(|first| place.contains(&first)) {
            return;
        }
        let icon = widgets::Icon::Fold(side.edge());
        if widgets::icon_button(ui, icon, side.toggle_label(true), false).clicked() {
            self.toggle_side(side);
            // The button is gone with the side: the keyboard goes to the
            // one that brings it back, and it is said where that is.
            self.panels.focus_strip = Some(side);
            self.voice
                .say_now(tr("Collapsed. The strip at its edge expands it again."));
        }
    }

    /// The strip a side put away leaves at the edge of the window: the
    /// names of its panels, any of which brings it back with that one on
    /// show. Nothing for a side with no panels.
    pub(in crate::tab) fn side_strip(
        &mut self,
        ui: &mut egui::Ui,
        side: Side,
    ) -> Option<egui::Rect> {
        if !self.layout.sides().contains(&(side, false)) {
            return None;
        }
        let panels: Vec<StripName> = (0..self.layout.panels.len())
            .map(|index| (index, &self.layout.panels[index]))
            .filter(|(_, panel)| panel.visible && panel.side == side)
            .map(|(index, panel)| StripName {
                id: panel.id,
                stack: panel.stack,
                name: panel.shown_name(),
                unreached: self.unreached(index),
            })
            .collect();
        let none = panels.iter().all(|panel| panel.unreached);
        let (left, right) = match side {
            Side::Left => (8, 4),
            Side::Right => (4, 8),
            Side::Top | Side::Bottom => (8, 8),
        };
        let margin = egui::Margin {
            left,
            right,
            top: 4,
            bottom: 4,
        };
        let id = ui.make_persistent_id(("strip", side as u8));
        let strip = match side {
            Side::Left => Panel::left(id),
            Side::Right => Panel::right(id),
            Side::Top => Panel::top(id),
            Side::Bottom => Panel::bottom(id),
        };
        let thickness = STRIP
            + match side.is_beside() {
                true => f32::from(left + right),
                false => 8.0,
            };
        let shown = strip
            .frame(egui::Frame::new().inner_margin(margin))
            .show_separator_line(false)
            .resizable(false)
            .exact_size(thickness)
            .show(ui, |ui| {
                let focus = self
                    .panels
                    .focus_strip
                    .take_if(|asked| *asked == side)
                    .is_some();
                let part = (none, egui::UiBuilder::new());
                self.part(ui, part, |_, ui| strip_names(ui, side, &panels, focus))
            });
        if let Some((id, stack)) = shown.inner {
            self.layout.reveal(side);
            if stack != 0 {
                self.panels.shown_tabs.retain(|(known, _)| *known != stack);
                self.panels.shown_tabs.push((stack, id));
            }
        }
        Some(shown.response.rect)
    }
}

/// The card of a strip: the arrow that brings the side back, where the
/// one that put it away was, and the name of each panel along it, down
/// it at the sides, read toward the output, and across it above and
/// below. Returns the panel chosen, with its set of tabs; the arrow
/// chooses none in particular.
fn strip_names(
    ui: &mut egui::Ui,
    side: Side,
    panels: &[StripName],
    focus: bool,
) -> Option<(u32, u32)> {
    let theme = theme::current();
    let card = ui.max_rect();
    ui.painter()
        .rect_filled(card, egui::CornerRadius::same(10), theme.background);
    let font = theme::font(Weight::Medium, 10.5);
    // At the top of a strip at the side, at the right of one across.
    let arrow = match side.is_beside() {
        true => egui::Rect::from_min_size(card.min, egui::vec2(card.width(), STRIP)),
        false => egui::Rect::from_min_max(egui::pos2(card.max.x - STRIP, card.min.y), card.max),
    };
    let mut chosen = strip_arrow(ui, side, arrow, focus);
    let mut at = if side.is_beside() { STRIP + 2.0 } else { 6.0 };
    for panel in panels {
        let StripName {
            id, stack, name, ..
        } = panel;
        let galley =
            (ui.painter()).layout_no_wrap(name.to_uppercase(), font.clone(), theme.text_dim);
        let (long, high) = (galley.size().x + 20.0, galley.size().y);
        let item = name_slot(side, card, at, long);
        at += long + 2.0;
        let response = ui.interact(
            item,
            ui.make_persistent_id(("strip name", id)),
            match panel.unreached {
                true => egui::Sense::CLICK,
                false => widgets::press_sense(ui),
            },
        );
        if panel.unreached {
            (ui.ctx()).accesskit_node_builder(response.id, |node| node.set_hidden());
        }
        let label = trf("Show {0}", &[name]);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
        let hovered = response.hovered() || response.has_focus();
        if hovered {
            ui.painter()
                .rect_filled(item.shrink(3.0), egui::CornerRadius::same(7), theme.hover);
        }
        let ink = if hovered { theme.text } else { theme.text_dim };
        paint_name(ui, side, item, galley, (high, ink));
        let response = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(side.toggle_label(false));
        if response.clicked() {
            chosen = Some((*id, *stack));
        }
    }
    ui.allocate_rect(card, egui::Sense::hover());
    chosen
}

/// The arrow of a strip, which brings the side back, in its `arrow` place:
/// it has the keyboard if asked to. Returns what clicking it chooses, no
/// panel in particular.
fn strip_arrow(
    ui: &mut egui::Ui,
    side: Side,
    arrow: egui::Rect,
    focus: bool,
) -> Option<(u32, u32)> {
    let builder = egui::UiBuilder::new()
        .max_rect(arrow)
        .layout(egui::Layout::top_down(egui::Align::Center));
    let icon = widgets::Icon::Unfold(side.edge());
    let back = ui.scope_builder(builder, |ui| {
        widgets::icon_button(ui, icon, side.toggle_label(false), false)
    });
    if focus {
        back.inner.request_focus();
    }
    back.inner.clicked().then_some((0, 0))
}

/// The room on the card of a strip for a name `long` at `at` along it.
fn name_slot(side: Side, card: egui::Rect, at: f32, long: f32) -> egui::Rect {
    match side.is_beside() {
        true => egui::Rect::from_min_size(
            card.min + egui::vec2(0.0, at),
            egui::vec2(card.width(), long),
        ),
        false => egui::Rect::from_min_size(
            card.min + egui::vec2(at, 0.0),
            egui::vec2(long, card.height()),
        ),
    }
}

/// Draws a name in its `item`, which is `high` tall, in `ink`: turned a
/// quarter, so that it reads from the top at the right and from the bottom
/// at the left, as it does in code editors.
fn paint_name(
    ui: &egui::Ui,
    side: Side,
    item: egui::Rect,
    galley: std::sync::Arc<egui::Galley>,
    (high, ink): (f32, egui::Color32),
) {
    let (start, angle) = match side {
        Side::Right => (
            egui::pos2(item.center().x + high / 2.0, item.top() + 10.0),
            std::f32::consts::FRAC_PI_2,
        ),
        Side::Left => (
            egui::pos2(item.center().x - high / 2.0, item.bottom() - 10.0),
            -std::f32::consts::FRAC_PI_2,
        ),
        Side::Top | Side::Bottom => (
            egui::pos2(item.left() + 10.0, item.center().y - high / 2.0),
            0.0,
        ),
    };
    let text = egui::epaint::TextShape::new(start, galley, ink).with_angle(angle);
    ui.painter().add(text);
}
