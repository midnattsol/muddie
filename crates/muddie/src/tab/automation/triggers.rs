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
        // What else it does that was asked for here, though still empty.
        if self.editor.extras_added.0 != index {
            self.editor.extras_added = (index, Vec::new());
        }
        let delete = self.rule_head(ui, Kind::Trigger, index, tr("Name this trigger"));
        let rule = &mut self.editor.draft.triggers[index];
        when(ui, rule, &mut self.editor.sample);
        ui.add_space(6.0);
        self.trigger_does(ui, index);
        ui.add_space(6.0);
        let rule = &mut self.editor.draft.triggers[index];
        also(ui, rule, &mut self.editor.extras_added.1, &text_panels);
        delete
    }

    /// What it does: its commands, or its script, checked as it is typed.
    fn trigger_does(&mut self, ui: &mut egui::Ui, index: usize) {
        let height = self.editor.detail_height;
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
    }
}

/// When it goes off: how the pattern is read, the pattern, and a line to
/// try it on with what it would do, all together.
fn when(ui: &mut egui::Ui, rule: &mut crate::rules::Trigger, sample: &mut String) {
    widgets::caption(ui, tr("When"));
    let kinds = [tr("Contains"), tr("Regular expression")];
    if let Some(kind) = widgets::segmented(ui, &kinds, usize::from(rule.when.regex)) {
        rule.when.regex = kind == 1;
    }
    let label = if rule.when.regex {
        tr("A line matches this regular expression")
    } else {
        tr("A line contains this text")
    };
    widgets::field(ui, label, &mut rule.when.pattern, tr("tells you"));
    widgets::text_box(ui, sample, tr("Paste a line from the MUD to try it"));
    let outcome = try_pattern(&rule.when.regex_source(), sample, |captures| {
        effects(rule, captures)
    });
    show_outcome(ui, outcome);
}

/// What a trigger would do with a match, said in a line.
fn effects(rule: &crate::rules::Trigger, captures: &regex::Captures) -> String {
    let mut effects = Vec::new();
    let sent: Vec<String> = crate::rules::commands(&rule.send)
        .map(|command| fill(command, captures))
        .collect();
    if rule.lua {
        effects.push(tr("runs the script").to_owned());
    } else if !sent.is_empty() {
        effects.push(format!("{} {}", tr("sends"), sent.join("; ")));
    }
    if !rule.when.speak.trim().is_empty() {
        effects.push(format!(
            "{} “{}”",
            tr("says"),
            fill(rule.when.speak.trim(), captures)
        ));
    }
    effects.join(" · ")
}

/// What it does besides: only what it was given, or asked for here, each
/// to take away again; and the rest to add.
fn also(
    ui: &mut egui::Ui,
    rule: &mut crate::rules::Trigger,
    added: &mut Vec<Extra>,
    text_panels: &[String],
) {
    let given = |extra: Extra| match extra {
        Extra::Say => !rule.when.speak.trim().is_empty(),
        Extra::Sound => !rule.when.sound.trim().is_empty(),
        Extra::Colour => !rule.when.highlight.trim().is_empty(),
        Extra::Hide => rule.when.gag,
        Extra::Panel => !rule.when.panel.trim().is_empty(),
    };
    let all = [
        Extra::Say,
        Extra::Sound,
        Extra::Colour,
        Extra::Hide,
        Extra::Panel,
    ];
    let (shown, missing): (Vec<Extra>, Vec<Extra>) = all
        .into_iter()
        .partition(|extra| given(*extra) || added.contains(extra));
    if !shown.is_empty() {
        widgets::caption(ui, tr("Also"));
    }
    let mut removed = None;
    for extra in shown {
        if extra_row(ui, rule, extra, text_panels) {
            removed = Some(extra);
        }
    }
    if let Some(extra) = removed {
        added.retain(|known| *known != extra);
        extra.take_from(rule);
    }
    if rule.when.gag && !rule.when.panel.trim().is_empty() {
        note(ui, tr("With the line hidden, it is only in the panel."));
    }
    if missing.is_empty() {
        return;
    }
    let add = widgets::quiet_button(ui, tr("+ Add an action"));
    egui::Popup::menu(&add).show(|ui| {
        widgets::menu_width(ui);
        for extra in missing {
            if widgets::menu_item(ui, extra.label(), "").clicked() {
                added.push(extra);
                if extra == Extra::Hide {
                    rule.when.gag = true;
                }
                ui.close();
            }
        }
    });
}

/// One thing it does besides: its name, with the cross that takes it away
/// at the far end, and what it takes under them. Returns whether taking it
/// away was asked.
fn extra_row(
    ui: &mut egui::Ui,
    rule: &mut crate::rules::Trigger,
    extra: Extra,
    text_panels: &[String],
) -> bool {
    let mut removed = false;
    ui.horizontal(|ui| {
        let label = RichText::new(extra.label()).color(theme::current().text_dim);
        ui.add(Label::new(label).selectable(false));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let what = trf("Remove “{0}”", &[extra.label()]);
            removed = widgets::close_button(ui, &what, 20.0).clicked();
        });
    });
    match extra {
        Extra::Say => drop(widgets::text_box(
            ui,
            &mut rule.when.speak,
            tr("%1 is here"),
        )),
        Extra::Sound => drop(widgets::text_box(ui, &mut rule.when.sound, "alert.ogg")),
        Extra::Colour => drop(widgets::text_box(
            ui,
            &mut rule.when.highlight,
            "yellow, #ffcc00",
        )),
        Extra::Hide => rule.when.gag = true,
        Extra::Panel => panel_choice(ui, &mut rule.when.panel, text_panels),
    }
    ui.add_space(4.0);
    removed
}

/// The panel of text it shows its line in.
fn panel_choice(ui: &mut egui::Ui, panel: &mut String, text_panels: &[String]) {
    let chosen = match panel.trim().is_empty() {
        true => tr("Choose a panel").to_owned(),
        false => panel.clone(),
    };
    crate::widgets::dropdown("trigger panel")
        .selected_text(chosen)
        .show_ui(ui, |ui| {
            for name in text_panels {
                if crate::widgets::choice(ui, *panel == *name, name).clicked() {
                    panel.clone_from(name);
                }
            }
        });
}

impl Extra {
    /// Takes it from the trigger.
    fn take_from(self, rule: &mut crate::rules::Trigger) {
        match self {
            Extra::Say => rule.when.speak.clear(),
            Extra::Sound => rule.when.sound.clear(),
            Extra::Colour => rule.when.highlight.clear(),
            Extra::Hide => rule.when.gag = false,
            Extra::Panel => rule.when.panel.clear(),
        }
    }

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
