//! The variables scripts keep: listing, adding and changing them, as text
//! or by their kind.

use super::*;

impl Tab {
    /// Makes a variable with a name not yet taken and selects it.
    pub(in crate::tab) fn add_variable(&mut self) {
        let known: Vec<String> = self
            .scripts
            .variables()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        let names = (1..).map(|number| {
            if number == 1 {
                "variable".to_owned()
            } else {
                format!("variable{number}")
            }
        });
        let Some(name) = names.take(500).find(|name| !known.contains(name)) else {
            return;
        };
        if let Err(error) = self
            .scripts
            .set_variable(&name, Some(serde_json::Value::String(String::new())))
        {
            self.note(&error);
        }
        let place = self
            .scripts
            .variables()
            .iter()
            .position(|(known, _)| *known == name);
        self.editor.selected[VARIABLES] = place.unwrap_or(0);
        self.editor.filter.clear();
    }

    /// Fills what a variable is edited through from its value.
    pub(in crate::tab) fn load_variable(&mut self, value: &serde_json::Value) {
        use serde_json::Value;
        self.editor.variable_kind = kind_of(value);
        self.editor.variable_text = match value {
            Value::String(text) => text.clone(),
            _ => String::new(),
        };
        self.editor.variable_number = value.as_f64().unwrap_or(0.0);
        self.editor.variable_flag = value.as_bool().unwrap_or(false);
        // The items of an array by their place, the fields of a table by
        // their key.
        self.editor.variable_items = match value {
            Value::Array(items) => items
                .iter()
                .map(|item| (String::new(), shown_value(item)))
                .collect(),
            Value::Object(fields) => fields
                .iter()
                .map(|(key, item)| (key.clone(), shown_value(item)))
                .collect(),
            _ => Vec::new(),
        };
    }

    /// The value that what is being edited stands for.
    pub(in crate::tab) fn edited_variable(&self) -> serde_json::Value {
        use serde_json::Value;
        match self.editor.variable_kind {
            NUMBER => {
                serde_json::Number::from_f64(self.editor.variable_number).map_or(
                    Value::Null,
                    |number| {
                        // A whole number is kept as one, not as 3.0.
                        let whole = self.editor.variable_number.fract() == 0.0
                            && self.editor.variable_number.abs() < 9e15;
                        if whole {
                            Value::from(self.editor.variable_number as i64)
                        } else {
                            Value::Number(number)
                        }
                    },
                )
            }
            BOOLEAN => Value::Bool(self.editor.variable_flag),
            ARRAY => Value::Array(
                self.editor
                    .variable_items
                    .iter()
                    .map(|(_, text)| read_value(text))
                    .collect(),
            ),
            TABLE => {
                let fields = (self.editor.variable_items.iter())
                    .filter(|(key, _)| !key.trim().is_empty())
                    .map(|(key, text)| (key.trim().to_owned(), read_value(text)));
                Value::Object(fields.collect())
            }
            _ => Value::String(self.editor.variable_text.clone()),
        }
    }

