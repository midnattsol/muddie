//! The details of a timer.

use super::*;

impl Tab {
    pub(in crate::tab) fn timer_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let theme = theme::current();
        let delete = self.rule_head(ui, Kind::Timer, index, tr("Timer name"));
        let name = list::rule_title(&self.editor.draft.timers, index);
        let rule = &mut self.editor.draft.timers[index];
        widgets::card(
            ui,
            Some(widgets::Glyph::Timer),
            tr("When"),
            |_| {},
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let current = usize::from(!rule.when.repeat);
                    if let Some(chosen) =
                        widgets::segmented(ui, &[tr("Every"), tr("Once, after")], current)
                    {
                        rule.when.repeat = chosen == 0;
                    }
                    let mut seconds = f64::from(rule.when.tenths) / 10.0;
                    let field = ui.add(
                        crate::widgets::number(&mut seconds)
                            .range(0.1..=86_400.0)
                            .speed(0.5)
                            .max_decimals(1),
                    );
                    ui.ctx().accesskit_node_builder(field.id, |node| {
                        node.set_label(tr("seconds"));
                    });
                    rule.when.tenths = (seconds * 10.0).round().clamp(1.0, 864_000.0) as u32;
                    ui.label(RichText::new(tr("seconds")).color(theme.text_dim));
                    // Said back in minutes when that is how one thinks of it.
                    if rule.when.tenths >= 600 {
                        let said = format!("= {}", rule.when.interval());
                        ui.label(RichText::new(said).color(theme.text_faint));
                    }
                });
                let counted = if rule.when.repeat {
                    tr("Counted from when the session starts or the rules are run again.")
                } else {
                    tr(
                        "Goes off once, that long after the session starts or the rules are run again.",
                    )
                };
                note(ui, counted);
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
                commands_hint: "save",
                script_hint: "if gmcp.get(\"Char.Vitals\").hp < 50 then\n\tsend(\"rest\")\nend",
                about_commands: tr("One command per line."),
                about_script: tr("Run each time the timer goes off."),
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
