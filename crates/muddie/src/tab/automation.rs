//! The automation window: triggers, aliases and macros made without a
//! script, in a list on the left with the selected one's details beside it.

use super::*;
use crate::rules::{Gauge, fill};
use crate::theme::Weight;
use crate::widgets;

/// How long edits must stop before they are saved and put to work.
const SAVE_AFTER: Duration = Duration::from_millis(900);

/// The sections of the window, in the order of the sidebar.
const TRIGGERS: usize = 0;
const ALIASES: usize = 1;
const MACROS: usize = 2;
const GAUGES: usize = 3;
const SCRIPTS: usize = 4;

/// Scripts to start from: what each is for, its file name, its text.
const EXAMPLES: [(&str, &str, &str); 5] = [
    (
        "A map the MUD draws itself",
        "drawn-map",
        include_str!("../../../../examples/scripts/drawn-map.luau"),
    ),
    (
        "A map the MUD sends whole",
        "server-map",
        include_str!("../../../../examples/scripts/server-map.luau"),
    ),
    (
        "A map read from the room descriptions",
        "text-mapper",
        include_str!("../../../../examples/scripts/text-mapper.luau"),
    ),
    (
        "Words to complete, given by the MUD",
        "completions",
        include_str!("../../../../examples/scripts/completions.luau"),
    ),
    (
        "Gauges from the MUD's vitals",
        "vitals-gmcp",
        include_str!("../../../../examples/scripts/vitals-gmcp.luau"),
    ),
];

impl Tab {
    /// Opens the automation window on one of its sections.
    pub fn open_automation(&mut self, section: usize) {
        if !self.rules_open {
            self.rules_draft = self.rules.clone();
            self.rules_seen = self.rules.clone();
        }
        self.rules_section = section;
        self.rules_open = true;
        self.list_scripts();
    }

    /// Opens the automation view where it was left.
    pub fn reopen_automation(&mut self) {
        self.open_automation(self.rules_section);
    }

    /// Whether the automation view is on screen instead of the game.
    pub fn automation_open(&self) -> bool {
        self.rules_open
    }

