//! The details of a trigger.

use super::*;

/// What a trigger can do besides its commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::tab) enum Extra {
    Speak,
    Sound,
    Highlight,
    Gag,
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
        let delete = self.rule_head(ui, Kind::Trigger, index, tr("Trigger name"));
        let rule = &mut self.editor.draft.triggers[index];
        when(ui, rule, &mut self.editor.sample, &mut self.editor.patterns);
        self.trigger_does(ui, index);
        let rule = &mut self.editor.draft.triggers[index];
        also(ui, rule, &mut self.editor.extras_added.1, &text_panels);
        delete
    }

    /// What it does: its commands, or its script, checked as it is typed.
    fn trigger_does(&mut self, ui: &mut egui::Ui, index: usize) {
        let name = list::rule_title(&self.editor.draft.triggers, index);
        let rule = &mut self.editor.draft.triggers[index];
        let mistake = if rule.lua {
            self.editor.checks.of(&self.scripts, &rule.script)
        } else {
            None
        };
        let said = action_block(
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
                sizes: &mut self.editor.code,
                name,
            },
            mistake.as_ref(),
        );
        if let Some(said) = said {
            self.voice.say_now(&said);
        }
    }
}

/// When it goes off: how the pattern is read, the pattern, and a line to
/// try it on with what it would do, together in a card.
fn when(
    ui: &mut egui::Ui,
    rule: &mut crate::rules::Trigger,
    sample: &mut String,
    patterns: &mut Patterns,
) {
    widgets::card(
        ui,
        Some(widgets::Glyph::When),
        tr("When"),
        |_| {},
        |ui| {
            let kinds = [tr("Contains"), tr("Regular expression")];
            if let Some(kind) = widgets::segmented(ui, &kinds, usize::from(rule.when.regex)) {
                rule.when.regex = kind == 1;
            }
            let label = if rule.when.regex {
                tr("A line matches this regular expression")
            } else {
                tr("A line contains this text")
            };
            let source = rule.when.regex_source();
            let invalid = patterns.of(&source).err().map(str::to_owned);
            pattern_field(
                ui,
                label,
                &mut rule.when.pattern,
                rule.when.regex,
                tr("tells you"),
                invalid.as_deref(),
            );
            let source = rule.when.regex_source();
            try_block(
                ui,
                sample,
                tr("Paste a line from the MUD to try it"),
                |sample| {
                    try_pattern(patterns, &source, sample, |captures| {
                        effects(rule, captures)
                    })
                },
            );
        },
    );
}

