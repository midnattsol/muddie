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

/// A section of a page: its caption, and the card of rows under it.
pub fn section<R>(ui: &mut Ui, title: &str, add_rows: impl FnOnce(&mut Ui) -> R) -> R {
    section_caption(ui, title);
    group(ui, add_rows)
}

/// The caption of a section of a page, set well apart from what comes
/// before it, and close to what it names.
pub fn section_caption(ui: &mut Ui, title: &str) {
    section_caption_with(ui, title, |_| {});
}

/// The caption of a section, with something that acts on the whole of it
/// at the right, such as a quiet button.
pub fn section_caption_with(ui: &mut Ui, title: &str, right: impl FnOnce(&mut Ui)) {
    if ui.min_rect().height() > 1.0 {
        ui.add_space(theme::size::GAP_L - ui.spacing().item_spacing.y);
    }
    ui.horizontal(|ui| {
        caption(ui, title);
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
    ui.add_space(2.0);
}

/// A word about something, in a pill: `strong` in the accent, for what is
/// so now, or else quiet, for what is not.
pub fn badge(ui: &mut Ui, text: &str, strong: bool) {
    let theme = theme::current();
    let (colour, fill) = match strong {
        true => (theme.accent, theme.accent.gamma_multiply(0.16)),
        false => (theme.text_dim, theme.raised),
    };
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), theme::font(Weight::Medium, 11.5), colour);
    let (rect, said) = ui.allocate_exact_size(galley.size() + vec2(16.0, 6.0), Sense::hover());
    said.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    ui.painter()
        .rect_filled(rect, CornerRadius::same(u8::MAX), fill);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, colour);
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
