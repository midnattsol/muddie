//! Where text and numbers are typed or chosen.

use super::*;

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

/// A single-line text box, as wide as the space allows.
pub fn text_box(ui: &mut Ui, value: &mut String, hint: &str) -> Response {
    named_by_hint(ui.add(single_line(value, hint, f32::INFINITY)), hint)
}

/// A field for something secret, as a password: its characters hidden.
pub fn secret_field(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    named_by_hint(ui.add(single_line(value, hint, width).password(true)), hint)
}

/// A text field for the right-hand side of a [`setting_row`].
pub fn row_field(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    named_by_hint(ui.add(single_line(value, hint, width)), hint)
}

/// One line to type in, `width` wide and as tall as every other control,
/// with `hint` in it while it is empty.
///
/// A field with only a hint is named by it for a screen reader, which
/// otherwise reads the hint as what is in it; one in a row is named by the
/// row, after.
fn single_line<'a>(value: &'a mut String, hint: &str, width: f32) -> TextEdit<'a> {
    TextEdit::singleline(value)
        .hint_text(RichText::new(hint).color(theme::current().text_faint))
        .margin(Margin::symmetric(9, 6))
        .desired_width(width)
        .min_size(vec2(0.0, theme::size::CONTROL))
}

/// A number to drag, or to click and type.
pub fn number<N: egui::emath::Numeric>(value: &mut N) -> egui::DragValue<'_> {
    egui::DragValue::new(value)
}

/// A number chosen along a line.
pub fn slider<N: egui::emath::Numeric>(
    value: &mut N,
    range: std::ops::RangeInclusive<N>,
) -> egui::Slider<'_> {
    egui::Slider::new(value, range)
}

/// A list to choose one thing from, shown when pressed, as wide as any
/// field. Give it what is chosen with `selected_text`, and its rows with
/// [`choice`].
pub fn dropdown(id: &str) -> egui::ComboBox<'_> {
    egui::ComboBox::from_id_salt(id)
        .height(320.0)
        .width(theme::size::FIELD_L)
}

/// One of a few values, chosen from a [`dropdown`] that names each. A
/// value not among them is shown by `other`. The response is marked
/// changed when another is chosen.
pub fn choose<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: &str,
    value: &mut T,
    choices: &[(T, String)],
    other: impl Fn(T) -> String,
) -> Response {
    let shown = (choices.iter())
        .find(|(choice, _)| *choice == *value)
        .map_or_else(|| other(*value), |(_, name)| name.clone());
    let mut changed = false;
    let mut response = dropdown(id)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (choice, name) in choices {
                changed |= choice_value(ui, value, *choice, name).changed();
            }
        })
        .response;
    if changed {
        response.mark_changed();
    }
    response
}

/// A row of a [`dropdown`], ticked like a choice of a menu if it is the
/// one chosen.
pub fn choice(ui: &mut Ui, selected: bool, text: &str) -> Response {
    menu_row(ui, Some(selected), None, text, "")
}

/// A row of a [`dropdown`] that sets `current` to `value` when chosen.
pub fn choice_value<V: PartialEq>(ui: &mut Ui, current: &mut V, value: V, text: &str) -> Response {
    let mut response = choice(ui, *current == value, text);
    if response.clicked() && *current != value {
        *current = value;
        response.mark_changed();
    }
    response
}

/// An editor of code, coloured as `language` is as it is typed, with the lines
/// numbered and the one being written marked, as tall as what it holds
/// and a line more, and `rows` lines at least. `mistake` is the line and the message of what keeps the code
/// from running, shown at that line and under the editor. `label` names
/// the editor for screen readers.
pub fn code_editor(
    ui: &mut Ui,
    label: &str,
    value: &mut String,
    hint: &str,
    language: Language,
    rows: usize,
    mistake: Option<&(usize, String)>,
) -> Response {
    let theme = theme::current();
    let font = egui::FontId::monospace(13.0);
    // As tall as what it holds, and a line to go on: `rows` at least.
    let lines = value.split('\n').count();
    let rows = rows.max(lines + 1);
    // Room for the numbers of as many lines as there are, or will fit.
    let lines = lines.max(rows);
    let digit = ui.fonts_mut(|fonts| fonts.glyph_width(&egui::FontId::monospace(11.5), '0'));
    let gutter = lines.to_string().len().max(2) as f32 * digit + 26.0;
    let mut coloured = |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
        let source = text.as_str();
        let mut job = match language {
            Language::Luau => crate::luau::highlight(source, &font, &theme),
            Language::Commands => {
                crate::syntax::layout(&crate::syntax::command_tokens(source), &font, &theme)
            }
        };
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
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(label));
    line_numbers(ui, &output, gutter, mistake);
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

