//! Groups of settings: their rows, captions, notes and notices.

use super::*;

/// A small heading in capitals over a group of controls.
pub fn caption(ui: &mut Ui, text: &str) {
    let label = RichText::new(text.to_uppercase())
        .font(theme::font(Weight::Medium, 11.0))
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

/// The name of a part of a [`card`], over it.
pub fn sub_label(ui: &mut Ui, text: &str) {
    let label = RichText::new(text)
        .font(theme::font(Weight::Regular, 12.5))
        .color(theme::current().text_dim);
    ui.add(egui::Label::new(label).selectable(false));
}

/// A card of a page, as the settings have, with a heading of its own: a
/// picture, its title, and at the right what acts on the whole of it. A
/// screen reader is told it is a group, by its title.
pub fn card<R>(
    ui: &mut Ui,
    glyph: Option<Glyph>,
    title: &str,
    right: impl FnOnce(&mut Ui),
    body: impl FnOnce(&mut Ui) -> R,
) -> R {
    let theme = theme::current();
    let builder = egui::UiBuilder::new()
        .accessibility_role(Role::Group)
        .accessibility_label(title);
    ui.scope_builder(builder, |ui| {
        Frame::new()
            .fill(theme.field)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin {
                left: 16,
                right: 16,
                top: 12,
                bottom: 14,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::field_look(ui.visuals_mut(), theme.surface, &theme);
                let id = ui.make_persistent_id(("card", title));
                right_last(
                    ui,
                    id,
                    &mut (),
                    |_, ui| {
                        if let Some(glyph) = glyph {
                            super::glyph(ui, glyph, 13.0, theme.accent);
                        }
                        let heading = RichText::new(title)
                            .font(theme::font(Weight::Semibold, 13.5))
                            .color(theme.text);
                        ui.add(egui::Label::new(heading).selectable(false));
                    },
                    |_, ui| right(ui),
                );
                ui.add_space(8.0);
                body(ui)
            })
            .inner
    })
    .inner
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
            // What is typed in or chosen from stands out from the card,
            // not sinks into it.
            theme::field_look(ui.visuals_mut(), theme.surface, &theme);
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
/// The most of a row its control may take.
const CONTROL_SHARE: f32 = 0.6;

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
        // The control is laid out first, from the right: it may take at
        // most this share of the row, so that a long value (cut short, and
        // whole under the pointer) leaves its words the room to be read.
        let most = vec2(size.x * CONTROL_SHARE, height);
        let control = ui
            .allocate_ui_with_layout(most, Layout::right_to_left(Align::Center), |ui| control(ui))
            .inner;
        ui.add_space(ROW_GAP);
        let column = vec2(ui.available_width(), height);
        let (shown, measured) = ui
            .allocate_ui_with_layout(column, Layout::top_down(Align::Min), |ui| {
                ui.add_space(((height - words) / 2.0).max(0.0));
                row_words(ui, label, about)
            })
            .inner;
        if (measured - words).abs() > 0.5 {
            ui.ctx().data_mut(|data| data.insert_temp(id, measured));
            ui.ctx().request_discard("a row's words changed height");
        }
        // The control is named and described by the row's words, and they
        // are not read again apart from it: a screen reader does not know
        // what words a control was labelled by, and reads them on their own.
        (ui.ctx()).accesskit_node_builder(control.id, |node| {
            node.set_label(label);
            // What a dropdown has chosen is told to screen readers on
            // Linux only as part of what it is described by: elsewhere
            // they read it themselves.
            let chosen = (cfg!(target_os = "linux")
                && node.role() == egui::accesskit::Role::ComboBox)
                .then(|| node.value().map(str::to_owned))
                .flatten();
            match (chosen, about) {
                (Some(chosen), Some(about)) => node.set_description(format!("{chosen}. {about}")),
                (Some(chosen), None) => node.set_description(chosen),
                (None, Some(about)) => node.set_description(about),
                (None, None) => {}
            }
        });
        for words in shown {
            (ui.ctx()).accesskit_node_builder(words, |node| node.set_hidden());
        }
        control
    })
    .inner
}

/// The title of a row, and what it is for under it. Returns the two, and
/// how tall they are.
fn row_words(ui: &mut Ui, label: &str, about: Option<&str>) -> (Vec<egui::Id>, f32) {
    const BETWEEN: f32 = 2.0;
    ui.spacing_mut().item_spacing.y = BETWEEN;
    let theme = theme::current();
    let title = ui.add(egui::Label::new(RichText::new(label).color(theme.text)).wrap());
    let mut height = title.rect.height();
    let mut shown = vec![title.id];
    if let Some(about) = about {
        let note = RichText::new(about)
            .font(theme::font(Weight::Regular, 12.0))
            .color(theme.text_dim);
        let note = ui.add(egui::Label::new(note).wrap());
        height += BETWEEN + note.rect.height();
        shown.push(note.id);
    }
    (shown, height)
}

/// A note on a page, outside its cards: what there is not yet, or what
/// something just done said.
pub fn page_note(ui: &mut Ui, text: &str) {
    let note = RichText::new(text)
        .font(theme::font(Weight::Regular, 12.0))
        .color(theme::current().text_dim);
    ui.add(egui::Label::new(note).wrap());
}

/// A notice over the corner of the window, which moves nothing: what it
/// says, marked if it is that something could not be done, and what can
/// be done about it under it, if anything.
pub fn toast(
    ui: &mut Ui,
    text: &str,
    problem: bool,
    actions: Option<&mut dyn FnMut(&mut Ui)>,
) -> Response {
    let theme = theme::current();
    let edge = if problem { theme.danger } else { theme.border };
    let shown = Frame::new()
        .fill(theme.raised)
        .stroke(Stroke::new(1.0, edge))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .shadow(egui::epaint::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(60),
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let colour = if problem { theme.danger } else { theme.text };
            ui.add(egui::Label::new(RichText::new(text).color(colour)).wrap());
            if let Some(actions) = actions {
                ui.add_space(6.0);
                ui.with_layout(Layout::right_to_left(Align::Center), actions);
            }
        });
    shown.response
}
