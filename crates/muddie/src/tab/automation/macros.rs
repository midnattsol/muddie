//! The details of a macro, and recording its key.

use super::*;

impl Tab {
    pub(in crate::tab) fn macro_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let groups = self.editor.draft.group_names();
        let theme = theme::current();
        let height = self.editor.detail_height;
        let recording = self.editor.recording_key;
        let mut toggle = false;
        let rule = &mut self.editor.draft.keys[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this macro"));
        group_row(ui, &mut rule.group, &groups);
        widgets::caption(ui, tr("Key"));
        ui.horizontal(|ui| {
            let shown = if recording {
                tr("Press the key…").to_owned()
            } else if rule.key.trim().is_empty() {
                tr("None").to_owned()
            } else {
                rule.key.trim().to_uppercase().replace('+', " + ")
            };
            let (colour, edge) = if recording {
                (theme.accent, theme.accent)
            } else {
                (theme.text, theme.border)
            };
            egui::Frame::new()
                .fill(theme.surface)
                .stroke(egui::Stroke::new(1.0, edge))
                .corner_radius(egui::CornerRadius::same(8))
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
        ui.add_space(6.0);
        let mistake = if rule.lua {
            self.editor.checks.of(&self.scripts, &rule.script)
        } else {
            None
        };
        action_block(
            ui,
            ActionEdit {
                lua: &mut rule.lua,
                commands: &mut rule.send,
                script: &mut rule.script,
                matched: false,
                commands_hint: "north",
                script_hint: "map.walk(map.find(\"Temple\")[1].id)",
                about_commands: tr("One command per line, sent as if you had typed them."),
                about_script: tr("Run when the key is pressed."),
            },
            script_rows(height, 300.0),
            mistake.as_ref(),
        );
        if toggle {
            self.editor.recording_key = !recording;
        }
        delete
    }

    /// While a macro's key is being recorded, takes the next key that can
    /// be bound as that key. Escape cancels.
    pub(in crate::tab) fn record_key(&mut self, ui: &mut egui::Ui) {
        let held = ui.input(|input| input.modifiers);
        let prefix = |modifiers: Modifiers| {
            let mut prefix = String::new();
            for (down, name) in [
                (modifiers.ctrl, "ctrl+"),
                (modifiers.alt, "alt+"),
                (modifiers.shift, "shift+"),
            ] {
                if down {
                    prefix.push_str(name);
                }
            }
            prefix
        };
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
            if let Some(rule) = self.editor.draft.keys.get_mut(index) {
                rule.key = key;
            }
            self.editor.recording_key = false;
            // The key was for the recorder: nothing else should act on it.
            ui.input_mut(|input| input.events.clear());
        }
    }
}
