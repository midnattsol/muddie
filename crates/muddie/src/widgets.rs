//! The interface's own controls, drawn to the theme instead of taking
//! the toolkit's defaults.

use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, Response, RichText, Role, Sense,
    Stroke, TextEdit, Ui, Vec2, WidgetInfo, vec2,
};

use crate::theme::{self, Weight};

/// A switch, for a setting that is simply on or off. `label` names it for
/// screen readers; show it yourself with [`switch_row`] if it needs showing.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let theme = theme::current();
    let size = vec2(34.0, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| WidgetInfo::selected(Role::Switch, ui.is_enabled(), *on, label));
    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_with_time(response.id, *on, 0.12);
        let track = theme::mix(theme.hover, theme.accent, how_on);
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(10), track);
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(2.0),
                CornerRadius::same(12),
                Stroke::new(1.5, theme.accent),
                egui::StrokeKind::Outside,
            );
        }
        let travel = rect.width() - rect.height();
        let centre = rect.left_center() + vec2(rect.height() / 2.0 + travel * how_on, 0.0);
        painter.circle_filled(centre, rect.height() / 2.0 - 2.5, Color32::WHITE);
    }
    response
}

/// A switch with its label to the left and the switch at the right edge.
pub fn switch_row(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            switch(ui, on, label)
        })
        .inner
    })
    .inner
}

/// The button for the main thing to do in a view, filled with the accent.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let theme = theme::current();
    let label = RichText::new(text)
        .font(theme::font(Weight::Medium, 13.5))
        .color(theme.on_accent);
    ui.add(
        egui::Button::new(label)
            .fill(theme.accent)
            .stroke(Stroke::NONE)
            .min_size(vec2(0.0, 30.0)),
    )
}

/// A button with no fill until the pointer is over it, for secondary
/// actions and toolbars.
pub fn quiet_button(ui: &mut Ui, text: &str) -> Response {
    let theme = theme::current();
    let label = RichText::new(text).color(theme.text_dim);
    ui.add(
        egui::Button::new(label)
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE),
    )
}

/// A small heading in capitals over a group of controls.
pub fn caption(ui: &mut Ui, text: &str) {
    let label = RichText::new(text.to_uppercase())
        .font(theme::font(Weight::Medium, 10.5))
        .color(theme::current().text_faint)
        .extra_letter_spacing(0.6);
    ui.label(label);
}

/// A text field with its label above it, as wide as the space allows.
pub fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str) -> Response {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let name = ui.label(
            RichText::new(label)
                .font(theme::font(Weight::Regular, 12.0))
                .color(theme::current().text_dim),
        );
        text_box(ui, value, hint).labelled_by(name.id)
    })
    .inner
}

/// A single-line text box in the interface's style.
pub fn text_box(ui: &mut Ui, value: &mut String, hint: &str) -> Response {
    ui.add(
        TextEdit::singleline(value)
            .hint_text(RichText::new(hint).color(theme::current().text_faint))
            .margin(Margin::symmetric(9, 6))
            .desired_width(f32::INFINITY),
    )
}

/// An entry in a sidebar: its name, and a count at the right if given.
pub fn nav_item(ui: &mut Ui, selected: bool, text: &str, count: Option<usize>) -> Response {
    let theme = theme::current();
    let height = 30.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Tab, ui.is_enabled(), selected, text));
    if ui.is_rect_visible(rect) {
        let fill = match (selected, response.hovered()) {
            (true, _) => theme.raised,
            (false, true) => theme.raised.gamma_multiply(0.6),
            (false, false) => Color32::TRANSPARENT,
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(7), fill);
        let colour = if selected { theme.text } else { theme.text_dim };
        let weight = if selected {
            Weight::Medium
        } else {
            Weight::Regular
        };
        painter.text(
            rect.left_center() + vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            text,
            theme::font(weight, 13.5),
            colour,
        );
        if let Some(count) = count {
            painter.text(
                rect.right_center() - vec2(10.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                count.to_string(),
                theme::font(Weight::Regular, 12.0),
                theme.text_faint,
            );
        }
    }
    response
}

/// A dot that says at a glance how something is doing.
pub fn status_dot(ui: &mut Ui, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, colour);
}

/// A framed block that groups related content.
pub fn card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let theme = theme::current();
    Frame::new()
        .fill(theme.raised.gamma_multiply(0.55))
        .stroke(Stroke::new(1.0, theme.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(14))
        .show(ui, add_contents)
        .inner
}

/// A row of a list: a title with a line of detail under it. A row that is
/// not `active` (a rule switched off, say) is shown faded.
pub fn list_row(ui: &mut Ui, selected: bool, title: &str, detail: &str, active: bool) -> Response {
    let theme = theme::current();
    let height = if detail.is_empty() { 34.0 } else { 48.0 };
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::ListItem, ui.is_enabled(), selected, title));
    if ui.is_rect_visible(rect) {
        let fill = match (selected, response.hovered()) {
            (true, _) => theme.accent.gamma_multiply(0.22),
            (false, true) => theme.raised.gamma_multiply(0.7),
            (false, false) => Color32::TRANSPARENT,
        };
        let fade = if active { 1.0 } else { 0.45 };
        let painter = ui.painter().with_clip_rect(rect.shrink2(vec2(8.0, 0.0)));
        ui.painter().rect_filled(rect, CornerRadius::same(8), fill);
        let left = rect.left() + 12.0;
        let title_y = if detail.is_empty() {
            rect.center().y
        } else {
            rect.top() + 16.0
        };
        painter.text(
            egui::pos2(left, title_y),
            egui::Align2::LEFT_CENTER,
            title,
            theme::font(Weight::Medium, 13.5),
            theme.text.gamma_multiply(fade),
        );
        if !detail.is_empty() {
            painter.text(
                egui::pos2(left, rect.top() + 33.0),
                egui::Align2::LEFT_CENTER,
                detail,
                theme::font(Weight::Regular, 12.0),
                theme.text_dim.gamma_multiply(fade),
            );
        }
    }
    response
}
