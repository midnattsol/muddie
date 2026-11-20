//! Where a thing is filed in the editor: its folder and its tags, as its
//! details show them, and the tags there are, beside the sections.

use super::*;
use list::parent_of;

impl Tab {
    /// The heading of a rule's details, the same for every kind: its name,
    /// its switch and the button that deletes it, then its folder and its
    /// tags, and why it does not run though it is switched on, if it does
    /// not. Returns whether deleting was asked.
    pub(in crate::tab) fn rule_head(
        &mut self,
        ui: &mut egui::Ui,
        kind: Kind,
        index: usize,
        hint: &str,
    ) -> bool {
        let folders = self.section_folders(kind);
        let known = self.editor.draft.tag_names();
        let rename = &mut self.editor.rename;
        let Some(list) = self.editor.draft.list_mut(kind) else {
            return false;
        };
        let head = list.head_mut(index);
        let delete = rule_header(ui, head.name, head.enabled, hint, rename);
        folder_row(ui, head.folder, &folders, false);
        tags_row(ui, ("rule tags", kind, index), head.tags, &known);
        let (on, folder, tags) = (*head.enabled, head.folder.clone(), head.tags.clone());
        if on && let Some(reason) = self.off_reason(kind, &folder, &tags) {
            note(ui, &reason);
        }
        delete
    }

    /// Why something switched on does not run, as saved: a folder it is in
    /// or a tag it carries is switched off.
    pub(in crate::tab) fn off_reason(
        &self,
        kind: Kind,
        folder: &str,
        tags: &[String],
    ) -> Option<String> {
        let draft = &self.editor.draft;
        if !folder.trim().is_empty()
            && let Some(off) =
                muddie_script::places_up(folder).find(|path| !draft.folder_on(kind, path))
        {
            return Some(trf("Does not run: the folder {0} is switched off.", &[off]));
        }
        let off = tags.iter().find(|tag| !draft.tag_on(tag))?;
        Some(trf(
            "Does not run: the tag {0} is switched off.",
            &[off.trim()],
        ))
    }

    /// The tags there are, under the sections: each with its switch, which
    /// is saved, and how many things carry it. Choosing one narrows every
    /// list to what carries it.
    pub(in crate::tab) fn sidebar_tags(&mut self, ui: &mut egui::Ui) {
        let names = self.editor.draft.tag_names();
        if (self.editor.tag_filter.as_deref())
            .is_some_and(|chosen| !names.iter().any(|name| tag_key(name) == tag_key(chosen)))
        {
            self.editor.tag_filter = None;
        }
        if names.is_empty() {
            return;
        }
        let theme = theme::current();
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            widgets::caption(ui, tr("Tags"));
        });
        ui.add_space(2.0);
        let carrying = self.carrying();
        for name in names {
            let chosen = (self.editor.tag_filter.as_deref())
                .is_some_and(|chosen| tag_key(chosen) == tag_key(&name));
            let mut on = self.editor.draft.tag_on(&name);
            let (rect, row) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
            row.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::Tab, true, chosen, name.as_str())
            });
            let fill = match (chosen, row.hovered()) {
                (true, _) => theme.raised,
                (false, true) => theme.raised.gamma_multiply(0.6),
                (false, false) => egui::Color32::TRANSPARENT,
            };
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(7), fill);
            let text = ui
                .painter()
                .with_clip_rect(rect.with_max_x(rect.right() - 76.0));
            let colour = match (chosen, on) {
                (true, _) => theme.text,
                (false, true) => theme.text_dim,
                (false, false) => theme.text_faint,
            };
            text.text(
                rect.left_center() + egui::vec2(12.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &name,
                theme::font(Weight::Regular, 13.5),
                colour,
            );
            ui.painter().text(
                rect.right_center() - egui::vec2(56.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                carrying
                    .get(&tag_key(&name))
                    .copied()
                    .unwrap_or(0)
                    .to_string(),
                theme::font(Weight::Regular, 12.0),
                theme.text_faint,
            );
            if row.clicked() {
                self.editor.tag_filter = (!chosen).then(|| name.clone());
            }
            // The switch, over the right end of the row.
            let place = egui::Rect::from_center_size(
                rect.right_center() - egui::vec2(25.0, 0.0),
                egui::vec2(34.0, 20.0),
            );
            let mut slot = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("tag switch", name.as_str()))
                    .max_rect(place),
            );
            if widgets::switch(&mut slot, &mut on, &name).changed() {
                self.editor.draft.set_tag_on(&name, on);
            }
        }
    }

    /// How many things carry each tag, of every kind, by the tag as it is
    /// compared: worked out in one pass.
    fn carrying(&self) -> std::collections::HashMap<String, usize> {
        let draft = &self.editor.draft;
        let mut counts = std::collections::HashMap::new();
        let mut count = |tags: &[String]| {
            let mut seen: Vec<String> = tags.iter().map(|tag| tag_key(tag)).collect();
            seen.sort();
            seen.dedup();
            for tag in seen {
                *counts.entry(tag).or_insert(0) += 1;
            }
        };
        for list in Kind::ALL.iter().filter_map(|&kind| draft.list(kind)) {
            (0..list.len()).for_each(|index| count(list.tags(index)));
        }
        draft.scripts.iter().for_each(|script| count(&script.tags));
        counts
    }
}

/// The folder something is in, chosen among those of its kind. Of several
/// things in more than one folder, it says so. Returns whether one was
/// chosen.
pub(in crate::tab) fn folder_row(
    ui: &mut egui::Ui,
    folder: &mut String,
    folders: &[String],
    several: bool,
) -> bool {
    let mut picked = false;
    filing_line(ui, tr("Folder"), |ui| {
        let shown = match (several, folder.trim()) {
            (true, _) => tr("Several folders").to_owned(),
            (false, "") => tr("None").to_owned(),
            (false, path) => path.replace('/', " › "),
        };
        widgets::dropdown("folder")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                if widgets::choice(ui, !several && folder.trim().is_empty(), tr("None")).clicked() {
                    folder.clear();
                    picked = true;
                }
                for path in folders {
                    let depth = path.matches('/').count();
                    let name = path.rsplit('/').next().unwrap_or(path);
                    let label = format!("{}{name}", "    ".repeat(depth));
                    if widgets::choice(ui, !several && folder == path, &label).clicked() {
                        folder.clone_from(path);
                        picked = true;
                    }
                }
            });
    });
    picked
}

/// The tags something carries.
pub(in crate::tab) fn tags_row(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    tags: &mut Vec<String>,
    known: &[String],
) {
    filing_line(ui, tr("Tags"), |ui| {
        ui.vertical(|ui| widgets::tag_field(ui, id, tags, known));
    });
}

/// A line of where a thing is filed: what it is at the left, as wide for
/// every line so that they line up, and the control.
fn filing_line(ui: &mut egui::Ui, label: &str, control: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(64.0, crate::theme::size::CONTROL),
            egui::Sense::hover(),
        );
        ui.painter().text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            theme::font(Weight::Regular, 13.5),
            theme::current().text_dim,
        );
        control(ui);
    });
}

/// The folder a place is in, for a script.
pub(in crate::tab) fn folder_of(place: &str) -> String {
    parent_of(place).to_owned()
}
