//! The list of a section of the editor: its folders and what is in them,
//! with the way to search it, narrow it to a tag, and add to it.

use super::*;
use crate::rules::RuleList;

/// What the list shows of one thing of a section.
pub(in crate::tab) struct Row {
    pub(in crate::tab) title: String,
    pub(in crate::tab) detail: String,
    /// Its own switch.
    pub(in crate::tab) on: bool,
    /// The folder it is in; empty for none.
    pub(in crate::tab) folder: String,
    pub(in crate::tab) tags: Vec<String>,
    /// It failed when last run.
    pub(in crate::tab) problem: bool,
}

/// Where a row or a folder was drawn, to find what the pointer is over.
pub(in crate::tab) enum Spot {
    Row(usize),
    Folder(String),
}

impl Tab {
    /// What the list shows of each thing of the current section, in order.
    pub(in crate::tab) fn automation_rows(&self) -> Vec<Row> {
        let draft = &self.editor.draft;
        match self.editor.section {
            TRIGGERS => rule_rows(&draft.triggers, |rule| rule.summary()),
            ALIASES => rule_rows(&draft.aliases, crate::rules::Rule::does),
            MACROS => rule_rows(&draft.macros, |rule| {
                format!("{}  {}", rule.when.key.trim(), rule.does())
            }),
            TIMERS => rule_rows(&draft.timers, |rule| {
                let how = if rule.when.repeat {
                    tr("every")
                } else {
                    tr("after")
                };
                format!("{how} {}  {}", rule.when.interval(), rule.does())
            }),
            GAUGES => draft.gauges.iter().map(gauge_row).collect(),
            VARIABLES => (self.scripts.variables().into_iter())
                .map(variable_row)
                .collect(),
            _ => (self.editor.script_files.iter())
                .map(|file| self.script_row(file))
                .collect(),
        }
    }

    /// A script: its file's name, where it runs, and how it is kept.
    fn script_row(&self, file: &std::path::Path) -> Row {
        let place = self.script_place(file);
        let kept = self.editor.draft.script(&place);
        let detail = match self.is_own_script(file) {
            true => tr("In this profile"),
            false => tr("In every session"),
        };
        Row {
            title: title_of(file),
            detail: detail.to_owned(),
            on: kept.enabled,
            folder: place
                .rsplit_once('/')
                .map_or("", |(folder, _)| folder)
                .to_owned(),
            tags: kept.tags,
            problem: self.editor.failed.contains(&title_of(file)),
        }
    }

