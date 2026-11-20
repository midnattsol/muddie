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

/// A saved profile to pick, as a card: a coloured initial, its name,
/// where it leads.
pub fn profile_card(ui: &mut Ui, width: f32, name: &str, place: &str) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 68.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), name));
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() || response.has_focus();
        let lift = ui.ctx().animate_bool_with_time(response.id, hovered, 0.12);
        let hue = profile_hue(name);
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
        initial_tile(painter, tile, name, fill);
        let painter = painter.with_clip_rect(rect.shrink2(vec2(12.0, 0.0)));
        two_lines(
            &painter,
            rect.left() + 68.0,
            rect.center().y,
            (name, 14.5),
            (place, 12.5),
        );
    }
    response
}

/// A saved profile in the list of the Profiles window: its coloured
/// initial, its name and where it leads, lit up when chosen or under the
/// pointer, and a dot when a session plays with it.
pub fn profile_row(ui: &mut Ui, selected: bool, name: &str, place: &str, in_use: bool) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
    let said = match in_use {
        true => format!("{name}, {place}, {}", crate::i18n::tr("In use")),
        false => format!("{name}, {place}"),
    };
    response.widget_info(|| WidgetInfo::selected(Role::Tab, ui.is_enabled(), selected, &said));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hovered = response.hovered() || response.has_focus();
    let fill = match (selected, hovered) {
        (true, _) => theme.raised,
        (false, true) => theme.raised.gamma_multiply(0.6),
        (false, false) => Color32::TRANSPARENT,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), fill);
    let tile =
        egui::Rect::from_center_size(rect.left_center() + vec2(24.0, 0.0), Vec2::splat(32.0));
    initial_tile(
        painter,
        tile,
        name,
        theme::mix(theme.surface, fill, fill.a() as f32 / 255.0),
    );
    let clip = rect.with_max_x(rect.right() - if in_use { 24.0 } else { 10.0 });
    two_lines(
        &painter.with_clip_rect(clip),
        rect.left() + 50.0,
        rect.center().y,
        (name, 13.5),
        (place, 12.0),
    );
    if in_use {
        painter.circle_filled(rect.right_center() - vec2(14.0, 0.0), 3.5, theme.accent);
    }
    response
}

/// A profile's initial on a tile of its colour, over `ground`.
pub fn initial_tile(painter: &egui::Painter, tile: egui::Rect, name: &str, ground: Color32) {
    let hue = profile_hue(name);
    let radius = CornerRadius::same((tile.height() * 0.28) as u8);
    painter.rect_filled(tile, radius, theme::mix(ground, hue, 0.22));
    let initial: String = name.chars().take(1).flat_map(char::to_uppercase).collect();
    let size = tile.height() * 0.4;
    painter.text(
        tile.center(),
        egui::Align2::CENTER_CENTER,
        initial,
        theme::font(Weight::Semibold, size),
        hue,
    );
}

/// The colour of a profile: the same name always gets the same one.
fn profile_hue(name: &str) -> Color32 {
    let hues = [12, 10, 11, 13, 14, 9];
    let sum: usize = name.bytes().map(usize::from).sum();
    theme::current().terminal[hues[sum % hues.len()]]
}

/// A name, and a line about it under it, starting at `left` and centred
/// on `middle`; the sizes of their type are given with them.
fn two_lines(
    painter: &egui::Painter,
    left: f32,
    middle: f32,
    (name, big): (&str, f32),
    (under, small): (&str, f32),
) {
    let theme = theme::current();
    let at = |dy: f32| egui::pos2(left, middle + dy);
    painter.text(
        at(-9.0),
        egui::Align2::LEFT_CENTER,
        name,
        theme::font(Weight::Medium, big),
        theme.text,
    );
    painter.text(
        at(10.0),
        egui::Align2::LEFT_CENTER,
        under,
        theme::font(Weight::Regular, small),
        theme.text_dim,
    );
}

/// A dot that says at a glance how something is doing.
pub fn status_dot(ui: &mut Ui, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, colour);
}
