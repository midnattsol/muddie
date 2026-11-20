//! Groups of settings: their rows, captions, notes and notices.

use super::*;

/// A small heading in capitals over a group of controls.
pub fn caption(ui: &mut Ui, text: &str) {
    let label = RichText::new(text.to_uppercase())
        .font(theme::font(Weight::Medium, 10.5))
        .color(theme::current().text_faint)
        .extra_letter_spacing(0.6);
    ui.label(label);
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
pub(super) fn row_start(ui: &mut Ui) {
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
        ui.set_min_height(theme::size::ROW);
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
    switch_row(ui, on, label)
}

/// What a row of a [`group`] is for, under it, always in sight: not in
/// a tip that only shows under the pointer.
pub fn row_note(ui: &mut Ui, text: &str) {
    let note = RichText::new(text)
        .font(theme::font(Weight::Regular, 12.0))
        .color(theme::current().text_dim);
    ui.add(egui::Label::new(note).wrap());
    ui.add_space(10.0);
}

/// A bar that says something about the page, with what to do about it at
/// the right.
pub fn notice_bar(ui: &mut Ui, text: &str, add: &mut dyn FnMut(&mut Ui)) {
    let theme = theme::current();
    Frame::new()
        .fill(theme.accent.gamma_multiply(0.14))
        .stroke(Stroke::new(1.0, theme.accent.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(text).color(theme.text));
                ui.with_layout(Layout::right_to_left(Align::Center), add);
            });
        });
}
