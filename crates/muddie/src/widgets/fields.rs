//! Where text and numbers are typed or chosen.

use super::*;

/// A text field with its label above it, as wide as the space allows.
pub fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str) -> Response {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let words = field_label(ui, label);
        let field = text_box(ui, value, hint);
        named_by(ui, &field, &words, label);
        field
    })
    .inner
}

/// The words above a field, or another control, that name it.
pub fn field_label(ui: &mut Ui, label: &str) -> Response {
    ui.label(
        RichText::new(label)
            .font(theme::font(Weight::Regular, 12.0))
            .color(theme::current().text_dim),
    )
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
pub fn choose<T: PartialEq + Clone>(
    ui: &mut Ui,
    id: &str,
    value: &mut T,
    choices: &[(T, String)],
    other: impl Fn(T) -> String,
) -> Response {
    choose_in(ui, id, value, &[("", choices.to_vec())], other)
}

/// The same, with the values in groups, each under its heading; a group
/// with no heading has none.
pub fn choose_in<T: PartialEq + Clone>(
    ui: &mut Ui,
    id: &str,
    value: &mut T,
    groups: &[(&str, Vec<(T, String)>)],
    other: impl Fn(T) -> String,
) -> Response {
    let mut all = groups.iter().flat_map(|(_, choices)| choices.iter());
    let shown = (all.find(|(choice, _)| *choice == *value))
        .map_or_else(|| other(value.clone()), |(_, name)| name.clone());
    let mut changed = false;
    let mut response = dropdown(id)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (heading, choices) in groups {
                if !heading.is_empty() {
                    menu_heading(ui, heading);
                }
                for (choice, name) in choices {
                    changed |= choice_value(ui, value, choice.clone(), name).changed();
                }
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

/// How tall a [`code_editor`] is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CodeHeight {
    /// As tall as what it holds and a line more, `fewest` lines at least and
    /// `most` at most: past that it scrolls.
    Auto { fewest: usize, most: usize },
    /// This many points tall, as the one who uses it made it; it scrolls.
    Fixed(f32),
    /// `rows` lines at least, growing with what it holds: for a page of its
    /// own, which scrolls itself.
    Fill(usize),
}

/// An editor of code, coloured as `language` is as it is typed, with the lines
/// numbered and the one being written marked, as tall as `height` says.
/// `mistake` is the line and the message of what keeps the code from
/// running, shown at that line and under the editor. `label` names the
/// editor for screen readers. What it is coloured and laid out as is kept,
/// and made again only when the text, the width or the theme changes.
pub fn code_editor(
    ui: &mut Ui,
    label: &str,
    value: &mut String,
    hint: &str,
    language: Language,
    height: CodeHeight,
    mistake: Option<&(usize, String)>,
) -> Response {
    let response = code_body(ui, label, value, hint, language, height, mistake);
    mistake_line(ui, mistake);
    response
}

/// The editor of [`code_editor`], without what keeps its code from running
/// written under it.
fn code_body(
    ui: &mut Ui,
    label: &str,
    value: &mut String,
    hint: &str,
    language: Language,
    height: CodeHeight,
    mistake: Option<&(usize, String)>,
) -> Response {
    let theme = theme::current();
    let font = egui::FontId::monospace(13.0);
    let line = code_line(ui);
    let lines = value.split('\n').count();
    // Room for the numbers of as many lines as there are, or will fit.
    let digit = ui.fonts_mut(|fonts| fonts.glyph_width(&egui::FontId::monospace(11.5), '0'));
    let gutter = (lines + 1).to_string().len().max(2) as f32 * digit + 26.0;
    let id = ui.make_persistent_id(("code editor", label));
    let mut coloured = |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
        cached_layout(
            ui,
            id,
            (language, wrap_width.to_bits()),
            text.as_str(),
            || {
                let source = text.as_str();
                let mut job = match language {
                    Language::Luau => crate::luau::highlight(source, &font, &theme),
                    Language::Commands => {
                        crate::syntax::layout(&crate::syntax::command_tokens(source), &font, &theme)
                    }
                };
                job.wrap.max_width = wrap_width;
                job
            },
        )
    };
    let margin = Margin {
        left: gutter as i8,
        right: 12,
        top: 10,
        bottom: 10,
    };
    let edge = CODE_EDGE;
    // Tab types a tab, as in any editor of code; after Escape, it goes on
    // to what is next instead, the keyboard staying where it was, until
    // something is typed or the keyboard leaves.
    let escaped_id = id.with("escaped");
    let escaped = ui.data(|data| data.get_temp::<bool>(escaped_id)) == Some(true);
    // How many lines are shown, and whether it scrolls past them.
    let (rows, most) = match height {
        CodeHeight::Auto { fewest, most } => ((lines + 1).clamp(fewest, most), Some(most)),
        CodeHeight::Fixed(points) => {
            let rows = (((points - edge) / line).floor() as usize).max(1);
            (rows, Some(rows))
        }
        CodeHeight::Fill(rows) => (rows.max(lines + 1), None),
    };
    let mut editor = |ui: &mut Ui| {
        let output = TextEdit::multiline(value)
            .id(id)
            .font(font.clone())
            .margin(margin)
            .desired_rows(rows.max(lines + 1))
            .lock_focus(!escaped)
            .desired_width(f32::INFINITY)
            .layouter(&mut coloured)
            .show(ui);
        // Inside what scrolls, so that the numbers scroll with the lines.
        line_numbers(ui, &output, gutter, mistake);
        // While it is empty, an example in its place, where the first line
        // is: on the line numbered 1.
        if value.is_empty() {
            let mut job = egui::text::LayoutJob::simple(
                hint.to_owned(),
                font.clone(),
                theme.text_faint,
                output.galley.rect.width().max(1.0),
            );
            job.wrap.max_width = output.response.response.rect.width() - gutter - 12.0;
            let example = ui.fonts_mut(|fonts| fonts.layout_job(job));
            let painter = ui.painter().with_clip_rect(output.response.response.rect);
            painter.galley(output.galley_pos, example, theme.text_faint);
        }
        output
    };
    let output = match most {
        // Taller than it is shown: it scrolls, its own height fixed.
        Some(most) if lines + 1 > most || matches!(height, CodeHeight::Fixed(_)) => {
            let shown = most as f32 * line + edge;
            egui::ScrollArea::vertical()
                .id_salt(("code scroll", label))
                .max_height(shown)
                .min_scrolled_height(shown)
                .auto_shrink([false, true])
                .show(ui, editor)
                .inner
        }
        _ => editor(ui),
    };
    let response = output.response.response.clone();
    let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
    let now_escaped = match escaped {
        false if response.lost_focus() && escape => {
            response.request_focus();
            true
        }
        true => response.has_focus() && !response.changed(),
        false => false,
    };
    if now_escaped != escaped {
        ui.data_mut(|data| data.insert_temp(escaped_id, now_escaped));
    }
    // Which editor has given Tab back, kept as an Option so that it can be
    // cleared.
    let freed = escaped_editor_id();
    let known = ui
        .data(|data| data.get_temp::<Option<egui::Id>>(freed))
        .flatten();
    if now_escaped {
        ui.data_mut(|data| data.insert_temp(freed, Some(response.id)));
    } else if known == Some(response.id) {
        ui.data_mut(|data| data.insert_temp::<Option<egui::Id>>(freed, None));
    }
    let how = crate::i18n::tr(
        "Tab inserts a tab. To move to the next control, press Escape and then Tab.",
    );
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_label(label);
        node.set_description(how);
        node.set_placeholder(hint);
    });
    response
}

