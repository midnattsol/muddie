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
        // The knob takes the colour that reads on the track: on a light
        // accent, a white knob would vanish once switched on.
        let knob = theme::mix(Color32::WHITE, theme.on_accent, how_on);
        painter.circle_filled(centre, rect.height() / 2.0 - 2.5, knob);
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
    let height = 36.0;
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
            rect.left_center() + vec2(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            text,
            theme::font(weight, 13.5),
            colour,
        );
        if let Some(count) = count {
            painter.text(
                rect.right_center() - vec2(12.0, 0.0),
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

/// A saved profile to pick, as a card: a coloured initial, its name,
/// where it leads.
pub fn profile_card(ui: &mut Ui, width: f32, name: &str, place: &str) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 68.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), name));
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() || response.has_focus();
        let lift = ui.ctx().animate_bool_with_time(response.id, hovered, 0.12);
        // The same name always gets the same colour.
        let hues = [12, 10, 11, 13, 14, 9];
        let sum: usize = name.bytes().map(usize::from).sum();
        let hue = theme.terminal[hues[sum % hues.len()]];
        let painter = ui.painter();
        let fill = theme::mix(theme.raised, theme.hover, lift);
        painter.rect_filled(rect, CornerRadius::same(14), fill);
        let edge = theme::mix(theme.border, hue, lift * 0.7);
        painter.rect_stroke(
            rect,
            CornerRadius::same(14),
            Stroke::new(1.0, edge),
            egui::StrokeKind::Inside,
        );
        let tile =
            egui::Rect::from_center_size(rect.left_center() + vec2(35.0, 0.0), Vec2::splat(40.0));
        painter.rect_filled(tile, CornerRadius::same(11), theme::mix(fill, hue, 0.22));
        let initial: String = name.chars().take(1).flat_map(char::to_uppercase).collect();
        painter.text(
            tile.center(),
            egui::Align2::CENTER_CENTER,
            initial,
            theme::font(Weight::Semibold, 16.0),
            hue,
        );
        let painter = painter.with_clip_rect(rect.shrink2(vec2(12.0, 0.0)));
        let left = rect.left() + 68.0;
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
    }
    response
}

/// A round button in the accent with an arrow: go. `label` says where,
/// for screen readers.
pub fn go_button(ui: &mut Ui, label: &str, size: f32) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = response.hovered() || response.has_focus();
        let fill = if hovered {
            theme::mix(theme.accent, Color32::WHITE, 0.14)
        } else {
            theme.accent
        };
        painter.circle_filled(rect.center(), size / 2.0, fill);
        let stroke = Stroke::new(1.8, theme.on_accent);
        let (centre, arm) = (rect.center(), size * 0.17);
        painter.line_segment(
            [centre - vec2(arm * 1.2, 0.0), centre + vec2(arm * 1.2, 0.0)],
            stroke,
        );
        painter.add(egui::Shape::line(
            vec![
                centre + vec2(arm * 0.2, -arm),
                centre + vec2(arm * 1.2, 0.0),
                centre + vec2(arm * 0.2, arm),
            ],
            stroke,
        ));
    }
    response.on_hover_text(label)
}

/// muddie's mark: the sun on the horizon, and its light on the water.
pub fn logo(ui: &mut Ui, size: f32) {
    let theme = theme::current();
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let centre = rect.center();
    let horizon = centre.y + size * 0.06;
    let above = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, horizon - size * 0.05));
    ui.painter().with_clip_rect(above).circle_filled(
        egui::pos2(centre.x, horizon),
        size * 0.3,
        theme.accent,
    );
    for (step, (half, strength)) in [(0.46, 1.0), (0.3, 0.6), (0.16, 0.32)]
        .into_iter()
        .enumerate()
    {
        let y = horizon + size * 0.13 * step as f32;
        let colour = theme.accent.gamma_multiply(strength);
        let stroke = Stroke::new((size * 0.055).max(1.5), colour);
        ui.painter().line_segment(
            [
                egui::pos2(centre.x - size * half, y),
                egui::pos2(centre.x + size * half, y),
            ],
            stroke,
        );
    }
}

/// A soft light that fades out from `centre`, as wide and tall as `radii`.
pub fn glow(painter: &egui::Painter, centre: egui::Pos2, radii: Vec2, colour: Color32) {
    const STEPS: u32 = 64;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(centre, colour);
    // A ring halfway at a quarter strength makes the light fall off softly.
    for (reach, strength) in [(0.5, 0.3), (1.0, 0.0)] {
        for step in 0..STEPS {
            let angle = step as f32 / STEPS as f32 * std::f32::consts::TAU;
            let offset = vec2(angle.cos() * radii.x, angle.sin() * radii.y) * reach;
            mesh.colored_vertex(centre + offset, colour.gamma_multiply(strength));
        }
    }
    for step in 0..STEPS {
        let next = (step + 1) % STEPS;
        let (inner, inner_next) = (1 + step, 1 + next);
        let (outer, outer_next) = (1 + STEPS + step, 1 + STEPS + next);
        mesh.add_triangle(0, inner, inner_next);
        mesh.add_triangle(inner, outer, outer_next);
        mesh.add_triangle(inner, outer_next, inner_next);
    }
    painter.add(mesh);
}

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

