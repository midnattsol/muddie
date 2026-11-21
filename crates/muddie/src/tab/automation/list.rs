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

/// A line of the list: a thing, by its place in the section, or a folder,
/// by its path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    pub(in crate::tab) fn narrowed(&self) -> bool {
        !self.editor.filter.trim().is_empty() || self.tag_filter().is_some()
    }

    /// The list of the section chosen: the way to add to it and to search
    /// it, then its folders and what is in them, as a tree: one stop of Tab,
    /// on what is chosen, and the arrows through the rest.
    pub(in crate::tab) fn automation_list(&mut self, ui: &mut egui::Ui) {
        if self.editor.section == DATA {
            return self.data_list(ui);
        }
        let kind = section_kind(self.editor.section);
        // Found once a frame, for the toolbar and the tree: a folder made or
        // folded from the toolbar is shown the frame after.
        let folders = kind
            .map(|kind| self.section_folders(kind))
            .unwrap_or_default();
        self.list_toolbar(ui, &folders);
        ui.add_space(4.0);
        let rows = self.automation_rows();
        let items = self.tree_items(&rows, &folders, kind);
        self.editor.visible = items.iter().map(|item| item.spot.clone()).collect();
        // A folder chosen that is no longer shown (deleted, or narrowed
        // out) gives way to what was chosen in the list before it.
        if let Some(path) = &self.editor.folder
            && !self.editor.visible.contains(&Spot::Folder(path.clone()))
            && self.editor.renaming.is_none()
        {
            self.editor.folder = None;
        }
        self.moved_by_keys(ui, &rows);
        self.choosing_keys(ui, &items);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if rows.is_empty() && folders.is_empty() && self.empty_list(ui) {
                    self.add_rule();
                }
                let current = self.current_spot();
                let mut spots = Vec::with_capacity(items.len());
                // A tree where there are folders, else a list, under the
                // section's name: a screen reader says where each line is
                // in it, how deep, and whether a folder is open.
                let role = match kind {
                    Some(_) => egui::accesskit::Role::Tree,
                    None => egui::accesskit::Role::List,
                };
                let whole = egui::UiBuilder::new()
                    .accessibility_role(role)
                    .accessibility_label(section_name(self.editor.section));
                ui.scope_builder(whole, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for item in &items {
                        let here = current.as_ref() == Some(&item.spot);
                        let rect = match &item.spot {
                            Spot::Folder(path) => {
                                let renaming = (self.editor.renaming.as_ref())
                                    .is_some_and(|named| named.path == *path);
                                match out_of_sight(ui, widgets::FOLDER_ROW)
                                    .filter(|_| !renaming && !here)
                                {
                                    Some(rect) => rect,
                                    None => self.folder_line(
                                        ui,
                                        kind.unwrap_or(Kind::Trigger),
                                        path,
                                        item,
                                    ),
                                }
                            }
                            Spot::Row(index) => {
                                let row = &rows[*index];
                                let tall = match row.detail.is_empty() && row.parts.is_empty() {
                                    true => widgets::ROW_SHORT,
                                    false => widgets::ROW_TALL,
                                };
                                // Out of sight, only its room is taken: what is
                                // drawn is what can be seen, and the one chosen,
                                // which keeps the keyboard.
                                match out_of_sight(ui, tall).filter(|_| !here) {
                                    Some(rect) => rect,
                                    None => self.list_line(ui, row, *index, item),
                                }
                            }
                        };
                        spots.push((item.spot.clone(), rect));
                    }
                });
                // What is under the last row takes a drop at the top level.
                let rest = ui.available_rect_before_wrap();
                ui.allocate_rect(rest, egui::Sense::hover());
                self.drag_in_list(ui, &spots, &rows);
            });
    }

    /// What the list shows, in the order it is shown: the folders of each
    /// folder, each followed by what is in it unless it is folded, then what
    /// is in the folder itself, with how deep each is, whether it is
    /// switched on with all around it, and its place among those beside it.
    /// What each folder holds is counted once for all.
    pub(in crate::tab) fn tree_items(
        &self,
        rows: &[Row],
        folders: &[String],
        kind: Option<Kind>,
    ) -> Vec<TreeItem> {
        let wanted: Vec<bool> = rows.iter().map(|row| self.wanted(row)).collect();
        let mut held: std::collections::HashMap<&str, usize> = Default::default();
        for (row, _) in rows.iter().zip(&wanted).filter(|(_, wanted)| **wanted) {
            let mut path = row.folder.as_str();
            while !path.is_empty() {
                *held.entry(path).or_default() += 1;
                path = parent_of(path);
            }
        }
        let mut items = Vec::new();
        let walk = Walk {
            rows,
            folders,
            kind,
            wanted: &wanted,
            held: &held,
        };
        self.walk(&walk, "", true, 0, &mut items);
        // Each one's place among those in the same folder.
        let mut sizes: std::collections::HashMap<String, usize> = Default::default();
        for item in &items {
            *sizes.entry(item.parent(rows).to_owned()).or_default() += 1;
        }
        let mut seen: std::collections::HashMap<String, usize> = Default::default();
        for item in &mut items {
            let parent = item.parent(rows).to_owned();
            let at = seen.entry(parent.clone()).or_default();
            *at += 1;
            item.place = (*at, sizes[&parent]);
        }
        items
    }

    fn walk(
        &self,
        walk: &Walk<'_>,
        folder: &str,
        on: bool,
        depth: usize,
        items: &mut Vec<TreeItem>,
    ) {
        if let Some(kind) = walk.kind {
            for path in (walk.folders.iter()).filter(|path| parent_of(path) == folder) {
                let count = walk.held.get(path.as_str()).copied().unwrap_or(0);
                if count == 0 && self.narrowed() {
                    continue;
                }
                let folder_on = on && self.editor.draft.folder_on(kind, path);
                let open = !self.editor.folded.contains(&(kind, path.clone())) || self.narrowed();
                items.push(TreeItem {
                    spot: Spot::Folder(path.clone()),
                    depth,
                    active: folder_on,
                    open,
                    count,
                    place: (0, 0),
                });
                if open {
                    self.walk(walk, path, folder_on, depth + 1, items);
                }
            }
        }
        for (index, row) in walk.rows.iter().enumerate() {
            if row.folder == folder && walk.wanted[index] {
                let tags_on = row.tags.iter().all(|tag| self.editor.draft.tag_on(tag));
                items.push(TreeItem {
                    spot: Spot::Row(index),
                    depth,
                    active: on && row.on && tags_on,
                    open: false,
                    count: 0,
                    place: (0, 0),
                });
            }
        }
    }

    /// A folder's line: chosen by a click, opened and closed by its arrow,
    /// by Enter or by the arrows, switched by its switch, named again from
    /// its menu, by F2 or by a double click, and dragged into another.
    /// Returns where it is.
    fn folder_line(
        &mut self,
        ui: &mut egui::Ui,
        kind: Kind,
        path: &str,
        item: &TreeItem,
    ) -> egui::Rect {
        let name = path.rsplit('/').next().unwrap_or(path);
        let renaming =
            (self.editor.renaming.as_ref()).is_some_and(|renaming| renaming.path == path);
        let rect = match renaming {
            true => self.folder_name_field(ui, kind, item),
            false => self.folder_handles(ui, kind, path, item),
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

    /// The line of a folder as it is drawn, chosen or not.
    pub(in crate::tab) fn folder_look<'a>(
        &self,
        path: &'a str,
        item: &TreeItem,
    ) -> widgets::FolderRow<'a> {
        let spot = Spot::Folder(path.to_owned());
        let current = self.current_spot().as_ref() == Some(&spot);
        widgets::FolderRow {
            name: path.rsplit('/').next().unwrap_or(path),
            depth: item.depth,
            open: item.open,
            active: item.active,
            count: item.count,
            selected: self.editor.folder.as_deref() == Some(path),
            key: Some(item_id(self.editor.section, &spot)),
            current,
            place: Some(item.place),
        }
    }

    /// A folder's line as it usually is. Returns where it is.
    fn folder_handles(
        &mut self,
        ui: &mut egui::Ui,
        kind: Kind,
        path: &str,
        item: &TreeItem,
    ) -> egui::Rect {
        let line = self.folder_look(path, item).show(ui);
        self.keep_keys(ui, &line);
        // Dragged too, without making every line a stop of Tab.
        let line = line.interact(egui::Sense::CLICK | egui::Sense::DRAG);
        if line.clicked() {
            // By the pointer on its arrow, or by the keyboard (Enter, or a
            // screen reader's action), it opens or closes; anywhere else
            // the pointer only chooses it.
            let by_pointer = ui.input(|input| input.pointer.primary_released());
            let on_arrow = (line.interact_pointer_pos())
                .is_some_and(|at| at.x < widgets::folder_arrow_end(line.rect, item.depth));
            if !by_pointer || on_arrow {
                self.fold(kind, path, item.open);
            }
            self.choose_folder(path);
        }
        if line.secondary_clicked() {
            self.choose_folder(path);
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

    /// The button that closes every folder while one is open, and else
    /// opens them all, as in a file tree; with none, it is not there. What
    /// was done is said, as the button's name changes under the keyboard.
    fn fold_all_button(&mut self, ui: &mut egui::Ui, kind: Kind, folders: &[String]) {
        if folders.is_empty() {
            return;
        }
        let any_open =
            (folders.iter()).any(|path| !self.editor.folded.contains(&(kind, path.clone())));
        let (icon, label) = match any_open {
            true => (widgets::Icon::CollapseAll, tr("Collapse all folders")),
            false => (widgets::Icon::ExpandAll, tr("Expand all folders")),
        };
        if !widgets::icon_button(ui, icon, label, false).clicked() {
            return;
        }
        self.editor.folded.retain(|(of, _)| *of != kind);
        if any_open {
            (self.editor.folded).extend(folders.iter().map(|path| (kind, path.clone())));
            // What was chosen inside a folder now closed gives way to the
            // folder it is in, at the top, so that what is chosen is seen.
            if let Some(spot) = self.current_spot() {
                let folder = match &spot {
                    Spot::Row(index) => self
                        .automation_rows()
                        .get(*index)
                        .map(|row| row.folder.clone()),
                    Spot::Folder(path) => Some(path.clone()),
                };
                let top = folder
                    .as_deref()
                    .and_then(|path| path.split('/').next())
                    .filter(|top| !top.is_empty());
                if let Some(top) = top {
                    self.choose_folder(top);
                }
            }
        }
        self.voice.say_now(match any_open {
            true => tr("All folders collapsed."),
            false => tr("All folders expanded."),
        });
    }

    /// Closes a folder that is `open`, or opens it. Nothing is said: the
    /// screen reader says it itself, from the folder's state, or on Linux
    /// from its description.
    pub(in crate::tab) fn fold(&mut self, kind: Kind, path: &str, open: bool) {
        let key = (kind, path.to_owned());
        self.editor.folded.retain(|folded| *folded != key);
        if open {
            self.editor.folded.push(key);
        }
    }

    /// While the keyboard is on a line of the list, the arrows are the
    /// list's, not for going to what is beside it; and the line is brought
    /// into sight when it was gone to by the keys.
    fn keep_keys(&mut self, ui: &mut egui::Ui, line: &egui::Response) {
        if line.has_focus() {
            let filter = egui::EventFilter {
                horizontal_arrows: true,
                vertical_arrows: true,
                ..Default::default()
            };
            ui.memory_mut(|memory| memory.set_focus_lock_filter(line.id, filter));
        }
        let here = self
            .current_spot()
            .is_some_and(|spot| item_id(self.editor.section, &spot) == line.id);
        if here && std::mem::take(&mut self.editor.reveal) {
            line.scroll_to_me(None);
        }
    }

    /// What can be done with a folder, from its menu.
    fn folder_menu(&mut self, ui: &mut egui::Ui, kind: Kind, path: &str) {
        widgets::menu_width(ui);
        if widgets::menu_item(ui, tr("Rename"), "F2").clicked() {
            self.start_renaming(path);
            ui.close();
        }
        if widgets::menu_item(ui, tr("New folder inside"), "").clicked() {
            self.new_folder(kind, path);
            ui.close();
        }
        widgets::menu_separator(ui);
        if widgets::menu_item(ui, tr("Delete folder"), tr("Del")).clicked() {
            self.delete_folder(kind, path);
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
        item: &TreeItem,
    ) -> egui::Rect {
        let section = self.editor.section;
        let chosen = self.chosen().contains(&index);
        let spot = Spot::Row(index);
        let tree = section_kind(section).is_some();
        let line = widgets::ListRow {
            title: &row.title,
            detail: &row.detail,
            parts: &row.parts,
            tags: &row.tags,
            selected: chosen,
            active: item.active,
            depth: item.depth,
            problem: row.problem,
            key: Some(item_id(section, &spot)),
            current: self.current_spot().as_ref() == Some(&spot),
            place: tree.then_some(item.place),
        }
        .show(ui);
        self.keep_keys(ui, &line);
        if line.clicked() {
            self.choose_row(index, ui.input(|input| input.modifiers));
        }
        if line.secondary_clicked() && !chosen {
            self.choose_row(index, Modifiers::NONE);
        }
        crate::widgets::context_menu_of(&line, |ui| self.row_menu(ui));
        let movable = section != VARIABLES && !self.narrowed();
        if movable {
            let line = line.interact(egui::Sense::DRAG);
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
    fn list_toolbar(&mut self, ui: &mut egui::Ui, folders: &[String]) {
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
                self.fold_all_button(ui, kind, folders);
            }
            widgets::text_box(ui, &mut self.editor.filter, tr("Search"));
        });
    }

    /// The folder chosen, or the folder of the one chosen: where what is
    /// added goes.
    pub(in crate::tab) fn chosen_folder(&self) -> String {
        if let Some(folder) = &self.editor.folder {
            return folder.clone();
        }
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

/// What going through a section's tree needs: its kind, if it has
/// folders, what is in it and whether each is shown, its folders, and how
/// many shown each folder holds.
struct Walk<'a> {
    kind: Option<Kind>,
    rows: &'a [Row],
    folders: &'a [String],
    wanted: &'a [bool],
    held: &'a std::collections::HashMap<&'a str, usize>,
}

/// One line of the list as it is shown: a folder or a thing, how many
/// folders in, whether it is switched on with all around it, for a folder
/// whether it is open and how many it holds, and its place among those in
/// the same folder (counted from 1) and how many they are.
pub(in crate::tab) struct TreeItem {
    pub(in crate::tab) spot: Spot,
    pub(in crate::tab) depth: usize,
    pub(in crate::tab) active: bool,
    pub(in crate::tab) open: bool,
    pub(in crate::tab) count: usize,
    pub(in crate::tab) place: (usize, usize),
}

impl TreeItem {
    /// The folder it is in.
    pub(in crate::tab) fn parent<'a>(&'a self, rows: &'a [Row]) -> &'a str {
        match &self.spot {
            Spot::Folder(path) => parent_of(path),
            Spot::Row(index) => rows[*index].folder.as_str(),
        }
    }
}

/// The id of a line of the list, the same every frame, for the keyboard
/// to be put on it.
pub(in crate::tab) fn item_id(section: usize, spot: &Spot) -> egui::Id {
    egui::Id::unique("automation item").with((section, spot))
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

/// What a rule is called in the list, and wherever it is named: its name,
/// or what sets it off, or that it is new.
pub(in crate::tab) fn rule_title<T: crate::rules::When>(
    rules: &Vec<crate::rules::Rule<T>>,
    index: usize,
) -> String {
    match RuleList::title(rules, index) {
        title if title.is_empty() => tr(T::NEW).to_owned(),
        title => title,
    }
}

/// The rows of rules of one kind, with what each does in a line.
fn rule_rows<T: crate::rules::When>(
    rules: &Vec<crate::rules::Rule<T>>,
    detail: impl Fn(&crate::rules::Rule<T>) -> Vec<(Part, String)>,
) -> Vec<Row> {
    (0..rules.len())
        .map(|index| {
            let title = rule_title(rules, index);
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