    /// A variable's details: its name, its type, its value as that type,
    /// how it is written in commands and scripts, and where it is kept.
    pub(in crate::tab) fn variable_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let variables = self.scripts.variables();
        let Some((name, value)) = variables.get(index).cloned() else {
            return false;
        };
        if self.editor.variable_loaded.as_ref() != Some(&name) {
            self.editor.variable_loaded = Some(name.clone());
            self.editor.variable_name.clone_from(&name);
            self.load_variable(&value);
        }
        let (delete, renamed) = self.variable_heading(ui);
        if self.rename_variable(ui, &variables, &name, &value, renamed) {
            return false;
        }
        self.variable_type_row(ui);
        ui.add_space(2.0);
        widgets::caption(ui, tr("Value"));
        match self.editor.variable_kind {
            NUMBER => self.number_value(ui),
            BOOLEAN => flag_value(ui, &mut self.editor.variable_flag),
            ARRAY | TABLE => self.items_value(ui),
            _ => text_value(ui, &mut self.editor.variable_text),
        }
        // Kept as soon as it is other than what is kept.
        let after = self.edited_variable();
        if after != value
            && let Err(error) = self.scripts.set_variable(&name, Some(after))
        {
            self.note(&error);
        }
        ui.add_space(8.0);
        self.variable_usage(ui, &name);
        let kept = match &self.profile {
            Some(profile) => trf("Saved in profile {0}.", &[profile]),
            None => tr("This session has no profile, so its variables last until it is closed. Save it as a profile, from its tab's menu, to keep them.").to_owned(),
        };
        note(ui, &kept);
        delete
    }

    /// The variable's name, to change, and the way to delete it. Returns
    /// whether deleting was asked for, and whether the name is done being
    /// typed.
    fn variable_heading(&mut self, ui: &mut egui::Ui) -> (bool, bool) {
        let (mut delete, mut renamed) = (false, false);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                delete =
                    widgets::icon_button(ui, widgets::Icon::Trash, tr("Delete"), false).clicked();
                let title = TextEdit::singleline(&mut self.editor.variable_name)
                    .font(egui::FontId::monospace(19.0))
                    .frame(egui::Frame::NONE)
                    .desired_width(f32::INFINITY);
                let field = ui.add(title);
                ui.ctx()
                    .accesskit_node_builder(field.id, |node| node.set_label(tr("Name")));
                renamed = field.lost_focus();
            });
        });
        ui.add(egui::Separator::default().spacing(2.0));
        (delete, renamed)
    }

    /// Gives the variable the name typed, once it is done being typed and
    /// is a name no other has; or says why it cannot. Returns whether it
    /// was renamed.
    fn rename_variable(
        &mut self,
        ui: &mut egui::Ui,
        variables: &[(String, serde_json::Value)],
        name: &str,
        value: &serde_json::Value,
        renamed: bool,
    ) -> bool {
        let wanted = self.editor.variable_name.trim().to_owned();
        if wanted == name {
            return false;
        }
        let is_a_name = wanted.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_')
            && wanted
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        if !is_a_name {
            note(
                ui,
                tr("A name is made of letters, digits and _, and does not start with a digit."),
            );
            return false;
        }
        if variables.iter().any(|(known, _)| *known == wanted) {
            note(ui, tr("There is already a variable of that name."));
            return false;
        }
        if !renamed {
            return false;
        }
        let moved = (self.scripts.set_variable(&wanted, Some(value.clone())))
            .and_then(|()| self.scripts.set_variable(name, None));
        if let Err(error) = moved {
            self.note(&error);
        }
        self.editor.variable_loaded = Some(wanted.clone());
        let place = self
            .scripts
            .variables()
            .iter()
            .position(|(known, _)| *known == wanted);
        self.editor.selected[VARIABLES] = place.unwrap_or(0);
        true
    }

    /// Its type, by the name a script knows it by. What it held is kept,
    /// as far as the new type can hold it.
    fn variable_type_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            widgets::caption(ui, tr("Type"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(kind) = widgets::segmented(ui, &TYPES, self.editor.variable_kind) {
                    let held = self.edited_variable();
                    self.load_variable(&converted(&held, kind));
                    self.editor.variable_kind = kind;
                }
            });
        });
    }

    /// A number: dragged, or typed.
    fn number_value(&mut self, ui: &mut egui::Ui) {
        let field = ui.add(
            crate::widgets::number(&mut self.editor.variable_number)
                .speed(1.0)
                .max_decimals(6)
                .min_decimals(0)
                .update_while_editing(false),
        );
        ui.ctx()
            .accesskit_node_builder(field.id, |node| node.set_label(tr("Value")));
        note(ui, tr("Drag it, or click it to type."));
    }

    /// The items of an array, or the fields of a table, each to change or
    /// remove, and the way to add one.
    fn items_value(&mut self, ui: &mut egui::Ui) {
        let keyed = self.editor.variable_kind == TABLE;
        if self.editor.variable_items.is_empty() {
            note(
                ui,
                if keyed {
                    tr("No fields yet.")
                } else {
                    tr("No items yet.")
                },
            );
        }
        let mut removed = None;
        widgets::group(ui, |ui| {
            for (place, (key, text)) in self.editor.variable_items.iter_mut().enumerate() {
                if item_row(ui, place, keyed, key, text) {
                    removed = Some(place);
                }
            }
        });
        if let Some(place) = removed {
            self.editor.variable_items.remove(place);
        }
        let add = if keyed {
            tr("Add a field")
        } else {
            tr("Add an item")
        };
        if crate::widgets::button(ui, add).clicked() {
            self.editor
                .variable_items
                .push((String::new(), String::new()));
        }
        note(
            ui,
            tr(
                "What reads as a number, true or false is kept as that; the rest as a string. A table inside is written as JSON.",
            ),
        );
    }

    /// How it is written in the commands of a rule and in a script, to
    /// copy.
    fn variable_usage(&self, ui: &mut egui::Ui, name: &str) {
        widgets::caption(ui, tr("Usage"));
        let (in_commands, in_scripts) = match self.editor.variable_kind {
            ARRAY => (format!("${name}.1"), format!("vars.{name}[1]")),
            TABLE => {
                let first = self
                    .editor
                    .variable_items
                    .first()
                    .map(|(key, _)| key.trim());
                let key = first.filter(|key| !key.is_empty()).unwrap_or("key");
                (format!("${name}.{key}"), format!("vars.{name}.{key}"))
            }
            _ => (format!("${name}"), format!("vars.{name}")),
        };
        widgets::group(ui, |ui| {
            for (place, written) in [
                (tr("In the commands of a rule"), in_commands),
                (tr("In a script"), in_scripts),
            ] {
                widgets::setting_row(ui, place, None, &mut |ui| {
                    let copy = crate::widgets::button(ui, tr("Copy"));
                    if copy.clicked() {
                        ui.ctx().copy_text(written.clone());
                    }
                    let shown = RichText::new(&written)
                        .font(egui::FontId::monospace(13.0))
                        .color(theme::current().text);
                    ui.label(shown);
                    copy
                });
            }
        });
    }
}

