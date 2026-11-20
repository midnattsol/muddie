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
        if let Some(list) = section_kind(section).and_then(|kind| self.editor.draft.list(kind)) {
            return list.len();
        }
        match section {
            GAUGES => self.editor.draft.gauges.len(),
            VARIABLES => self.scripts.variable_count(),
            SCRIPTS => self.editor.script_files.len(),
            _ => 0,
        }
    }

    /// What is chosen in the current section, in order: those marked, or
    /// the one selected.
    pub(in crate::tab) fn chosen(&self) -> Vec<usize> {
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
        let section = self.editor.section;
        let anchor = self.editor.selected[section];
        let marked = &mut self.editor.marked;
        if modifiers.shift {
            let visible = &self.editor.visible;
            let at = |row: usize| visible.iter().position(|&shown| shown == row);
            if let (Some(from), Some(to)) = (at(anchor), at(index)) {
                *marked = visible[from.min(to)..=from.max(to)].to_vec();
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

    /// The keys that choose, while no field has them: Ctrl+A chooses all
    /// that is shown, Shift and an arrow add the one above or below what
    /// is chosen, and Escape leaves the one selected.
    pub(in crate::tab) fn choosing_keys(&mut self, ui: &mut egui::Ui) {
        if ui.memory(|memory| memory.focused().is_some()) {
            return;
        }
        let (all, up, down, back) = ui.input_mut(|input| {
            (
                input.consume_key(Modifiers::COMMAND, Key::A),
                input.consume_key(Modifiers::SHIFT, Key::ArrowUp),
                input.consume_key(Modifiers::SHIFT, Key::ArrowDown),
                self.editor.marked.len() > 1 && input.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        let visible = self.editor.visible.clone();
        if all && visible.len() > 1 {
            self.editor.marked = visible.clone();
        }
        if back {
            self.editor.marked.clear();
        }
        let chosen = self.chosen();
        let at = |row: usize| visible.iter().position(|&shown| shown == row);
        let ends = (chosen.iter().filter_map(|&row| at(row)).min())
            .zip(chosen.iter().filter_map(|&row| at(row)).max());
        let added = match (ends, up, down) {
            (Some((first, _)), true, _) if first > 0 => Some(visible[first - 1]),
            (Some((_, last)), _, true) => visible.get(last + 1).copied(),
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
        match section_kind(section).and_then(|kind| self.editor.draft.list_mut(kind)) {
            Some(list) => list.reorder(&order),
            None if section == GAUGES => {
                crate::rules::reorder(&mut self.editor.draft.gauges, &order)
            }
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
            GAUGES => draft.gauges.get(index).is_some_and(|gauge| gauge.enabled),
            SCRIPTS => self.script_entry(index).is_some_and(|entry| entry.enabled),
            section => (section_kind(section).and_then(|kind| draft.list(kind)))
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
        for index in self.chosen() {
            match self.editor.section {
                GAUGES => self.editor.draft.gauges[index].enabled = on,
                SCRIPTS => {
                    if let Some(mut entry) = self.script_entry(index) {
                        entry.enabled = on;
                        self.editor.draft.set_script(entry);
                    }
                }
                section => {
                    if let Some(list) =
                        section_kind(section).and_then(|kind| self.editor.draft.list_mut(kind))
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
        let chosen = self.chosen();
        let section = self.editor.section;
        if section == SCRIPTS {
            let places: Vec<String> = (chosen.iter())
                .filter_map(|&index| self.editor.script_files.get(index))
                .map(|file| self.script_place(file))
                .collect();
            for place in places {
                let name = place.rsplit('/').next().unwrap_or(&place).to_owned();
                if list::parent_of(&place) != folder {
                    self.move_script_files(&place, &joined(folder, &name));
                }
            }
            self.editor.marked.clear();
            return;
        }
        if let Some(list) = section_kind(section).and_then(|kind| self.editor.draft.list_mut(kind))
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
                    if let Some(list) =
                        section_kind(section).and_then(|kind| self.editor.draft.list_mut(kind))
                    {
                        retag(list.head_mut(index).tags);
                    }
                }
            }
        }
    }

    /// What can be done with what is chosen, from the menu of a row:
    /// switching it, putting it in a folder, deleting it.
    pub(in crate::tab) fn row_menu(&mut self, ui: &mut egui::Ui) {
        widgets::menu_width(ui);
        let chosen = self.chosen();
        let switchable = self.editor.section != VARIABLES;
        if switchable {
            let on = self.chosen_on(&chosen);
            let label = if on { tr("Disable") } else { tr("Enable") };
            if widgets::menu_item(ui, label, "").clicked() {
                self.switch_chosen(!on);
                ui.close();
            }
        }
        if let Some(kind) = section_kind(self.editor.section) {
            widgets::menu_separator(ui);
            widgets::menu_heading(ui, tr("Move to folder"));
            if widgets::menu_item(ui, tr("None"), "").clicked() {
                self.file_chosen("");
                ui.close();
            }
            for path in self.section_folders(kind) {
                let depth = path.matches('/').count();
                let name = path.rsplit('/').next().unwrap_or(&path);
                let label = format!("{}{name}", "    ".repeat(depth));
                if widgets::menu_item(ui, &label, "").clicked() {
                    self.file_chosen(&path);
                    ui.close();
                }
            }
        }
        widgets::menu_separator(ui);
        if widgets::menu_item(ui, tr("Delete"), "").clicked() {
            self.delete_chosen();
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
        if let Some(kind) = section_kind(section) {
            self.bulk_filing(ui, kind, &chosen);
        }
        note(ui, tr("What is changed here is changed in all of them."));
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
