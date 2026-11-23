//! Switches: on or off, with or without their label beside them.

use super::*;

/// A switch, for a setting that is simply on or off. `label` names it for
/// screen readers; show it yourself with [`switch_row`] if it needs showing.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    sized_switch(ui, on, label, vec2(40.0, 22.0))
}

/// A smaller switch, for one of many in a list, which should not outweigh
/// what it switches.
pub fn small_switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    sized_switch(ui, on, label, vec2(30.0, 17.0))
}

fn sized_switch(ui: &mut Ui, on: &mut bool, label: &str, size: Vec2) -> Response {
    let theme = theme::current();
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::selected(Role::Switch, ui.is_enabled(), *on, label));
    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_with_time(response.id, *on, 0.14);
        let painter = ui.painter();
        let round = CornerRadius::same((size.y / 2.0) as u8);
        // Off: a hollow track and a grey knob. On: the track filled with
        // the accent, and a knob that stands out from it.
        let hollow = if response.hovered() {
            theme.hover
        } else {
            theme.raised
        };
        painter.rect_filled(rect, round, theme::mix(hollow, theme.accent, how_on));
        let edge = theme.text_faint.gamma_multiply(0.6 * (1.0 - how_on));
        painter.rect_stroke(
            rect,
            round,
            Stroke::new(1.0, edge),
            egui::StrokeKind::Inside,
        );
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(2.5),
                CornerRadius::same((size.y / 2.0 + 3.0) as u8),
                Stroke::new(1.5, theme.accent),
                egui::StrokeKind::Outside,
            );
        }
        let travel = rect.width() - rect.height();
        let centre = rect.left_center() + vec2(rect.height() / 2.0 + travel * how_on, 0.0);
        // A white knob, as switches have; on an accent too light for it
        // to be seen, such as the yellow of high contrast, a dark one.
        let seen = crate::output_style::contrast(Color32::WHITE, theme.accent) >= 1.8;
        let knob_on = if seen {
            Color32::WHITE
        } else {
            theme.on_accent
        };
        let knob = theme::mix(theme.text_dim, knob_on, how_on);
        let radius = rect.height() / 2.0 - 3.0;
        painter.circle_filled(
            centre + vec2(0.0, 1.0),
            radius + 0.5,
            Color32::from_black_alpha(50),
        );
        painter.circle_filled(centre, radius, knob);
    }
    response
}