    /// The automation view, which takes the session's whole space: the
    /// sections, the list of the current one, and the selected item.
    pub(super) fn automation_page(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let theme = theme::current();
        let mut open = true;
        // Escape goes back to the game, unless it is cancelling a key.
        if !self.recording_key
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
        {
            open = false;
        }
        let page = egui::Frame::new()
            .fill(theme.surface)
            .inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 2,
                bottom: 8,
            });
        CentralPanel::default().frame(page).show(ui, |ui| {
            // The sections stand alone at the left, with air after them.
            // The list and the details are two cards; the gap between
            // them is the one place worth dragging, to trade their widths.
            const SECTIONS: f32 = 176.0;
            const AIR: f32 = 18.0;
            const HANDLE: f32 = 12.0;
            let full = ui.available_rect_before_wrap();
            let most = (full.width() - SECTIONS - AIR - HANDLE - 460.0).max(240.0);
            self.panes[1] = self.panes[1].clamp(240.0, most);
            let column = |left: f32, width: f32| {
                egui::Rect::from_min_max(
                    egui::pos2(left, full.top()),
                    egui::pos2(left + width, full.bottom()),
                )
            };
            let sections = column(full.left(), SECTIONS);
            let list = column(sections.right() + AIR, self.panes[1]);
            let handle = column(list.right(), HANDLE);
            let detail = column(handle.right(), full.right() - handle.right());

            let id = ui.make_persistent_id("automation handle");
            let response = ui
                .interact(handle, id, egui::Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Splitter, true, tr("Resize"))
            });
            if response.dragged() {
                self.panes[1] += response.drag_delta().x;
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if response.hovered() || response.dragged() {
                // A short grip, only while it can be taken.
                let grip = handle.center();
                let stroke = egui::Stroke::new(3.0, theme.accent);
                ui.painter().line_segment(
                    [grip - egui::vec2(0.0, 18.0), grip + egui::vec2(0.0, 18.0)],
                    stroke,
                );
            }

            let card = egui::CornerRadius::same(14);
            ui.painter().rect_filled(list, card, theme.field);
            ui.painter().rect_filled(detail, card, theme.field);
            let mut pane =
                |name: &str,
                 rect: egui::Rect,
                 raised: bool,
                 add: &mut dyn FnMut(&mut Self, &mut egui::Ui)| {
                    // Each pane under its own name, or what one remembers
                    // (how far it is scrolled, say) would be the other's too.
                    let builder = egui::UiBuilder::new()
                        .id_salt(("automation pane", name))
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(egui::Align::Min));
                    ui.scope_builder(builder, |ui| {
                        ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                        if raised {
                            // On a card, fields stand out from it.
                            ui.visuals_mut().text_edit_bg_color = Some(theme.surface);
                            ui.visuals_mut().extreme_bg_color = theme.surface;
                        }
                        add(self, ui);
                    });
                };
            pane(
                "sections",
                sections.shrink2(egui::vec2(2.0, 8.0)),
                false,
                &mut |tab, ui| tab.automation_sections(ui),
            );
            pane(
                "list",
                list.shrink2(egui::vec2(10.0, 12.0)),
                true,
                &mut |tab, ui| tab.automation_list(ui),
            );
            let inner = detail.shrink2(egui::vec2(26.0, 18.0));
            pane("detail", inner, true, &mut |tab, ui| {
                tab.detail_height = inner.height();
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| tab.automation_detail(ui));
            });
        });
        // Edits are saved by themselves once they stop for a moment, and
        // when the window closes.
        if self.rules_draft != self.rules_seen {
            self.rules_seen = self.rules_draft.clone();
            self.rules_changed_at = Some(Instant::now());
        }
        let settled = self
            .rules_changed_at
            .is_some_and(|at| at.elapsed() >= SAVE_AFTER);
        // The script being edited is saved the same way.
        if self.script_text != self.script_seen {
            self.script_seen = self.script_text.clone();
            self.script_changed_at = Some(Instant::now());
        }
        if let Some(at) = self.script_changed_at {
            if at.elapsed() >= SAVE_AFTER {
                self.flush_script();
            } else {
                ctx.request_repaint_after(SAVE_AFTER.saturating_sub(at.elapsed()));
            }
        }
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
        self.flush_script();
        if self.rules_draft != self.rules {
            self.save_rules();
        }
    }

    fn automation_sections(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 2.0;
        let heading = RichText::new(tr("Automation"))
            .font(theme::font(Weight::Semibold, 17.0))
            .color(theme::current().text);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.add(Label::new(heading).selectable(false));
        });
        ui.add_space(12.0);
        let counts = [
            self.rules_draft.triggers.len(),
            self.rules_draft.aliases.len(),
            self.rules_draft.keys.len(),
            self.rules_draft.gauges.len(),
            self.script_files.len(),
        ];
        let names = [
            tr("Triggers"),
            tr("Aliases"),
            tr("Macros"),
            tr("Gauges"),
            tr("Scripts"),
        ];
        for (section, name) in names.into_iter().enumerate() {
            let selected = self.rules_section == section;
            if widgets::nav_item(ui, selected, name, Some(counts[section])).clicked() {
                self.flush_script();
                self.rules_section = section;
                self.rules_sample.clear();
                self.recording_key = false;
            }
        }
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            let saved = self.rules_draft == self.rules && self.script_text == self.script_saved;
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
        });
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
                        format!(
                            "→ {}",
                            crate::rules::commands(&rule.send)
                                .collect::<Vec<_>>()
                                .join("; ")
                        ),
                        rule.enabled,
                    )
                })
                .collect(),
            MACROS => (draft.keys.iter())
                .map(|rule| {
                    (
                        untitled(&rule.name, &rule.key, "New macro"),
                        format!(
                            "{}  →  {}",
                            rule.key,
                            crate::rules::commands(&rule.send)
                                .collect::<Vec<_>>()
                                .join("; ")
                        ),
                        rule.enabled,
                    )
                })
                .collect(),
            GAUGES => (draft.gauges.iter())
                .map(|rule| {
                    (
                        untitled(&rule.name, &rule.value, "New gauge"),
                        rule.value.clone(),
                        rule.enabled,
                    )
                })
                .collect(),
            _ => (self.script_files.iter())
                .map(|file| {
                    let name = file
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    let everywhere = file.parent().is_some_and(|dir| !dir.ends_with("scripts"));
                    let detail = if everywhere {
                        tr("In every session")
                    } else {
                        tr("In this profile")
                    };
                    (name, detail.to_owned(), true)
                })
                .collect(),
        }
    }

    /// Looks again for the script files that can be edited.
    fn list_scripts(&mut self) {
        self.script_files = match &self.profiles {
            Some(profiles) => profiles.editable_scripts(self.profile.as_deref()),
            None => Vec::new(),
        };
    }

    /// Saves the script being edited if it changed, renames its file if
    /// its name was changed, and runs the scripts again.
    fn flush_script(&mut self) {
        self.script_changed_at = None;
        let Some(path) = self.script_loaded.clone() else {
            return;
        };
        let mut changed = false;
        if self.script_text != self.script_saved {
            let written = (path.parent().map_or(Ok(()), std::fs::create_dir_all))
                .and_then(|()| std::fs::write(&path, &self.script_text));
            match written {
                Ok(()) => {
                    self.script_saved = self.script_text.clone();
                    changed = true;
                }
                Err(error) => self.note(&trf(
                    "Could not save the script: {0}",
                    &[&error.to_string()],
                )),
            }
        }
        let name = self.script_name.trim().trim_end_matches(".luau").to_owned();
        let current = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let own = path.parent().is_some_and(|dir| dir.ends_with("scripts"));
        if own && name != current && crate::profile::valid_name(&name) {
            let renamed = path.with_file_name(format!("{name}.luau"));
            if !renamed.exists() && std::fs::rename(&path, &renamed).is_ok() {
                self.script_loaded = Some(renamed.clone());
                self.list_scripts();
                let index = self.script_files.iter().position(|file| *file == renamed);
                self.rules_selected[SCRIPTS] = index.unwrap_or(0);
                changed = true;
            }
        }
        if changed {
            self.load_scripts();
        }
    }

    fn automation_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let add = widgets::primary_button(ui, "+");
            ui.ctx().accesskit_node_builder(add.id, |node| {
                node.set_label(tr("Add"));
            });
            let add = add.on_hover_text(tr("Add"));
            if self.rules_section == SCRIPTS {
                // A script can start empty or from an example.
                egui::Popup::menu(&add).show(|ui| {
                    widgets::menu_width(ui);
                    if widgets::menu_item(ui, tr("Empty script"), "").clicked() {
                        self.add_script(None);
                        ui.close();
                    }
                    widgets::menu_separator(ui);
                    widgets::menu_heading(ui, tr("From an example"));
                    for (title, name, text) in EXAMPLES {
                        if widgets::menu_item(ui, tr(title), "").clicked() {
                            self.add_script(Some((name, text)));
                            ui.close();
                        }
                    }
                });
            } else if add.clicked() {
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
            MACROS => {
                draft.keys.push(KeyBinding {
                    enabled: true,
                    ..KeyBinding::default()
                });
                draft.keys.len() - 1
            }
            GAUGES => {
                draft.gauges.push(Gauge {
                    enabled: true,
                    segments: true,
                    low: 30,
                    ..Gauge::default()
                });
                draft.gauges.len() - 1
            }
            _ => return self.add_script(None),
        };
        self.rules_selected[self.rules_section] = index;
        self.rules_filter.clear();
        self.rules_sample.clear();
    }

    /// Makes an empty script file for the profile and selects it.
    pub(super) fn add_script(&mut self, example: Option<(&str, &str)>) {
        self.flush_script();
        let (stem, text) = example.unwrap_or(("script", ""));
        let (Some(profiles), Some(profile)) = (&self.profiles, &self.profile) else {
            return self.note(tr(
                "Scripts of your own are kept per profile: save or choose a profile first.",
            ));
        };
        let dir = profiles.scripts_dir(profile);
        let free = (1..).map(|n| {
            dir.join(if n == 1 {
                format!("{stem}.luau")
            } else {
                format!("{stem}-{n}.luau")
            })
        });
        let path = free.take(500).find(|path| !path.exists());
        let made = path
            .ok_or_else(|| "too many scripts".to_owned())
            .and_then(|path| {
                std::fs::create_dir_all(&dir)
                    .and_then(|()| std::fs::write(&path, text))
                    .map(|()| path)
                    .map_err(|error| error.to_string())
            });
        match made {
            Ok(path) => {
                self.list_scripts();
                let index = self.script_files.iter().position(|file| *file == path);
                self.rules_selected[SCRIPTS] = index.unwrap_or(0);
                self.rules_filter.clear();
                if !text.is_empty() {
                    self.load_scripts();
                }
            }
            Err(error) => self.note(&trf("Could not save the script: {0}", &[&error])),
        }
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
                MACROS => tr("A macro sends commands at the press of a key."),
                GAUGES => tr(
                    "A gauge is a bar that follows a number the MUD reports,\nsuch as your health.",
                ),
                _ => tr(
                    "Scripts are Luau programs: for what triggers cannot do,\nsuch as building the map from what your MUD sends.",
                ),
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
            MACROS => self.macro_detail(ui, index),
            GAUGES => self.gauge_detail(ui, index),
            _ => self.script_detail(ui, index),
        };
        if delete {
            let draft = &mut self.rules_draft;
            match section {
                TRIGGERS => drop(draft.triggers.remove(index)),
                ALIASES => drop(draft.aliases.remove(index)),
                MACROS => drop(draft.keys.remove(index)),
                GAUGES => {
                    // Take the bar off the screen too.
                    let gone = draft.gauges.remove(index);
                    let name = if gone.name.trim().is_empty() {
                        gone.value
                    } else {
                        gone.name
                    };
                    self.gauges.set(name.trim(), None);
                }
                _ => {
                    let path = self.script_files[index].clone();
                    self.script_loaded = None;
                    self.script_changed_at = None;
                    if let Err(error) = std::fs::remove_file(&path) {
                        self.note(&trf(
                            "Could not delete the script: {0}",
                            &[&error.to_string()],
                        ));
                    }
                    self.list_scripts();
                    self.load_scripts();
                }
            }
            self.rules_selected[section] = index.saturating_sub(1);
            self.recording_key = false;
        }
    }

    fn trigger_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let height = self.detail_height;
        let rule = &mut self.rules_draft.triggers[index];
        let delete = rule_header(
            ui,
            &mut rule.name,
            &mut rule.enabled,
            tr("Name this trigger"),
        );
        let (left, right) = detail_columns(ui);
        let mut first = column(ui, left, "first");
        {
            let ui = &mut first;
            widgets::caption(ui, tr("When"));
            let label = if rule.regex {
                tr("A line matches this regular expression")
            } else {
                tr("A line contains this text")
            };
            widgets::field(ui, label, &mut rule.pattern, tr("tells you"));
            widgets::switch_row(ui, &mut rule.regex, tr("Regular expression"));
            ui.add_space(6.0);
            widgets::caption(ui, tr("Do"));
            widgets::editor(
                ui,
                tr("Send commands, one per line"),
                &mut rule.send,
                "kill %1\nloot",
                4,
            );
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
            widgets::field(
                ui,
                tr("Colour the match"),
                &mut rule.highlight,
                "yellow, #ffcc00",
            );
            widgets::switch_row(ui, &mut rule.gag, tr("Hide the line"));
        }
        let used = first.min_rect();
        let mut second = match right {
            Some(right) => column(ui, right, "second"),
            None => first,
        };
        {
            let ui = &mut second;
            if right.is_none() {
                ui.add_space(6.0);
            }
            // Beside the rest, the script takes the height there is.
            let rows = if right.is_some() {
                script_rows(height, 250.0)
            } else {
                8
            };
            script_block(
                ui,
                tr("Luau, run after the rest. m holds the match, line the whole line."),
                &mut rule.script,
                "if m[1] ~= \"you\" then\n\tsend(\"assist \" .. m[1])\nend",
                rows,
            );
            ui.add_space(6.0);
            widgets::caption(ui, tr("Try it"));
            widgets::text_box(ui, &mut self.rules_sample, tr("Paste a line from the MUD"));
            let rule = &self.rules_draft.triggers[index];
            let outcome = try_pattern(&rule.regex_source(), &self.rules_sample, |captures| {
                let mut effects = Vec::new();
                let sent: Vec<String> = crate::rules::commands(&rule.send)
                    .map(|command| fill(command, captures))
                    .collect();
                if !sent.is_empty() {
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
        }
        ui.allocate_rect(used.union(second.min_rect()), egui::Sense::hover());
        delete
    }

    fn alias_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let height = self.detail_height;
        let rule = &mut self.rules_draft.aliases[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this alias"));
        let (left, right) = detail_columns(ui);
        let mut first = column(ui, left, "first");
        {
            let ui = &mut first;
            widgets::caption(ui, tr("When"));
            let label = if rule.regex {
                tr("What you type matches this regular expression")
            } else {
                tr("You type this word, alone or followed by more")
            };
            widgets::field(ui, label, &mut rule.pattern, "k");
            widgets::switch_row(ui, &mut rule.regex, tr("Regular expression"));
            ui.add_space(6.0);
            widgets::caption(ui, tr("Do"));
            widgets::editor(
                ui,
                tr("Send instead, one command per line"),
                &mut rule.send,
                "kill %1",
                4,
            );
            note(ui, tr("%1 stands for what follows the word."));
        }
        let used = first.min_rect();
        let mut second = match right {
            Some(right) => column(ui, right, "second"),
            None => first,
        };
        {
            let ui = &mut second;
            if right.is_none() {
                ui.add_space(6.0);
            }
            let rows = if right.is_some() {
                script_rows(height, 250.0)
            } else {
                8
            };
            script_block(
                ui,
                tr("Luau, run after the rest. m holds the match, input what was typed."),
                &mut rule.script,
                "send(\"say \" .. string.upper(m[1]))",
                rows,
            );
            ui.add_space(6.0);
            widgets::caption(ui, tr("Try it"));
            widgets::text_box(ui, &mut self.rules_sample, tr("Type a command"));
            let rule = &self.rules_draft.aliases[index];
            let outcome = try_pattern(&rule.regex_source(), &self.rules_sample, |captures| {
                let sent: Vec<String> = crate::rules::commands(&rule.send)
                    .map(|command| fill(command, captures))
                    .collect();
                format!("{} {}", tr("sends"), sent.join("; "))
            });
            show_outcome(ui, outcome);
        }
        ui.allocate_rect(used.union(second.min_rect()), egui::Sense::hover());
        delete
    }

    fn macro_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let theme = theme::current();
        let height = self.detail_height;
        let recording = self.recording_key;
        let mut toggle = false;
        let rule = &mut self.rules_draft.keys[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this macro"));
        let (left, right) = detail_columns(ui);
        let mut first = column(ui, left, "first");
        {
            let ui = &mut first;
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
                    .fill(theme.field)
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
                toggle = ui.button(label).clicked();
            });
            note(
                ui,
                tr("Function keys, keys with Ctrl or Alt, and the numeric keypad can be used."),
            );
            ui.add_space(6.0);
            widgets::caption(ui, tr("Do"));
            widgets::editor(
                ui,
                tr("Send commands, one per line"),
                &mut rule.send,
                "north",
                4,
            );
        }
        let used = first.min_rect();
        let mut second = match right {
            Some(right) => column(ui, right, "second"),
            None => first,
        };
        {
            let ui = &mut second;
            if right.is_none() {
                ui.add_space(6.0);
            }
            let rows = if right.is_some() {
                script_rows(height, 150.0)
            } else {
                8
            };
            script_block(
                ui,
                tr("Luau, run when the key is pressed."),
                &mut rule.script,
                "map.walk(map.find(\"Temple\")[1].id)",
                rows,
            );
        }
        ui.allocate_rect(used.union(second.min_rect()), egui::Sense::hover());
        if toggle {
            self.recording_key = !recording;
        }
        delete
    }

    fn gauge_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let theme = theme::current();
        let rule = &mut self.rules_draft.gauges[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this gauge"));
        let (left, right) = detail_columns(ui);
        let mut first = column(ui, left, "first");
        {
            let ui = &mut first;
            widgets::caption(ui, tr("Follows"));
            widgets::field(ui, tr("Value"), &mut rule.value, "Char.Vitals.hp");
            widgets::field(ui, tr("Maximum"), &mut rule.max, "Char.Vitals.maxhp");
            note(
                ui,
                tr(
                    "Where the MUD reports it: a GMCP package and the fields inside it, such as Char.Vitals.health.current, or the name of a MSDP variable. The maximum can also be a number.",
                ),
            );
            ui.add_space(6.0);
            widgets::caption(ui, tr("Look"));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(RichText::new(tr("Colour")).color(theme.text_dim));
                ui.add_space(8.0);
                for name in crate::panels::GAUGE_COLOURS {
                    let colour = crate::panels::colour_named(name).unwrap_or(theme.accent);
                    if widgets::swatch(ui, colour, rule.colour == name, tr(name)).clicked() {
                        rule.colour = name.to_owned();
                    }
                }
            });
            widgets::switch_row(ui, &mut rule.segments, tr("Draw it as cells"));
            ui.horizontal(|ui| {
                let label = ui.label(tr("Warn when under"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::DragValue::new(&mut rule.low)
                            .range(0..=100)
                            .suffix(" %"),
                    )
                    .labelled_by(label.id);
                });
            });
        }
        let used = first.min_rect();
        let mut second = match right {
            Some(right) => column(ui, right, "second"),
            None => first,
        };
        {
            let ui = &mut second;
            if right.is_none() {
                ui.add_space(6.0);
            }
            widgets::caption(ui, tr("Preview"));
            let look = muddie_script::GaugeLook {
                colour: rule.colour.clone(),
                segments: rule.segments,
                low: f64::from(rule.low),
            };
            let name = if rule.name.trim().is_empty() {
                tr("Gauge")
            } else {
                rule.name.trim()
            };
            // As it will sit under the output: full, half and nearly empty.
            let strip = egui::Frame::new()
                .fill(theme.background)
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin::symmetric(16, 14));
            strip.show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(22.0, 12.0);
                    for value in [100.0, 64.0, 18.0] {
                        crate::panels::draw_gauge(ui, name, value, 100.0, &look);
                    }
                });
            });
        }
        ui.allocate_rect(used.union(second.min_rect()), egui::Sense::hover());
        delete
    }

    fn script_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let theme = theme::current();
        let path = self.script_files[index].clone();
        if self.script_loaded.as_ref() != Some(&path) {
            self.flush_script();
            // A file that is not there yet is an empty script.
            self.script_text = std::fs::read_to_string(&path).unwrap_or_default();
            self.script_saved = self.script_text.clone();
            self.script_seen = self.script_text.clone();
            self.script_name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            self.script_loaded = Some(path.clone());
        }
        let own = path.parent().is_some_and(|dir| dir.ends_with("scripts"));
        let mut delete = false;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if own {
                    let label = RichText::new(tr("Delete")).color(theme.danger);
                    delete = ui
                        .add(egui::Button::new(label).fill(egui::Color32::TRANSPARENT))
                        .clicked();
                }
                let title = TextEdit::singleline(&mut self.script_name)
                    .font(theme::font(Weight::Semibold, 20.0))
                    .frame(egui::Frame::NONE)
                    .interactive(own)
                    .desired_width(f32::INFINITY);
                let field = ui.add(title);
                ui.ctx().accesskit_node_builder(field.id, |node| {
                    node.set_label(tr("Name"));
                });
            });
        });
        note(ui, &path.display().to_string());
        // The editor is the whole of the view.
        let rows = script_rows(self.detail_height, 96.0);
        let hint = "-- Luau\non_gmcp(\"Room.Info\", function(room)\n\tmap.visit({id = room.num, name = room.name, exits = room.exits})\nend)";
        widgets::code_editor(ui, tr("Script"), &mut self.script_text, hint, rows);
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

