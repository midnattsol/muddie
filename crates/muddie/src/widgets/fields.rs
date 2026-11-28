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
    let typed_id = ui.make_persistent_id((id_salt, "tag typed"));
    let mut typed: String = ui.data_mut(|data| data.get_temp(typed_id).unwrap_or_default());
    let carried = |tags: &[String], tag: &str| {
        let key = crate::rules::tag_key(tag);
        tags.iter().any(|known| crate::rules::tag_key(known) == key)
    };
    let mut changed = false;
    let adding_id = typed_id.with("tag added");
    let mut adding_one = false;
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
        let added = ui
            .data_mut(|data| data.remove_temp::<bool>(adding_id))
            .is_some();
        // The tag taken off was where the keyboard was: it goes on to
        // the field, not nowhere.
        let key = typed_id.with("to the field");
        focus_until_there(ui, key, &field, gone.is_some() || added);
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
                if tag_offered(ui, tag).clicked() {
                    tags.push(tag.clone());
                    typed.clear();
                    changed = true;
                    // Nor is the keyboard lost with the button it was on.
                    adding_one = true;
                }
            }
        });
    }
    ui.data_mut(|data| data.insert_temp(typed_id, typed));
    if adding_one {
        ui.data_mut(|data| data.insert_temp(adding_id, true));
    }
    changed
}

/// A tag there is that is not carried, as a pill with a plus; clicked, it
/// is added.
fn tag_offered(ui: &mut Ui, tag: &str) -> Response {
    let theme = theme::current();
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
    let name = crate::i18n::trf("Add the tag {0}", &[tag]);
    (ui.ctx()).accesskit_node_builder(add.id, |node| node.set_label(name.clone()));
    name_tip(add, &name)
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
    let name = crate::i18n::trf("Remove the tag {0}", &[tag]);
    (ui.ctx()).accesskit_node_builder(chip.id, |node| node.set_label(name.clone()));
    name_tip(chip, &name)
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
