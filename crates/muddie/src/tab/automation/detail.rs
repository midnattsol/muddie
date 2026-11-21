//! What the details of every kind of rule have in common: their heading,
//! what they do, and trying them.

use super::*;

/// A line of small print under a control.
pub(in crate::tab) fn note(ui: &mut egui::Ui, text: &str) {
    let small = RichText::new(text)
        .font(theme::font(Weight::Regular, 11.5))
        .color(theme::current().text_faint);
    ui.label(small);
}

/// Beside a switch that is off, that it is: what is not running should
/// not look as if it were.
pub(in crate::tab) fn off_badge(ui: &mut egui::Ui, on: bool) {
    if !on {
        widgets::badge(ui, tr("Disabled"), false);
    }
}

/// What a rule does: either the commands it sends or the script it runs.
pub(in crate::tab) struct ActionEdit<'a> {
    pub(in crate::tab) lua: &'a mut bool,
    pub(in crate::tab) commands: &'a mut String,
    pub(in crate::tab) script: &'a mut String,
    /// The rule matches something, so there are groups to refer to.
    pub(in crate::tab) matched: bool,
    pub(in crate::tab) commands_hint: &'a str,
    pub(in crate::tab) script_hint: &'a str,
    pub(in crate::tab) about_commands: &'a str,
    pub(in crate::tab) about_script: &'a str,
}

/// The part of a rule that says what it does, the same for triggers,
/// aliases and macros: a choice between commands and a Lua script, and
/// the editor of the one chosen, `rows` lines tall.
pub(in crate::tab) fn action_block(
    ui: &mut egui::Ui,
    action: ActionEdit<'_>,
    rows: usize,
    mistake: Option<&(usize, String)>,
) {
    ui.horizontal(|ui| {
        widgets::caption(ui, tr("Do"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let current = usize::from(*action.lua);
            match widgets::segmented(ui, &[tr("Commands"), "Luau"], current) {
                Some(0) => *action.lua = false,
                Some(_) => {
                    *action.lua = true;
                    // Start the script from the commands there were.
                    if action.script.trim().is_empty() {
                        *action.script =
                            crate::rules::commands_as_lua(action.commands, action.matched);
                    }
                }
                None => {}
            }
        });
    });
    if *action.lua {
        widgets::code_editor(ui, "Luau", action.script, action.script_hint, rows, mistake);
        note(ui, action.about_script);
    } else {
        let theme = theme::current();
        let editor = ui.add(
            TextEdit::multiline(action.commands)
                .font(egui::FontId::monospace(13.0))
                .hint_text(
                    RichText::new(action.commands_hint)
                        .font(egui::FontId::monospace(13.0))
                        .color(theme.text_faint),
                )
                .margin(egui::Margin::symmetric(12, 10))
                .desired_rows(rows)
                .desired_width(f32::INFINITY),
        );
        ui.ctx().accesskit_node_builder(editor.id, |node| {
            node.set_label(tr("Commands"));
        });
        note(ui, action.about_commands);
    }
}

/// What trying a rule on a sample gave.
pub(in crate::tab) enum Outcome {
    /// Nothing to try it on yet.
    Idle,
    /// The pattern is not a valid regular expression.
    Invalid(String),
    NoMatch,
    /// It matches, with a description of what the rule would do.
    Match(String),
}

pub(in crate::tab) fn try_pattern(
    source: &str,
    sample: &str,
    effect: impl FnOnce(&regex::Captures) -> String,
) -> Outcome {
    let regex = match regex::Regex::new(source) {
        Ok(regex) => regex,
        Err(error) => {
            // The last line of the error says what is wrong.
            let reason = error.to_string();
            return Outcome::Invalid(reason.lines().last().unwrap_or_default().trim().to_owned());
        }
    };
    if sample.is_empty() {
        return Outcome::Idle;
    }
    match regex.captures(sample) {
        Some(captures) => Outcome::Match(effect(&captures)),
        None => Outcome::NoMatch,
    }
}

pub(in crate::tab) fn show_outcome(ui: &mut egui::Ui, outcome: Outcome) {
    let theme = theme::current();
    let (text, colour) = match outcome {
        Outcome::Idle => return,
        Outcome::Invalid(reason) => (
            format!("{} {reason}", tr("The pattern is not valid:")),
            theme.danger,
        ),
        Outcome::NoMatch => (tr("Does not match.").to_owned(), theme.text_dim),
        Outcome::Match(effect) if effect.is_empty() => (tr("Matches.").to_owned(), theme.good),
        Outcome::Match(effect) => (format!("{} {effect}", tr("Matches:")), theme.good),
    };
    ui.label(RichText::new(text).color(colour));
}

pub(in crate::tab) fn rule_header(
    ui: &mut egui::Ui,
    name: &mut String,
    enabled: &mut bool,
    hint: &str,
    rename: &mut bool,
) -> bool {
    let mut delete = false;
    // The name, then what is at the right of it, in the order they are
    // read.
    let id = ui.make_persistent_id("rule header");
    let font = theme::font(Weight::Semibold, 20.0);
    let state = *enabled;
    widgets::right_last(
        ui,
        id,
        |ui| {
            name_field(ui, name, font.clone(), hint, rename, true);
        },
        |ui| {
            off_badge(ui, state);
            widgets::switch(ui, enabled, tr("Enabled")).on_hover_text(tr("Enabled"));
            delete = widgets::icon_button(ui, widgets::Icon::Trash, tr("Delete"), false).clicked();
        },
    );
    ui.add(egui::Separator::default().spacing(2.0));
    delete
}

