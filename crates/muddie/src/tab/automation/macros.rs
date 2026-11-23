//! The details of a macro, and recording its key.

use super::*;

impl Tab {
    pub(in crate::tab) fn macro_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let theme = theme::current();
        let recording = self.editor.recording_key;
        let mut toggle = false;
        let delete = self.rule_head(ui, Kind::Macro, index, tr("Macro name"));
        let name = list::rule_title(&self.editor.draft.macros, index);
        let clashes = key_clashes(&self.editor.draft.macros, index);
        let rule = &mut self.editor.draft.macros[index];
        widgets::card(
            ui,
            Some(widgets::Glyph::Key),
            tr("Key"),
            |_| {},
            |ui| {
                ui.horizontal(|ui| {
                    let shown = if recording {
                        tr("Press the key…").to_owned()
                    } else if rule.when.key.trim().is_empty() {
                        tr("None").to_owned()
                    } else {
                        key_shown(&rule.when.key)
                    };
                    let (colour, edge) = if recording {
                        (theme.accent, theme.accent)
                    } else {
                        (theme.text, theme.border)
                    };
                    crate::widgets::inset(theme.surface)
                        .stroke(egui::Stroke::new(1.0, edge))
                        .inner_margin(egui::Margin::symmetric(14, 7))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(shown)
                                    .font(theme::font(Weight::Medium, 13.5))
                                    .color(colour),
                            );
                        });
                    let label = if recording {
                        tr("Cancel")
                    } else {
                        tr("Record a key")
                    };
                    toggle = crate::widgets::button(ui, label).clicked();
                });
                note(
                    ui,
                    tr("Function keys, keys with Ctrl or Alt, and the numeric keypad can be used."),
                );
                for clash in &clashes {
                    note(ui, clash);
                }
            },
        );
        let mistake = if rule.luau {
            self.editor.checks.of(&self.scripts, &rule.code)
        } else {
            None
        };
        let said = action_block(
            ui,
            ActionEdit {
                luau: &mut rule.luau,
                commands: &mut rule.send,
                code: &mut rule.code,
                matched: false,
                commands_hint: "north",
                script_hint: "map.go(\"Temple\")",
                about_commands: tr("One command per line."),
                about_script: tr("Run when the key is pressed."),
                sizes: &mut self.editor.code,
                name,
            },
            mistake.as_ref(),
        );
        if let Some(said) = said {
            self.voice.say_now(&said);
        }
        if toggle {
            self.editor.recording_key = !recording;
        }
        delete
    }

    /// While a macro's key is being recorded, takes the next key that can
    /// be bound as that key. Escape cancels.
    pub(in crate::tab) fn record_key(&mut self, ui: &mut egui::Ui) {
        let held = ui.input(|input| input.modifiers);
        let prefix = |modifiers: Modifiers| crate::tab::keys::key_name(modifiers, "");
        // The keypad first: egui would report its keys as plain digits.
        let mut recorded =
            (self.keypad_pressed.drain(..).next()).map(|name| format!("{}{name}", prefix(held)));
        let events = ui.input(|input| input.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            if key == Key::Escape {
                self.editor.recording_key = false;
                ui.input_mut(|input| input.events.clear());
                return;
            }
            let name = key.name().to_lowercase();
            let function_key =
                name.len() > 1 && name.starts_with('f') && name[1..].parse::<u8>().is_ok();
            if recorded.is_none() && (modifiers.ctrl || modifiers.alt || function_key) {
                recorded = Some(format!("{}{name}", prefix(modifiers)));
            }
        }
        if let Some(key) = recorded {
            let index = self.editor.selected[MACROS];
            if let Some(rule) = self.editor.draft.macros.get_mut(index) {
                rule.when.key = key;
                // Said, as it is not where the keyboard is; and what else
                // has it.
                let mut said = vec![trf("Key: {0}.", &[&key_shown(&rule.when.key)])];
                said.extend(key_clashes(&self.editor.draft.macros, index));
                self.voice.say_now(&said.join(" "));
            }
            self.editor.recording_key = false;
            // The key was for the recorder: nothing else should act on it.
            ui.input_mut(|input| input.events.clear());
        }
    }
}

/// A macro's key as it is shown: `ALT + F5`.
fn key_shown(key: &str) -> String {
    key.trim().to_uppercase().replace('+', " + ")
}

/// What else has the key of the macro at `index`: one of Muddie's
/// shortcuts, which the macro goes before, or other macros, of which the
/// first switched on runs.
fn key_clashes(macros: &Vec<crate::rules::Macro>, index: usize) -> Vec<String> {
    let Some(key) = macros.get(index).map(|rule| rule.when.key.trim()) else {
        return Vec::new();
    };
    if key.is_empty() {
        return Vec::new();
    }
    let shown = key_shown(key);
    let mut clashes = Vec::new();
    let muddies =
        crate::shortcuts::of_macro(key).and_then(|keys| crate::shortcuts::taken_by(&keys, None));
    if let Some(does) = muddies {
        clashes.push(trf(
            "{0} is also Muddie's: {1}. This macro goes before it.",
            &[&shown, does],
        ));
    }
    let others: Vec<String> = (macros.iter().enumerate())
        .filter(|(other, rule)| *other != index && rule.when.key.trim().eq_ignore_ascii_case(key))
        .map(|(other, _)| list::rule_title(macros, other))
        .collect();
    if !others.is_empty() {
        clashes.push(trf(
            "{0} is also the key of {1}: the first switched on runs.",
            &[&shown, &others.join(", ")],
        ));
    }
    clashes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_macros_key_says_what_else_has_it() {
        let made = |name: &str, key: &str| {
            let mut rule = crate::rules::Macro {
                name: name.into(),
                ..Default::default()
            };
            rule.when.key = key.into();
            rule
        };
        let with_key = match cfg!(target_os = "macos") {
            true => "ctrl",
            false => "alt",
        };
        let macros = vec![
            made("Heal", "f5"),
            made("Recent", &format!("{with_key}+1")),
            made("Rest", "F5"),
        ];
        assert_eq!(
            key_clashes(&macros, 0),
            ["F5 is also the key of Rest: the first switched on runs."]
        );
        let recent = format!("{} + 1", with_key.to_uppercase());
        assert_eq!(
            key_clashes(&macros, 1),
            [format!(
                "{recent} is also Muddie's: Reads the most recent lines. This macro goes before it."
            )]
        );
    }
}
