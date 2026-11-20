//! The folders of the editor's lists: making them, naming them, taking
//! them away, and putting into them what is dragged. Those of scripts are
//! folders on the disk; the others are kept with the rules.

use super::*;
use crate::rules::joined;
use list::{Row, Spot, parent_of};

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
}

/// A folder being named: where it is, and its name as typed.
pub(in crate::tab) struct Renaming {
    pub(in crate::tab) path: String,
    pub(in crate::tab) typed: String,
}

impl Tab {
    /// The folders of a kind, in their order. Those of scripts are those
    /// on the disk.
    pub(in crate::tab) fn section_folders(&self, kind: Kind) -> Vec<String> {
        let kept = self.editor.draft.folder_paths(kind);
        if kind != Kind::Script {
            return kept;
        }
        let on_disk = match (&self.profiles, &self.profile) {
            (Some(profiles), Some(profile)) => profiles.script_folders(profile),
            _ => Vec::new(),
        };
        let mut paths: Vec<String> = kept
            .into_iter()
            .filter(|path| on_disk.contains(path))
            .collect();
        paths.extend(
            on_disk
                .into_iter()
                .filter(|path| !paths.contains(path))
                .collect::<Vec<_>>(),
        );
        paths
    }

    /// Makes a folder in `parent`, with those above it open, and starts
    /// naming it.
    pub(in crate::tab) fn new_folder(&mut self, kind: Kind, parent: &str) {
        let taken = self.section_folders(kind);
        let base = tr("New folder");
        let name = (1..100)
            .map(|n| match n {
                1 => base.to_owned(),
                n => format!("{base} {n}"),
            })
            .find(|name| !taken.contains(&joined(parent, name)))
            .unwrap_or_else(|| base.to_owned());
        let path = joined(parent, &name);
        if kind == Kind::Script {
            let Some(dir) = self.own_scripts_dir() else {
                return self.note(tr(
                    "Scripts of your own are kept per profile: save or choose a profile first.",
                ));
            };
            if let Err(error) = std::fs::create_dir_all(dir.join(&path)) {
                return self.note(&trf(
                    "Could not make the folder: {0}",
                    &[&error.to_string()],
                ));
            }
        } else {
            self.editor.draft.set_folder_on(kind, &path, true);
        }
        let inside = |folded: &(Kind, String)| {
            folded.0 == kind && (path == folded.1 || path.starts_with(&format!("{}/", folded.1)))
        };
        self.editor.folded.retain(|folded| !inside(folded));
        self.editor.renaming = Some(Renaming { path, typed: name });
    }

    /// Starts typing a folder's name.
    pub(in crate::tab) fn start_renaming(&mut self, path: &str) {
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        self.editor.renaming = Some(Renaming {
            path: path.to_owned(),
            typed: name,
        });
    }

    /// The field a folder's name is typed in, in place of its line: Enter
    /// or leaving it names the folder so, Escape leaves it as it was.
    /// Returns where it is.
    pub(in crate::tab) fn folder_name_field(
        &mut self,
        ui: &mut egui::Ui,
        kind: Kind,
        depth: usize,
    ) -> egui::Rect {
        let Some(renaming) = &mut self.editor.renaming else {
            return egui::Rect::NOTHING;
        };
        let shown = ui.horizontal(|ui| {
            ui.add_space(widgets::INDENT * depth as f32 + 40.0);
            let field = ui.add(
                TextEdit::singleline(&mut renaming.typed)
                    .margin(egui::Margin::symmetric(8, 5))
                    .desired_width(f32::INFINITY),
            );
            ui.ctx()
                .accesskit_node_builder(field.id, |node| node.set_label(tr("Folder name")));
            if !field.has_focus() && !field.lost_focus() {
                field.request_focus();
            }
            field
        });
        let field = shown.inner;
        let cancel = ui.input(|input| input.key_pressed(Key::Escape));
        if cancel {
            self.editor.renaming = None;
        } else if field.lost_focus() {
            let renaming = self.editor.renaming.take();
            if let Some(Renaming { path, typed }) = renaming {
                let name = typed.trim();
                if !name.is_empty()
                    && !name.contains('/')
                    && name != path.rsplit('/').next().unwrap_or("")
                {
                    self.move_folder(kind, &path, &joined(parent_of(&path), name));
                }
            }
        }
        shown.response.rect
    }

    /// Gives a folder, with all it holds, another place: another name, or
    /// another folder it is in. Refused into itself or where there is one
    /// already. Returns whether it moved.
    pub(in crate::tab) fn move_folder(&mut self, kind: Kind, from: &str, to: &str) -> bool {
        if from == to || to.starts_with(&format!("{from}/")) {
            return false;
        }
        if self.section_folders(kind).iter().any(|path| path == to) {
            self.note(&trf("There is a folder called {0} already.", &[to]));
            return false;
        }
        if kind == Kind::Script {
            if !self.move_script_files(from, to) {
                return false;
            }
        } else {
            self.editor.draft.move_folder(kind, from, to);
        }
        for (folded_kind, path) in &mut self.editor.folded {
            if *folded_kind == kind {
                match path.strip_prefix(from) {
                    Some("") => *path = to.to_owned(),
                    Some(rest) if rest.starts_with('/') => *path = format!("{to}{rest}"),
                    _ => {}
                }
            }
        }
        true
    }

