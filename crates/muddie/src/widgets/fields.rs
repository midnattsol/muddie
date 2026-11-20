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
    ui.add(single_line(value, hint, f32::INFINITY))
}

/// A field for something secret, as a password: its characters hidden.
pub fn secret_field(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    ui.add(single_line(value, hint, width).password(true))
}

/// A text field for the right-hand side of a [`setting_row`].
pub fn row_field(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    ui.add(single_line(value, hint, width))
}

/// One line to type in, `width` wide and as tall as every other control,
/// with `hint` in it while it is empty.
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
    menu_row(ui, Some(selected), text, "")
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
    // Room for the numbers of as many lines as there are, or will fit.
    let lines = value.split('\n').count().max(rows);
    let digit = ui.fonts_mut(|fonts| fonts.glyph_width(&egui::FontId::monospace(11.5), '0'));
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
