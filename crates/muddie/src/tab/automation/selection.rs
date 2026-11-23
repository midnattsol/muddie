//! What is chosen in the editor's list, one or several, and what is done
//! with all of them at once: switching them, filing them, tagging them,
//! moving them and deleting them. One chosen shows its details; several
//! show what they have in common.

use super::*;
use crate::rules::{joined, moved_order};

impl Tab {
    /// How many things the current section holds.
    pub(in crate::tab) fn section_len(&self) -> usize {
        let section = self.editor.section;
        if let Some(list) = section.kind().and_then(|kind| self.editor.draft.list(kind)) {
            return list.len();
        }
        match section {
            VARIABLES => self.scripts.variable_count(),
            SCRIPTS => self.editor.script_files.len(),
            _ => 0,
        }
    }

    /// What is chosen in the current section, in order: those marked, or
    /// the one selected; nothing while a folder is chosen.
    pub(in crate::tab) fn chosen(&self) -> Vec<usize> {
        if self.editor.folder.is_some() {
            return Vec::new();
        }
        if self.editor.marked.len() > 1 {
            let mut marked = self.editor.marked.clone();
            marked.sort_unstable();
            return marked;
        }
        let selected = self.editor.selected[self.editor.section];
        match selected < self.section_len() {
            true => vec![selected],
            false => Vec::new(),
        }
    }

    /// Chooses a row as a click with these keys held does: alone; with
    /// Ctrl, added to or taken from what is chosen; with Shift, all from
    /// the one chosen before to it, in the order they are shown.
    pub(in crate::tab) fn choose_row(&mut self, index: usize, modifiers: Modifiers) {
        self.editor.folder = None;
        let section = self.editor.section;
        let anchor = self.editor.selected[section];
        let marked = &mut self.editor.marked;
        if modifiers.shift {
            let rows = list::rows_in(&self.editor.visible);
            let at = |row: usize| rows.iter().position(|&shown| shown == row);
            if let (Some(from), Some(to)) = (at(anchor), at(index)) {
                *marked = rows[from.min(to)..=from.max(to)].to_vec();
            }
        } else if modifiers.command {
            if marked.is_empty() {
                marked.push(anchor);
            }
            match marked.iter().position(|&known| known == index) {
                Some(at) => drop(marked.remove(at)),
                None => marked.push(index),
            }
            self.editor.selected[section] = marked.last().copied().unwrap_or(index);
            if marked.len() < 2 {
                marked.clear();
            }
        } else {
            marked.clear();
            self.editor.selected[section] = index;
        }
        self.editor.sample.clear();
        self.editor.recording_key = false;
    }

    /// Chooses a folder, alone: what is done next is done to it.
    pub(in crate::tab) fn choose_folder(&mut self, path: &str) {
        self.editor.folder = Some(path.to_owned());
        self.editor.marked.clear();
        self.editor.sample.clear();
        self.editor.recording_key = false;
    }

    /// The line of the list that is chosen, and has the keyboard when the
    /// list has it: the folder chosen, or else the one selected.
    pub(in crate::tab) fn current_spot(&self) -> Option<Spot> {
        if let Some(path) = &self.editor.folder {
            return Some(Spot::Folder(path.clone()));
        }
        let selected = self.editor.selected[self.editor.section];
        (selected < self.section_len()).then_some(Spot::Row(selected))
    }

    /// Chooses a line, as the plain arrows do, and puts the keyboard on it
    /// if it was in the list.
    fn go_to_line(&mut self, ui: &mut egui::Ui, spot: &Spot, keyboard: bool) {
        match spot {
            Spot::Row(index) => self.choose_row(*index, Modifiers::NONE),
            Spot::Folder(path) => self.choose_folder(path),
        }
        self.editor.reveal = true;
        if keyboard {
            let id = list::item_id(self.editor.section, spot);
            ui.memory_mut(|memory| memory.request_focus(id));
        }
    }