/// The editor of code which has the keyboard but has given Tab back,
/// after Escape: the keyboard is in it, but it is not being typed in.
pub fn escaped_editor(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.data(|data| data.get_temp::<Option<egui::Id>>(escaped_editor_id()))
        .flatten()
}

fn escaped_editor_id() -> egui::Id {
    egui::Id::unique("escaped code editor")
}

/// The margin above and below the lines of a [`code_editor`], together.
const CODE_EDGE: f32 = 20.0;
/// How many lines a [`code_editor`] that fits what it holds shows at most,
/// before it scrolls, and how many one made taller can show.
const MOST_LINES: usize = 20;
const TALLEST: usize = 60;

/// How tall a line of code is.
fn code_line(ui: &Ui) -> f32 {
    ui.fonts_mut(|fonts| fonts.row_height(&egui::FontId::monospace(13.0)))
}

/// Under an editor of code, what keeps its code from running, by its line.
fn mistake_line(ui: &mut Ui, mistake: Option<&(usize, String)>) {
    if let Some((line, message)) = mistake {
        let text = format!("{} {line}: {message}", crate::i18n::tr("Line"));
        ui.label(
            RichText::new(text)
                .font(theme::font(Weight::Regular, 12.0))
                .color(theme::current().danger),
        );
    }
}

