//! Rows to choose among: in a sidebar, a list, a group, or as cards.

use super::*;

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

/// A row of a [`group`] that leads somewhere: its title, what it holds,
/// a word about it at the right if there is one, and its menu. Returns
/// the row and the button of its menu.
pub fn link_row(
    ui: &mut Ui,
    title: &str,
    detail: &str,
    badge: Option<&str>,
) -> (Response, Response) {
    row_start(ui);
    let theme = theme::current();
    ui.horizontal(|ui| {
        ui.set_min_height(theme::size::ROW + 6.0);
        let more = ui
            .with_layout(Layout::right_to_left(Align::Center), |ui| {
                let more = icon_button(
                    ui,
                    Icon::More,
                    &crate::i18n::trf("{0} options", &[title]),
                    false,
                );
                if let Some(badge) = badge {
                    let word = RichText::new(badge)
                        .font(theme::font(Weight::Medium, 11.5))
                        .color(theme.accent);
                    ui.add(egui::Label::new(word).selectable(false));
                }
                let (rect, open) = ui.allocate_exact_size(ui.available_size(), Sense::click());
                open.widget_info(|| {
                    WidgetInfo::labeled(Role::Button, true, format!("{title}, {detail}"))
                });
                if open.hovered() {
                    ui.painter().rect_filled(
                        rect.expand2(vec2(8.0, 2.0)),
                        CornerRadius::same(8),
                        theme.hover,
                    );
                }
                let painter = ui.painter();
                let top = rect.left_center() - vec2(0.0, 9.0);
                painter.text(
                    top,
                    egui::Align2::LEFT_CENTER,
                    title,
                    theme::font(Weight::Medium, 14.0),
                    theme.text,
                );
                let under = rect.left_center() + vec2(0.0, 9.0);
                painter.text(
                    under,
                    egui::Align2::LEFT_CENTER,
                    detail,
                    theme::font(Weight::Regular, 12.0),
                    theme.text_dim,
                );
                (more, open)
            })
            .inner;
        (more.1, more.0)
    })
    .inner
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

/// A dot that says at a glance how something is doing.
pub fn status_dot(ui: &mut Ui, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, colour);
}