    /// The keys of the list, while it has the keyboard or nothing does, as
    /// in any tree: the arrows go up and down it, Home and End to its ends,
    /// the left one closes a folder or goes to the one around, the right
    /// one opens it or goes into it; with Shift, the arrows choose more;
    /// Ctrl+A chooses all that is shown; Escape leaves one chosen; Delete
    /// deletes what is chosen, a folder too, and F2 renames it.
    pub(in crate::tab) fn choosing_keys(&mut self, ui: &mut egui::Ui, items: &[list::TreeItem]) {
        let current = self.current_spot();
        let section = self.editor.section;
        let focused = ui.memory(|memory| memory.focused());
        let keyboard = focused.is_some();
        let ours = current
            .as_ref()
            .is_some_and(|spot| focused == Some(list::item_id(section, spot)));
        if keyboard && !ours {
            return;
        }
        // Those with Shift first: without it, the plain ones would take
        // them too.
        let (wider_up, wider_down) = ui.input_mut(|input| {
            (
                input.consume_key(Modifiers::SHIFT, Key::ArrowUp),
                input.consume_key(Modifiers::SHIFT, Key::ArrowDown),
            )
        });
        let pressed = |key: Key| ui.input_mut(|input| input.consume_key(Modifiers::NONE, key));
        let (up, down, home, end) = (
            pressed(Key::ArrowUp),
            pressed(Key::ArrowDown),
            pressed(Key::Home),
            pressed(Key::End),
        );
        let (left, right) = (pressed(Key::ArrowLeft), pressed(Key::ArrowRight));
        let (delete, rename) = (pressed(Key::Delete), pressed(Key::F2));
        let all = ui.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::A));
        let back = self.editor.marked.len() > 1 && widgets::take_escape(ui);
        let visible = self.editor.visible.clone();
        let rows = list::rows_in(&visible);
        if all && rows.len() > 1 {
            self.editor.folder = None;
            self.editor.marked = rows.clone();
        }
        if back {
            self.editor.marked.clear();
        }
        let kind = section.kind();
        if let (Some(Spot::Folder(path)), Some(kind)) = (&current, kind) {
            if delete {
                return self.delete_folder(kind, path);
            }
            if rename {
                return self.start_renaming(path);
            }
        }
        if delete && !self.chosen().is_empty() {
            self.delete_chosen();
            self.list_scripts();
            return;
        }
        if rename && self.chosen().len() == 1 {
            self.editor.rename = true;
        }
        let at = current
            .as_ref()
            .and_then(|spot| visible.iter().position(|shown| shown == spot));
        // Left and right: a folder closed or opened, or the one around, or
        // the first in it.
        if left || right {
            let Some(at) = at else { return };
            let item = &items[at];
            match (&item.spot, kind) {
                (Spot::Folder(path), Some(kind)) if left && item.open && !self.narrowed() => {
                    self.fold(kind, path, true)
                }
                (Spot::Folder(path), Some(kind)) if right && !item.open => {
                    self.fold(kind, path, false)
                }
                (Spot::Folder(_), _) if right => {
                    if let Some(next) = items.get(at + 1).filter(|next| next.depth > item.depth) {
                        let spot = next.spot.clone();
                        self.go_to_line(ui, &spot, keyboard);
                    }
                }
                _ if left => {
                    let around = items[..at]
                        .iter()
                        .rev()
                        .find(|before| before.depth < item.depth);
                    if let Some(around) = around {
                        let spot = around.spot.clone();
                        self.go_to_line(ui, &spot, keyboard);
                    }
                }
                _ => {}
            }
            return;
        }
        // Going: to the one above or below the one chosen, or to an end.
        let to = match (up, down, home, end, at) {
            (true, _, _, _, Some(at)) => Some(at.saturating_sub(1)),
            (_, true, _, _, Some(at)) => Some((at + 1).min(visible.len().saturating_sub(1))),
            (_, _, true, _, _) => Some(0),
            (_, _, _, true, _) => visible.len().checked_sub(1),
            (true, _, _, _, None) | (_, true, _, _, None) => Some(0),
            _ => None,
        };
        if let Some(spot) = to.and_then(|to| visible.get(to)) {
            self.go_to_line(ui, spot, keyboard);
            return;
        }
        // Widening: what is chosen, and the one above or below it, of the
        // things, not the folders.
        if self.editor.folder.is_some() {
            return;
        }
        let chosen = self.chosen();
        let place = |row: usize| rows.iter().position(|&shown| shown == row);
        let shown: Vec<usize> = chosen.iter().filter_map(|&row| place(row)).collect();
        let (first, last) = (shown.iter().min().copied(), shown.iter().max().copied());
        let added = match (first, last, wider_up, wider_down) {
            (Some(first), _, true, _) if first > 0 => Some(rows[first - 1]),
            (_, Some(last), _, true) => rows.get(last + 1).copied(),
            _ => None,
        };
        if let Some(row) = added {
            let mut marked = chosen;
            marked.push(row);
            self.editor.marked = marked;
        }
    }

    /// Takes the things at `moving` and puts them, in the order they were,
    /// before what is at `to`, keeping them chosen.
    pub(in crate::tab) fn move_rows(&mut self, moving: &[usize], to: usize) {
        let section = self.editor.section;
        let order = moved_order(self.section_len(), moving, to);
        match section
            .kind()
            .and_then(|kind| self.editor.draft.list_mut(kind))
        {
            Some(list) => list.reorder(&order),
            None => return,
        }
        let now = |old: usize| order.iter().position(|&index| index == old).unwrap_or(old);
        self.editor.selected[section] = now(self.editor.selected[section]);
        self.editor.marked = self.editor.marked.iter().map(|&old| now(old)).collect();
    }

    /// Whether all that is chosen is switched on.
    fn chosen_on(&self, chosen: &[usize]) -> bool {
        chosen.iter().all(|&index| self.row_on(index))
    }

    /// Whether one thing of the section is switched on, by its own switch.
    fn row_on(&self, index: usize) -> bool {
        let draft = &self.editor.draft;
        match self.editor.section {
            SCRIPTS => self.script_entry(index).is_some_and(|entry| entry.enabled),
            section => (section.kind().and_then(|kind| draft.list(kind)))
                .is_some_and(|list| index < list.len() && list.enabled(index)),
        }
    }

    /// How a script of the list is kept.
    fn script_entry(&self, index: usize) -> Option<crate::rules::ScriptEntry> {
        let file = self.editor.script_files.get(index)?;
        Some(self.editor.draft.script(&self.script_place(file)))
    }

    /// Switches all that is chosen on or off.
    pub(in crate::tab) fn switch_chosen(&mut self, on: bool) {
        self.switch_rows(&self.chosen(), on);
    }

    /// Switches the things at `rows` on or off.
    pub(in crate::tab) fn switch_rows(&mut self, rows: &[usize], on: bool) {
        for &index in rows {
            match self.editor.section {
                SCRIPTS => {
                    if let Some(mut entry) = self.script_entry(index) {
                        entry.enabled = on;
                        self.editor.draft.set_script(entry);
                    }
                }
                section => {
                    if let Some(list) = section
                        .kind()
                        .and_then(|kind| self.editor.draft.list_mut(kind))
                    {
                        *list.head_mut(index).enabled = on;
                    }
                }
            }
        }
    }

    /// The folder and the tags of each that is chosen, of a section that
    /// has them.
    fn chosen_filing(&self, chosen: &[usize]) -> Vec<(String, Vec<String>)> {
        let rows = self.automation_rows();
        (chosen.iter())
            .filter_map(|&index| rows.get(index))
            .map(|row| (row.folder.clone(), row.tags.clone()))
            .collect()
    }

    /// Puts all that is chosen in a folder.
    pub(in crate::tab) fn file_chosen(&mut self, folder: &str) {
        self.file_rows(&self.chosen(), folder);
    }

    /// Puts the things at `rows` in a folder.
    pub(in crate::tab) fn file_rows(&mut self, rows: &[usize], folder: &str) {
        let chosen = rows.to_vec();
        let section = self.editor.section;
        if section == SCRIPTS {
            let places: Vec<String> = (chosen.iter())
                .filter_map(|&index| self.editor.script_files.get(index))
                .map(|file| self.script_place(file))
                .collect();
            for place in places {
                let name = name_of(&place).to_owned();
                if list::parent_of(&place) != folder {
                    self.move_script_files(&place, &joined(folder, &name));
                }
            }
            self.editor.marked.clear();
            return;
        }
        if let Some(list) = section
            .kind()
            .and_then(|kind| self.editor.draft.list_mut(kind))
        {
            for index in chosen {
                *list.head_mut(index).folder = folder.to_owned();
            }
        }
    }

    /// Gives all that is chosen the tags `added`, and takes `removed`.
    fn tag_chosen(&mut self, added: &[String], removed: &[String]) {
        let retag = |tags: &mut Vec<String>| {
            tags.retain(|tag| !removed.iter().any(|gone| tag_key(gone) == tag_key(tag)));
            for tag in added {
                if !tags.iter().any(|known| tag_key(known) == tag_key(tag)) {
                    tags.push(tag.clone());
                }
            }
        };
        for index in self.chosen() {
            match self.editor.section {
                SCRIPTS => {
                    if let Some(mut entry) = self.script_entry(index) {
                        retag(&mut entry.tags);
                        self.editor.draft.set_script(entry);
                    }
                }
                section => {
                    if let Some(list) = section
                        .kind()
                        .and_then(|kind| self.editor.draft.list_mut(kind))
                    {
                        retag(list.head_mut(index).tags);
                    }
                }
            }
        }
    }

    /// What a row's menu acts on: the row it was opened on, if that is not
    /// among what is chosen; else all that is chosen.
    fn menu_rows(&self) -> Vec<usize> {
        match &self.editor.menu_target {
            Some(Spot::Row(index)) => vec![*index],
            _ => self.chosen(),
        }
    }

    /// What can be done with what a row's menu acts on, from the menu:
    /// switching it, putting it in a folder, renaming it, deleting it.
    pub(in crate::tab) fn row_menu(&mut self, ui: &mut egui::Ui) {
        widgets::menu_width(ui);
        let chosen = self.menu_rows();
        let switchable = self.editor.section != VARIABLES;
        if switchable {
            let on = self.chosen_on(&chosen);
            let label = if on { tr("Disable") } else { tr("Enable") };
            if widgets::menu_item(ui, label, "").clicked() {
                self.switch_rows(&chosen, !on);
                ui.close();
            }
        }
        if let Some(kind) = self.editor.section.kind() {
            widgets::menu_separator(ui);
            widgets::menu_heading(ui, tr("Move to folder"));
            let places = std::iter::once(String::new()).chain(self.section_folders(kind));
            for place in places {
                if widgets::menu_item(ui, &folder_label(&place), "").clicked() {
                    self.file_rows(&chosen, &place);
                    ui.close();
                }
            }
        }
        widgets::menu_separator(ui);
        // Its name is in its details: renamed, it is chosen.
        if let [only] = chosen[..]
            && widgets::menu_item(ui, tr("Rename"), "F2").clicked()
        {
            self.choose_row(only, Modifiers::NONE);
            self.editor.rename = true;
            ui.close();
        }
        if widgets::menu_item(ui, tr("Delete"), tr("Del")).clicked() {
            self.delete_rows(&chosen);
            self.list_scripts();
            ui.close();
        }
    }

    /// The details of several chosen at once: how many, what they have in
    /// common, and what can be done with all of them.
    pub(in crate::tab) fn bulk_detail(&mut self, ui: &mut egui::Ui) {
        let chosen = self.chosen();
        let section = self.editor.section;
        let theme = theme::current();
        ui.spacing_mut().item_spacing.y = 10.0;
        let mut delete = false;
        let mut on = self.chosen_on(&chosen);
        let mut switched = false;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                delete =
                    widgets::icon_button(ui, widgets::Icon::Trash, tr("Delete"), false).clicked();
                if section != VARIABLES {
                    switched = widgets::switch(ui, &mut on, tr("Enabled"))
                        .on_hover_text(tr("Enabled"))
                        .changed();
                }
                let said = trn("{0} chosen", "{0} chosen", chosen.len() as u64);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(said)
                            .font(theme::font(Weight::Semibold, 20.0))
                            .color(theme.text),
                    );
                });
            });
        });
        ui.add(egui::Separator::default().spacing(2.0));
        if switched {
            self.switch_chosen(on);
        }
        if let Some(kind) = section.kind() {
            self.bulk_filing(ui, kind, &chosen);
        }
        note(ui, tr("Changes apply to every selected item."));
        if delete {
            self.delete_chosen();
            self.list_scripts();
        }
    }

    /// The folder and the tags of several at once: the folder all are in,
    /// if they are in one, and the tags all carry.
    fn bulk_filing(&mut self, ui: &mut egui::Ui, kind: Kind, chosen: &[usize]) {
        let filing = self.chosen_filing(chosen);
        let common_folder = (filing.first())
            .map(|(folder, _)| folder.clone())
            .filter(|first| filing.iter().all(|(folder, _)| folder == first));
        let mut folder = common_folder.clone().unwrap_or_default();
        let folders = self.section_folders(kind);
        if folder_row(ui, &mut folder, &folders, common_folder.is_none()) {
            self.file_chosen(&folder);
        }
        let carried_by_all = |tag: &String| {
            filing
                .iter()
                .all(|(_, tags)| tags.iter().any(|known| tag_key(known) == tag_key(tag)))
        };
        let common: Vec<String> = (filing.first())
            .map(|(_, tags)| {
                tags.iter()
                    .filter(|tag| carried_by_all(tag))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let mut tags = common.clone();
        let known = self.editor.draft.tag_names();
        tags_row(ui, ("chosen tags", kind), &mut tags, &known);
        let added: Vec<String> = tags
            .iter()
            .filter(|tag| !common.contains(tag))
            .cloned()
            .collect();
        let removed: Vec<String> = common
            .iter()
            .filter(|tag| !tags.contains(tag))
            .cloned()
            .collect();
        if !added.is_empty() || !removed.is_empty() {
            self.tag_chosen(&added, &removed);
        }
    }
}
