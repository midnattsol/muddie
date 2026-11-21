//! Having back what was deleted in the editor.

use super::*;

/// What was last deleted in the editor, to have it back.
pub(in crate::tab) struct Undo {
    /// What it was called, or how many there were.
    pub(in crate::tab) title: String,
    pub(in crate::tab) section: usize,
    pub(in crate::tab) index: usize,
    pub(in crate::tab) what: Undone,
    pub(in crate::tab) since: Instant,
}

/// What it takes to put back what was deleted.
pub(in crate::tab) enum Undone {
    /// A step of the editor's history, which undoes it: the bar is shown
    /// while the history has this many steps.
    Step(usize),
    Variables(Vec<(String, serde_json::Value)>),
    Scripts(Vec<(PathBuf, String)>),
    /// A folder of scripts, by its path, and the names of what it held,
    /// which went to the folder it was in.
    ScriptFolder(String, Vec<String>),
}

impl Tab {
    /// Puts back what was last deleted.
    pub(in crate::tab) fn undo_delete(&mut self) {
        let Some(undo) = self.editor.undo.take() else {
            return;
        };
        match undo.what {
            Undone::Step(_) => return self.undo_step(false),
            Undone::Variables(variables) => {
                for (name, value) in variables {
                    if let Err(error) = self.scripts.set_variable(&name, Some(value)) {
                        self.note(&error);
                    }
                }
            }
            Undone::Scripts(scripts) => {
                for (path, code) in scripts {
                    let written = (path.parent().map_or(Ok(()), std::fs::create_dir_all))
                        .and_then(|()| std::fs::write(&path, &code));
                    match written {
                        Ok(()) => self.scripts.load_file(&path),
                        Err(error) => self.note(&trf(
                            "Could not save the script: {0}",
                            &[&error.to_string()],
                        )),
                    }
                }
                self.list_scripts();
                self.run_actions();
            }
            Undone::ScriptFolder(path, held) => {
                let made =
                    (self.own_scripts_dir()).map(|dir| std::fs::create_dir_all(dir.join(&path)));
                if let Some(Err(error)) = made {
                    self.note(&trf(
                        "Could not make the folder: {0}",
                        &[&error.to_string()],
                    ));
                }
                let parent = list::parent_of(&path).to_owned();
                for name in held {
                    let from = crate::rules::joined(&parent, &name);
                    self.move_script_files(&from, &crate::rules::joined(&path, &name));
                }
                self.list_scripts();
                self.open_automation(undo.section);
                return self.choose_folder(&path);
            }
        }
        self.open_automation(undo.section);
        self.editor.selected[undo.section] = undo.index;
    }

