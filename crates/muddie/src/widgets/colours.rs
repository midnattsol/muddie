//! Colours to choose.

use super::*;

/// A round swatch to pick a colour by. `label` names it for screen readers.
pub fn swatch(ui: &mut Ui, colour: Color32, selected: bool, label: &str) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(Role::RadioButton, ui.is_enabled(), selected, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.circle_filled(rect.center(), 8.0, colour);
        if selected || response.hovered() || response.has_focus() {
            let ring = if selected {
                theme.text
            } else {
                theme.text_faint
            };
            painter.circle_stroke(rect.center(), 11.0, Stroke::new(1.5, ring));
        }
    }
    response.on_hover_text(label)
}

/// The choice of no colour, beside the [`swatch`]es: an empty ring,
/// struck through.
pub fn swatch_none(ui: &mut Ui, selected: bool, label: &str) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
    response
        .widget_info(|| WidgetInfo::selected(Role::RadioButton, ui.is_enabled(), selected, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let line = Stroke::new(1.3, theme.text_faint);
        painter.circle_stroke(rect.center(), 7.5, line);
        let reach = vec2(5.3, 5.3);
        painter.line_segment(
            [
                rect.center() + vec2(-reach.x, reach.y),
                rect.center() + vec2(reach.x, -reach.y),
            ],
            line,
        );
        if selected || response.hovered() || response.has_focus() {
            let ring = if selected {
                theme.text
            } else {
                theme.text_faint
            };
            painter.circle_stroke(rect.center(), 11.0, Stroke::new(1.5, ring));
        }
    }
    response.on_hover_text(label)
}