/// The pictures of the small buttons of a toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// A target: find where I am.
    Locate,
    /// Four tiles: put in order.
    Tidy,
    /// Stacked sheets: everything at once.
    Layers,
    /// A square: stop.
    Stop,
}

/// A small button with a picture. `label` says what it does, to screen
/// readers and on hover; `on` shows it as switched on.
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str, on: bool) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, ui.is_enabled(), on, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = response.hovered() || response.has_focus();
        if on {
            painter.rect_filled(
                rect,
                CornerRadius::same(7),
                theme.accent.gamma_multiply(0.2),
            );
        } else if hovered {
            painter.rect_filled(rect, CornerRadius::same(7), theme.hover);
        }
        let colour = match (on, hovered) {
            (true, _) => theme.accent,
            (false, true) => theme.text,
            (false, false) => theme.text_dim,
        };
        let stroke = Stroke::new(1.4, colour);
        let centre = rect.center();
        match icon {
            Icon::Locate => {
                painter.circle_stroke(centre, 5.0, stroke);
                painter.circle_filled(centre, 1.6, colour);
                for direction in [
                    vec2(1.0, 0.0),
                    vec2(-1.0, 0.0),
                    vec2(0.0, 1.0),
                    vec2(0.0, -1.0),
                ] {
                    painter
                        .line_segment([centre + direction * 5.0, centre + direction * 8.0], stroke);
                }
            }
            Icon::Tidy => {
                for offset in [
                    vec2(-3.6, -3.6),
                    vec2(3.6, -3.6),
                    vec2(-3.6, 3.6),
                    vec2(3.6, 3.6),
                ] {
                    let tile = egui::Rect::from_center_size(centre + offset, Vec2::splat(5.0));
                    painter.rect_stroke(
                        tile,
                        CornerRadius::same(1),
                        stroke,
                        egui::StrokeKind::Middle,
                    );
                }
            }
            Icon::Layers => {
                for (step, strength) in [(4.5, 0.45), (0.0, 0.7), (-4.5, 1.0)] {
                    let middle = centre + vec2(0.0, step);
                    let sheet = vec![
                        middle + vec2(-7.0, 0.0),
                        middle + vec2(0.0, -3.2),
                        middle + vec2(7.0, 0.0),
                        middle + vec2(0.0, 3.2),
                    ];
                    let stroke = Stroke::new(1.4, colour.gamma_multiply(strength));
                    painter.add(egui::Shape::closed_line(sheet, stroke));
                }
            }
            Icon::Stop => {
                let square = egui::Rect::from_center_size(centre, Vec2::splat(9.0));
                painter.rect_filled(square, CornerRadius::same(2), colour);
            }
        }
    }
    response.on_hover_text(label)
}

/// An editor for Luau code: coloured as it is typed, with the lines
/// numbered and the one being written marked, as tall as `rows` lines at
/// least. `mistake` is the line and the message of what keeps the code
/// from running, shown at that line and under the editor. `label` names
/// the editor for screen readers.
pub fn code_editor(
    ui: &mut Ui,
    label: &str,
    value: &mut String,
    hint: &str,
    rows: usize,
    mistake: Option<&(usize, String)>,
) -> Response {
    let theme = theme::current();
    let font = egui::FontId::monospace(13.0);
    let small = egui::FontId::monospace(11.5);
    // Room for the numbers of as many lines as there are, or will fit.
    let lines = value.split('\n').count().max(rows);
    let digit = ui.fonts_mut(|fonts| fonts.glyph_width(&small, '0'));
    let gutter = lines.to_string().len().max(2) as f32 * digit + 26.0;
    let mut coloured = |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
        let mut job = crate::luau::highlight(text.as_str(), &font, &theme);
        job.wrap.max_width = wrap_width;
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    };
    let margin = Margin {
        left: gutter as i8,
        right: 12,
        top: 10,
        bottom: 10,
    };
    let output = TextEdit::multiline(value)
        .font(font.clone())
        .hint_text(
            RichText::new(hint)
                .font(font.clone())
                .color(theme.text_faint),
        )
        .margin(margin)
        .desired_rows(rows)
        .lock_focus(true)
        .desired_width(f32::INFINITY)
        .layouter(&mut coloured)
        .show(ui);
    let response = output.response.response.clone();
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_label(label);
    });

    let rect = response.rect;
    let painter = ui.painter().with_clip_rect(rect);
    let edge = rect.left() + gutter - 12.0;
    painter.vline(
        edge,
        rect.y_range().shrink(1.0),
        Stroke::new(1.0, theme.border),
    );
    let writing = (output.cursor_range)
        .filter(|_| response.has_focus())
        .map(|range| output.galley.layout_from_cursor(range.primary).row);
    // A line of code can wrap into several rows; only its first is numbered.
    let (mut number, mut first_row) = (1, true);
    for (index, row) in output.galley.rows.iter().enumerate() {
        let placed = row.rect().translate(output.galley_pos.to_vec2());
        let current = writing == Some(index);
        if current {
            let band = egui::Rect::from_x_y_ranges(rect.x_range().shrink(1.0), placed.y_range());
            painter.rect_filled(band, 0.0, theme.text.gamma_multiply(0.045));
        }
        if first_row {
            let wrong = mistake.is_some_and(|(line, _)| *line == number);
            let colour = match (wrong, current) {
                (true, _) => theme.danger,
                (false, true) => theme.text_dim,
                (false, false) => theme.text_faint,
            };
            painter.text(
                egui::pos2(edge - 9.0, placed.center().y),
                egui::Align2::RIGHT_CENTER,
                number.to_string(),
                small.clone(),
                colour,
            );
            if wrong {
                let mark =
                    egui::Rect::from_x_y_ranges(rect.left()..=rect.left() + 3.0, placed.y_range());
                painter.rect_filled(mark, 0.0, theme.danger);
            }
            number += 1;
        }
        first_row = row.ends_with_newline;
    }
    if let Some((line, message)) = mistake {
        let text = format!("{} {line}: {message}", crate::i18n::tr("Line"));
        ui.label(
            RichText::new(text)
                .font(theme::font(Weight::Regular, 12.0))
                .color(theme.danger),
        );
    }
    response
}

