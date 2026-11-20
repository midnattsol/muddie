//! The details of an alias.

use super::*;

impl Tab {
    pub(in crate::tab) fn alias_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let height = self.editor.detail_height;
        let delete = self.rule_head(ui, Kind::Alias, index, tr("Name this alias"));
        let rule = &mut self.editor.draft.aliases[index];
        widgets::caption(ui, tr("When"));
        let label = if rule.when.regex {
            tr("What you type matches this regular expression")
        } else {
            tr("You type this word, alone or followed by more")
        };
        widgets::field(ui, label, &mut rule.when.pattern, "k");
        widgets::switch_row(ui, &mut rule.when.regex, tr("Regular expression"));
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
                matched: true,
                commands_hint: "kill %1",
                script_hint: "send(\"say \" .. string.upper(m[1]))",
                about_commands: tr("One command per line. %1 stands for what follows the word."),
                about_script: tr("m holds the match, input what was typed."),
            },
            script_rows(height, 400.0),
            mistake.as_ref(),
        );
        ui.add_space(6.0);
        widgets::caption(ui, tr("Try it"));
        widgets::text_box(ui, &mut self.editor.sample, tr("Type a command"));
        let rule = &self.editor.draft.aliases[index];
        let outcome = try_pattern(&rule.when.regex_source(), &self.editor.sample, |captures| {
            if rule.lua {
                return tr("runs the script").to_owned();
            }
            let sent: Vec<String> = crate::rules::commands(&rule.send)
                .map(|command| fill(command, captures))
                .collect();
            format!("{} {}", tr("sends"), sent.join("; "))
        });
        show_outcome(ui, outcome);
        delete
    }
}
