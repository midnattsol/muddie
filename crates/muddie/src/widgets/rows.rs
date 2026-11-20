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
/// form, with a hairline between rows. See [`setting_row`].
pub fn group<R>(ui: &mut Ui, add_rows: impl FnOnce(&mut Ui) -> R) -> R {
    let theme = theme::current();
    Frame::new()
        .fill(theme.field)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(16, 0))
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

/// A row of a [`group`]. At the left what it is, and under that, when it
/// needs saying, what it is for: always in sight, not in a tip that only
/// shows under the pointer. At the right its control, in the middle of
/// the row's height. The words wrap in their own column; the control
/// keeps its size. Returns the control.
pub fn setting_row(
    ui: &mut Ui,
    label: &str,
    about: Option<&str>,
    control: &mut dyn FnMut(&mut Ui) -> Response,
) -> Response {
    row_start(ui);
    row(ui, label, about, control)
}

/// A row of a [`group`] with a switch.
pub fn switch_setting(ui: &mut Ui, on: &mut bool, label: &str, about: Option<&str>) -> Response {
    row_start(ui);
    row(ui, label, about, &mut |ui| switch(ui, on, label))
}

/// The room above and below the words of a row, and the least height of
/// one, a little more than a control's row so that the cards breathe.
const ROW_PADDING: f32 = 12.0;
const ROW_HEIGHT: f32 = theme::size::ROW + 4.0;
/// The room between the words of a row and its control.
const ROW_GAP: f32 = 16.0;

/// The words and the control of a row, with no hairline above. How tall
/// the words are is known once they are laid out, so it is kept for the
/// next frame, and a frame laid out with the wrong height is done again.
pub(super) fn row(
    ui: &mut Ui,
    label: &str,
    about: Option<&str>,
    control: &mut dyn FnMut(&mut Ui) -> Response,
) -> Response {
    let id = ui.scope_id().with(("row", label));
    let words = ui
        .ctx()
        .data(|data| data.get_temp::<f32>(id))
        .unwrap_or(0.0);
    let height = (words + 2.0 * ROW_PADDING).max(ROW_HEIGHT);
    let size = vec2(ui.available_width(), height);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), |ui| {
        ui.set_min_size(size);
        let control = control(ui);
        ui.add_space(ROW_GAP);
        let column = vec2(ui.available_width(), height);
        let (title, measured) = ui
            .allocate_ui_with_layout(column, Layout::top_down(Align::Min), |ui| {
                ui.add_space(((height - words) / 2.0).max(0.0));
                row_words(ui, label, about)
            })
            .inner;
        if (measured - words).abs() > 0.5 {
            ui.ctx().data_mut(|data| data.insert_temp(id, measured));
            ui.ctx().request_discard("a row's words changed height");
        }
        if let Some(about) = about {
            (ui.ctx()).accesskit_node_builder(control.id, |node| node.set_description(about));
        }
        control.labelled_by(title.id)
    })
    .inner
}

/// The title of a row, and what it is for under it. Returns the title and
/// how tall the two are.
fn row_words(ui: &mut Ui, label: &str, about: Option<&str>) -> (Response, f32) {
    const BETWEEN: f32 = 2.0;
    ui.spacing_mut().item_spacing.y = BETWEEN;
    let theme = theme::current();
    let title = ui.add(egui::Label::new(RichText::new(label).color(theme.text)).wrap());
    let mut height = title.rect.height();
    if let Some(about) = about {
        let note = RichText::new(about)
            .font(theme::font(Weight::Regular, 12.0))
            .color(theme.text_dim);
        height += BETWEEN + ui.add(egui::Label::new(note).wrap()).rect.height();
    }
    (title, height)
}

/// A note on a page, outside its cards: what there is not yet, or what
/// something just done said.
pub fn page_note(ui: &mut Ui, text: &str) {
    let note = RichText::new(text)
        .font(theme::font(Weight::Regular, 12.0))
        .color(theme::current().text_dim);
    ui.add(egui::Label::new(note).wrap());
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
