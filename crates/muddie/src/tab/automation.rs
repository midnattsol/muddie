//! The automation window: triggers, aliases and macros made without a
//! script, in a list on the left with the selected one's details beside it.

use super::*;
use crate::rules::{Timer, fill};
use crate::theme::Weight;
use crate::widgets;

/// How long edits must stop before they are saved and put to work.
const SAVE_AFTER: Duration = Duration::from_millis(900);

/// The sections of the window, in the order of the sidebar.
const TRIGGERS: usize = 0;
const ALIASES: usize = 1;
const MACROS: usize = 2;
const TIMERS: usize = 3;
pub(super) const GAUGES: usize = 4;
const VARIABLES: usize = 5;
pub(super) const SCRIPTS: usize = 6;

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

    /// Chooses one of the rules of a section.
    pub(super) fn select_rule(&mut self, section: usize, index: usize) {
        self.rules_selected[section] = index;
    }

    /// Opens the automation view where it was left.
    pub fn reopen_automation(&mut self) {
        self.open_automation(self.rules_section);
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
            const SECTIONS: f32 = 208.0;
            const AIR: f32 = 22.0;
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
        self.rules_autosave(ctx, open);
    }

    /// Saves what is being edited once the edits stop for a moment, and
    /// at once when what they are made in closes.
    pub(super) fn rules_autosave(&mut self, ctx: &egui::Context, open: bool) {
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
        ui.spacing_mut().item_spacing.y = 4.0;
        let heading = RichText::new(tr("Editor"))
            .font(theme::font(Weight::Semibold, 17.0))
            .color(theme::current().text);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.add(Label::new(heading).selectable(false));
        });
        ui.add_space(16.0);
        let counts = [
            self.rules_draft.triggers.len(),
            self.rules_draft.aliases.len(),
            self.rules_draft.keys.len(),
            self.rules_draft.timers.len(),
            self.rules_draft.gauges.len(),
            self.scripts.variables().len(),
            self.script_files.len(),
        ];
        let names = [
            tr("Triggers"),
            tr("Aliases"),
            tr("Macros"),
            tr("Timers"),
            tr("Gauges"),
            tr("Variables"),
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
        self.sidebar_groups(ui);
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            let again = widgets::quiet_button(ui, tr("Run everything again"));
            if again
                .on_hover_text(tr(
                    "Loads the scripts and the rules afresh, as when connecting.",
                ))
                .clicked()
            {
                self.flush_script();
                self.load_scripts();
            }
        });
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

    /// The groups under the sections: each with the switch of how it
    /// starts a session. Choosing one narrows the lists to it.
    fn sidebar_groups(&mut self, ui: &mut egui::Ui) {
        let names = self.rules_draft.group_names();
        if names.is_empty() {
            self.group_filter = None;
            return;
        }
        let theme = theme::current();
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            widgets::caption(ui, tr("Groups"));
        });
        ui.add_space(2.0);
        if self
            .group_filter
            .as_ref()
            .is_some_and(|chosen| !names.contains(chosen))
        {
            self.group_filter = None;
        }
        for name in names {
            let chosen = self.group_filter.as_ref() == Some(&name);
            let mut on = self.rules_draft.group_enabled(&name);
            let (rect, row) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
            row.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, name.as_str())
            });
            if chosen {
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(7), theme.raised);
            } else if row.hovered() {
                ui.painter().rect_filled(
                    rect,
                    egui::CornerRadius::same(7),
                    theme.raised.gamma_multiply(0.6),
                );
            }
            let colour = if on { theme.text_dim } else { theme.text_faint };
            let text = ui
                .painter()
                .with_clip_rect(rect.with_max_x(rect.right() - 48.0));
            text.text(
                rect.left_center() + egui::vec2(12.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &name,
                theme::font(Weight::Regular, 13.5),
                if chosen { theme.text } else { colour },
            );
            if row.clicked() {
                self.group_filter = (!chosen).then(|| name.clone());
            }
            // The switch, over the right end of the row.
            let place = egui::Rect::from_center_size(
                rect.right_center() - egui::vec2(27.0, 0.0),
                egui::vec2(34.0, 20.0),
            );
            let mut slot = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("group switch", name.as_str()))
                    .max_rect(place),
            );
            if widgets::switch(&mut slot, &mut on, &name).changed() {
                self.rules_draft.set_group_enabled(&name, on);
            }
        }
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

    /// The group of each rule of the current section, in their order.
    fn row_groups(&self) -> Vec<String> {
        let draft = &self.rules_draft;
        let groups: Vec<&String> = match self.rules_section {
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
        {
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
        }
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
                    let on = name.is_empty() || self.rules_draft.group_enabled(&name);
                    if !name.is_empty() {
                        let folded = self.folded_groups.contains(&name);
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
                                self.folded_groups.retain(|known| *known != name);
                            } else {
                                self.folded_groups.push(name.clone());
                            }
                        }
                        if folded {
                            continue;
                        }
                    }
                    for index in members {
                        let (title, detail, enabled) = &rows[index];
                        let selected = self.rules_selected[section] == index;
                        let row = widgets::list_row(ui, selected, title, detail, *enabled && on);
                        if row.clicked() {
                            self.rules_selected[section] = index;
                            self.rules_sample.clear();
                            self.recording_key = false;
                        }
                        // The gauges are shown in the order of this list,
                        // so its rows can be dragged to another place.
                        if section == GAUGES && filter.is_empty() {
                            places.push((index, row.rect));
                            let row = row.interact(egui::Sense::drag());
                            if row.drag_started() {
                                self.gauge_dragged = Some(index);
                            }
                            if self.gauge_dragged == Some(index) {
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

    /// While a gauge is dragged in the list, marks where it would go, and
    /// puts it there once let go.
    fn drag_gauge(&mut self, ui: &egui::Ui, places: &[(usize, egui::Rect)]) {
        let Some(from) = self.gauge_dragged else {
            return;
        };
        let pointer = ui.ctx().pointer_latest_pos();
        // Before the first row whose middle is under the pointer, or last.
        let before = pointer
            .and_then(|pointer| (places.iter()).find(|(_, rect)| pointer.y < rect.center().y));
        let to = before.map_or(self.rules_draft.gauges.len(), |(index, _)| *index);
        let y = match (before, places.last()) {
            (Some((_, rect)), _) => rect.top(),
            (None, Some((_, rect))) => rect.bottom(),
            (None, None) => return self.gauge_dragged = None,
        };
        let across = places[0].1.x_range();
        ui.painter()
            .hline(across, y, egui::Stroke::new(2.0, theme::current().accent));
        if !ui.input(|input| input.pointer.any_down()) {
            self.gauge_dragged = None;
            self.move_gauge(from, to);
        }
    }

    /// Moves a gauge to before the one now at `to`, keeping it chosen.
    pub(super) fn move_gauge(&mut self, from: usize, to: usize) {
        let gauges = &mut self.rules_draft.gauges;
        if from >= gauges.len() || to == from || to == from + 1 {
            return;
        }
        let gauge = gauges.remove(from);
        let at = if to > from { to - 1 } else { to }.min(gauges.len());
        gauges.insert(at, gauge);
        self.rules_selected[GAUGES] = at;
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
            TIMERS => self.timer_detail(ui, index),
            GAUGES => self.gauge_detail(ui, index),
            VARIABLES => self.variable_detail(ui, index),
            _ => self.script_detail(ui, index),
        };
        if delete {
            let draft = &mut self.rules_draft;
            match section {
                TRIGGERS => drop(draft.triggers.remove(index)),
                ALIASES => drop(draft.aliases.remove(index)),
                MACROS => drop(draft.keys.remove(index)),
                TIMERS => drop(draft.timers.remove(index)),
                GAUGES => {
                    // Take it off the screen too.
                    let gone = draft.gauges.remove(index);
                    self.gauges.set(gone.shown_name(), None);
                }
                VARIABLES => {
                    if let Some((name, _)) = self.scripts.variables().get(index) {
                        let forgotten = self.scripts.set_variable(name, None);
                        if let Err(error) = forgotten {
                            self.note(&error);
                        }
                    }
                    self.variable_loaded = None;
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
        let text_panels: Vec<String> = (self.layout.panels.iter())
            .filter(|panel| panel.kind == super::layout::Kind::Text)
            .map(|panel| panel.name.clone())
            .collect();
        let groups = self.rules_draft.group_names();
        let height = self.detail_height;
        let rule = &mut self.rules_draft.triggers[index];
        let delete = rule_header(
            ui,
            &mut rule.name,
            &mut rule.enabled,
            tr("Name this trigger"),
        );
        group_row(ui, &mut rule.group, &groups);
        widgets::caption(ui, tr("When"));
        let label = if rule.regex {
            tr("A line matches this regular expression")
        } else {
            tr("A line contains this text")
        };
        widgets::field(ui, label, &mut rule.pattern, tr("tells you"));
        widgets::switch_row(ui, &mut rule.regex, tr("Regular expression"));
        ui.add_space(6.0);
        let mistake = if rule.lua {
            self.checks.of(&self.scripts, &rule.script)
        } else {
            None
        };
        action_block(
            ui,
            Action {
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
        widgets::caption(ui, tr("Also"));
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
        ui.horizontal(|ui| {
            let name = ui
                .label(RichText::new(tr("Show it in the panel")).color(theme::current().text_dim));
            let shown = if rule.panel.trim().is_empty() {
                tr("None").to_owned()
            } else {
                rule.panel.clone()
            };
            crate::widgets::dropdown("trigger panel")
                .selected_text(shown)
                .width(200.0)
                .show_ui(ui, |ui| {
                    if crate::widgets::choice(ui, rule.panel.trim().is_empty(), tr("None"))
                        .clicked()
                    {
                        rule.panel.clear();
                    }
                    for panel in &text_panels {
                        if crate::widgets::choice(ui, rule.panel == *panel, panel).clicked() {
                            rule.panel.clone_from(panel);
                        }
                    }
                })
                .response
                .labelled_by(name.id);
        });
        note(
            ui,
            tr(
                "With the line hidden, it is only in the panel. Panels are made in Preferences, Edit the layout.",
            ),
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
        delete
    }

    fn alias_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let groups = self.rules_draft.group_names();
        let height = self.detail_height;
        let rule = &mut self.rules_draft.aliases[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this alias"));
        group_row(ui, &mut rule.group, &groups);
        widgets::caption(ui, tr("When"));
        let label = if rule.regex {
            tr("What you type matches this regular expression")
        } else {
            tr("You type this word, alone or followed by more")
        };
        widgets::field(ui, label, &mut rule.pattern, "k");
        widgets::switch_row(ui, &mut rule.regex, tr("Regular expression"));
        ui.add_space(6.0);
        let mistake = if rule.lua {
            self.checks.of(&self.scripts, &rule.script)
        } else {
            None
        };
        action_block(
            ui,
            Action {
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
        widgets::text_box(ui, &mut self.rules_sample, tr("Type a command"));
        let rule = &self.rules_draft.aliases[index];
        let outcome = try_pattern(&rule.regex_source(), &self.rules_sample, |captures| {
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

    fn macro_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let groups = self.rules_draft.group_names();
        let theme = theme::current();
        let height = self.detail_height;
        let recording = self.recording_key;
        let mut toggle = false;
        let rule = &mut self.rules_draft.keys[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this macro"));
        group_row(ui, &mut rule.group, &groups);
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
                .fill(theme.surface)
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
            toggle = crate::widgets::button(ui, label).clicked();
        });
        note(
            ui,
            tr("Function keys, keys with Ctrl or Alt, and the numeric keypad can be used."),
        );
        ui.add_space(6.0);
        let mistake = if rule.lua {
            self.checks.of(&self.scripts, &rule.script)
        } else {
            None
        };
        action_block(
            ui,
            Action {
                lua: &mut rule.lua,
                commands: &mut rule.send,
                script: &mut rule.script,
                matched: false,
                commands_hint: "north",
                script_hint: "map.walk(map.find(\"Temple\")[1].id)",
                about_commands: tr("One command per line, sent as if you had typed them."),
                about_script: tr("Run when the key is pressed."),
            },
            script_rows(height, 300.0),
            mistake.as_ref(),
        );
        if toggle {
            self.recording_key = !recording;
        }
        delete
    }

    fn timer_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let groups = self.rules_draft.group_names();
        let theme = theme::current();
        let height = self.detail_height;
        let rule = &mut self.rules_draft.timers[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this timer"));
        group_row(ui, &mut rule.group, &groups);
        widgets::caption(ui, tr("When"));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let current = usize::from(!rule.repeat);
            if let Some(chosen) = widgets::segmented(ui, &[tr("Every"), tr("Once, after")], current)
            {
                rule.repeat = chosen == 0;
            }
            let mut seconds = f64::from(rule.tenths) / 10.0;
            let field = ui.add(
                crate::widgets::number(&mut seconds)
                    .range(0.1..=86_400.0)
                    .speed(0.5)
                    .max_decimals(1),
            );
            ui.ctx().accesskit_node_builder(field.id, |node| {
                node.set_label(tr("seconds"));
            });
            rule.tenths = (seconds * 10.0).round().clamp(1.0, 864_000.0) as u32;
            ui.label(RichText::new(tr("seconds")).color(theme.text_dim));
            // Said back in minutes when that is how one thinks of it.
            if rule.tenths >= 600 {
                let said = format!("= {}", rule.interval());
                ui.label(RichText::new(said).color(theme.text_faint));
            }
        });
        let counted = if rule.repeat {
            tr("Counted from when the session starts or the rules are run again.")
        } else {
            tr("Goes off once, that long after the session starts or the rules are run again.")
        };
        ui.label(
            RichText::new(counted)
                .font(theme::font(Weight::Regular, 11.5))
                .color(theme.text_faint),
        );
        ui.add_space(6.0);
        let mistake = if rule.lua {
            self.checks.of(&self.scripts, &rule.script)
        } else {
            None
        };
        action_block(
            ui,
            Action {
                lua: &mut rule.lua,
                commands: &mut rule.send,
                script: &mut rule.script,
                matched: false,
                commands_hint: "save",
                script_hint: "if gmcp(\"Char.Vitals\").hp < 50 then\n\tsend(\"rest\")\nend",
                about_commands: tr("One command per line."),
                about_script: tr("Run each time the timer goes off."),
            },
            script_rows(height, 300.0),
            mistake.as_ref(),
        );
        delete
    }

    /// Makes a variable with a name not yet taken and selects it.
    fn add_variable(&mut self) {
        let known: Vec<String> = self
            .scripts
            .variables()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        let names = (1..).map(|number| {
            if number == 1 {
                "variable".to_owned()
            } else {
                format!("variable{number}")
            }
        });
        let Some(name) = names.take(500).find(|name| !known.contains(name)) else {
            return;
        };
        if let Err(error) = self
            .scripts
            .set_variable(&name, Some(serde_json::Value::String(String::new())))
        {
            self.note(&error);
        }
        let place = self
            .scripts
            .variables()
            .iter()
            .position(|(known, _)| *known == name);
        self.rules_selected[VARIABLES] = place.unwrap_or(0);
        self.rules_filter.clear();
    }

    /// Fills what a variable is edited through from its value.
    fn load_variable(&mut self, value: &serde_json::Value) {
        use serde_json::Value;
        self.variable_kind = kind_of(value);
        self.variable_text = match value {
            Value::String(text) => text.clone(),
            _ => String::new(),
        };
        self.variable_number = value.as_f64().unwrap_or(0.0);
        self.variable_flag = value.as_bool().unwrap_or(false);
        // The items of an array by their place, the fields of a table by
        // their key.
        self.variable_items = match value {
            Value::Array(items) => items
                .iter()
                .map(|item| (String::new(), shown_value(item)))
                .collect(),
            Value::Object(fields) => fields
                .iter()
                .map(|(key, item)| (key.clone(), shown_value(item)))
                .collect(),
            _ => Vec::new(),
        };
    }

    /// The value that what is being edited stands for.
    fn edited_variable(&self) -> serde_json::Value {
        use serde_json::Value;
        match self.variable_kind {
            NUMBER => {
                serde_json::Number::from_f64(self.variable_number).map_or(Value::Null, |number| {
                    // A whole number is kept as one, not as 3.0.
                    let whole =
                        self.variable_number.fract() == 0.0 && self.variable_number.abs() < 9e15;
                    if whole {
                        Value::from(self.variable_number as i64)
                    } else {
                        Value::Number(number)
                    }
                })
            }
            BOOLEAN => Value::Bool(self.variable_flag),
            ARRAY => Value::Array(
                self.variable_items
                    .iter()
                    .map(|(_, text)| read_value(text))
                    .collect(),
            ),
            TABLE => {
                let fields = (self.variable_items.iter())
                    .filter(|(key, _)| !key.trim().is_empty())
                    .map(|(key, text)| (key.trim().to_owned(), read_value(text)));
                Value::Object(fields.collect())
            }
            _ => Value::String(self.variable_text.clone()),
        }
    }

    fn variable_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let theme = theme::current();
        let variables = self.scripts.variables();
        let Some((name, value)) = variables.get(index).cloned() else {
            return false;
        };
        if self.variable_loaded.as_ref() != Some(&name) {
            self.variable_loaded = Some(name.clone());
            self.variable_name.clone_from(&name);
            self.load_variable(&value);
        }
        let mut delete = false;
        let mut renamed = false;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = RichText::new(tr("Delete")).color(theme.danger);
                delete = ui
                    .add(egui::Button::new(label).fill(egui::Color32::TRANSPARENT))
                    .clicked();
                let title = TextEdit::singleline(&mut self.variable_name)
                    .font(egui::FontId::monospace(19.0))
                    .frame(egui::Frame::NONE)
                    .desired_width(f32::INFINITY);
                let field = ui.add(title);
                ui.ctx().accesskit_node_builder(field.id, |node| {
                    node.set_label(tr("Name"));
                });
                // Renamed once its name is done being typed.
                renamed = field.lost_focus();
            });
        });
        ui.add(egui::Separator::default().spacing(2.0));
        let wanted = self.variable_name.trim().to_owned();
        let is_a_name = wanted.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_')
            && wanted
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        let taken = variables.iter().any(|(known, _)| *known == wanted);
        if wanted != name && !is_a_name {
            note(
                ui,
                tr("A name is made of letters, digits and _, and does not start with a digit."),
            );
        } else if wanted != name && taken {
            note(ui, tr("There is already a variable of that name."));
        } else if wanted != name && renamed {
            let moved = (self.scripts.set_variable(&wanted, Some(value.clone())))
                .and_then(|()| self.scripts.set_variable(&name, None));
            if let Err(error) = moved {
                self.note(&error);
            }
            self.variable_loaded = Some(wanted.clone());
            let place = self
                .scripts
                .variables()
                .iter()
                .position(|(known, _)| *known == wanted);
            self.rules_selected[VARIABLES] = place.unwrap_or(0);
            return false;
        }

        // Its type, by the name a script knows it by.
        ui.horizontal(|ui| {
            widgets::caption(ui, tr("Type"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(kind) = widgets::segmented(ui, &TYPES, self.variable_kind) {
                    // What it held is kept, as far as the new type can hold it.
                    let held = self.edited_variable();
                    self.load_variable(&converted(&held, kind));
                    self.variable_kind = kind;
                }
            });
        });
        ui.add_space(2.0);
        widgets::caption(ui, tr("Value"));
        match self.variable_kind {
            NUMBER => {
                let field = ui.add(
                    crate::widgets::number(&mut self.variable_number)
                        .speed(1.0)
                        .max_decimals(6)
                        .min_decimals(0)
                        .update_while_editing(false),
                );
                ui.ctx().accesskit_node_builder(field.id, |node| {
                    node.set_label(tr("Value"));
                });
                note(ui, tr("Drag it, or click it to type."));
            }
            BOOLEAN => {
                ui.horizontal(|ui| {
                    widgets::switch(ui, &mut self.variable_flag, tr("Value"));
                    let said = if self.variable_flag { "true" } else { "false" };
                    ui.label(
                        RichText::new(said)
                            .font(egui::FontId::monospace(13.5))
                            .color(theme.text),
                    );
                });
            }
            ARRAY | TABLE => {
                let keyed = self.variable_kind == TABLE;
                let mut removed = None;
                if self.variable_items.is_empty() {
                    note(
                        ui,
                        if keyed {
                            tr("No fields yet.")
                        } else {
                            tr("No items yet.")
                        },
                    );
                }
                widgets::group(ui, |ui| {
                    for (place, (key, text)) in self.variable_items.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.set_min_height(40.0);
                            if keyed {
                                let field = ui.add(
                                    TextEdit::singleline(key)
                                        .font(egui::FontId::monospace(13.0))
                                        .hint_text(RichText::new("key").color(theme.text_faint))
                                        .margin(egui::Margin::symmetric(9, 6))
                                        .desired_width(150.0),
                                );
                                ui.ctx().accesskit_node_builder(field.id, |node| {
                                    node.set_label(tr("Key"));
                                });
                                ui.label(
                                    RichText::new("=")
                                        .font(egui::FontId::monospace(13.0))
                                        .color(theme.text_faint),
                                );
                            } else {
                                // Counted from one, as a script counts them.
                                let number = RichText::new(format!("{:>2}", place + 1))
                                    .font(egui::FontId::monospace(12.5))
                                    .color(theme.text_faint);
                                ui.add(Label::new(number).selectable(false));
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if widgets::close_button(ui, tr("Remove"), 22.0).clicked() {
                                        removed = Some(place);
                                    }
                                    let field = ui.add(
                                        TextEdit::singleline(text)
                                            .font(egui::FontId::monospace(13.0))
                                            .hint_text(
                                                RichText::new("value").color(theme.text_faint),
                                            )
                                            .margin(egui::Margin::symmetric(9, 6))
                                            .desired_width(f32::INFINITY),
                                    );
                                    ui.ctx().accesskit_node_builder(field.id, |node| {
                                        node.set_label(tr("Value"));
                                    });
                                },
                            );
                        });
                    }
                });
                if let Some(place) = removed {
                    self.variable_items.remove(place);
                }
                let add = if keyed {
                    tr("Add a field")
                } else {
                    tr("Add an item")
                };
                if crate::widgets::button(ui, add).clicked() {
                    self.variable_items.push((String::new(), String::new()));
                }
                note(
                    ui,
                    tr(
                        "What reads as a number, true or false is kept as that; the rest as a string. A table inside is written as JSON.",
                    ),
                );
            }
            _ => {
                // As tall as what it holds, within reason.
                let rows = self.variable_text.lines().count().clamp(1, 10);
                let field = ui.add(
                    TextEdit::multiline(&mut self.variable_text)
                        .font(egui::FontId::monospace(13.0))
                        .margin(egui::Margin::symmetric(12, 9))
                        .desired_rows(rows)
                        .desired_width(f32::INFINITY),
                );
                ui.ctx().accesskit_node_builder(field.id, |node| {
                    node.set_label(tr("Value"));
                });
            }
        }
        // Kept as soon as it is other than what is kept.
        let after = self.edited_variable();
        if after != value
            && let Err(error) = self.scripts.set_variable(&name, Some(after))
        {
            self.note(&error);
        }
        ui.add_space(8.0);
        widgets::caption(ui, tr("Using it"));
        // As it is written, in each of the two places.
        let (in_commands, in_scripts) = match self.variable_kind {
            ARRAY => (format!("${name}.1"), format!("vars.{name}[1]")),
            TABLE => {
                let key = self
                    .variable_items
                    .first()
                    .map(|(key, _)| key.trim())
                    .filter(|key| !key.is_empty())
                    .unwrap_or("key");
                (format!("${name}.{key}"), format!("vars.{name}.{key}"))
            }
            _ => (format!("${name}"), format!("vars.{name}")),
        };
        widgets::group(ui, |ui| {
            for (place, written) in [
                (tr("In the commands of a rule"), in_commands),
                (tr("In a script"), in_scripts),
            ] {
                widgets::setting_row(ui, place, &mut |ui| {
                    let copy = crate::widgets::button(ui, tr("Copy"));
                    if copy.clicked() {
                        ui.ctx().copy_text(written.clone());
                    }
                    ui.label(
                        RichText::new(&written)
                            .font(egui::FontId::monospace(13.0))
                            .color(theme.text),
                    );
                    copy
                });
            }
        });
        // Where it is kept, which is with the session's profile or nowhere.
        let kept = match &self.profile {
            Some(profile) => trf("Kept with the profile {0}.", &[profile]),
            None => tr("This session has no saved profile, so its variables last until it is closed. Save the profile, in Session, Connection, to keep them.").to_owned(),
        };
        note(ui, &kept);
        delete
    }

    fn gauge_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        use crate::rules::{GaugeKind, Step};
        let theme = theme::current();
        // Alt and an arrow move it in the list, for those who do not drag.
        let (up, down) = ui.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp),
                input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown),
            )
        });
        if up && index > 0 {
            self.move_gauge(index, index - 1);
            return false;
        }
        if down {
            self.move_gauge(index, index + 2);
            return false;
        }
        let panels: Vec<(u32, String)> = (self.layout.panels.iter())
            .filter(|panel| panel.kind == super::layout::Kind::Gauges)
            .map(|panel| (panel.id, panel.name.clone()))
            .collect();
        let mut add_panel = false;
        ui.set_max_width(ui.available_width().min(760.0));
        let rule = &mut self.rules_draft.gauges[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this gauge"));
        // What it is drawn as.
        let kinds = [
            GaugeKind::Bar,
            GaugeKind::Cells,
            GaugeKind::Number,
            GaugeKind::Light,
            GaugeKind::Text,
        ];
        ui.horizontal(|ui| {
            widgets::caption(ui, tr("Kind"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let names = [
                    tr("Bar"),
                    tr("Cells"),
                    tr("Number"),
                    tr("Light"),
                    tr("Text"),
                ];
                let current = kinds
                    .iter()
                    .position(|kind| *kind == rule.kind)
                    .unwrap_or(0);
                if let Some(chosen) = widgets::segmented(ui, &names, current) {
                    rule.kind = kinds[chosen];
                }
            });
        });
        let about = match rule.kind {
            GaugeKind::Bar | GaugeKind::Cells => tr("How full something is, of its maximum."),
            GaugeKind::Number => tr("A number as it is: gold, level, experience."),
            GaugeKind::Light => {
                tr("On or off, for what is so or is not: hidden, poisoned, in combat.")
            }
            GaugeKind::Text => tr("What the MUD says, as words: a stance, a target, a place."),
        };
        note(ui, about);
        ui.add_space(4.0);
        widgets::caption(ui, tr("Follows"));
        if rule.kind.has_level() {
            ui.columns(2, |columns| {
                widgets::field(
                    &mut columns[0],
                    tr("Value"),
                    &mut rule.value,
                    "Char.Vitals.hp",
                );
                let none = if rule.kind == GaugeKind::Number {
                    tr("none")
                } else {
                    "100"
                };
                widgets::field(&mut columns[1], tr("Maximum"), &mut rule.max, none);
            });
        } else {
            widgets::field(ui, tr("Value"), &mut rule.value, "Char.Status.poisoned");
        }
        let hint = match rule.kind {
            GaugeKind::Light => tr(
                "It is on when the value is true, a number other than 0, or words other than no, false or off.",
            ),
            _ => tr(
                "Where the MUD reports it: a GMCP package and the fields inside it, such as Char.Vitals.health.current, or the name of a MSDP variable. The maximum can also be a number. Debug shows what there is.",
            ),
        };
        note(ui, hint);
        ui.add_space(4.0);
        if rule.kind == GaugeKind::Light {
            widgets::caption(ui, tr("Colour when on"));
            swatches(ui, &mut rule.colour);
            ui.add_space(4.0);
            widgets::caption(ui, tr("Colour when off"));
            ui.horizontal(|ui| {
                swatches(ui, &mut rule.off_colour);
                let unlit = rule.off_colour.trim().is_empty();
                if widgets::swatch_none(ui, unlit, tr("Unlit")).clicked() {
                    rule.off_colour.clear();
                }
            });
            note(
                ui,
                tr(
                    "With a colour for each, it says which of the two it is: red for poisoned, green for not.",
                ),
            );
        } else {
            widgets::caption(ui, tr("Colour"));
            swatches(ui, &mut rule.colour);
        }
        if rule.kind.has_level() {
            ui.add_space(4.0);
            widgets::caption(ui, tr("Colour by level"));
            if rule.steps.is_empty() {
                note(
                    ui,
                    tr(
                        "It can turn another colour as it gets low: yellow under 60 %, red under 30 %.",
                    ),
                );
            }
            let mut removed = None;
            for (place, step) in rule.steps.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    let label = ui.label(RichText::new(tr("Under")).color(theme.text_dim));
                    ui.add(
                        crate::widgets::number(&mut step.under)
                            .range(1..=100)
                            .suffix(" %"),
                    )
                    .labelled_by(label.id);
                    ui.add_space(6.0);
                    ui.push_id(place, |ui| swatches(ui, &mut step.colour));
                    if widgets::close_button(ui, tr("Remove"), 22.0).clicked() {
                        removed = Some(place);
                    }
                });
            }
            if let Some(place) = removed {
                rule.steps.remove(place);
            }
            // Kept in order, highest first, once nothing is being changed.
            let busy = ui.input(|input| input.pointer.any_down())
                || ui.memory(|memory| memory.focused().is_some());
            if !busy {
                rule.steps.sort_by_key(|step| std::cmp::Reverse(step.under));
            }
            if crate::widgets::button(ui, tr("Add a level")).clicked() {
                // Under the lowest there is, or a first one at a third.
                let lowest = rule.steps.iter().map(|step| step.under).min();
                let (under, colour) = match lowest {
                    None => (60, "yellow"),
                    Some(lowest) => (lowest / 2, "red"),
                };
                rule.steps.push(Step {
                    under: under.max(1),
                    colour: colour.to_owned(),
                });
            }
        }
        // Which panel of gauges it is shown in.
        ui.add_space(4.0);
        widgets::caption(ui, tr("Shown in"));
        let order = tr(
            "In the order of the list: drag a gauge there, or press Alt+Up or Alt+Down, to move it.",
        );
        if panels.is_empty() {
            ui.horizontal(|ui| {
                note(ui, tr("There is no panel of gauges to show it in."));
                add_panel = crate::widgets::button(ui, tr("Add a panel of gauges")).clicked();
            });
        } else {
            let home = panels
                .iter()
                .find(|(id, _)| *id == rule.panel)
                .unwrap_or(&panels[0]);
            crate::widgets::dropdown("gauge panel")
                .selected_text(home.1.clone())
                .width(220.0)
                .show_ui(ui, |ui| {
                    for (id, name) in &panels {
                        crate::widgets::choice_value(ui, &mut rule.panel, *id, name);
                    }
                });
            note(ui, order);
        }
        ui.add_space(4.0);
        widgets::caption(ui, tr("Preview"));
        let look = |text: &str| muddie_script::GaugeLook {
            colour: rule.colour.clone(),
            kind: match rule.kind {
                GaugeKind::Bar => "bar",
                GaugeKind::Cells => "cells",
                GaugeKind::Number => "number",
                GaugeKind::Light => "light",
                GaugeKind::Text => "text",
            }
            .to_owned(),
            steps: rule
                .steps
                .iter()
                .map(|step| (f64::from(step.under), step.colour.clone()))
                .collect(),
            text: text.to_owned(),
            off_colour: rule.off_colour.clone(),
            ..muddie_script::GaugeLook::default()
        };
        let name = if rule.name.trim().is_empty() {
            tr("Gauge")
        } else {
            rule.name.trim()
        };
        // As it will sit under the output, at the levels that matter.
        let strip = egui::Frame::new()
            .fill(theme.background)
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::symmetric(16, 14));
        strip.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(22.0, 12.0);
                match rule.kind {
                    GaugeKind::Light => {
                        crate::panels::draw_gauge(ui, name, 1.0, 1.0, &look(""));
                        crate::panels::draw_gauge(ui, name, 0.0, 1.0, &look(""));
                    }
                    GaugeKind::Text => {
                        crate::panels::draw_gauge(ui, name, 0.0, 0.0, &look(tr("standing")))
                    }
                    GaugeKind::Number if rule.max.trim().is_empty() => {
                        crate::panels::draw_gauge(ui, name, 1250.0, 0.0, &look(""));
                    }
                    _ => {
                        // Full, and just under each level it changes at.
                        let mut levels: Vec<f64> = vec![100.0];
                        let mut under: Vec<u8> = rule.steps.iter().map(|step| step.under).collect();
                        under.sort_unstable_by(|a, b| b.cmp(a));
                        under.dedup();
                        levels.extend(
                            under
                                .iter()
                                .take(3)
                                .map(|under| (f64::from(*under) - 8.0).max(1.0)),
                        );
                        if levels.len() == 1 {
                            levels.extend([64.0, 18.0]);
                        }
                        for value in levels {
                            crate::panels::draw_gauge(ui, name, value.round(), 100.0, &look(""));
                        }
                    }
                }
            });
        });
        if add_panel {
            self.add_panel(super::layout::Kind::Gauges);
        }
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
        let mistake = self.checks.of(&self.scripts, &self.script_text);
        widgets::code_editor(
            ui,
            tr("Script"),
            &mut self.script_text,
            hint,
            rows,
            mistake.as_ref(),
        );
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

