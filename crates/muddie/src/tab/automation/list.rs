//! The list of a section of the editor: its folders and what is in them,
//! with the way to search it, narrow it to a tag, and add to it.

use super::*;
use crate::rules::Part;
use crate::rules::RuleList;

mod keys;
mod rows;
mod tree;

pub(in crate::tab) use rows::*;
pub(in crate::tab) use tree::*;

impl Tab {
    /// The list of the section chosen: the way to add to it and to search
    /// it, then its folders and what is in them, as a tree: one stop of Tab,
    /// on what is chosen, and the arrows through the rest.
    pub(in crate::tab) fn automation_list(&mut self, ui: &mut egui::Ui) {
        if self.editor.section == DATA {
            return self.data_list(ui);
        }
        let kind = self.editor.section.kind();
        // Found once a frame, for the toolbar and the tree: a folder made or
        // folded from the toolbar is shown the frame after.
        let folders = kind
            .map(|kind| self.section_folders(kind))
            .unwrap_or_default();
        let toolbar = ui.cursor().top();
        self.list_toolbar(ui, &folders);
        let toolbar =
            egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), toolbar..=ui.cursor().top());
        ui.add_space(4.0);
        let rows = self.automation_rows();
        let top = self.top_level_strip(ui, &rows, toolbar);
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
        self.moved_by_keys(ui, &rows, &folders);
        self.choosing_keys(ui, &items);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                widgets::keep_clear_of_bar(ui);
                ui.spacing_mut().item_spacing.y = widgets::LINE_GAP;
                if rows.is_empty() && folders.is_empty() && self.empty_list(ui) {
                    self.add_rule();
                }
                let spots = self.list_lines(ui, kind, &items, &rows);
                // What is under the last row takes a drop at the top level.
                let rest = ui.available_rect_before_wrap();
                ui.allocate_rect(rest, egui::Sense::hover());
                self.drag_in_list(ui, &spots, &rows, top);
            });
    }

    /// The lines of the list, in a tree where there are folders, else in a
    /// list, under the section's name. Returns where each was drawn.
    fn list_lines(
        &mut self,
        ui: &mut egui::Ui,
        kind: Option<Kind>,
        items: &[TreeItem],
        rows: &[Row],
    ) -> Vec<(Spot, egui::Rect)> {
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
            ui.spacing_mut().item_spacing.y = widgets::LINE_GAP;
            for item in items {
                let here = current.as_ref() == Some(&item.spot);
                let rect = match &item.spot {
                    Spot::Folder(path) => {
                        let renaming = (self.editor.renaming.as_ref())
                            .is_some_and(|named| named.path == *path);
                        match out_of_sight(ui, widgets::FOLDER_ROW).filter(|_| !renaming && !here) {
                            Some(rect) => rect,
                            None => self.folder_line(ui, kind.unwrap_or(Kind::Trigger), path, item),
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
        spots
    }

    /// While what is dragged is in a folder, a strip to let it go on, to
    /// take it out of every folder: over the toolbar, of no use while
    /// dragging, so that nothing in the list moves from under the pointer.
    /// Returns where it is.
    fn top_level_strip(
        &self,
        ui: &egui::Ui,
        rows: &[Row],
        toolbar: egui::Rect,
    ) -> Option<egui::Rect> {
        let nested = match &self.editor.dragged {
            Some(Dragged::Folder(path)) => !parent_of(path).is_empty(),
            Some(Dragged::Row(index)) => rows.get(*index).is_some_and(|row| !row.folder.is_empty()),
            None => false,
        };
        if !nested {
            return None;
        }
        let theme = theme::current();
        let rect = toolbar;
        let over = (ui.ctx().pointer_latest_pos()).is_some_and(|at| rect.contains(at));
        let fill = match over {
            true => theme::mix(theme.field, theme.accent, 0.18),
            false => theme.field,
        };
        let edge = if over { theme.accent } else { theme.border };
        let round = egui::CornerRadius::same(8);
        ui.painter().rect_filled(rect, round, fill);
        let stroke = egui::Stroke::new(1.0, edge);
        ui.painter()
            .rect_stroke(rect, round, stroke, egui::StrokeKind::Inside);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            tr("Move to the top level"),
            theme::font(Weight::Medium, 12.5),
            theme.text_dim,
        );
        Some(rect)
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
        let tree = section.kind().is_some();
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
        // The menu acts on it, or on all that is chosen if it is among
        // them, and does not choose it: it is ringed while the menu is open.
        if widgets::menu_pressed(&line) {
            self.editor.menu_target = (!chosen).then_some(Spot::Row(index));
        }
        let open = crate::widgets::context_menu_of(&line, |ui| self.row_menu(ui));
        self.ring_menu_target(ui, &line, Spot::Row(index), open.is_some());
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

    /// The button that adds one, which for scripts offers to start from an
    /// example, the one that makes a folder, and the field to search.
    fn list_toolbar(&mut self, ui: &mut egui::Ui, folders: &[String]) {
        ui.horizontal(|ui| {
            let add = widgets::primary_button(ui, "+");
            ui.ctx()
                .accesskit_node_builder(add.id, |node| node.set_label(tr("Add")));
            let add = widgets::name_tip(add, tr("Add"));
            if self.editor.section == SCRIPTS {
                crate::widgets::menu_of(&add).show(|ui| self.new_script_menu(ui));
            } else if add.clicked() {
                self.add_rule();
            }
            if let Some(kind) = self.editor.section.kind() {
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
        let index = match section
            .kind()
            .and_then(|kind| self.editor.draft.list_mut(kind))
        {
            Some(list) => list.add(&folder),
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
