//! The tree of the list: its folders and rules in order, folded or open, the
//! menu of a folder, and the paths of folders.

use super::*;

impl Tab {
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
                let open = self.folder_open(kind, path);
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
    pub(super) fn folder_line(
        &mut self,
        ui: &mut egui::Ui,
        kind: Kind,
        path: &str,
        item: &TreeItem,
    ) -> egui::Rect {
        let name = name_of(path);
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
            name: name_of(path),
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
        // The menu acts on it, and does not choose it: it is ringed while
        // the menu is open.
        if widgets::menu_pressed(&line) {
            let chosen = self.editor.folder.as_deref() == Some(path);
            self.editor.menu_target = (!chosen).then(|| Spot::Folder(path.to_owned()));
        }
        if line.double_clicked() {
            self.start_renaming(path);
        }
        if line.drag_started() && !self.narrowed() {
            self.editor.dragged = Some(Dragged::Folder(path.to_owned()));
        }
        let open = crate::widgets::context_menu_of(&line, |ui| self.folder_menu(ui, kind, path));
        self.ring_menu_target(ui, &line, Spot::Folder(path.to_owned()), open.is_some());
        line.rect
    }

    /// The button that closes every folder while one is open, and else
    /// opens them all, as in a file tree; with none, it is not there. What
    /// was done is said, as the button's name changes under the keyboard.
    pub(super) fn fold_all_button(&mut self, ui: &mut egui::Ui, kind: Kind, folders: &[String]) {
        if folders.is_empty() {
            return;
        }
        let any_open = (folders.iter()).any(|path| self.folder_open(kind, path));
        let (icon, label) = match any_open {
            true => (widgets::Icon::CollapseAll, tr("Collapse all folders")),
            false => (widgets::Icon::ExpandAll, tr("Expand all folders")),
        };
        if !widgets::icon_button(ui, icon, label, false).clicked() {
            return;
        }
        self.editor.opened.retain(|(of, _)| *of != kind);
        if !any_open {
            (self.editor.opened).extend(folders.iter().map(|path| (kind, path.clone())));
        } else {
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
        self.editor.opened.retain(|known| *known != key);
        if !open {
            self.editor.opened.push(key);
        }
    }

    /// Whether a folder is open: as it was left, or because what is shown
    /// is narrowed by a search or a tag, which shows all it finds.
    pub(in crate::tab) fn folder_open(&self, kind: Kind, path: &str) -> bool {
        self.narrowed()
            || (self.editor.opened.iter()).any(|(of, known)| *of == kind && known == path)
    }

    /// Opens a folder and those it is in.
    pub(in crate::tab) fn open_to(&mut self, kind: Kind, path: &str) {
        for place in muddie_script::places_up(path).filter(|place| !place.is_empty()) {
            if !self.folder_open(kind, place) {
                self.editor.opened.push((kind, place.to_owned()));
            }
        }
    }

    /// Opens the folders the one chosen in the section is in, for it to be
    /// seen where it is: the others stay as they were.
    pub(in crate::tab) fn reveal_chosen(&mut self) {
        let Some(kind) = self.editor.section.kind() else {
            return;
        };
        let folder = match self.current_spot() {
            Some(Spot::Row(index)) => self
                .automation_rows()
                .get(index)
                .map(|row| row.folder.clone()),
            Some(Spot::Folder(path)) => Some(parent_of(&path).to_owned()),
            None => None,
        };
        if let Some(folder) = folder.filter(|folder| !folder.is_empty()) {
            self.open_to(kind, &folder);
        }
    }

    /// The folders a folder can be moved into: none of its own, nor
    /// itself.
    pub(in crate::tab) fn folders_for(&self, kind: Kind, path: &str) -> Vec<String> {
        (self.section_folders(kind).into_iter())
            .filter(|folder| Self::folder_fits(path, folder))
            .collect()
    }

    /// Moves a folder into another, or out of every one, from its menu or
    /// its details, and says where; the keyboard is put on it.
    pub(in crate::tab) fn folder_moved_to(
        &mut self,
        ui: &egui::Ui,
        kind: Kind,
        path: &str,
        parent: &str,
    ) {
        if parent_of(path) == parent {
            return;
        }
        let chosen = self.editor.folder.as_deref() == Some(path);
        if let Some(now) = self.put_folder(kind, path, parent, None) {
            self.editor.reveal = chosen;
            if chosen && widgets::keyboard_in_use(ui.ctx()) {
                let id = item_id(self.editor.section, &Spot::Folder(now));
                ui.memory_mut(|memory| memory.request_focus(id));
            }
            self.say_moved(parent);
        }
    }

    /// While the menu opened on a line it does not choose is open, the line
    /// is ringed, as what the menu acts on; once it closes, it is not.
    pub(super) fn ring_menu_target(
        &mut self,
        ui: &egui::Ui,
        line: &egui::Response,
        spot: Spot,
        open: bool,
    ) {
        if self.editor.menu_target.as_ref() != Some(&spot) {
            return;
        }
        if !open {
            self.editor.menu_target = None;
            return;
        }
        widgets::target_ring(ui, line.rect);
    }

    /// What can be done with a folder, from its menu.
    fn folder_menu(&mut self, ui: &mut egui::Ui, kind: Kind, path: &str) {
        widgets::menu_width(ui);
        if widgets::menu_item(ui, tr("Rename…"), "F2").clicked() {
            self.start_renaming(path);
            ui.close();
        }
        if widgets::menu_item(ui, tr("New folder inside"), "").clicked() {
            self.new_folder(kind, path);
            ui.close();
        }
        widgets::menu_separator(ui);
        widgets::menu_heading(ui, tr("Move to folder"));
        let here = parent_of(path).to_owned();
        let places = std::iter::once(String::new()).chain(self.folders_for(kind, path));
        for place in places.filter(|place| *place != here) {
            if widgets::menu_item(ui, &folder_label(&place), "").clicked() {
                self.folder_moved_to(ui, kind, path, &place);
                ui.close();
            }
        }
        widgets::menu_separator(ui);
        if widgets::menu_item(ui, tr("Delete folder"), tr("Del")).clicked() {
            self.delete_folder(kind, path);
            ui.close();
        }
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
pub(in crate::tab) fn item_id(section: Section, spot: &Spot) -> egui::Id {
    egui::Id::unique("automation item").with((section, spot))
}

/// The folder a place is in; empty for none.
pub(in crate::tab) fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The things among lines of the list, in their order, not the folders.
pub(in crate::tab) fn rows_in(spots: &[Spot]) -> Vec<usize> {
    (spots.iter())
        .filter_map(|spot| match spot {
            Spot::Row(index) => Some(*index),
            Spot::Folder(_) => None,
        })
        .collect()
}

/// The last part of a place: a folder's or a script's own name.
pub(in crate::tab) fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A folder as it is offered to be chosen: its name, set in as deep as it
/// is; and the top level, which is no folder, as None.
pub(in crate::tab) fn folder_label(path: &str) -> String {
    match path {
        "" => tr("None").to_owned(),
        path => format!(
            "{}{}",
            "    ".repeat(path.matches('/').count()),
            name_of(path)
        ),
    }
}
