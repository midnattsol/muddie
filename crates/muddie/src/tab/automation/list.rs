//! The list of a section of the editor: its rows, by group, with the way to
//! search them and to add one.

use super::*;

impl Tab {
    /// The titles and details of the rules of the current section, and
    /// whether each is on.
    pub(in crate::tab) fn automation_rows(&self) -> Vec<(String, String, bool)> {
        let draft = &self.editor.draft;
        match self.editor.section {
            TRIGGERS => draft.triggers.iter().map(trigger_row).collect(),
            ALIASES => draft.aliases.iter().map(alias_row).collect(),
            MACROS => draft.keys.iter().map(macro_row).collect(),
            TIMERS => draft.timers.iter().map(timer_row).collect(),
            GAUGES => draft.gauges.iter().map(gauge_row).collect(),
            VARIABLES => self
                .scripts
                .variables()
                .into_iter()
                .map(variable_row)
                .collect(),
            _ => (self.editor.script_files.iter())
                .map(|file| script_row(file, &draft.scripts_off))
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

    /// The list of the section chosen: the way to add one and to search,
    /// then its rows under the names of their groups.
    pub(in crate::tab) fn automation_list(&mut self, ui: &mut egui::Ui) {
        if self.editor.section == DATA {
            return self.data_list(ui);
        }
        self.list_toolbar(ui);
        ui.add_space(4.0);
        let rows = self.automation_rows();
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
                let groups = self.row_groups();
                for name in group_order(&groups) {
                    if self
                        .editor
                        .group_filter
                        .as_ref()
                        .is_some_and(|wanted| *wanted != name)
                    {
                        continue;
                    }
                    let members = self.members(&rows, &groups, &name);
                    if members.is_empty() {
                        continue;
                    }
                    let on = name.is_empty() || self.editor.draft.group_enabled(&name);
                    if !name.is_empty() && self.group_folded(ui, &name, on, members.len()) {
                        continue;
                    }
                    for index in members {
                        self.list_row(ui, &rows[index], index, on, &mut places);
                    }
                }
                self.drag_gauge(ui, &places);
            });
    }

    /// The button that adds one, which for scripts offers to start from an
    /// example, and the field to search the list.
    fn list_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let add = widgets::primary_button(ui, "+");
            ui.ctx()
                .accesskit_node_builder(add.id, |node| node.set_label(tr("Add")));
            let add = add.on_hover_text(tr("Add"));
            if self.editor.section == SCRIPTS {
                egui::Popup::menu(&add).show(|ui| self.new_script_menu(ui));
            } else if add.clicked() {
                self.add_rule();
            }
            widgets::text_box(ui, &mut self.editor.filter, tr("Search"));
        });
    }

    /// A script can start empty or from an example.
    fn new_script_menu(&mut self, ui: &mut egui::Ui) {
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
    }

    /// The rows of a group that pass what is searched for.
    fn members(
        &self,
        rows: &[(String, String, bool)],
        groups: &[String],
        name: &str,
    ) -> Vec<usize> {
        let filter = self.editor.filter.trim().to_lowercase();
        let group_of = |index: usize| groups.get(index).map_or("", String::as_str);
        (0..rows.len())
            .filter(|&index| group_of(index) == name)
            .filter(|&index| {
                let (title, detail, _) = &rows[index];
                filter.is_empty()
                    || title.to_lowercase().contains(&filter)
                    || detail.to_lowercase().contains(&filter)
            })
            .collect()
    }

    /// A group's header, pressed to fold or unfold it. Returns whether it
    /// is folded.
    fn group_folded(&mut self, ui: &mut egui::Ui, name: &str, on: bool, count: usize) -> bool {
        let folded = self.editor.folded_groups.iter().any(|known| known == name);
        ui.add_space(8.0);
        if group_header(ui, name, on, count, folded).clicked() {
            if folded {
                self.editor.folded_groups.retain(|known| known != name);
            } else {
                self.editor.folded_groups.push(name.to_owned());
            }
        }
        folded
    }

    /// One rule's row: chosen by a click, and for a gauge, dragged to
    /// another place, as the gauges are shown in the order of this list.
    fn list_row(
        &mut self,
        ui: &mut egui::Ui,
        (title, detail, enabled): &(String, String, bool),
        index: usize,
        group_on: bool,
        places: &mut Vec<(usize, egui::Rect)>,
    ) {
        let section = self.editor.section;
        let selected = self.editor.selected[section] == index;
        let row = widgets::list_row(ui, selected, title, detail, *enabled && group_on);
        if row.clicked() {
            self.editor.selected[section] = index;
            self.editor.sample.clear();
            self.editor.recording_key = false;
        }
        if section != GAUGES || !self.editor.filter.trim().is_empty() {
            return;
        }
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

/// The groups in the order they are listed: those with no group first,
/// then the rest by name.
fn group_order(groups: &[String]) -> Vec<String> {
    let mut names: Vec<String> = groups
        .iter()
        .filter(|group| !group.is_empty())
        .cloned()
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names.insert(0, String::new());
    names
}

/// A group's header: an arrow that points down when open, its name, said
/// to be off when it is, and how many it holds.
fn group_header(
    ui: &mut egui::Ui,
    name: &str,
    on: bool,
    count: usize,
    folded: bool,
) -> egui::Response {
    let theme = theme::current();
    let (rect, header) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::click());
    header.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, !folded, name));
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
    let label = match on {
        true => name.to_uppercase(),
        false => format!("{} · {}", name.to_uppercase(), tr("off")),
    };
    let start = rect.left_center() + egui::vec2(26.0, 0.0);
    ui.painter().text(
        start,
        egui::Align2::LEFT_CENTER,
        label,
        theme::font(Weight::Medium, 10.5),
        colour,
    );
    let end = rect.right_center() - egui::vec2(12.0, 0.0);
    let small = theme::font(Weight::Regular, 11.5);
    ui.painter().text(
        end,
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        small,
        theme.text_faint,
    );
    header
}