    /// Whether a row is shown, by what is searched for and the tag chosen.
    fn wanted(&self, row: &Row) -> bool {
        let filter = self.editor.filter.trim().to_lowercase();
        let found = filter.is_empty()
            || row.title.to_lowercase().contains(&filter)
            || row.detail.to_lowercase().contains(&filter)
            || row
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&filter));
        let tagged = (self.tag_filter())
            .is_none_or(|wanted| row.tags.iter().any(|tag| tag_key(tag) == tag_key(wanted)));
        found && tagged
    }

    /// The tag the list is narrowed to, where things carry tags.
    fn tag_filter(&self) -> Option<&str> {
        let tagged = section_kind(self.editor.section).is_some();
        self.editor.tag_filter.as_deref().filter(|_| tagged)
    }

    /// Whether the list is narrowed, so that what is in it cannot be put
    /// in order by dragging.
    fn narrowed(&self) -> bool {
        !self.editor.filter.trim().is_empty() || self.tag_filter().is_some()
    }

    /// The list of the section chosen: the way to add to it and to search
    /// it, then its folders and what is in them.
    pub(in crate::tab) fn automation_list(&mut self, ui: &mut egui::Ui) {
        if self.editor.section == DATA {
            return self.data_list(ui);
        }
        self.list_toolbar(ui);
        ui.add_space(4.0);
        let rows = self.automation_rows();
        let kind = section_kind(self.editor.section);
        let folders = kind
            .map(|kind| self.section_folders(kind))
            .unwrap_or_default();
        self.moved_by_keys(ui, &rows);
        self.choosing_keys(ui);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if rows.is_empty() && folders.is_empty() {
                    ui.add_space(24.0);
                    let hint = RichText::new(tr("Nothing here yet.\nPress + to make one."))
                        .color(theme::current().text_faint);
                    ui.vertical_centered(|ui| ui.label(hint));
                }
                let mut spots = Vec::new();
                let tree = Tree {
                    kind,
                    rows: &rows,
                    folders: &folders,
                };
                self.branch(ui, &tree, "", true, &mut spots);
                self.editor.visible = (spots.iter())
                    .filter_map(|(spot, _)| match spot {
                        Spot::Row(index) => Some(*index),
                        Spot::Folder(_) => None,
                    })
                    .collect();
                // What is under the last row takes a drop at the top level.
                let rest = ui.available_rect_before_wrap();
                ui.allocate_rect(rest, egui::Sense::hover());
                self.drag_in_list(ui, &spots, &rows);
            });
    }

    /// The folders in `folder`, each with what is in it unless folded, then
    /// what is in `folder` itself. `on` is whether the folders around it
    /// are switched on.
    fn branch(
        &mut self,
        ui: &mut egui::Ui,
        tree: &Tree<'_>,
        folder: &str,
        on: bool,
        spots: &mut Vec<(Spot, egui::Rect)>,
    ) {
        let depth = if folder.is_empty() {
            0
        } else {
            folder.matches('/').count() + 1
        };
        if let Some(kind) = tree.kind {
            for path in (tree.folders.iter()).filter(|path| parent_of(path) == folder) {
                let count = (tree.rows.iter())
                    .filter(|row| {
                        row.folder == *path || row.folder.starts_with(&format!("{path}/"))
                    })
                    .filter(|row| self.wanted(row))
                    .count();
                if count == 0 && self.narrowed() {
                    continue;
                }
                let folder_on = self.editor.draft.folder_on(kind, path);
                let open = !self.editor.folded.contains(&(kind, path.clone())) || self.narrowed();
                let rect = self.folder_line(ui, kind, path, depth, (open, on && folder_on), count);
                spots.push((Spot::Folder(path.clone()), rect));
                if open {
                    self.branch(ui, tree, path, on && folder_on, spots);
                }
            }
        }
        for (index, row) in tree.rows.iter().enumerate() {
            if row.folder == folder && self.wanted(row) {
                let tags_on = row.tags.iter().all(|tag| self.editor.draft.tag_on(tag));
                let rect = self.list_line(ui, row, index, depth, on && row.on && tags_on);
                spots.push((Spot::Row(index), rect));
            }
        }
    }

    /// A folder's line: opened and closed by a click, switched by its
    /// switch, named again from its menu or by a double click, and dragged
    /// into another. Returns where it is.
    fn folder_line(
        &mut self,
        ui: &mut egui::Ui,
        kind: Kind,
        path: &str,
        depth: usize,
        (open, active): (bool, bool),
        count: usize,
    ) -> egui::Rect {
        let name = path.rsplit('/').next().unwrap_or(path);
        let renaming =
            (self.editor.renaming.as_ref()).is_some_and(|renaming| renaming.path == path);
        let rect = match renaming {
            true => self.folder_name_field(ui, kind, depth, (open, active, count)),
            false => self.folder_handles(ui, kind, path, depth, (open, active, count)),
        };
        // The switch, over the right end of the line, named or not.
        let place = egui::Rect::from_center_size(
            rect.right_center() - egui::vec2(25.0, 0.0),
            egui::vec2(34.0, 20.0),
        );
        let mut slot = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("folder switch", kind, path))
                .max_rect(place),
        );
        let mut on = self.editor.draft.folder_on(kind, path);
        if widgets::switch(&mut slot, &mut on, name).changed() {
            self.editor.draft.set_folder_on(kind, path, on);
        }
        rect
    }

    /// A folder's line as it usually is: opened and closed by a click,
    /// named again by a double click or from its menu, and dragged into
    /// another. Returns where it is.
    fn folder_handles(
        &mut self,
        ui: &mut egui::Ui,
        kind: Kind,
        path: &str,
        depth: usize,
        (open, active, count): (bool, bool, usize),
    ) -> egui::Rect {
        let name = path.rsplit('/').next().unwrap_or(path);
        let line = widgets::folder_row(ui, name, depth, open, active, count);
        let line = line.interact(egui::Sense::click_and_drag());
        if line.clicked() {
            let key = (kind, path.to_owned());
            match self.editor.folded.contains(&key) {
                true => self.editor.folded.retain(|folded| *folded != key),
                false => self.editor.folded.push(key),
            }
        }
        if line.double_clicked() {
            self.start_renaming(path);
        }
        if line.drag_started() && !self.narrowed() {
            self.editor.dragged = Some(Dragged::Folder(path.to_owned()));
        }
        line.context_menu(|ui| self.folder_menu(ui, kind, path));
        line.rect
    }

    /// What can be done with a folder, from its menu.
    fn folder_menu(&mut self, ui: &mut egui::Ui, kind: Kind, path: &str) {
        widgets::menu_width(ui);
        if widgets::menu_item(ui, tr("Rename"), "").clicked() {
            self.start_renaming(path);
            ui.close();
        }
        if widgets::menu_item(ui, tr("New folder inside"), "").clicked() {
            self.new_folder(kind, path);
            ui.close();
        }
        widgets::menu_separator(ui);
        if widgets::menu_item(ui, tr("Delete folder"), "").clicked() {
            self.remove_folder(kind, path);
            ui.close();
        }
    }

    /// One thing's line: chosen by a click, with Ctrl or Shift for several,
    /// dragged to another place with whatever is chosen with it, and with a
    /// menu of what can be done with what is chosen. Returns where it is.
    fn list_line(
        &mut self,
        ui: &mut egui::Ui,
        row: &Row,
        index: usize,
        depth: usize,
        active: bool,
    ) -> egui::Rect {
        let section = self.editor.section;
        let chosen = self.chosen().contains(&index);
        let line = widgets::ListRow {
            title: &row.title,
            detail: &row.detail,
            tags: &row.tags,
            selected: chosen,
            active,
            depth,
            problem: row.problem,
        }
        .show(ui);
        if line.clicked() {
            self.choose_row(index, ui.input(|input| input.modifiers));
        }
        if line.secondary_clicked() && !chosen {
            self.choose_row(index, Modifiers::NONE);
        }
        line.context_menu(|ui| self.row_menu(ui));
        let movable = section != VARIABLES && !self.narrowed();
        if movable {
            let line = line.interact(egui::Sense::drag());
            if line.drag_started() {
                self.editor.dragged = Some(Dragged::Row(index));
            }
            if line.hovered() && self.editor.dragged.is_none() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
        }
        line.rect
    }

    /// Alt and an arrow move the one chosen up or down among those in its
    /// folder, for those who do not drag.
    fn moved_by_keys(&mut self, ui: &mut egui::Ui, rows: &[Row]) {
        let section = self.editor.section;
        if !matches!(section, TRIGGERS | ALIASES | MACROS | TIMERS | GAUGES) || self.narrowed() {
            return;
        }
        let (up, down) = ui.input_mut(|input| {
            (
                input.consume_key(Modifiers::ALT, Key::ArrowUp),
                input.consume_key(Modifiers::ALT, Key::ArrowDown),
            )
        });
        let from = self.editor.selected[section];
        let Some(chosen) = rows.get(from) else {
            return;
        };
        let alike: Vec<usize> = (0..rows.len())
            .filter(|&index| rows[index].folder == chosen.folder)
            .collect();
        let at = alike.iter().position(|&index| index == from).unwrap_or(0);
        let to = match (up, down) {
            (true, _) if at > 0 => alike[at - 1],
            (_, true) if at + 1 < alike.len() => alike[at + 1] + 1,
            _ => return,
        };
        self.move_rows(&[from], to);
    }

    /// The button that adds one, which for scripts offers to start from an
    /// example, the one that makes a folder, and the field to search.
    fn list_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let add = widgets::primary_button(ui, "+");
            ui.ctx()
                .accesskit_node_builder(add.id, |node| node.set_label(tr("Add")));
            let add = add.on_hover_text(tr("Add"));
            if self.editor.section == SCRIPTS {
                crate::widgets::menu_of(&add).show(|ui| self.new_script_menu(ui));
            } else if add.clicked() {
                self.add_rule();
            }
            if let Some(kind) = section_kind(self.editor.section) {
                let icon = widgets::Icon::NewFolder;
                if widgets::icon_button(ui, icon, tr("New folder"), false).clicked() {
                    let folder = self.chosen_folder();
                    self.new_folder(kind, &folder);
                }
            }
            widgets::text_box(ui, &mut self.editor.filter, tr("Search"));
        });
    }

    /// The folder of the one chosen, where what is added goes.
    pub(in crate::tab) fn chosen_folder(&self) -> String {
        let rows = self.automation_rows();
        (rows.get(self.editor.selected[self.editor.section]))
            .map(|row| row.folder.clone())
            .unwrap_or_default()
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

    /// Adds an empty one to the current section, in the folder of the one
    /// chosen, and chooses it.
    pub(in crate::tab) fn add_rule(&mut self) {
        let section = self.editor.section;
        let folder = self.chosen_folder();
        let index = match section_kind(section).and_then(|kind| self.editor.draft.list_mut(kind)) {
            Some(list) => list.add(&folder),
            None if section == GAUGES => {
                // In the first panel of gauges there is.
                let panel = (self.layout.panels.iter())
                    .find(|panel| panel.kind == super::layout::Kind::Gauges)
                    .map_or(0, |panel| panel.id);
                let gauges = &mut self.editor.draft.gauges;
                gauges.push(crate::rules::Gauge {
                    panel,
                    ..crate::rules::Gauge::starter()
                });
                gauges.len() - 1
            }
            None if section == VARIABLES => return self.add_variable(),
            None => return self.add_script(None),
        };
        self.editor.selected[section] = index;
        self.editor.filter.clear();
        self.editor.sample.clear();
    }
}

