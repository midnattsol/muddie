//! The list of a section of the editor: its folders and what is in them,
//! with the way to search it, narrow it to a tag, and add to it.

use super::*;
use crate::rules::Part;
use crate::rules::RuleList;

/// What the list shows of one thing of a section.
pub(in crate::tab) struct Row {
    pub(in crate::tab) title: String,
    /// What it does in a line, in words: what a search looks in, and what
    /// a screen reader is told.
    pub(in crate::tab) detail: String,
    /// The same, part by part, each with its picture, as it is shown.
    pub(in crate::tab) parts: Vec<(widgets::Glyph, String)>,
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
            TRIGGERS => rule_rows(&draft.triggers, crate::rules::Rule::summary),
            ALIASES => rule_rows(&draft.aliases, |rule| rule.does().into_iter().collect()),
            MACROS => rule_rows(&draft.macros, |rule| {
                let key = (Part::Key, rule.when.key.trim().to_uppercase());
                std::iter::once(key).chain(rule.does()).collect()
            }),
            TIMERS => rule_rows(&draft.timers, |rule| {
                let how = if rule.when.repeat {
                    tr("every")
                } else {
                    tr("after")
                };
                let time = (Part::Time, format!("{how} {}", rule.when.interval()));
                std::iter::once(time).chain(rule.does()).collect()
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
            parts: Vec::new(),
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
                if rows.is_empty() && folders.is_empty() && self.empty_list(ui) {
                    self.add_rule();
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
                let renaming =
                    (self.editor.renaming.as_ref()).is_some_and(|named| named.path == *path);
                let rect = match out_of_sight(ui, widgets::FOLDER_ROW).filter(|_| !renaming) {
                    Some(rect) => rect,
                    None => self.folder_line(ui, kind, path, depth, (open, on && folder_on), count),
                };
                spots.push((Spot::Folder(path.clone()), rect));
                if open {
                    self.branch(ui, tree, path, on && folder_on, spots);
                }
            }
        }
        for (index, row) in tree.rows.iter().enumerate() {
            if row.folder == folder && self.wanted(row) {
                // Out of sight, only its room is taken: what is drawn is
                // what can be seen, and the one chosen, which keeps the
                // keyboard.
                let tall = match row.detail.is_empty() && row.parts.is_empty() {
                    true => widgets::ROW_SHORT,
                    false => widgets::ROW_TALL,
                };
                let chosen = self.editor.selected[self.editor.section] == index;
                let hidden = out_of_sight(ui, tall).filter(|_| !chosen);
                let rect = match hidden {
                    Some(rect) => rect,
                    None => {
                        let tags_on = row.tags.iter().all(|tag| self.editor.draft.tag_on(tag));
                        self.list_line(ui, row, index, depth, on && row.on && tags_on)
                    }
                };
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
            rect.right_center() - egui::vec2(21.0, 0.0),
            egui::vec2(30.0, 17.0),
        );
        let mut slot = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("folder switch", kind, path))
                .max_rect(place),
        );
        let mut on = self.editor.draft.folder_on(kind, path);
        if widgets::small_switch(&mut slot, &mut on, name).changed() {
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
        crate::widgets::context_menu_of(&line, |ui| self.folder_menu(ui, kind, path));
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
            parts: &row.parts,
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
        crate::widgets::context_menu_of(&line, |ui| self.row_menu(ui));
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
        // Its name is chosen, to be typed over at once.
        self.editor.rename = true;
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

impl Tab {
    /// What an empty section shows: its picture, that there is nothing in
    /// it yet, and the button that makes the first. Returns whether it was
    /// pressed.
    fn empty_list(&self, ui: &mut egui::Ui) -> bool {
        use widgets::Glyph;
        let theme = theme::current();
        let (glyph, new) = match self.editor.section {
            TRIGGERS => (Glyph::When, tr("New trigger")),
            ALIASES => (Glyph::Send, tr("New alias")),
            MACROS => (Glyph::Key, tr("New macro")),
            TIMERS => (Glyph::Timer, tr("New timer")),
            GAUGES => (Glyph::Gauge, tr("New gauge")),
            VARIABLES => (Glyph::Type, tr("New variable")),
            _ => (Glyph::Script, tr("New script")),
        };
        let mut pressed = false;
        ui.add_space(36.0);
        ui.vertical_centered(|ui| {
            widgets::glyph(ui, glyph, 26.0, theme.text_faint);
            ui.add_space(6.0);
            let said = RichText::new(tr("Nothing here yet."))
                .font(theme::font(Weight::Medium, 13.5))
                .color(theme.text_dim);
            ui.add(egui::Label::new(said).selectable(false));
            ui.add_space(8.0);
            pressed = widgets::primary_button(ui, new).clicked();
        });
        pressed
    }
}

/// If a line `tall` high, where it would go next, is out of sight of the
/// list, takes its room and returns where it is; else leaves it to be
/// drawn.
fn out_of_sight(ui: &mut egui::Ui, tall: f32) -> Option<egui::Rect> {
    let at = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), tall));
    if ui.clip_rect().expand(tall).intersects(at) {
        return None;
    }
    Some(ui.allocate_space(at.size()).1)
}

/// A part of what a rule does as it is shown: its picture, and its words.
fn shown_part(part: Part, text: String) -> (widgets::Glyph, String) {
    use widgets::Glyph;
    match part {
        Part::Send => (Glyph::Send, text),
        Part::Script => (Glyph::Code, text),
        Part::Say => (Glyph::Say, text),
        Part::Sound => (Glyph::Sound, text),
        Part::Colour => (Glyph::Colour, text),
        Part::Panel => (Glyph::Panel, text),
        Part::Hide => (Glyph::Hide, tr("hides the line").to_owned()),
        Part::Key => (Glyph::Key, text),
        Part::Time => (Glyph::Timer, text),
    }
}

/// The rows of rules of one kind, with what each does in a line.
fn rule_rows<T: crate::rules::When>(
    rules: &Vec<crate::rules::Rule<T>>,
    detail: impl Fn(&crate::rules::Rule<T>) -> Vec<(Part, String)>,
) -> Vec<Row> {
    (0..rules.len())
        .map(|index| {
            let title = match RuleList::title(rules, index) {
                title if title.is_empty() => tr(T::NEW).to_owned(),
                title => title,
            };
            let rule = &rules[index];
            let parts: Vec<(widgets::Glyph, String)> = (detail(rule).into_iter())
                .filter(|(part, text)| *part == Part::Hide || !text.is_empty())
                .map(|(part, text)| shown_part(part, text))
                .collect();
            Row {
                title,
                detail: (parts.iter())
                    .map(|(_, text)| text.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                parts,
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
        parts: Vec::new(),
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
        parts: Vec::new(),
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
