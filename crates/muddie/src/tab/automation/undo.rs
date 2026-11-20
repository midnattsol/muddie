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
}