/// The regions of the details: one column, or two side by side when there
/// is room, so that a rule's script can stand beside the rest of it.
fn detail_columns(ui: &egui::Ui) -> (egui::Rect, Option<egui::Rect>) {
    let full = ui.available_rect_before_wrap();
    if full.width() < 820.0 {
        return (full, None);
    }
    let (mut left, mut right) = (full, full);
    left.set_width((full.width() * 0.4).clamp(360.0, 520.0));
    right.min.x = left.max.x + 32.0;
    (left, Some(right))
}

/// A column of the details, laid out from the top of `rect`.
fn column(ui: &mut egui::Ui, rect: egui::Rect, name: &str) -> egui::Ui {
    // Named, so that what is in one column is not taken for what is
    // in the other.
    let builder = egui::UiBuilder::new()
        .id_salt(name)
        .max_rect(rect)
        .layout(egui::Layout::top_down(egui::Align::Min));
    let mut column = ui.new_child(builder);
    column.spacing_mut().item_spacing.y = 10.0;
    column
}

/// How many rows of code fit in a view `height` tall once `taken` of it
/// has gone to what surrounds the editor.
fn script_rows(height: f32, taken: f32) -> usize {
    (((height - taken) / 17.0) as usize).max(8)
}

/// A line of small print under a control.
fn note(ui: &mut egui::Ui, text: &str) {
    let small = RichText::new(text)
        .font(theme::font(Weight::Regular, 11.5))
        .color(theme::current().text_faint);
    ui.label(small);
}

/// A rule's script: its title, what it is given, and the editor.
fn script_block(ui: &mut egui::Ui, about: &str, script: &mut String, hint: &str, rows: usize) {
    widgets::caption(ui, tr("Script"));
    widgets::code_editor(ui, tr("Script"), script, hint, rows);
    note(ui, about);
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
