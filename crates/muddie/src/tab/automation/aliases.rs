//! The details of an alias.

use super::*;

impl Tab {
    pub(in crate::tab) fn alias_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let delete = self.rule_head(ui, Kind::Alias, index, tr("Alias name"));
        let rule = &mut self.editor.draft.aliases[index];
        let (sample, patterns) = (&mut self.editor.sample, &mut self.editor.patterns);
        widgets::card(
            ui,
            Some(widgets::Glyph::When),
            tr("When"),
            |_| {},
            |ui| {
                let kinds = [tr("Word"), tr("Regular expression")];
                if let Some(kind) = widgets::segmented(ui, &kinds, usize::from(rule.when.regex)) {
                    rule.when.regex = kind == 1;
                }
                let label = if rule.when.regex {
                    tr("What you type matches this regular expression")
                } else {
                    tr("You type this word, alone or followed by more")
                };
                let source = rule.when.regex_source();
                let invalid = patterns.of(&source).err().map(str::to_owned);
                pattern_field(
                    ui,
                    label,
                    &mut rule.when.pattern,
                    rule.when.regex,
                    "k",
                    invalid.as_deref(),
                );
                let source = rule.when.regex_source();
                let rule = &*rule;
                try_block(ui, sample, tr("Type a command"), |sample| {
                    try_pattern(patterns, &source, sample, |captures| {
                        if rule.lua {
                            return tr("runs the script").to_owned();
                        }
                        let sent: Vec<String> = (crate::rules::commands(&rule.send).iter())
                            .map(|command| fill(command, captures))
                            .collect();
                        format!("{} {}", tr("sends"), sent.join("; "))
                    })
                });
            },
        );
        let name = list::rule_title(&self.editor.draft.aliases, index);
        let rule = &mut self.editor.draft.aliases[index];
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
                commands_hint: "kill %1",
                script_hint: "send(\"say \" .. string.upper(m[1]))",
                about_commands: tr("One command per line. %1 stands for what follows the word."),
                about_script: tr("m holds the match, input what was typed."),
                sizes: &mut self.editor.code,
                name,
            },
            mistake.as_ref(),
        );
        if let Some(said) = said {
            self.voice.say_now(&said);
        }
        delete
    }
}