/// A [`code_editor`] which fits what it holds, from `language`'s fewest
/// lines to [`MOST_LINES`], and takes the room `spare` under the page it is
/// on, which is what was left empty the frame before: what it holds is seen
/// at once where there is room for it. Until it is made taller or shorter
/// with the grip under it: dragged, or with the arrow keys once Tab is on
/// it. A double click, or Home, gives it back its own height. `height` is
/// the height it was made, 0 while it is its own. Returns what to say when
/// the keyboard changed it.
#[allow(clippy::too_many_arguments)]
pub fn sizable_code_editor(
    ui: &mut Ui,
    label: &str,
    value: &mut String,
    hint: &str,
    language: Language,
    height: &mut f32,
    spare: f32,
    mistake: Option<&(usize, String)>,
) -> (Response, Option<String>) {
    let fewest = language.fewest_lines();
    let line = code_line(ui);
    let lines = value.split('\n').count();
    let rows = |points: f32| (((points - CODE_EDGE) / line).floor() as usize).max(1);
    let grip_id = ui.make_persistent_id(("code grip", label));
    // Its own height: what it holds, or the room it had and what was left
    // empty under the page, whichever is more.
    let fit = (lines + 1).clamp(fewest, MOST_LINES);
    let had = ui.data(|data| data.get_temp::<usize>(grip_id.with("shown")));
    // Half a point over, so that a page which just fits is not taken for
    // one a line short.
    let room = had.map_or(0, |had| rows(had as f32 * line + CODE_EDGE + spare + 0.5));
    let own = fit.max(room);
    if had.is_some_and(|had| had != own) && *height <= 0.0 {
        ui.ctx()
            .request_discard("an editor of code took the room under it");
    }
    ui.data_mut(|data| data.insert_temp(grip_id.with("shown"), own));
    let size = match *height > 0.0 {
        true => CodeHeight::Fixed(*height),
        false => CodeHeight::Auto {
            fewest: own,
            most: own.max(MOST_LINES),
        },
    };
    let response = code_body(ui, label, value, hint, language, size, mistake);
    let shown_rows = match *height > 0.0 {
        true => rows(*height),
        false => (lines + 1).clamp(own, own.max(MOST_LINES)),
    };
    let shown = shown_rows as f32 * line + CODE_EDGE;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 12.0), Sense::hover());
    let grip = super::edge(
        ui,
        rect,
        grip_id,
        crate::i18n::tr("Editor height"),
        false,
        true,
    );
    let mut said = None;
    if grip.moved != 0.0 {
        let by = match grip.by_keys {
            true => grip.moved.signum() * line,
            false => grip.moved,
        };
        let lowest = 3.0 * line + CODE_EDGE;
        *height = (shown + by).clamp(lowest, TALLEST as f32 * line + CODE_EDGE);
        if grip.by_keys {
            let count = rows(*height).to_string();
            said = Some(crate::i18n::trf("{0} lines", &[&count]));
        }
    }
    let home = grip.response.has_focus()
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Home));
    if (grip.response.double_clicked() || home) && *height > 0.0 {
        *height = 0.0;
        said = home.then(|| crate::i18n::tr("Fits its contents").to_owned());
    }
    let count = match *height > 0.0 {
        true => rows(*height),
        false => shown_rows,
    };
    // How tall it is, in words: a number alone, as a value, would be read
    // as a percentage of the most it can be, which means nothing here.
    let words = crate::i18n::trf("{0} lines", &[&count.to_string()]);
    ui.ctx()
        .accesskit_node_builder(grip.response.id, |node| node.set_description(words));
    // A short bar in the middle, which stands out while it can be taken.
    let theme = theme::current();
    let active = grip.response.hovered() || grip.response.dragged() || grip.response.has_focus();
    let colour = match active {
        true => theme.accent,
        false => theme.border,
    };
    let bar = egui::Rect::from_center_size(rect.center(), vec2(36.0, 4.0));
    ui.painter().rect_filled(bar, CornerRadius::same(2), colour);
    mistake_line(ui, mistake);
    (response, said)
}

