//! The details of a trigger.

use super::*;

/// What a trigger can do besides its commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::tab) enum Extra {
    Say,
    Sound,
    Colour,
    Hide,
    Panel,
}

impl Tab {
    pub(in crate::tab) fn trigger_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let text_panels: Vec<String> = (self.layout.panels.iter())
            .filter(|panel| panel.kind == super::layout::Kind::Text)
            .map(|panel| panel.name.clone())
            .collect();
        let groups = self.editor.draft.group_names();
        let height = self.editor.detail_height;
        // What else it does that was asked for here, though still empty.
        if self.editor.extras_added.0 != index {
            self.editor.extras_added = (index, Vec::new());
        }
        let rule = &mut self.editor.draft.triggers[index];
        let delete = rule_header(
            ui,
            &mut rule.name,
            &mut rule.enabled,
            tr("Name this trigger"),
        );
        group_row(ui, &mut rule.group, &groups);

        // When: how the pattern is read, the pattern, and a line to try
        // it on with what it would do, all together.
        widgets::caption(ui, tr("When"));
        let kinds = [tr("Contains"), tr("Regular expression")];
        if let Some(kind) = widgets::segmented(ui, &kinds, usize::from(rule.regex)) {
            rule.regex = kind == 1;
        }
        let label = if rule.regex {
            tr("A line matches this regular expression")
        } else {
            tr("A line contains this text")
        };
        widgets::field(ui, label, &mut rule.pattern, tr("tells you"));
        widgets::text_box(
            ui,
            &mut self.editor.sample,
            tr("Paste a line from the MUD to try it"),
        );
        let rule = &self.editor.draft.triggers[index];
        let outcome = try_pattern(&rule.regex_source(), &self.editor.sample, |captures| {
            let mut effects = Vec::new();
            let sent: Vec<String> = crate::rules::commands(&rule.send)
                .map(|command| fill(command, captures))
                .collect();
            if rule.lua {
                effects.push(tr("runs the script").to_owned());
            } else if !sent.is_empty() {
                effects.push(format!("{} {}", tr("sends"), sent.join("; ")));
            }
            if !rule.speak.trim().is_empty() {
                effects.push(format!(
                    "{} “{}”",
                    tr("says"),
                    fill(rule.speak.trim(), captures)
                ));
            }
            effects.join(" · ")
        });
        show_outcome(ui, outcome);
        ui.add_space(6.0);

        let rule = &mut self.editor.draft.triggers[index];
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
                matched: true,
                commands_hint: "kill %1\nloot",
                script_hint: "if m[1] ~= \"you\" then\n\tsend(\"assist \" .. m[1])\nend",
                about_commands: tr("One command per line. %1, %2… are the groups of the pattern."),
                about_script: tr("m holds the match, line the whole line."),
            },
            script_rows(height, 560.0),
            mistake.as_ref(),
        );
        ui.add_space(6.0);

        // Also: only what it does besides, each to take away again, and
        // the rest to add.
        let added = &mut self.editor.extras_added.1;
        let shown = |extra: Extra, set: bool| set || added.contains(&extra);
        let visible = [
            (Extra::Say, shown(Extra::Say, !rule.speak.trim().is_empty())),
            (
                Extra::Sound,
                shown(Extra::Sound, !rule.sound.trim().is_empty()),
            ),
            (
                Extra::Colour,
                shown(Extra::Colour, !rule.highlight.trim().is_empty()),
            ),
            (Extra::Hide, shown(Extra::Hide, rule.gag)),
            (
                Extra::Panel,
                shown(Extra::Panel, !rule.panel.trim().is_empty()),
            ),
        ];
        if visible.iter().any(|(_, on)| *on) {
            widgets::caption(ui, tr("Also"));
        }
        let mut removed = None;
        for (extra, on) in visible {
            if !on {
                continue;
            }
            // Its name, with the cross that takes it away at the far end,
            // and what it takes under them.
            ui.horizontal(|ui| {
                let label = RichText::new(extra.label()).color(theme::current().text_dim);
                ui.add(Label::new(label).selectable(false));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let what = trf("Remove “{0}”", &[extra.label()]);
                    if widgets::close_button(ui, &what, 20.0).clicked() {
                        removed = Some(extra);
                    }
                });
            });
            match extra {
                Extra::Say => drop(widgets::text_box(ui, &mut rule.speak, tr("%1 is here"))),
                Extra::Sound => drop(widgets::text_box(ui, &mut rule.sound, "alert.ogg")),
                Extra::Colour => drop(widgets::text_box(
                    ui,
                    &mut rule.highlight,
                    "yellow, #ffcc00",
                )),
                Extra::Hide => rule.gag = true,
                Extra::Panel => {
                    let chosen = match rule.panel.trim().is_empty() {
                        true => tr("Choose a panel").to_owned(),
                        false => rule.panel.clone(),
                    };
                    crate::widgets::dropdown("trigger panel")
                        .selected_text(chosen)
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for panel in &text_panels {
                                if crate::widgets::choice(ui, rule.panel == *panel, panel).clicked()
                                {
                                    rule.panel.clone_from(panel);
                                }
                            }
                        });
                }
            }
            ui.add_space(4.0);
        }
        if let Some(extra) = removed {
            added.retain(|known| *known != extra);
            match extra {
                Extra::Say => rule.speak.clear(),
                Extra::Sound => rule.sound.clear(),
                Extra::Colour => rule.highlight.clear(),
                Extra::Hide => rule.gag = false,
                Extra::Panel => rule.panel.clear(),
            }
        }
        if rule.gag && !rule.panel.trim().is_empty() {
            note(ui, tr("With the line hidden, it is only in the panel."));
        }
        let missing: Vec<Extra> = (visible.iter())
            .filter(|(_, on)| !*on)
            .map(|(extra, _)| *extra)
            .collect();
        if !missing.is_empty() {
            let add = widgets::quiet_button(ui, tr("+ Add an action"));
            egui::Popup::menu(&add).show(|ui| {
                widgets::menu_width(ui);
                for extra in missing {
                    if widgets::menu_item(ui, extra.label(), "").clicked() {
                        added.push(extra);
                        if extra == Extra::Hide {
                            rule.gag = true;
                        }
                        ui.close();
                    }
                }
            });
        }
        delete
    }
}

impl Extra {
    fn label(self) -> &'static str {
        match self {
            Extra::Say => tr("Say"),
            Extra::Sound => tr("Play a sound"),
            Extra::Colour => tr("Colour the match"),
            Extra::Hide => tr("Hide the line"),
            Extra::Panel => tr("Show it in the panel"),
        }
    }
}
