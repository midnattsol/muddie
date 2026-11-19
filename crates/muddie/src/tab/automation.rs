//! The automation window: triggers, aliases and macros made without a
//! script, in a list on the left with the selected one's details beside it.

use super::*;
use crate::rules::fill;
use crate::theme::Weight;
use crate::widgets;

/// How long edits must stop before they are saved and put to work.
const SAVE_AFTER: Duration = Duration::from_millis(900);

/// The sections of the window, in the order of the sidebar.
const TRIGGERS: usize = 0;
const ALIASES: usize = 1;
const MACROS: usize = 2;

impl Tab {
    /// Opens the automation window on one of its sections.
    pub fn open_automation(&mut self, section: usize) {
        if !self.rules_open {
            self.rules_draft = self.rules.clone();
            self.rules_seen = self.rules.clone();
        }
        self.rules_section = section;
        self.rules_open = true;
    }

    pub(super) fn automation_window(&mut self, ctx: &egui::Context) {
        if !self.rules_open {
            return;
        }
        let mut open = true;
        let room = ctx.content_rect().size() - egui::vec2(80.0, 150.0);
        let height = room.y.clamp(320.0, 560.0);
        widgets::dialog(
            ctx,
            "automation",
            tr("Automation"),
            &mut open,
            940.0,
            |ui| {
                // The panes get a region of their own, under the title.
                let size = egui::vec2(ui.available_width(), height);
                let (region, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(region));
                let plain = egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 4));
                Panel::left("automation sections")
                    .frame(plain)
                    .exact_size(168.0)
                    .show_separator_line(false)
                    .show(ui, |ui| self.automation_sections(ui));
                let list = egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4));
                Panel::left("automation list")
                    .frame(list)
                    .exact_size(280.0)
                    .show(ui, |ui| self.automation_list(ui));
                let detail = egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 6));
                CentralPanel::default().frame(detail).show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| self.automation_detail(ui));
                });
            },
        );
        // Edits are saved by themselves once they stop for a moment, and
        // when the window closes.
        if self.rules_draft != self.rules_seen {
            self.rules_seen = self.rules_draft.clone();
            self.rules_changed_at = Some(Instant::now());
        }
        let settled = self
            .rules_changed_at
            .is_some_and(|at| at.elapsed() >= SAVE_AFTER);
        if !open {
            self.close_automation();
        } else if self.rules_draft != self.rules && settled {
            self.rules_changed_at = None;
            self.save_rules();
        } else if let Some(at) = self.rules_changed_at {
            ctx.request_repaint_after(SAVE_AFTER.saturating_sub(at.elapsed()));
        }
    }

    /// Closes the automation window, saving what was still unsaved.
    pub fn close_automation(&mut self) {
        self.rules_open = false;
        self.recording_key = false;
        self.rules_changed_at = None;
        if self.rules_draft != self.rules {
            self.save_rules();
        }
    }

    fn automation_sections(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 2.0;
        let counts = [
            self.rules_draft.triggers.len(),
            self.rules_draft.aliases.len(),
            self.rules_draft.keys.len(),
        ];
        for (section, name) in [tr("Triggers"), tr("Aliases"), tr("Macros")]
            .into_iter()
            .enumerate()
        {
            let selected = self.rules_section == section;
            if widgets::nav_item(ui, selected, name, Some(counts[section])).clicked() {
                self.rules_section = section;
                self.rules_sample.clear();
                self.recording_key = false;
            }
        }
        ui.add_space(14.0);
        let saved = self.rules_draft == self.rules;
        let state = if saved {
            tr("All changes saved")
        } else {
            tr("Saving…")
        };
        ui.label(
            RichText::new(state)
                .font(theme::font(Weight::Regular, 11.5))
                .color(theme::current().text_faint),
        );
    }

    /// The titles and details of the rules of the current section.
    fn automation_rows(&self) -> Vec<(String, String, bool)> {
        let untitled = |name: &str, fallback: &str, new: &'static str| {
            let shown = if !name.trim().is_empty() {
                name
            } else {
                fallback
            };
            if shown.trim().is_empty() {
                tr(new).to_owned()
            } else {
                shown.trim().to_owned()
            }
        };
        let draft = &self.rules_draft;
        match self.rules_section {
            TRIGGERS => (draft.triggers.iter())
                .map(|rule| {
                    (
                        untitled(&rule.name, &rule.pattern, "New trigger"),
                        rule.summary(),
                        rule.enabled,
                    )
                })
                .collect(),
            ALIASES => (draft.aliases.iter())
                .map(|rule| {
                    (
                        untitled(&rule.name, &rule.pattern, "New alias"),
                        format!("→ {}", rule.send),
                        rule.enabled,
                    )
                })
                .collect(),
            _ => (draft.keys.iter())
                .map(|rule| {
                    (
                        untitled(&rule.name, &rule.key, "New macro"),
                        format!("{}  →  {}", rule.key, rule.send),
                        rule.enabled,
                    )
                })
                .collect(),
        }
    }

    fn automation_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let add = widgets::primary_button(ui, "+");
            ui.ctx().accesskit_node_builder(add.id, |node| {
                node.set_label(tr("Add"));
            });
            if add.on_hover_text(tr("Add")).clicked() {
                self.add_rule();
            }
            widgets::text_box(ui, &mut self.rules_filter, tr("Search"));
        });
        ui.add_space(4.0);
        let rows = self.automation_rows();
        let filter = self.rules_filter.trim().to_lowercase();
        let section = self.rules_section;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if rows.is_empty() {
                    ui.add_space(24.0);
                    let hint = RichText::new(tr("Nothing here yet.\nPress + to make one."))
                        .color(theme::current().text_faint);
                    ui.vertical_centered(|ui| ui.label(hint));
                }
                for (index, (title, detail, enabled)) in rows.iter().enumerate() {
                    let matches = filter.is_empty()
                        || title.to_lowercase().contains(&filter)
                        || detail.to_lowercase().contains(&filter);
                    if !matches {
                        continue;
                    }
                    let selected = self.rules_selected[section] == index;
                    if widgets::list_row(ui, selected, title, detail, *enabled).clicked() {
                        self.rules_selected[section] = index;
                        self.rules_sample.clear();
                        self.recording_key = false;
                    }
                }
            });
    }

    /// Adds an empty rule to the current section and selects it.
    fn add_rule(&mut self) {
        let draft = &mut self.rules_draft;
        let index = match self.rules_section {
            TRIGGERS => {
                draft.triggers.push(Trigger {
                    enabled: true,
                    ..Trigger::default()
                });
                draft.triggers.len() - 1
            }
            ALIASES => {
                draft.aliases.push(Alias {
                    enabled: true,
                    ..Alias::default()
                });
                draft.aliases.len() - 1
            }
            _ => {
                draft.keys.push(KeyBinding {
                    enabled: true,
                    ..KeyBinding::default()
                });
                draft.keys.len() - 1
            }
        };
        self.rules_selected[self.rules_section] = index;
        self.rules_filter.clear();
        self.rules_sample.clear();
    }

    fn automation_detail(&mut self, ui: &mut egui::Ui) {
        let section = self.rules_section;
        let index = self.rules_selected[section];
        let count = self.automation_rows().len();
        if index >= count {
            ui.add_space(60.0);
            let hint = match section {
                TRIGGERS => tr(
                    "A trigger reacts to what the MUD sends:\nit can answer, speak, play a sound, colour or hide the line.",
                ),
                ALIASES => {
                    tr("An alias turns a short command you type\ninto the commands you mean.")
                }
                _ => tr("A macro sends commands at the press of a key."),
            };
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(hint).color(theme::current().text_faint))
            });
            return;
        }
        ui.spacing_mut().item_spacing.y = 10.0;
        let delete = match section {
            TRIGGERS => self.trigger_detail(ui, index),
            ALIASES => self.alias_detail(ui, index),
            _ => self.macro_detail(ui, index),
        };
        if delete {
            let draft = &mut self.rules_draft;
            match section {
                TRIGGERS => drop(draft.triggers.remove(index)),
                ALIASES => drop(draft.aliases.remove(index)),
                _ => drop(draft.keys.remove(index)),
            }
            self.rules_selected[section] = index.saturating_sub(1);
            self.recording_key = false;
        }
    }

    fn trigger_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let rule = &mut self.rules_draft.triggers[index];
        let delete = rule_header(
            ui,
            &mut rule.name,
            &mut rule.enabled,
            tr("Name this trigger"),
        );
        widgets::caption(ui, tr("When"));
        let label = if rule.regex {
            tr("A line matches this regular expression")
        } else {
            tr("A line contains this text")
        };
        widgets::field(ui, label, &mut rule.pattern, tr("tells you"));
        widgets::switch_row(ui, &mut rule.regex, tr("Regular expression"));
        ui.add_space(4.0);
        widgets::caption(ui, tr("Do"));
        widgets::field(ui, tr("Send commands"), &mut rule.send, "kill %1; loot");
        ui.columns(2, |columns| {
            widgets::field(
                &mut columns[0],
                tr("Say"),
                &mut rule.speak,
                tr("%1 is here"),
            );
            widgets::field(
                &mut columns[1],
                tr("Play a sound"),
                &mut rule.sound,
                "alert.ogg",
            );
        });
        ui.columns(2, |columns| {
            widgets::field(
                &mut columns[0],
                tr("Colour the match"),
                &mut rule.highlight,
                "yellow, #ffcc00",
            );
            columns[1].add_space(22.0);
            widgets::switch_row(&mut columns[1], &mut rule.gag, tr("Hide the line"));
        });
        ui.add_space(4.0);
        widgets::caption(ui, tr("Try it"));
        widgets::text_box(ui, &mut self.rules_sample, tr("Paste a line from the MUD"));
        let rule = &self.rules_draft.triggers[index];
        let outcome = try_pattern(&rule.regex_source(), &self.rules_sample, |captures| {
            let mut effects = Vec::new();
            if !rule.send.trim().is_empty() {
                effects.push(format!(
                    "{} {}",
                    tr("sends"),
                    fill(rule.send.trim(), captures)
                ));
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
        delete
    }

    fn alias_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let rule = &mut self.rules_draft.aliases[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this alias"));
        widgets::caption(ui, tr("When"));
        let label = if rule.regex {
            tr("What you type matches this regular expression")
        } else {
            tr("You type this word, alone or followed by more")
        };
        widgets::field(ui, label, &mut rule.pattern, "k");
        widgets::switch_row(ui, &mut rule.regex, tr("Regular expression"));
        ui.add_space(4.0);
        widgets::caption(ui, tr("Do"));
        widgets::field(ui, tr("Send instead"), &mut rule.send, "kill %1");
        let hint = tr("%1 stands for what follows the word. Separate several commands with ;");
        ui.label(
            RichText::new(hint)
                .font(theme::font(Weight::Regular, 11.5))
                .color(theme::current().text_faint),
        );
        ui.add_space(4.0);
        widgets::caption(ui, tr("Try it"));
        widgets::text_box(ui, &mut self.rules_sample, tr("Type a command"));
        let rule = &self.rules_draft.aliases[index];
        let outcome = try_pattern(&rule.regex_source(), &self.rules_sample, |captures| {
            format!("{} {}", tr("sends"), fill(rule.send.trim(), captures))
        });
        show_outcome(ui, outcome);
        delete
    }

    fn macro_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let theme = theme::current();
        let recording = self.recording_key;
        let rule = &mut self.rules_draft.keys[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this macro"));
        widgets::caption(ui, tr("Key"));
        ui.horizontal(|ui| {
            let shown = if recording {
                tr("Press the key…").to_owned()
            } else if rule.key.trim().is_empty() {
                tr("None").to_owned()
            } else {
                rule.key.trim().to_uppercase().replace('+', " + ")
            };
            let colour = if recording { theme.accent } else { theme.text };
            egui::Frame::new()
                .fill(theme.background)
                .stroke(egui::Stroke::new(
                    1.0,
                    if recording {
                        theme.accent
                    } else {
                        theme.border
                    },
                ))
                .corner_radius(egui::CornerRadius::same(7))
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
            if ui.button(label).clicked() {
                self.recording_key = !recording;
            }
        });
        let hint = tr("Function keys, keys with Ctrl or Alt, and the numeric keypad can be used.");
        ui.label(
            RichText::new(hint)
                .font(theme::font(Weight::Regular, 11.5))
                .color(theme.text_faint),
        );
        ui.add_space(4.0);
        widgets::caption(ui, tr("Do"));
        let rule = &mut self.rules_draft.keys[index];
        widgets::field(ui, tr("Send commands"), &mut rule.send, "north");
        delete
    }

    /// While a macro's key is being recorded, takes the next key that can
    /// be bound as that key. Escape cancels.
    pub(super) fn record_key(&mut self, ui: &mut egui::Ui) {
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
                self.recording_key = false;
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
            let index = self.rules_selected[MACROS];
            if let Some(rule) = self.rules_draft.keys.get_mut(index) {
                rule.key = key;
            }
            self.recording_key = false;
            // The key was for the recorder: nothing else should act on it.
            ui.input_mut(|input| input.events.clear());
        }
    }
}