/// Lays out `text` as `build` says, or gives back how it was laid out the
/// last time, if neither it nor `key` nor the theme changed since: colouring
/// and laying out a long text every frame is what costs most in an editor.
pub fn cached_layout(
    ui: &Ui,
    id: egui::Id,
    key: impl std::hash::Hash,
    text: &str,
    build: impl FnOnce() -> egui::text::LayoutJob,
) -> std::sync::Arc<egui::Galley> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (text, theme::generation()).hash(&mut hasher);
    key.hash(&mut hasher);
    let wanted = hasher.finish();
    let slot = id.with("laid out");
    let kept = ui.data(|data| data.get_temp::<(u64, std::sync::Arc<egui::Galley>)>(slot));
    if let Some((known, galley)) = kept
        && known == wanted
    {
        return galley;
    }
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(build()));
    ui.data_mut(|data| data.insert_temp(slot, (wanted, galley.clone())));
    galley
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
    // An empty line has no height of its own: it is as tall as any other.
    let tall = code_line(ui);
    for (index, row) in output.galley.rows.iter().enumerate() {
        let placed = row.rect().translate(output.galley_pos.to_vec2());
        let placed = placed.with_max_y(placed.top() + placed.height().max(tall));
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

/// What the editors of code of a page remember: how tall those of commands
/// and of scripts were made (see [`Language::slot`]; 0 fits them to what
/// they hold), and whether one is open larger, over the window.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CodeSizes {
    pub heights: [f32; 2],
    pub expanded: bool,
    /// The room left empty under the page they are on, the frame before.
    pub spare: f32,
}

/// What a [`code_editor`] holds, for its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Luau,
    /// The commands a rule sends, a line each.
    Commands,
}

impl Language {
    /// How many lines an editor of it shows at least: a script needs more
    /// room than a few commands.
    pub fn fewest_lines(self) -> usize {
        match self {
            Self::Luau => 8,
            Self::Commands => 3,
        }
    }

    /// Where the height its editors are made is kept, of the two.
    pub fn slot(self) -> usize {
        match self {
            Self::Commands => 0,
            Self::Luau => 1,
        }
    }
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

/// A field to search in: the magnifier, on the line of what is typed, and
/// the text with `hint` in it while it is empty, `size` big and `width`
/// wide, no frame of its own (it goes in one). Named `label` for a screen
/// reader. `lock_focus` keeps Tab in it, where Tab does something there.
pub fn search_field(ui: &mut Ui, text: &mut String, look: SearchLook<'_>) -> Response {
    let theme = theme::current();
    let hint = RichText::new(look.hint)
        .font(theme::font(Weight::Regular, look.size))
        .color(theme.text_faint);
    let glyph_size = (look.size - 1.0).max(12.0);
    let (glyph, _) =
        ui.allocate_exact_size(vec2(glyph_size + 2.0, look.size + 5.0), Sense::hover());
    let mut edit = TextEdit::singleline(text)
        .frame(Frame::NONE)
        .font(theme::font(Weight::Regular, look.size))
        .hint_text(hint)
        .lock_focus(look.lock_focus)
        .desired_width(look.width);
    if let Some(id) = look.id {
        edit = edit.id(id);
    }
    let edit = edit.show(ui);
    super::paint_glyph(
        ui.painter(),
        egui::pos2(
            glyph.center().x,
            edit.galley_pos.y + edit.galley.size().y / 2.0,
        ),
        egui::Align2::CENTER_CENTER,
        super::Glyph::Search,
        glyph_size,
        theme.text_faint,
    );
    let label = look.label.to_owned();
    let response = edit.response.response;
    (ui.ctx()).accesskit_node_builder(response.id, |node| node.set_label(label));
    response
}

/// How a [`search_field`] looks and is named.
pub struct SearchLook<'a> {
    pub hint: &'a str,
    pub label: &'a str,
    pub size: f32,
    pub width: f32,
    pub id: Option<egui::Id>,
    pub lock_focus: bool,
}
