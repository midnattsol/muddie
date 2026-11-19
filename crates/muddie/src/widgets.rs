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

/// Gives a menu room to breathe; call it first in the menu.
pub fn menu_width(ui: &mut Ui) {
    ui.set_min_width(232.0);
    ui.spacing_mut().item_spacing.y = 1.0;
}

/// How wide a row of this menu is: as wide as its widest, known from the
/// frame before, and at least `needed`.
fn menu_row_width(ui: &mut Ui, needed: f32) -> f32 {
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

fn menu_row(ui: &mut Ui, checked: Option<bool>, text: &str, shortcut: &str) -> Response {
    let theme = theme::current();
    let font = theme::font(Weight::Regular, 13.5);
    let small = theme::font(Weight::Regular, 12.0);
    let measure = |text: &str, font: &egui::FontId| {
        let galley = ui
            .painter()
            .layout_no_wrap(text.to_owned(), font.clone(), theme.text);
        galley.size().x
    };
    let right = if shortcut.is_empty() {
        22.0
    } else {
        measure(shortcut, &small) + 28.0
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
                rect.right_center() - vec2(10.0, 0.0),
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

/// The cross that closes something. `label` says what, for screen readers.
pub fn close_button(ui: &mut Ui, label: &str, size: f32) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = response.hovered() || response.has_focus();
        if hovered {
            painter.rect_filled(rect, CornerRadius::same(6), theme.hover);
        }
        let colour = if hovered {
            theme.text
        } else {
            theme.text_faint
        };
        let arm = size * 0.2;
        let stroke = Stroke::new(1.3, colour);
        let centre = rect.center();
        painter.line_segment(
            [centre - Vec2::splat(arm), centre + Vec2::splat(arm)],
            stroke,
        );
        painter.line_segment([centre + vec2(arm, -arm), centre + vec2(-arm, arm)], stroke);
    }
    response.on_hover_text(label)
}

/// A dialog over the window, which is dimmed behind it: a title, a cross
/// to close it, and the contents. Escape and a click outside close it too.
pub fn dialog<R>(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    open: &mut bool,
    width: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    if !*open {
        return None;
    }
    let theme = theme::current();
    let frame = Frame::new()
        .fill(theme.surface)
        .stroke(Stroke::new(1.0, theme.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(22))
        .shadow(egui::Shadow {
            offset: [0, 16],
            blur: 48,
            spread: 0,
            color: Color32::from_black_alpha(if theme.dark { 150 } else { 60 }),
        });
    let mut closed = false;
    let width = width.min(ctx.content_rect().width() - 80.0);
    let modal = egui::Modal::new(egui::Id::unique(id))
        .backdrop_color(Color32::from_black_alpha(if theme.dark { 150 } else { 70 }))
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                let heading = RichText::new(title)
                    .font(theme::font(Weight::Semibold, 17.0))
                    .color(theme.text);
                ui.add(egui::Label::new(heading).selectable(false));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    closed = close_button(ui, crate::i18n::tr("Close"), 24.0).clicked();
                });
            });
            ui.add_space(8.0);
            add_contents(ui)
        });
    if closed || modal.should_close() {
        *open = false;
    }
    Some(modal.inner)
}

/// The primary button at the size of a view's one main action.
pub fn big_primary_button(ui: &mut Ui, text: &str) -> Response {
    let theme = theme::current();
    let label = RichText::new(text)
        .font(theme::font(Weight::Medium, 14.0))
        .color(theme.on_accent);
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = vec2(18.0, 8.0);
        ui.add(
            egui::Button::new(label)
                .fill(theme.accent)
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::same(9))
                .min_size(vec2(0.0, 40.0)),
        )
    })
    .inner
}

/// A saved profile to pick: a coloured initial, its name, where it leads.
pub fn profile_row(ui: &mut Ui, name: &str, place: &str) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), name));
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() || response.has_focus();
        let painter = ui.painter().with_clip_rect(rect);
        if hovered {
            painter.rect_filled(rect, CornerRadius::same(9), theme.hover);
        }
        // The same name always gets the same colour.
        let hues = [12, 10, 11, 13, 14, 9];
        let sum: usize = name.bytes().map(usize::from).sum();
        let hue = theme.terminal[hues[sum % hues.len()]];
        let centre = rect.left_center() + vec2(30.0, 0.0);
        painter.circle_filled(centre, 17.0, theme::mix(theme.surface, hue, 0.2));
        let initial: String = name.chars().take(1).flat_map(char::to_uppercase).collect();
        painter.text(
            centre,
            egui::Align2::CENTER_CENTER,
            initial,
            theme::font(Weight::Semibold, 14.5),
            hue,
        );
        let left = rect.left() + 60.0;
        painter.text(
            egui::pos2(left, rect.center().y - 9.0),
            egui::Align2::LEFT_CENTER,
            name,
            theme::font(Weight::Medium, 14.5),
            theme.text,
        );
        painter.text(
            egui::pos2(left, rect.center().y + 10.0),
            egui::Align2::LEFT_CENTER,
            place,
            theme::font(Weight::Regular, 12.5),
            theme.text_dim,
        );
        // A chevron: this row goes somewhere.
        let tip = rect.right_center() - vec2(20.0, 0.0);
        let colour = if hovered {
            theme.text
        } else {
            theme.text_faint
        };
        painter.add(egui::Shape::line(
            vec![tip + vec2(-4.0, -5.0), tip, tip + vec2(-4.0, 5.0)],
            Stroke::new(1.5, colour),
        ));
    }
    response
}
