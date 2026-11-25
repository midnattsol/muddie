//! Moving a rule or a folder in the list with the keys, and saying where it went.

use super::*;

impl Tab {
    /// While the keyboard is on a line of the list, the arrows are the
    /// list's, not for going to what is beside it; and the line is brought
    /// into sight when it was gone to by the keys.
    pub(super) fn keep_keys(&mut self, ui: &mut egui::Ui, line: &egui::Response) {
        widgets::hold_arrows(ui, line);
        let here = self
            .current_spot()
            .is_some_and(|spot| item_id(self.editor.section, &spot) == line.id);
        if here && std::mem::take(&mut self.editor.reveal) {
            line.scroll_to_me(None);
        }
    }

    /// Alt and an arrow move the one chosen, for those who do not drag:
    /// up or down among those in its folder, a folder among the folders
    /// beside it; and Alt+Left out of the folder it is in, a level up,
    /// which is said.
    pub(super) fn moved_by_keys(&mut self, ui: &mut egui::Ui, rows: &[Row], folders: &[String]) {
        let section = self.editor.section;
        if self.narrowed() {
            return;
        }
        let (up, down, out) = ui.input_mut(|input| {
            (
                input.consume_key(crate::shortcuts::WITH_ARROWS, Key::ArrowUp),
                input.consume_key(crate::shortcuts::WITH_ARROWS, Key::ArrowDown),
                input.consume_key(crate::shortcuts::WITH_ARROWS, Key::ArrowLeft),
            )
        });
        if !(up || down || out) {
            return;
        }
        let kind = section.kind();
        if let (Some(path), Some(kind)) = (self.editor.folder.clone(), kind) {
            return self.folder_moved_by_keys(ui, kind, &path, folders, (up, down, out));
        }
        let from = self.editor.selected[section];
        let Some(chosen) = rows.get(from) else {
            return;
        };
        if out {
            if kind.is_some() && !chosen.folder.is_empty() {
                let to = parent_of(&chosen.folder).to_owned();
                self.file_chosen(&to);
                self.say_moved(&to);
            }
            return;
        }
        if !matches!(section, TRIGGERS | ALIASES | MACROS | TIMERS | GAUGES) {
            return;
        }
        let alike: Vec<usize> = (0..rows.len())
            .filter(|&index| rows[index].folder == chosen.folder)
            .collect();
        let at = alike.iter().position(|&index| index == from).unwrap_or(0);
        let (to, now) = match (up, down) {
            (true, _) if at > 0 => (alike[at - 1], at - 1),
            (_, true) if at + 1 < alike.len() => (alike[at + 1] + 1, at + 1),
            _ => return,
        };
        self.move_rows(&[from], to);
        self.say_place(up, now, alike.len());
    }

    /// Says where something moved up or down by the keys now is among those
    /// beside it, from 1: a screen reader is not told otherwise.
    pub(in crate::tab) fn say_place(&mut self, up: bool, at: usize, of: usize) {
        let (place, count) = ((at + 1).to_string(), of.to_string());
        let said = match up {
            true => trf("Moved up, {0} of {1}.", &[&place, &count]),
            false => trf("Moved down, {0} of {1}.", &[&place, &count]),
        };
        self.voice.say_now(&said);
    }

    /// A folder moved by the keys: up or down among the folders beside it,
    /// or out of the one it is in, put after it. The keyboard stays on it.
    fn folder_moved_by_keys(
        &mut self,
        ui: &mut egui::Ui,
        kind: Kind,
        path: &str,
        folders: &[String],
        (up, down, out): (bool, bool, bool),
    ) {
        let parent = parent_of(path).to_owned();
        let moved = if out {
            if parent.is_empty() {
                return;
            }
            self.put_folder(kind, path, parent_of(&parent), Some((&parent, true)))
        } else {
            let beside: Vec<&String> = (folders.iter())
                .filter(|folder| parent_of(folder) == parent)
                .collect();
            let Some(at) = beside.iter().position(|folder| *folder == path) else {
                return;
            };
            let (next, after, now) = match (up, down) {
                (true, _) if at > 0 => (beside[at - 1].clone(), false, at - 1),
                (_, true) if at + 1 < beside.len() => (beside[at + 1].clone(), true, at + 1),
                _ => return,
            };
            let count = beside.len();
            let moved = self.put_folder(kind, path, &parent, Some((&next, after)));
            if moved.is_some() {
                self.say_place(up, now, count);
            }
            moved
        };
        let Some(now) = moved else {
            return;
        };
        let section = self.editor.section;
        let had_keys = ui.memory(|memory| memory.focused())
            == Some(item_id(section, &Spot::Folder(path.to_owned())));
        self.choose_folder(&now);
        self.editor.reveal = true;
        if had_keys {
            let id = item_id(section, &Spot::Folder(now.clone()));
            ui.memory_mut(|memory| memory.request_focus(id));
        }
        if out {
            self.say_moved(parent_of(&now));
        }
    }
}
