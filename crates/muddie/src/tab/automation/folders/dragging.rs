//! Dragging in the list of rules and folders: where what is dragged would land,
//! and putting it there.

use super::*;

/// What is being dragged in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::tab) enum Dragged {
    Row(usize),
    Folder(String),
}

/// Where what is dragged would go.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Landing {
    Into(String),
    Before(usize),
    After(usize),
    /// Beside a folder, in the folder it is in: before it, or `after`.
    Beside(String, bool),
}

impl Landing {
    /// The folder what is dropped goes into.
    fn folder<'a>(&'a self, rows: &'a [Row]) -> &'a str {
        match self {
            Landing::Into(path) => path,
            Landing::Before(index) | Landing::After(index) => &rows[*index].folder,
            Landing::Beside(path, _) => parent_of(path),
        }
    }
}

/// How long what is dragged waits over a closed folder before it opens.
const SPRING: std::time::Duration = std::time::Duration::from_millis(700);
/// How near the edge of the list what is dragged scrolls it.
const EDGE: f32 = 28.0;

impl Tab {
    /// While something is dragged in the list, marks where it would go and
    /// says so by the pointer, opens a closed folder held over, scrolls the
    /// list at its edges, and puts it there once let go. Let go out of the
    /// list, or with Escape, it stays where it was. `top` is where it goes
    /// to the top level, shown while it is in a folder.
    pub(in crate::tab) fn drag_in_list(
        &mut self,
        ui: &egui::Ui,
        spots: &[(Spot, egui::Rect)],
        rows: &[Row],
        top: Option<egui::Rect>,
    ) {
        let Some(dragged) = self.editor.dragged.clone() else {
            return;
        };
        if ui.input(|input| input.key_pressed(Key::Escape)) {
            self.editor.dragged = None;
            return;
        }
        let kind = self.editor.section.kind();
        let list = ui.clip_rect();
        let Some(pointer) = ui.ctx().pointer_latest_pos() else {
            return;
        };
        // At the edges of the list, it scrolls.
        let scroll = match pointer.y {
            y if y < list.top() + EDGE && y >= list.top() - EDGE => 8.0,
            y if y > list.bottom() - EDGE && y <= list.bottom() + EDGE => -8.0,
            _ => 0.0,
        };
        if scroll != 0.0 {
            ui.scroll_with_delta(egui::vec2(0.0, scroll));
            ui.ctx().request_repaint();
        }
        let (landing, spring) = self.landing_at(ui, pointer, spots, rows, top);
        // A closed folder held over opens.
        match (&spring, &self.editor.drag_hover) {
            (Some(path), Some((held, since))) if held == path => {
                if since.elapsed() >= SPRING {
                    self.fold(kind.unwrap_or(Kind::Trigger), path, false);
                } else {
                    ui.ctx().request_repaint_after(SPRING - since.elapsed());
                }
            }
            (Some(path), _) => self.editor.drag_hover = Some((path.clone(), Instant::now())),
            (None, _) => self.editor.drag_hover = None,
        }
        let landing = landing.filter(|landing| self.drop_changes(&dragged, landing, rows));
        drop_hint(ui, landing.as_ref(), pointer, spots, rows);
        if ui.input(|input| input.pointer.any_down()) {
            return;
        }
        self.editor.dragged = None;
        self.editor.drag_hover = None;
        if let Some(landing) = landing {
            self.drop_dragged(dragged, landing, rows);
        }
    }

    /// Where what is dragged would land with the pointer at `pointer`:
    /// the line that marks it is drawn here. Returns it, and the closed
    /// folder the pointer is held over, if one.
    fn landing_at(
        &self,
        ui: &egui::Ui,
        pointer: egui::Pos2,
        spots: &[(Spot, egui::Rect)],
        rows: &[Row],
        top: Option<egui::Rect>,
    ) -> (Option<Landing>, Option<String>) {
        let kind = self.editor.section.kind();
        let list = ui.clip_rect();
        let accent = theme::current().accent;
        let line = |y: f32, rect: egui::Rect, depth: usize| {
            let left = rect.left() + widgets::INDENT * depth as f32;
            ui.painter()
                .hline(left..=rect.right(), y, egui::Stroke::new(2.0, accent));
        };
        // Only lines in sight: those scrolled away are under the toolbar.
        let over = (spots.iter())
            .find(|(_, rect)| rect.contains(pointer))
            .filter(|_| list.contains(pointer));
        let mut spring = None;
        let landing = match over {
            _ if top.is_some_and(|top| top.contains(pointer)) => Some(Landing::Into(String::new())),
            Some((Spot::Folder(path), rect)) if kind.is_some() => {
                let kind = kind.unwrap_or(Kind::Trigger);
                let open = self.folder_open(kind, path);
                let depth = path.matches('/').count();
                let band = rect.height() / 4.0;
                if pointer.y < rect.top() + band {
                    line(rect.top(), *rect, depth);
                    Some(Landing::Beside(path.clone(), false))
                } else if pointer.y > rect.bottom() - band && !open {
                    line(rect.bottom(), *rect, depth);
                    Some(Landing::Beside(path.clone(), true))
                } else {
                    if !open {
                        spring = Some(path.clone());
                    }
                    Some(Landing::Into(path.clone()))
                }
            }
            Some((Spot::Row(index), rect)) => {
                let before = pointer.y < rect.center().y;
                let folder = &rows[*index].folder;
                let depth = if folder.is_empty() {
                    0
                } else {
                    folder.matches('/').count() + 1
                };
                line(
                    if before { rect.top() } else { rect.bottom() },
                    *rect,
                    depth,
                );
                Some(match before {
                    true => Landing::Before(*index),
                    false => Landing::After(*index),
                })
            }
            // Under the last line, the top level; out of the list, nowhere.
            _ if list.contains(pointer) => Some(Landing::Into(String::new())),
            _ => None,
        };
        (landing, spring)
    }