/// The numbers of the lines in the gutter, the line being written marked,
/// and the line of a mistake in its colour. A line of code can wrap into
/// several rows; only its first is numbered.
fn line_numbers(
    ui: &Ui,
    output: &egui::text_edit::TextEditOutput,
    gutter: f32,
    mistake: Option<&(usize, String)>,
) {
    let theme = theme::current();
    let small = egui::FontId::monospace(11.5);
    let rect = output.response.rect;
    let painter = ui.painter().with_clip_rect(rect);
    let edge = rect.left() + gutter - 12.0;
    painter.vline(
        edge,
        rect.y_range().shrink(1.0),
        Stroke::new(1.0, theme.border),
    );
    let writing = (output.cursor_range)
        .filter(|_| output.response.has_focus())
        .map(|range| output.galley.layout_from_cursor(range.primary).row);
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
            let at = egui::pos2(edge - 9.0, placed.center().y);
            painter.text(
                at,
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
}

/// What a [`code_editor`] holds, for its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Luau,
    /// The commands a rule sends, a line each.
    Commands,
}

/// The tags of something: each a pill with a cross to take it off, a
/// field to add one, and under it those there are that it does not carry,
/// narrowed to what is typed, to add with a click. Returns whether the
/// tags changed.
pub fn tag_field(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    tags: &mut Vec<String>,
    known: &[String],
) -> bool {
    let theme = theme::current();
    let typed_id = ui.make_persistent_id((id_salt, "tag typed"));
    let mut typed: String = ui.data_mut(|data| data.get_temp(typed_id).unwrap_or_default());
    let carried = |tags: &[String], tag: &str| {
        let key = crate::rules::tag_key(tag);
        tags.iter().any(|known| crate::rules::tag_key(known) == key)
    };
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        let mut gone = None;
        for (index, tag) in tags.iter().enumerate() {
            if tag_chip(ui, tag).clicked() {
                gone = Some(index);
            }
        }
        if let Some(index) = gone {
            tags.remove(index);
            changed = true;
        }
        let field = ui.add(single_line(&mut typed, crate::i18n::tr("Add a tag"), 140.0));
        ui.ctx().accesskit_node_builder(field.id, |node| {
            node.set_label(crate::i18n::tr("Add a tag"))
        });
        if field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            let tag = typed.trim();
            // Spelled as one there is, if it is one.
            let tag = (known.iter())
                .find(|known| crate::rules::tag_key(known) == crate::rules::tag_key(tag))
                .map_or(tag, String::as_str)
                .to_owned();
            if !tag.is_empty() && !carried(tags, &tag) {
                tags.push(tag);
                changed = true;
            }
            typed.clear();
            field.request_focus();
        }
    });
    let wanted = typed.trim().to_lowercase();
    let offered: Vec<&String> = (known.iter())
        .filter(|tag| !carried(tags, tag))
        .filter(|tag| tag.to_lowercase().contains(&wanted))
        .collect();
    if !offered.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for tag in offered {
                let add = ui.add(
                    egui::Button::new(
                        RichText::new(format!("+ {tag}"))
                            .font(theme::font(Weight::Regular, 12.0))
                            .color(theme.text_dim),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0, theme.border))
                    .corner_radius(CornerRadius::same(255)),
                );
                let add = add.on_hover_text(crate::i18n::trf("Add the tag {0}", &[tag]));
                if add.clicked() {
                    tags.push(tag.clone());
                    typed.clear();
                    changed = true;
                }
            }
        });
    }
    ui.data_mut(|data| data.insert_temp(typed_id, typed));
    changed
}

/// A tag carried, as a pill with a cross; clicked, it is taken off.
fn tag_chip(ui: &mut Ui, tag: &str) -> Response {
    let colour = tag_colour(tag);
    let text = RichText::new(format!("{tag}  ×"))
        .font(theme::font(Weight::Medium, 12.0))
        .color(colour);
    let chip = ui.add(
        egui::Button::new(text)
            .fill(colour.gamma_multiply(0.14))
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(255)),
    );
    ui.ctx().accesskit_node_builder(chip.id, |node| {
        node.set_label(crate::i18n::trf("Remove the tag {0}", &[tag]));
    });
    chip.on_hover_text(crate::i18n::trf("Remove the tag {0}", &[tag]))
}

/// Names a field by its hint for a screen reader, when nothing else names
/// it.
fn named_by_hint(field: Response, hint: &str) -> Response {
    (field.ctx).accesskit_node_builder(field.id, |node| {
        if node.label().is_none() {
            node.set_label(hint);
        }
    });
    field
}
