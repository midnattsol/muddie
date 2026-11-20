//! The rows of a menu.

use super::*;

/// Gives a menu room to breathe; call it first in the menu.
pub fn menu_width(ui: &mut Ui) {
    ui.set_min_width(232.0);
    ui.spacing_mut().item_spacing.y = 1.0;
}

/// How wide a row of this menu is: as wide as its widest, known from the
/// frame before, and at least `needed`.
pub(super) fn menu_row_width(ui: &mut Ui, needed: f32) -> f32 {
    let menu = ui.scope_id().with("row width");
    let widest = ui.data(|data| data.get_temp::<f32>(menu)).unwrap_or(232.0);
    let width = needed.max(widest);
    ui.data_mut(|data| data.insert_temp(menu, width));
    width
}

/// A line between groups of a menu.
pub fn menu_separator(ui: &mut Ui) {
    let width = menu_row_width(ui, 0.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 9.0), Sense::hover());
    let stroke = Stroke::new(1.0, theme::current().border);
    ui.painter()
        .hline(rect.x_range().shrink(6.0), rect.center().y, stroke);
}

/// A line of text in a menu that is not a choice: what the menu is about.
pub fn menu_heading(ui: &mut Ui, text: &str) {
    let label = RichText::new(text)
        .font(theme::font(Weight::Regular, 12.0))
        .color(theme::current().text_faint);
    Frame::new()
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| ui.label(label));
}

/// A choice in a menu, with its keyboard shortcut at the right if it has
/// one.
pub fn menu_item(ui: &mut Ui, text: &str, shortcut: &str) -> Response {
    menu_row(ui, None, text, shortcut)
}

/// A choice in a menu that is on or off; a tick shows it is on.
pub fn menu_check(ui: &mut Ui, on: &mut bool, text: &str) -> Response {
    let mut response = menu_row(ui, Some(*on), text, "");
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response
}

pub(super) fn menu_row(ui: &mut Ui, checked: Option<bool>, text: &str, shortcut: &str) -> Response {
    let theme = theme::current();
    let font = theme::font(Weight::Regular, 13.5);
    let small = theme::font(Weight::Regular, 12.0);
    let measure = |text: &str, font: &egui::FontId| {
        let galley = ui
            .painter()
            .layout_no_wrap(text.to_owned(), font.clone(), theme.text);
        galley.size().x
    };
    // The tick has the right edge to itself, wherever there can be one.
    let tick = if checked.is_some() { 24.0 } else { 0.0 };
    let right = if shortcut.is_empty() {
        22.0_f32.max(tick)
    } else {
        measure(shortcut, &small) + 28.0 + tick
    };
    let needed = 10.0 + measure(text, &font) + right + 10.0;
    let width = menu_row_width(ui, needed);
    let size = vec2(width, 30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| match checked {
        Some(on) => WidgetInfo::selected(Role::MenuItemCheckBox, enabled, on, text),
        None => WidgetInfo::labeled(Role::MenuItem, enabled, text),
    });
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if enabled && (response.hovered() || response.has_focus()) {
            painter.rect_filled(rect, CornerRadius::same(6), theme.hover);
        }
        let colour = if enabled {
            theme.text
        } else {
            theme.text_faint
        };
        painter.text(
            rect.left_center() + vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            text,
            font,
            colour,
        );
        if !shortcut.is_empty() {
            painter.text(
                rect.right_center() - vec2(10.0 + tick, 0.0),
                egui::Align2::RIGHT_CENTER,
                shortcut,
                small,
                theme.text_faint,
            );
        }
        if checked == Some(true) {
            let centre = rect.right_center() - vec2(17.0, 0.0);
            let tick = [
                centre + vec2(-4.5, 0.0),
                centre + vec2(-1.5, 3.2),
                centre + vec2(4.5, -3.5),
            ];
            painter.add(egui::Shape::line(
                tick.to_vec(),
                Stroke::new(1.6, theme.accent),
            ));
        }
    }
    response
}