/// A variable's value as it is shown to be edited: text as it is, the
/// rest as JSON writes it.
fn shown_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_default()
        }
        other => other.to_string(),
    }
}

/// What was typed as a variable's value: a number, true or false, a table
/// if it reads as one, and text otherwise.
fn read_value(typed: &str) -> serde_json::Value {
    let trimmed = typed.trim();
    let table = trimmed.starts_with(['{', '[']);
    let plain =
        matches!(trimmed, "true" | "false") || trimmed.parse::<f64>().is_ok_and(f64::is_finite);
    let read = (table || plain)
        .then(|| serde_json::from_str(trimmed).ok())
        .flatten();
    read.unwrap_or_else(|| serde_json::Value::String(typed.to_owned()))
}

/// The group a rule belongs to: typed, or picked among those there are.
fn group_row(ui: &mut egui::Ui, group: &mut String, known: &[String]) {
    let theme = theme::current();
    ui.horizontal(|ui| {
        let name = ui.label(RichText::new(tr("Group")).color(theme.text_dim));
        let field = ui.add(
            TextEdit::singleline(group)
                .hint_text(RichText::new(tr("None")).color(theme.text_faint))
                .margin(egui::Margin::symmetric(9, 5))
                .desired_width(200.0),
        );
        field.labelled_by(name.id);
        if !known.is_empty() {
            let pick = widgets::icon_button(ui, widgets::Icon::More, tr("Choose a group"), false);
            egui::Popup::menu(&pick).show(|ui| {
                widgets::menu_width(ui);
                if widgets::menu_item(ui, tr("None"), "").clicked() {
                    group.clear();
                    ui.close();
                }
                widgets::menu_separator(ui);
                for name in known {
                    if widgets::menu_item(ui, name, "").clicked() {
                        group.clone_from(name);
                        ui.close();
                    }
                }
            });
        }
    });
    ui.add_space(2.0);
}