/// The name at the head of the details, whatever they are of, edited where
/// it is: in its own font, with no frame. When renaming is asked for (F2,
/// or Rename in a row's menu) it is given the keys with its text chosen,
/// to be typed over.
pub(in crate::tab) fn name_field(
    ui: &mut egui::Ui,
    name: &mut String,
    font: egui::FontId,
    hint: &str,
    rename: &mut bool,
    editable: bool,
) -> egui::Response {
    let id = ui.make_persistent_id("detail name");
    let field = ui.add(
        TextEdit::singleline(name)
            .id(id)
            .font(font)
            .hint_text(RichText::new(hint).color(theme::current().text_faint))
            .frame(egui::Frame::NONE)
            .interactive(editable)
            .desired_width(f32::INFINITY),
    );
    ui.ctx()
        .accesskit_node_builder(field.id, |node| node.set_label(tr("Name")));
    if std::mem::take(rename) && editable {
        field.request_focus();
        if let Some(mut state) = TextEdit::load_state(ui.ctx(), id) {
            let all = egui::text::CCursorRange::select_all_str(name);
            state.cursor.set_char_range(Some(all));
            state.store(ui.ctx(), id);
        }
    }
    field
}

/// How many rows of code fit in a view `height` tall once `taken` of it
/// has gone to what surrounds the editor.
pub(in crate::tab) fn script_rows(height: f32, taken: f32) -> usize {
    (((height - taken) / 17.0) as usize).max(8)
}

/// The colours to choose among, as a row of swatches.
pub(in crate::tab) fn swatches(ui: &mut egui::Ui, colour: &mut String) {
    let theme = theme::current();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for name in crate::panels::GAUGE_COLOURS {
            let shown = crate::panels::colour_named(name).unwrap_or(theme.accent);
            if widgets::swatch(ui, shown, *colour == name, tr(name)).clicked() {
                *colour = name.to_owned();
            }
        }
    });
}

impl Tab {
    pub(in crate::tab) fn automation_detail(&mut self, ui: &mut egui::Ui) {
        if self.editor.section == DATA {
            return self.data_view(ui);
        }
        let section = self.editor.section;
        let index = self.editor.selected[section];
        if self.chosen().len() > 1 {
            return self.bulk_detail(ui);
        }
        if index >= self.section_len() {
            ui.add_space(60.0);
            let hint = RichText::new(empty_hint(section)).color(theme::current().text_faint);
            ui.vertical_centered(|ui| ui.label(hint));
            return;
        }
        ui.spacing_mut().item_spacing.y = 10.0;
        let delete = match section {
            TRIGGERS => self.trigger_detail(ui, index),
            ALIASES => self.alias_detail(ui, index),
            MACROS => self.macro_detail(ui, index),
            TIMERS => self.timer_detail(ui, index),
            GAUGES => self.gauge_detail(ui, index),
            VARIABLES => self.variable_detail(ui, index),
            _ => self.script_detail(ui, index),
        };
        if delete {
            self.delete_chosen();
            self.list_scripts();
        }
    }
}

/// What a section is for, said while nothing in it is chosen.
fn empty_hint(section: usize) -> &'static str {
    match section {
        TRIGGERS => tr(
            "A trigger reacts to what the MUD sends:\nit can answer, speak, play a sound, colour or hide the line.",
        ),
        ALIASES => tr("An alias turns a short command you type\ninto the commands you mean."),
        MACROS => tr("A macro sends commands at the press of a key."),
        TIMERS => tr("A timer does something after a while,\nonce or again and again."),
        GAUGES => tr(
            "A gauge shows something the MUD reports: a bar for your health,\na light for whether you are poisoned, a number for your gold.",
        ),
        VARIABLES => tr(
            "A variable is a value with a name, kept from one session to the next:\nwho you are hunting, what you carry, how many you have killed.",
        ),
        _ => tr(
            "Scripts are Luau code that is always loaded: what no rule can do,\nsuch as building the map or acting on what your MUD reports.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(source: &str, sample: &str) -> String {
        match try_pattern(source, sample, |captures| fill("kill %1", captures)) {
            Outcome::Idle => "idle".into(),
            Outcome::Invalid(reason) => format!("invalid: {reason}"),
            Outcome::NoMatch => "no match".into(),
            Outcome::Match(effect) => format!("match: {effect}"),
        }
    }

    #[test]
    fn trying_a_pattern_says_what_would_happen() {
        assert_eq!(
            outcome(r"^(\w+) attacks", "Orc attacks you"),
            "match: kill Orc"
        );
        assert_eq!(outcome(r"^(\w+) attacks", "You attack"), "no match");
        assert_eq!(outcome(r"^(\w+) attacks", ""), "idle");
        assert!(outcome("(unclosed", "x").starts_with("invalid: "));
        // An invalid pattern is reported even with nothing to try it on.
        assert!(outcome("(unclosed", "").starts_with("invalid: "));
    }
}