/// True or false, with the word a script reads.
fn flag_value(ui: &mut egui::Ui, flag: &mut bool) {
    ui.horizontal(|ui| {
        widgets::switch(ui, flag, tr("Value"));
        let said = if *flag { "true" } else { "false" };
        ui.label(
            RichText::new(said)
                .font(egui::FontId::monospace(13.5))
                .color(theme::current().text),
        );
    });
}

/// Text, as tall as what it holds, within reason.
fn text_value(ui: &mut egui::Ui, text: &mut String) {
    let rows = text.lines().count().clamp(1, 10);
    let field = ui.add(
        TextEdit::multiline(text)
            .font(egui::FontId::monospace(13.0))
            .margin(egui::Margin::symmetric(12, 9))
            .desired_rows(rows)
            .desired_width(f32::INFINITY),
    );
    ui.ctx()
        .accesskit_node_builder(field.id, |node| node.set_label(tr("Value")));
}

/// An item of an array, numbered from one as a script counts them, or a
/// field of a table with its key. Returns whether removing it was asked.
fn item_row(
    ui: &mut egui::Ui,
    place: usize,
    keyed: bool,
    key: &mut String,
    text: &mut String,
) -> bool {
    let theme = theme::current();
    let mono = || egui::FontId::monospace(13.0);
    let mut removed = false;
    ui.horizontal(|ui| {
        ui.set_min_height(40.0);
        if keyed {
            let hint = RichText::new("key").color(theme.text_faint);
            let field = ui.add(
                TextEdit::singleline(key)
                    .font(mono())
                    .hint_text(hint)
                    .margin(egui::Margin::symmetric(9, 6))
                    .desired_width(150.0),
            );
            ui.ctx()
                .accesskit_node_builder(field.id, |node| node.set_label(tr("Field name")));
            ui.label(RichText::new("=").font(mono()).color(theme.text_faint));
        } else {
            let number = RichText::new(format!("{:>2}", place + 1))
                .font(egui::FontId::monospace(12.5))
                .color(theme.text_faint);
            ui.add(Label::new(number).selectable(false));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            removed = widgets::close_button(ui, tr("Remove"), 22.0).clicked();
            let hint = RichText::new("value").color(theme.text_faint);
            let field = ui.add(
                TextEdit::singleline(text)
                    .font(mono())
                    .hint_text(hint)
                    .margin(egui::Margin::symmetric(9, 6))
                    .desired_width(f32::INFINITY),
            );
            ui.ctx()
                .accesskit_node_builder(field.id, |node| node.set_label(tr("Value")));
        });
    });
    removed
}