/// The types a variable can be, by the names a script knows them by,
/// which are the same in every language.
const TYPES: [&str; 5] = ["string", "number", "boolean", "array", "table"];
const NUMBER: usize = 1;
const BOOLEAN: usize = 2;
const ARRAY: usize = 3;
const TABLE: usize = 4;

/// Which of [`TYPES`] a value is.
fn kind_of(value: &serde_json::Value) -> usize {
    use serde_json::Value;
    match value {
        Value::Number(_) => NUMBER,
        Value::Bool(_) => BOOLEAN,
        Value::Array(_) => ARRAY,
        Value::Object(_) => TABLE,
        Value::String(_) | Value::Null => 0,
    }
}

/// A value as another type, keeping of it what that type can hold.
fn converted(value: &serde_json::Value, kind: usize) -> serde_json::Value {
    use serde_json::Value;
    let truthy = |value: &Value| match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64() != Some(0.0),
        Value::String(text) => !matches!(
            text.trim().to_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        ),
        Value::Array(items) => !items.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
        Value::Null => false,
    };
    let empty =
        |value: &Value| matches!(value, Value::String(text) if text.is_empty()) || value.is_null();
    match kind {
        NUMBER => {
            let number = match value {
                Value::Number(number) => number.as_f64(),
                Value::String(text) => text.trim().parse().ok(),
                Value::Bool(flag) => Some(f64::from(u8::from(*flag))),
                _ => None,
            };
            read_value(
                &number
                    .filter(|number| number.is_finite())
                    .unwrap_or(0.0)
                    .to_string(),
            )
        }
        BOOLEAN => Value::Bool(truthy(value)),
        ARRAY => match value {
            Value::Array(_) => value.clone(),
            Value::Object(fields) => Value::Array(fields.values().cloned().collect()),
            other if empty(other) => Value::Array(Vec::new()),
            other => Value::Array(vec![other.clone()]),
        },
        TABLE => match value {
            Value::Object(_) => value.clone(),
            Value::Array(items) => {
                let numbered = items
                    .iter()
                    .enumerate()
                    .map(|(place, item)| ((place + 1).to_string(), item.clone()));
                Value::Object(numbered.collect())
            }
            other if empty(other) => Value::Object(serde_json::Map::new()),
            other => Value::Object([("value".to_owned(), other.clone())].into_iter().collect()),
        },
        _ => Value::String(shown_value(value)),
    }
}