/// A section's tree: its kind, if it has folders, what is in it, and its
/// folders.
struct Tree<'a> {
    kind: Option<Kind>,
    rows: &'a [Row],
    folders: &'a [String],
}

/// The folder a place is in; empty for none.
pub(in crate::tab) fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The rows of rules of one kind, with what each does in a line.
fn rule_rows<T: crate::rules::When>(
    rules: &Vec<crate::rules::Rule<T>>,
    detail: impl Fn(&crate::rules::Rule<T>) -> String,
) -> Vec<Row> {
    (0..rules.len())
        .map(|index| {
            let title = match RuleList::title(rules, index) {
                title if title.is_empty() => tr(T::NEW).to_owned(),
                title => title,
            };
            let rule = &rules[index];
            Row {
                title,
                detail: detail(rule),
                on: rule.enabled,
                folder: rule.folder.trim().to_owned(),
                tags: rule.tags.clone(),
                problem: false,
            }
        })
        .collect()
}

fn gauge_row(rule: &crate::rules::Gauge) -> Row {
    use crate::rules::GaugeKind;
    let kind = match rule.kind {
        GaugeKind::Bar => tr("Bar"),
        GaugeKind::Cells => tr("Cells"),
        GaugeKind::Number => tr("Number"),
        GaugeKind::Light => tr("Light"),
        GaugeKind::Text => tr("Text"),
    };
    let title = match rule.shown_name() {
        "" => tr("New gauge").to_owned(),
        name => name.to_owned(),
    };
    Row {
        title,
        detail: format!("{kind}  ·  {}", rule.value.trim()),
        on: rule.enabled,
        folder: String::new(),
        tags: Vec::new(),
        problem: false,
    }
}

fn variable_row((name, value): (String, serde_json::Value)) -> Row {
    let held = shown_value(&value).replace('\n', " ");
    Row {
        title: name,
        detail: format!("{}  ·  {held}", TYPES[kind_of(&value)]),
        on: true,
        folder: String::new(),
        tags: Vec::new(),
        problem: false,
    }
}

/// What a script is called in the list, and in what its errors say: its
/// file's name.
pub(in crate::tab) fn title_of(file: &std::path::Path) -> String {
    (file.file_name().unwrap_or_default().to_string_lossy()).into_owned()
}