/// A choice between a few views, as a row of words with the current one
/// raised. Returns the one clicked.
pub fn segmented(ui: &mut Ui, choices: &[&str], current: usize) -> Option<usize> {
    let theme = theme::current();
    let mut chosen = None;
    Frame::new()
        .fill(theme.field)
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            // In their order, whichever way the row they are in runs.
            let widths = choices.iter().map(|name| {
                let font = theme::font(Weight::Medium, 12.5);
                let text = ui
                    .painter()
                    .layout_no_wrap((*name).to_owned(), font, theme.text);
                text.size().x + 24.0
            });
            let total = widths.sum::<f32>() + 2.0 * choices.len().saturating_sub(1) as f32;
            let row = Layout::left_to_right(Align::Center);
            ui.allocate_ui_with_layout(vec2(total, 24.0), row, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (index, name) in choices.iter().enumerate() {
                    let selected = index == current;
                    let font = theme::font(Weight::Medium, 12.5);
                    let width = ui
                        .painter()
                        .layout_no_wrap((*name).to_owned(), font.clone(), theme.text)
                        .size()
                        .x;
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(width + 24.0, 24.0), Sense::click());
                    response.widget_info(|| {
                        WidgetInfo::selected(Role::Tab, ui.is_enabled(), selected, *name)
                    });
                    let hovered = response.hovered() || response.has_focus();
                    if selected {
                        ui.painter()
                            .rect_filled(rect, CornerRadius::same(7), theme.raised);
                    }
                    let colour = if selected || hovered {
                        theme.text
                    } else {
                        theme.text_dim
                    };
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        *name,
                        font,
                        colour,
                    );
                    if response.clicked() && !selected {
                        chosen = Some(index);
                    }
                }
            });
        });
    chosen
}

/// A card holding rows of settings that belong together, in place of a
/// form: each row names a thing on the left and has its control on the
/// right, with a hairline between rows.
pub fn group<R>(ui: &mut Ui, add_rows: impl FnOnce(&mut Ui) -> R) -> R {
    let theme = theme::current();
    Frame::new()
        .fill(theme.field)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(14, 3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            // Fields inside stand out from the card, not sink into it.
            ui.visuals_mut().text_edit_bg_color = Some(theme.surface);
            ui.set_width(ui.available_width());
            add_rows(ui)
        })
        .inner
}

/// Starts a row of a [`group`]: draws the hairline above every row but
/// the first.
fn row_start(ui: &mut Ui) {
    if ui.min_rect().height() > 2.0 {
        row_divider(ui);
    }
}

/// The hairline between two rows of a [`group`], for a row that is drawn
/// in a region of its own and so cannot tell it is not the first.
pub fn row_divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    let stroke = Stroke::new(1.0, theme::current().border);
    ui.painter().hline(rect.x_range(), rect.center().y, stroke);
}

/// A row of a [`group`]: `label` on the left, the control on the right.
pub fn setting_row(
    ui: &mut Ui,
    label: &str,
    control: &mut dyn FnMut(&mut Ui) -> Response,
) -> Response {
    row_start(ui);
    ui.horizontal(|ui| {
        ui.set_min_height(42.0);
        let name = ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            control(ui).labelled_by(name.id)
        })
        .inner
    })
    .inner
}

/// A row of a [`group`] with a switch.
pub fn switch_setting(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    row_start(ui);
    ui.horizontal(|ui| {
        ui.set_min_height(42.0);
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            switch(ui, on, label)
        })
        .inner
    })
    .inner
}

/// A text field for the right-hand side of a [`setting_row`].
pub fn row_field(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    ui.add(
        TextEdit::singleline(value)
            .hint_text(RichText::new(hint).color(theme::current().text_faint))
            .margin(Margin::symmetric(9, 6))
            .desired_width(width),
    )
}