/// What a trigger would do with a match, said in a line.
fn effects(rule: &crate::rules::Trigger, captures: &regex::Captures) -> String {
    let mut effects = Vec::new();
    let sent: Vec<String> = (crate::rules::commands(&rule.send).iter())
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

/// What it does besides, in a card: only what it was given, or asked for
/// here, each on a line of its own with what it takes and the cross that
/// takes it away; and a button to add the rest.
fn also(
    ui: &mut egui::Ui,
    rule: &mut crate::rules::Trigger,
    added: &mut Vec<Extra>,
    text_panels: &[String],
) {
    let given = |extra: Extra| match extra {
        Extra::Speak => !rule.when.speak.trim().is_empty(),
        Extra::Sound => !rule.when.sound.trim().is_empty(),
        Extra::Highlight => !rule.when.highlight.trim().is_empty(),
        Extra::Gag => rule.when.gag,
        Extra::Panel => !rule.when.panel.trim().is_empty(),
    };
    let all = [
        Extra::Speak,
        Extra::Sound,
        Extra::Highlight,
        Extra::Gag,
        Extra::Panel,
    ];
    let (shown, missing): (Vec<Extra>, Vec<Extra>) = all
        .into_iter()
        .partition(|extra| given(*extra) || added.contains(extra));
    widgets::card(
        ui,
        Some(widgets::Glyph::Also),
        tr("Also"),
        |_| {},
        |ui| {
            let mut removed = None;
            for extra in &shown {
                if extra_row(ui, rule, *extra, text_panels) {
                    removed = Some(*extra);
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
            let add = widgets::add_button(ui, tr("Add an action"));
            crate::widgets::menu_of(&add).show(|ui| {
                widgets::menu_width(ui);
                for extra in missing {
                    if widgets::menu_item_with(ui, extra.glyph(), extra.label(), "").clicked() {
                        added.push(extra);
                        if extra == Extra::Gag {
                            rule.when.gag = true;
                        }
                        ui.close();
                    }
                }
            });
        },
    );
}

/// One thing it does besides, on a line: its picture and its name, what it
/// takes, and the cross that takes it away, next to it. Returns whether
/// taking it away was asked.
fn extra_row(
    ui: &mut egui::Ui,
    rule: &mut crate::rules::Trigger,
    extra: Extra,
    text_panels: &[String],
) -> bool {
    let theme = theme::current();
    let mut removed = false;
    let id = ui.make_persistent_id(("extra", extra.label()));
    widgets::right_last(
        ui,
        id,
        &mut (),
        |_, ui| {
            widgets::glyph(ui, extra.glyph(), 12.0, theme.text_dim);
            // The names as wide as each other, so that what they take
            // lines up.
            let label = RichText::new(extra.label()).color(theme.text);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_width(150.0);
                ui.add(Label::new(label).selectable(false).truncate());
            });
            ui.scope(|ui| {
                ui.set_width(ui.available_width());
                match extra {
                    Extra::Speak => {
                        widgets::text_box(ui, &mut rule.when.speak, tr("%1 is here"));
                    }
                    Extra::Sound => {
                        widgets::text_box(ui, &mut rule.when.sound, "alert.ogg");
                    }
                    Extra::Highlight => super::detail::swatches(ui, &mut rule.when.highlight),
                    Extra::Gag => {
                        let text =
                            RichText::new(tr("The line is not shown.")).color(theme.text_dim);
                        ui.label(text);
                    }
                    Extra::Panel => panel_choice(ui, &mut rule.when.panel, text_panels),
                }
            });
        },
        |_, ui| {
            let what = trf("Remove “{0}”", &[extra.label()]);
            removed = widgets::close_button(ui, &what, 20.0).clicked();
        },
    );
    removed
}

/// The panel of text it shows its line in.
fn panel_choice(ui: &mut egui::Ui, panel: &mut String, text_panels: &[String]) {
    let choices: Vec<(String, String)> = (text_panels.iter())
        .map(|name| (name.clone(), name.clone()))
        .collect();
    crate::widgets::choose(ui, "trigger panel", panel, &choices, |panel| {
        match panel.trim().is_empty() {
            true => tr("Choose a panel").to_owned(),
            false => panel,
        }
    });
}

impl Extra {
    /// Takes it from the trigger.
    fn take_from(self, rule: &mut crate::rules::Trigger) {
        match self {
            Extra::Speak => rule.when.speak.clear(),
            Extra::Sound => rule.when.sound.clear(),
            Extra::Highlight => rule.when.highlight.clear(),
            Extra::Gag => rule.when.gag = false,
            Extra::Panel => rule.when.panel.clear(),
        }
    }

    fn glyph(self) -> widgets::Glyph {
        match self {
            Extra::Speak => widgets::Glyph::Say,
            Extra::Sound => widgets::Glyph::Sound,
            Extra::Highlight => widgets::Glyph::Colour,
            Extra::Gag => widgets::Glyph::Hide,
            Extra::Panel => widgets::Glyph::Panel,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Extra::Speak => tr("Say"),
            Extra::Sound => tr("Play a sound"),
            Extra::Highlight => tr("Colour the match"),
            Extra::Gag => tr("Hide the line"),
            Extra::Panel => tr("Show it in the panel"),
        }
    }
}
