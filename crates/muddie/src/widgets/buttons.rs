//! Buttons, of every weight, and the choices made by pressing one.

use super::*;

/// The ordinary button, for something to do that is not the main thing.
pub fn button(ui: &mut Ui, text: impl Into<egui::WidgetText>) -> Response {
    ui.add(egui::Button::new(text).min_size(vec2(0.0, theme::size::CONTROL)))
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
            .min_size(vec2(0.0, theme::size::CONTROL)),
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

/// A small button that shows a sign, such as `+`: `label` says what it
/// does, to a screen reader and when pointed at. Without a `frame` it is
/// only the sign until the pointer is over it.
pub fn sign_button(ui: &mut Ui, sign: &str, label: &str, frame: bool) -> Response {
    let theme = theme::current();
    let text = RichText::new(sign)
        .font(theme::font(Weight::Regular, 16.0))
        .color(if frame { theme.text } else { theme.text_dim });
    let response = ui.add(
        egui::Button::new(text)
            .frame(frame)
            .min_size(vec2(26.0, 26.0)),
    );
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_label(label);
    });
    response.on_hover_text(label)
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

/// What looks like a field to search in and opens the palette: where
/// anything can be found, with the keys that open it from anywhere.
pub fn palette_button(ui: &mut Ui, width: f32, keys: &str) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click());
    let said = crate::i18n::tr("Search or run a command");
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), said));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if response.hovered() {
            theme.hover
        } else {
            theme.field
        };
        painter.rect_filled(rect, CornerRadius::same(8), fill);
        painter.rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(1.0, theme.border),
            egui::StrokeKind::Inside,
        );
        // A magnifying glass, the words, and the keys at the far end.
        let glass = rect.left_center() + vec2(14.0, -1.0);
        let stroke = Stroke::new(1.3, theme.text_faint);
        painter.circle_stroke(glass, 4.0, stroke);
        painter.line_segment([glass + vec2(3.0, 3.0), glass + vec2(6.0, 6.0)], stroke);
        let small = theme::font(Weight::Regular, 12.5);
        let clip = painter.with_clip_rect(rect.shrink(4.0));
        clip.text(
            rect.left_center() + vec2(28.0, 0.0),
            egui::Align2::LEFT_CENTER,
            said,
            small.clone(),
            theme.text_faint,
        );
        clip.text(
            rect.right_center() - vec2(10.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            keys,
            small,
            theme.text_faint,
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

/// A tab of a row of them: the one shown is filled, and one with
/// something in it that has not been looked at stands out, with a dot,
/// and says so to a screen reader.
pub fn tab(ui: &mut Ui, text: &str, on: bool, fresh: bool) -> Response {
    let theme = theme::current();
    let weight = if on || fresh {
        Weight::Medium
    } else {
        Weight::Regular
    };
    let marked = crate::panels::colour_named("yellow").unwrap_or(theme.accent);
    let colour = match (on, fresh) {
        (true, _) => theme.text,
        (false, true) => marked,
        (false, false) => theme.text_dim,
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::font(weight, 12.5), colour);
    // Room for the dot beside the name.
    let dot = if fresh { 10.0 } else { 0.0 };
    let size = vec2(galley.size().x + 18.0 + dot, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        let said = match fresh {
            true => format!("{text}, {}", crate::i18n::tr("new")),
            false => text.to_owned(),
        };
        WidgetInfo::selected(Role::Tab, ui.is_enabled(), on, said)
    });
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if on {
            painter.rect_filled(rect, CornerRadius::same(7), theme.raised);
        } else if response.hovered() || response.has_focus() {
            painter.rect_filled(
                rect,
                CornerRadius::same(7),
                theme.raised.gamma_multiply(0.6),
            );
        }
        let at = egui::pos2(rect.left() + 9.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(at, galley, colour);
        if fresh {
            painter.circle_filled(rect.right_center() - vec2(9.0, 0.0), 3.0, marked);
        }
    }
    response
}