/// The colours to choose among, as a row of swatches.
fn swatches(ui: &mut egui::Ui, colour: &mut String) {
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

/// What checking a script said, kept for the scripts last looked at, so
/// that one being shown is not compiled again at every frame.
#[derive(Default)]
pub(super) struct Checks(Vec<(u64, Option<(usize, String)>)>);

impl Checks {
    /// The mistake in `code`, if it has one.
    pub fn of(&mut self, engine: &Engine, code: &str) -> Option<(usize, String)> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        code.hash(&mut hasher);
        let key = hasher.finish();
        if let Some((_, known)) = self.0.iter().find(|(known, _)| *known == key) {
            return known.clone();
        }
        let found = engine.check(code);
        if self.0.len() >= 8 {
            self.0.remove(0);
        }
        self.0.push((key, found.clone()));
        found
    }
}

/// What a rule does: either the commands it sends or the script it runs.
struct Action<'a> {
    lua: &'a mut bool,
    commands: &'a mut String,
    script: &'a mut String,
    /// The rule matches something, so there are groups to refer to.
    matched: bool,
    commands_hint: &'a str,
    script_hint: &'a str,
    about_commands: &'a str,
    about_script: &'a str,
}

/// The part of a rule that says what it does, the same for triggers,
/// aliases and macros: a choice between commands and a Lua script, and
/// the editor of the one chosen, `rows` lines tall.
fn action_block(
    ui: &mut egui::Ui,
    action: Action<'_>,
    rows: usize,
    mistake: Option<&(usize, String)>,
) {
    ui.horizontal(|ui| {
        widgets::caption(ui, tr("Do"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let current = usize::from(*action.lua);
            match widgets::segmented(ui, &[tr("Commands"), tr("Lua")], current) {
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
        widgets::code_editor(
            ui,
            tr("Lua"),
            action.script,
            action.script_hint,
            rows,
            mistake,
        );
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
    fn values_are_read_as_what_they_look_like() {
        use serde_json::json;
        assert_eq!(read_value(" 42 "), json!(42));
        assert_eq!(read_value("3.5"), json!(3.5));
        assert_eq!(read_value("true"), json!(true));
        assert_eq!(read_value("{\"hp\": 10}"), json!({"hp": 10}));
        assert_eq!(read_value("[1, 2]"), json!([1, 2]));
        // What only looks like one of those is text, kept as typed.
        assert_eq!(read_value("{broken"), json!("{broken"));
        assert_eq!(read_value("orc chieftain "), json!("orc chieftain "));
        assert_eq!(read_value("NaN"), json!("NaN"));
        assert_eq!(read_value(""), json!(""));
        // And what is shown reads back as what it was.
        for value in [json!(42), json!(true), json!("orc"), json!({"a": [1, 2]})] {
            assert_eq!(read_value(&shown_value(&value)), value);
        }
    }

    #[test]
    fn a_variable_changes_type_keeping_what_the_new_one_can_hold() {
        use serde_json::json;
        let to = |value: serde_json::Value, kind: usize| converted(&value, kind);
        assert_eq!(to(json!("12"), NUMBER), json!(12));
        assert_eq!(to(json!("orc"), NUMBER), json!(0));
        assert_eq!(to(json!(true), NUMBER), json!(1));
        assert_eq!(to(json!(7.5), 0), json!("7.5"));
        assert_eq!(to(json!("no"), BOOLEAN), json!(false));
        assert_eq!(to(json!("yes"), BOOLEAN), json!(true));
        assert_eq!(to(json!(0), BOOLEAN), json!(false));
        assert_eq!(to(json!("orc"), ARRAY), json!(["orc"]));
        assert_eq!(to(json!(""), ARRAY), json!([]));
        assert_eq!(to(json!({"a": 1, "b": 2}), ARRAY), json!([1, 2]));
        assert_eq!(to(json!(["axe", 3]), TABLE), json!({"1": "axe", "2": 3}));
        assert_eq!(to(json!(5), TABLE), json!({"value": 5}));
        // Each is the type it was made into, and stays so made again.
        for kind in 0..TYPES.len() {
            let made = to(json!("12"), kind);
            assert_eq!(kind_of(&made), kind);
            assert_eq!(to(made.clone(), kind), made);
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