    /// The bar that says what was deleted, with the way to have it back,
    /// for a few seconds after.
    pub(in crate::tab) fn undo_bar(&mut self, ui: &mut egui::Ui) {
        const LASTS: std::time::Duration = std::time::Duration::from_secs(10);
        let Some(undo) = &self.editor.undo else {
            return;
        };
        let left = LASTS.saturating_sub(undo.since.elapsed());
        if left.is_zero() {
            self.editor.undo = None;
            return;
        }
        ui.ctx().request_repaint_after(left);
        let theme = theme::current();
        let said = match undo.title.trim().is_empty() {
            true => tr("Deleted.").to_owned(),
            false => trf("Deleted {0}.", &[undo.title.trim()]),
        };
        let mut again = false;
        egui::Frame::new()
            .fill(theme.raised)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(said).color(theme.text));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        again = widgets::button(ui, tr("Undo")).clicked();
                    });
                });
            });
        ui.add_space(8.0);
        // Ctrl+Z in a field undoes what is typed there; and the history's
        // steps are undone by the keys of the page.
        let disk = !matches!(undo.what, Undone::Step(_));
        let keys = disk
            && automation::typing_in(ui.ctx()).is_none()
            && ui.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::Z));
        if again || keys {
            self.undo_delete();
        }
    }

    /// Deletes what is chosen in a section, all at once and to be had
    /// back at once: rules, gauges, which are taken off the screen too,
    /// variables, or scripts, with what they made.
    pub(in crate::tab) fn delete_chosen(&mut self) {
        self.delete_rows(&self.chosen());
    }

    /// Deletes the things at `rows`, to be had back at once. What was
    /// chosen stays chosen, where it now is, unless it was deleted: then
    /// the one before the first deleted is.
    pub(in crate::tab) fn delete_rows(&mut self, rows: &[usize]) {
        let section = self.editor.section;
        let mut rows = rows.to_vec();
        rows.sort_unstable();
        let Some(&first) = rows.first() else {
            return;
        };
        self.keep_for_undo(section, &rows);
        // From the last, so that those before keep their places.
        for &index in rows.iter().rev() {
            self.remove_entry(section, index);
        }
        // Those after one deleted are a place nearer the start.
        let shifted = |index: usize| index - rows.iter().filter(|&&row| row < index).count();
        let selected = self.editor.selected[section];
        self.editor.selected[section] = match rows.contains(&selected) {
            true => first.saturating_sub(1),
            false => shifted(selected),
        };
        let marked: Vec<usize> = (self.editor.marked.iter())
            .filter(|index| !rows.contains(index))
            .map(|&index| shifted(index))
            .collect();
        self.editor.marked = if marked.len() > 1 { marked } else { Vec::new() };
        self.editor.recording_key = false;
    }

    /// Keeps what is about to be deleted, to have it back for a while.
    fn keep_for_undo(&mut self, section: usize, indices: &[usize]) {
        let rows = self.automation_rows();
        let title = match indices {
            [index] => (rows.get(*index))
                .map(|row| row.title.clone())
                .unwrap_or_default(),
            _ => trn("{0} item", "{0} items", indices.len() as u64),
        };
        let what = match section {
            VARIABLES => {
                let variables = self.scripts.variables();
                let kept = indices
                    .iter()
                    .filter_map(|&index| variables.get(index).cloned());
                Undone::Variables(kept.collect())
            }
            SCRIPTS => Undone::Scripts(
                (indices.iter())
                    .filter_map(|&index| self.editor.script_files.get(index))
                    .map(|path| {
                        (
                            path.clone(),
                            std::fs::read_to_string(path).unwrap_or_default(),
                        )
                    })
                    .collect(),
            ),
            // The deletion is the history's next step.
            _ => Undone::Step(self.editor.history.len() + 1),
        };
        self.editor.undo = Some(Undo {
            title,
            section,
            index: indices[0],
            what,
            since: Instant::now(),
        });
    }

    /// Deletes one entry of a section.
    fn remove_entry(&mut self, section: usize, index: usize) {
        let draft = &mut self.editor.draft;
        if let Some(list) = section_kind(section).and_then(|kind| draft.list_mut(kind)) {
            return list.remove(index);
        }
        match section {
            GAUGES => {
                let gone = draft.gauges.remove(index);
                self.gauges.set(gone.shown_name(), None);
            }
            VARIABLES => self.forget_variable(index),
            _ => self.delete_script(index),
        }
    }

    /// Forgets a variable.
    fn forget_variable(&mut self, index: usize) {
        if let Some((name, _)) = self.scripts.variables().get(index)
            && let Err(error) = self.scripts.set_variable(name, None)
        {
            self.note(&error);
        }
        self.editor.variable_loaded = None;
    }

    /// Deletes a script's file; what it made goes with it, the others stay.
    fn delete_script(&mut self, index: usize) {
        let path = self.editor.script_files[index].clone();
        self.editor.script_loaded = None;
        self.editor.script_settle.clear();
        if let Err(error) = std::fs::remove_file(&path) {
            self.note(&trf(
                "Could not delete the script: {0}",
                &[&error.to_string()],
            ));
        }
        self.scripts.unload_file(&path);
        self.run_actions();
    }
}
