//! The list of a section of the editor: its rows, by group, with the way to
//! search them and to add one.

use super::*;

impl Tab {
    /// The titles and details of the rules of the current section.
    pub(in crate::tab) fn automation_rows(&self) -> Vec<(String, String, bool)> {
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
        let draft = &self.editor.draft;
        match self.editor.section {
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
            TIMERS => (draft.timers.iter())
                .map(|rule| {
                    let how = if rule.repeat {
                        tr("every")
                    } else {
                        tr("after")
                    };
                    let does = if rule.lua {
                        "Luau".to_owned()
                    } else {
                        crate::rules::commands(&rule.send)
                            .collect::<Vec<_>>()
                            .join("; ")
                    };
                    let detail = format!("{how} {}  →  {does}", rule.interval());
                    (
                        untitled(&rule.name, &rule.send, "New timer"),
                        detail,
                        rule.enabled,
                    )
                })
                .collect(),
            GAUGES => (draft.gauges.iter())
                .map(|rule| {
                    let kind = match rule.kind {
                        crate::rules::GaugeKind::Bar => tr("Bar"),
                        crate::rules::GaugeKind::Cells => tr("Cells"),
                        crate::rules::GaugeKind::Number => tr("Number"),
                        crate::rules::GaugeKind::Light => tr("Light"),
                        crate::rules::GaugeKind::Text => tr("Text"),
                    };
                    let detail = format!("{kind}  ·  {}", rule.value.trim());
                    (
                        untitled(&rule.name, &rule.value, "New gauge"),
                        detail,
                        rule.enabled,
                    )
                })
                .collect(),
            VARIABLES => (self.scripts.variables().into_iter())
                .map(|(name, value)| {
                    let held = shown_value(&value).replace('\n', " ");
                    (name, format!("{}  ·  {held}", TYPES[kind_of(&value)]), true)
                })
                .collect(),
            _ => (self.editor.script_files.iter())
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
                    let on = !draft.scripts_off.contains(&crate::rules::script_key(file));
                    (name, detail.to_owned(), on)
                })
                .collect(),
        }
    }

    /// The group of each rule of the current section, in their order.
    pub(in crate::tab) fn row_groups(&self) -> Vec<String> {
        let draft = &self.editor.draft;
        let groups: Vec<&String> = match self.editor.section {
            TRIGGERS => draft.triggers.iter().map(|rule| &rule.group).collect(),
            ALIASES => draft.aliases.iter().map(|rule| &rule.group).collect(),
            MACROS => draft.keys.iter().map(|rule| &rule.group).collect(),
            TIMERS => draft.timers.iter().map(|rule| &rule.group).collect(),
            _ => Vec::new(),
        };
        groups
            .into_iter()
            .map(|group| group.trim().to_owned())
            .collect()
    }

    pub(in crate::tab) fn automation_list(&mut self, ui: &mut egui::Ui) {
        if self.editor.section == DATA {
            return self.data_list(ui);
        }
        {
            ui.horizontal(|ui| {
                let add = widgets::primary_button(ui, "+");
                ui.ctx().accesskit_node_builder(add.id, |node| {
                    node.set_label(tr("Add"));
                });
                let add = add.on_hover_text(tr("Add"));
                if self.editor.section == SCRIPTS {
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
                widgets::text_box(ui, &mut self.editor.filter, tr("Search"));
            });
        }
        ui.add_space(4.0);
        let rows = self.automation_rows();
        let filter = self.editor.filter.trim().to_lowercase();
        let section = self.editor.section;
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
                // Where each gauge's row is, to drop a dragged one among them.
                let mut places: Vec<(usize, egui::Rect)> = Vec::new();
                // Under the name of their group, those that have one last.
                let groups = self.row_groups();
                let group_of = |index: usize| groups.get(index).cloned().unwrap_or_default();
                let mut names: Vec<String> = groups
                    .iter()
                    .filter(|group| !group.is_empty())
                    .cloned()
                    .collect();
                names.sort_by_key(|name| name.to_lowercase());
                names.dedup();
                names.insert(0, String::new());
                let theme = theme::current();
                for name in names {
                    if self
                        .editor
                        .group_filter
                        .as_ref()
                        .is_some_and(|wanted| *wanted != name)
                    {
                        continue;
                    }
                    let members: Vec<usize> = (0..rows.len())
                        .filter(|&index| group_of(index) == name)
                        .filter(|&index| {
                            let (title, detail, _) = &rows[index];
                            filter.is_empty()
                                || title.to_lowercase().contains(&filter)
                                || detail.to_lowercase().contains(&filter)
                        })
                        .collect();
                    if members.is_empty() {
                        continue;
                    }
                    let on = name.is_empty() || self.editor.draft.group_enabled(&name);
                    if !name.is_empty() {
                        let folded = self.editor.folded_groups.contains(&name);
                        ui.add_space(8.0);
                        let (rect, header) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 24.0),
                            egui::Sense::click(),
                        );
                        header.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::Button,
                                true,
                                !folded,
                                name.as_str(),
                            )
                        });
                        // A small arrow that points down when open.
                        let tip = rect.left_center() + egui::vec2(14.0, 0.0);
                        let arrow = if folded {
                            vec![
                                tip + egui::vec2(-2.0, -4.0),
                                tip + egui::vec2(2.0, 0.0),
                                tip + egui::vec2(-2.0, 4.0),
                            ]
                        } else {
                            vec![
                                tip + egui::vec2(-4.0, -2.0),
                                tip + egui::vec2(0.0, 2.0),
                                tip + egui::vec2(4.0, -2.0),
                            ]
                        };
                        ui.painter().add(egui::Shape::line(
                            arrow,
                            egui::Stroke::new(1.4, theme.text_faint),
                        ));
                        let colour = if on { theme.text_dim } else { theme.text_faint };
                        let label = if on {
                            name.to_uppercase()
                        } else {
                            format!("{} · {}", name.to_uppercase(), tr("off"))
                        };
                        ui.painter().text(
                            rect.left_center() + egui::vec2(26.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            label,
                            theme::font(Weight::Medium, 10.5),
                            colour,
                        );
                        ui.painter().text(
                            rect.right_center() - egui::vec2(12.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            members.len().to_string(),
                            theme::font(Weight::Regular, 11.5),
                            theme.text_faint,
                        );
                        if header.clicked() {
                            if folded {
                                self.editor.folded_groups.retain(|known| *known != name);
                            } else {
                                self.editor.folded_groups.push(name.clone());
                            }
                        }
                        if folded {
                            continue;
                        }
                    }
                    for index in members {
                        let (title, detail, enabled) = &rows[index];
                        let selected = self.editor.selected[section] == index;
                        let row = widgets::list_row(ui, selected, title, detail, *enabled && on);
                        if row.clicked() {
                            self.editor.selected[section] = index;
                            self.editor.sample.clear();
                            self.editor.recording_key = false;
                        }
                        // The gauges are shown in the order of this list,
                        // so its rows can be dragged to another place.
                        if section == GAUGES && filter.is_empty() {
                            places.push((index, row.rect));
                            let row = row.interact(egui::Sense::drag());
                            if row.drag_started() {
                                self.editor.gauge_dragged = Some(index);
                            }
                            if self.editor.gauge_dragged == Some(index) {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                            } else if row.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                            }
                        }
                    }
                }
                self.drag_gauge(ui, &places);
            });
    }

    /// Adds an empty rule to the current section and selects it.
    pub(in crate::tab) fn add_rule(&mut self) {
        let draft = &mut self.editor.draft;
        let index = match self.editor.section {
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
            TIMERS => {
                draft.timers.push(Timer::default());
                draft.timers.len() - 1
            }
            GAUGES => {
                // In the first panel of gauges there is.
                let panel = (self.layout.panels.iter())
                    .find(|panel| panel.kind == super::layout::Kind::Gauges)
                    .map_or(0, |panel| panel.id);
                draft.gauges.push(crate::rules::Gauge {
                    panel,
                    ..crate::rules::Gauge::starter()
                });
                draft.gauges.len() - 1
            }
            VARIABLES => return self.add_variable(),
            _ => return self.add_script(None),
        };
        self.editor.selected[self.editor.section] = index;
        self.editor.filter.clear();
        self.editor.sample.clear();
    }
}