    /// Whether letting what is dragged go there would change anything it
    /// can: a folder is not put into itself, nor where it is already.
    fn drop_changes(&self, dragged: &Dragged, landing: &Landing, rows: &[Row]) -> bool {
        let folder = landing.folder(rows);
        match dragged {
            Dragged::Folder(path) => match landing {
                Landing::Beside(beside, _) => beside != path && Self::folder_fits(path, folder),
                _ => Self::folder_fits(path, folder) && parent_of(path) != folder,
            },
            Dragged::Row(_) => true,
        }
    }

    /// Puts what was dragged where it was let go: into a folder, or before
    /// or after one of the list, in its folder.
    fn drop_dragged(&mut self, dragged: Dragged, drop: Landing, rows: &[Row]) {
        let section = self.editor.section;
        let folder = drop.folder(rows).to_owned();
        match (dragged, section.kind()) {
            (Dragged::Folder(path), Some(kind)) => {
                let beside = match &drop {
                    Landing::Beside(beside, after) => Some((beside.as_str(), *after)),
                    _ => None,
                };
                if let Some(now) = self.put_folder(kind, &path, &folder, beside) {
                    self.choose_folder(&now);
                }
            }
            (Dragged::Row(from), _) => {
                // What is chosen goes together, if the one dragged is of it.
                let chosen = self.chosen();
                let moving = match chosen.contains(&from) {
                    true => chosen,
                    false => vec![from],
                };
                self.editor.marked = if moving.len() > 1 {
                    moving.clone()
                } else {
                    Vec::new()
                };
                self.editor.selected[section] = from;
                let folder_differs = moving.iter().any(|&index| rows[index].folder != folder);
                if section.kind().is_some() && folder_differs {
                    self.file_chosen(&folder);
                }
                // Into a folder they keep their order; before or after one
                // of the list they take its place. Scripts are in the order
                // of their names.
                let to = match drop {
                    Landing::Before(index) => Some(index),
                    Landing::After(index) => Some(index + 1),
                    Landing::Into(_) | Landing::Beside(..) => None,
                };
                if let (Some(to), false) = (to, section == SCRIPTS) {
                    self.move_rows(&moving, to);
                }
            }
            (Dragged::Folder(_), None) => {}
        }
        // What was moved is seen where it went, in a folder that was
        // closed too.
        self.reveal_chosen();
    }
}

/// What dropping it there would do, by the pointer: the ring on the folder
/// it goes into, the cursor and what it says; nothing where it can not go.
fn drop_hint(
    ui: &egui::Ui,
    landing: Option<&Landing>,
    pointer: egui::Pos2,
    spots: &[(Spot, egui::Rect)],
    rows: &[Row],
) {
    match landing {
        Some(landing) => {
            if let Landing::Into(path) = landing
                && let Some((_, rect)) = spots
                    .iter()
                    .find(|(spot, _)| *spot == Spot::Folder(path.clone()))
            {
                widgets::target_ring(ui, *rect);
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let said = match landing.folder(rows) {
                "" => tr("Move to the top level").to_owned(),
                folder => trf("Move into {0}", &[name_of(folder)]),
            };
            egui::Area::new(egui::Id::unique("drop hint"))
                .order(egui::Order::Tooltip)
                .interactable(false)
                .fixed_pos(pointer + egui::vec2(16.0, 18.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(said));
                });
        }
        None => ui.ctx().set_cursor_icon(egui::CursorIcon::NotAllowed),
    }
}