/// The top of a rule's details: its name, set large, with the switch that
/// turns it on and the button that deletes it. Returns true to delete.
fn rule_header(ui: &mut egui::Ui, name: &mut String, enabled: &mut bool, hint: &str) -> bool {
    let theme = theme::current();
    let mut delete = false;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = RichText::new(tr("Delete")).color(theme.danger);
            delete = ui
                .add(egui::Button::new(label).fill(egui::Color32::TRANSPARENT))
                .clicked();
            widgets::switch(ui, enabled, tr("Enabled"));
            ui.label(RichText::new(tr("Enabled")).color(theme.text_dim));
            let title = TextEdit::singleline(name)
                .font(theme::font(Weight::Semibold, 20.0))
                .hint_text(RichText::new(hint).color(theme.text_faint))
                .frame(egui::Frame::NONE)
                .desired_width(f32::INFINITY);
            let field = ui.add(title);
            ui.ctx().accesskit_node_builder(field.id, |node| {
                node.set_label(tr("Name"));
            });
        });
    });
    ui.add(egui::Separator::default().spacing(2.0));
    delete
}

/// What trying a rule on a sample gave.
enum Outcome {
    /// Nothing to try it on yet.
    Idle,
    /// The pattern is not a valid regular expression.
    Invalid(String),
    NoMatch,
    /// It matches, with a description of what the rule would do.
    Match(String),
}

fn try_pattern(
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

fn show_outcome(ui: &mut egui::Ui, outcome: Outcome) {
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
