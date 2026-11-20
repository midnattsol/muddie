//! Having back what was deleted in the editor.

use super::*;

/// What was last deleted in the editor, to have it back.
pub(in crate::tab) struct Undo {
    pub(in crate::tab) title: String,
    pub(in crate::tab) section: usize,
    pub(in crate::tab) index: usize,
    pub(in crate::tab) what: Undone,
    pub(in crate::tab) since: Instant,
}

/// What it takes to put back what was deleted.
pub(in crate::tab) enum Undone {
    /// The rules as they were: of all of them, as the list is one.
    Rules(crate::rules::Rules),
    Variable(String, serde_json::Value),
    Script(PathBuf, String),
    Nothing,
}

impl Tab {
    /// Puts back what was last deleted.
    pub(in crate::tab) fn undo_delete(&mut self) {
        let Some(undo) = self.editor.undo.take() else {
            return;
        };
        match undo.what {
            Undone::Rules(rules) => self.editor.draft = rules,
            Undone::Variable(name, value) => {
                if let Err(error) = self.scripts.set_variable(&name, Some(value)) {
                    self.note(&error);
                }
            }
            Undone::Script(path, code) => match std::fs::write(&path, &code) {
                Ok(()) => {
                    self.list_scripts();
                    self.scripts.load_file(&path);
                    self.run_actions();
                }
                Err(error) => self.note(&trf(
                    "Could not save the script: {0}",
                    &[&error.to_string()],
                )),
            },
            Undone::Nothing => return,
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
        if again || ui.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::Z)) {
            self.undo_delete();
        }
    }

    /// Keeps what is about to be deleted, to have it back for a while.
    pub(in crate::tab) fn keep_for_undo(&mut self, section: usize, index: usize) {
        let title = (self.automation_rows().get(index))
            .map(|row| row.0.clone())
            .unwrap_or_default();
        let what = match section {
            VARIABLES => match self.scripts.variables().get(index) {
                Some((name, value)) => Undone::Variable(name.clone(), value.clone()),
                None => Undone::Nothing,
            },
            SCRIPTS => {
                let path = self.editor.script_files[index].clone();
                let code = std::fs::read_to_string(&path).unwrap_or_default();
                Undone::Script(path, code)
            }
            _ => Undone::Rules(self.editor.draft.clone()),
        };
        self.editor.undo = Some(Undo {
            title,
            section,
            index,
            what,
            since: Instant::now(),
        });
    }

    /// Deletes an entry of a section: a rule, a gauge, which is taken off
    /// the screen too, a variable, or a script, with what it made.
    pub(in crate::tab) fn remove_entry(&mut self, section: usize, index: usize) {
        let draft = &mut self.editor.draft;
        match section {
            TRIGGERS => drop(draft.triggers.remove(index)),
            ALIASES => drop(draft.aliases.remove(index)),
            MACROS => drop(draft.keys.remove(index)),
            TIMERS => drop(draft.timers.remove(index)),
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
        self.list_scripts();
        self.scripts.unload_file(&path);
        self.run_actions();
    }
}