    /// Takes a folder away; what it held goes to the folder it was in.
    pub(in crate::tab) fn remove_folder(&mut self, kind: Kind, path: &str) {
        if kind != Kind::Script {
            return self.editor.draft.remove_folder(kind, path);
        }
        let Some(dir) = self.own_scripts_dir() else {
            return;
        };
        let held: Vec<String> = (std::fs::read_dir(dir.join(path))
            .into_iter()
            .flatten()
            .flatten())
        .map(|entry| {
            let path = entry.path();
            let name = match path.is_dir() {
                true => path.file_name(),
                false => path.file_stem(),
            };
            name.unwrap_or_default().to_string_lossy().into_owned()
        })
        .collect();
        for name in held {
            if !self.move_script_files(&joined(path, &name), &joined(parent_of(path), &name)) {
                return;
            }
        }
        if let Err(error) = std::fs::remove_dir(dir.join(path)) {
            return self.note(&trf(
                "Could not delete the folder: {0}",
                &[&error.to_string()],
            ));
        }
        self.editor.draft.remove_folder(Kind::Script, path);
        self.rules.remove_folder(Kind::Script, path);
        self.list_scripts();
    }

    /// Moves a script or a folder of scripts on the disk, from one place
    /// among them to another, with how they are kept and what they made:
    /// those that were running run from where they are now. Returns
    /// whether it moved.
    pub(in crate::tab) fn move_script_files(&mut self, from: &str, to: &str) -> bool {
        let Some(dir) = self.own_scripts_dir() else {
            return false;
        };
        let (source, target) = match dir.join(from).is_dir() {
            true => (dir.join(from), dir.join(to)),
            false => (
                dir.join(format!("{from}.luau")),
                dir.join(format!("{to}.luau")),
            ),
        };
        if target.exists() {
            self.note(&trf("There is one called {0} already.", &[to]));
            return false;
        }
        self.flush_script();
        let running: Vec<PathBuf> = (self.own_scripts().into_iter())
            .map(|(file, _)| file)
            .filter(|file| file.starts_with(&source) && self.scripts.is_loaded(file))
            .collect();
        let moved = (target.parent().map_or(Ok(()), std::fs::create_dir_all))
            .and_then(|()| std::fs::rename(&source, &target));
        if let Err(error) = moved {
            self.note(&trf("Could not move it: {0}", &[&error.to_string()]));
            return false;
        }
        let now = |file: &std::path::Path| match file.strip_prefix(&source) {
            Ok(inner) if !inner.as_os_str().is_empty() => target.join(inner),
            _ => target.clone(),
        };
        for file in running {
            self.scripts.unload_file(&file);
            self.scripts.load_file(&now(&file));
        }
        if let Some(loaded) = self.editor.script_loaded.clone()
            && loaded.starts_with(&source)
        {
            self.editor.script_loaded = Some(now(&loaded));
        }
        for rules in [&mut self.editor.draft, &mut self.rules] {
            rules.move_scripts(from, to);
        }
        self.list_scripts();
        self.run_actions();
        true
    }

    /// While something is dragged in the list, marks where it would go, and
    /// puts it there once let go.
    pub(in crate::tab) fn drag_in_list(
        &mut self,
        ui: &egui::Ui,
        spots: &[(Spot, egui::Rect)],
        rows: &[Row],
    ) {
        let Some(dragged) = self.editor.dragged.clone() else {
            return;
        };
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let foldered = section_kind(self.editor.section).is_some();
        let pointer = ui.ctx().pointer_latest_pos();
        let over = pointer.and_then(|pointer| {
            let spot = spots
                .iter()
                .find(|(_, rect)| rect.y_range().contains(pointer.y))?;
            Some((spot, pointer))
        });
        let accent = theme::current().accent;
        let drop = match over {
            Some(((Spot::Folder(path), rect), _)) if foldered => {
                let rect = *rect;
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(8),
                    egui::Stroke::new(2.0, accent),
                    egui::StrokeKind::Inside,
                );
                Landing::Into(path.clone())
            }
            Some(((Spot::Row(index), rect), pointer)) => {
                let before = pointer.y < rect.center().y;
                let y = if before { rect.top() } else { rect.bottom() };
                ui.painter()
                    .hline(rect.x_range(), y, egui::Stroke::new(2.0, accent));
                match before {
                    true => Landing::Before(*index),
                    false => Landing::After(*index),
                }
            }
            _ => Landing::Into(String::new()),
        };
        if ui.input(|input| input.pointer.any_down()) {
            return;
        }
        self.editor.dragged = None;
        self.drop_dragged(dragged, drop, rows);
    }

    /// Puts what was dragged where it was let go: into a folder, or before
    /// or after one of the list, in its folder.
    fn drop_dragged(&mut self, dragged: Dragged, drop: Landing, rows: &[Row]) {
        let section = self.editor.section;
        let folder = match &drop {
            Landing::Into(path) => path.clone(),
            Landing::Before(index) | Landing::After(index) => rows[*index].folder.clone(),
        };
        let name = |place: &str| place.rsplit('/').next().unwrap_or(place).to_owned();
        match (dragged, section_kind(section)) {
            (Dragged::Folder(path), Some(kind)) => {
                if parent_of(&path) != folder {
                    self.move_folder(kind, &path, &joined(&folder, &name(&path)));
                }
            }
            (Dragged::Row(from), Some(Kind::Script)) => {
                let place = self.script_place(&self.editor.script_files[from].clone());
                if rows[from].folder != folder {
                    self.move_script_files(&place, &joined(&folder, &name(&place)));
                }
            }
            (Dragged::Folder(_), None) => {}
            (Dragged::Row(from), kind) => {
                if let Some(list) = kind.and_then(|kind| self.editor.draft.list_mut(kind)) {
                    *list.head_mut(from).folder = folder.clone();
                }
                let to = match drop {
                    Landing::Before(index) => index,
                    Landing::After(index) => index + 1,
                    Landing::Into(_) => rows.len(),
                };
                self.editor.selected[section] = from;
                self.shift_row(from, to);
            }
        }
    }
}