/// A rule's title: its name, or what it is made of, or what a new one of
/// its kind is called.
fn untitled(name: &str, fallback: &str, new: &'static str) -> String {
    let shown = if name.trim().is_empty() {
        fallback
    } else {
        name
    };
    if shown.trim().is_empty() {
        tr(new).to_owned()
    } else {
        shown.trim().to_owned()
    }
}

/// What a rule sends, in a line.
fn sends(send: &str) -> String {
    crate::rules::commands(send).collect::<Vec<_>>().join("; ")
}

fn trigger_row(rule: &Trigger) -> (String, String, bool) {
    (
        untitled(&rule.name, &rule.pattern, "New trigger"),
        rule.summary(),
        rule.enabled,
    )
}

fn alias_row(rule: &Alias) -> (String, String, bool) {
    (
        untitled(&rule.name, &rule.pattern, "New alias"),
        format!("→ {}", sends(&rule.send)),
        rule.enabled,
    )
}

fn macro_row(rule: &crate::rules::KeyBinding) -> (String, String, bool) {
    let detail = format!("{}  →  {}", rule.key, sends(&rule.send));
    (
        untitled(&rule.name, &rule.key, "New macro"),
        detail,
        rule.enabled,
    )
}

fn timer_row(rule: &crate::rules::Timer) -> (String, String, bool) {
    let how = if rule.repeat {
        tr("every")
    } else {
        tr("after")
    };
    let does = if rule.lua {
        "Luau".to_owned()
    } else {
        sends(&rule.send)
    };
    let detail = format!("{how} {}  →  {does}", rule.interval());
    (
        untitled(&rule.name, &rule.send, "New timer"),
        detail,
        rule.enabled,
    )
}

fn gauge_row(rule: &crate::rules::Gauge) -> (String, String, bool) {
    use crate::rules::GaugeKind;
    let kind = match rule.kind {
        GaugeKind::Bar => tr("Bar"),
        GaugeKind::Cells => tr("Cells"),
        GaugeKind::Number => tr("Number"),
        GaugeKind::Light => tr("Light"),
        GaugeKind::Text => tr("Text"),
    };
    let detail = format!("{kind}  ·  {}", rule.value.trim());
    (
        untitled(&rule.name, &rule.value, "New gauge"),
        detail,
        rule.enabled,
    )
}

fn variable_row((name, value): (String, serde_json::Value)) -> (String, String, bool) {
    let held = shown_value(&value).replace('\n', " ");
    (name, format!("{}  ·  {held}", TYPES[kind_of(&value)]), true)
}

/// A script: its file's name, where it runs, and whether it is on.
fn script_row(file: &std::path::Path, off: &[String]) -> (String, String, bool) {
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
    (
        name,
        detail.to_owned(),
        !off.contains(&crate::rules::script_key(file)),
    )
}
