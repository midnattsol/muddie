//! The folders of the editor's lists: making them, naming them, taking
//! them away, and putting into them what is dragged. Those of scripts are
//! folders on the disk; the others are kept with the rules.

use super::*;
use crate::rules::joined;
use list::{Row, Spot, parent_of};

mod dragging;

pub(in crate::tab) use dragging::Dragged;
/// A folder being named: where it is, its name as typed, and whether the
/// field is yet to be given the keys.
pub(in crate::tab) struct Renaming {
    pub(in crate::tab) path: String,
    pub(in crate::tab) typed: String,
    pub(in crate::tab) fresh: bool,
    /// It is chosen once named: a folder made, or one that was chosen.
    pub(in crate::tab) choose: bool,
}

impl Tab {
    /// The folders of a kind, in their order. Those of scripts are those
    /// on the disk.
    pub(in crate::tab) fn section_folders(&self, kind: Kind) -> Vec<String> {
        let kept = self.editor.draft.folder_paths(kind);
        if kind != Kind::Script {
            return kept;
        }
        let on_disk = self.editor.script_folders.clone();
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
            self.list_scripts();
        } else {
            self.editor.draft.set_folder_on(kind, &path, true);
        }
        // It and those it is in are open, for it to be seen and named.
        self.open_to(kind, &path);
        self.editor.renaming = Some(Renaming {
            path,
            typed: name,
            fresh: true,
            choose: true,
        });
    }

    /// Starts typing a folder's name.
    pub(in crate::tab) fn start_renaming(&mut self, path: &str) {
        let name = name_of(path).to_owned();
        self.editor.renaming = Some(Renaming {
            path: path.to_owned(),
            typed: name,
            fresh: true,
            choose: self.editor.folder.as_deref() == Some(path),
        });
    }

    /// A folder's line while its name is typed: the line as it always is,
    /// with the name a field in its place, its text chosen at first so that
    /// typing replaces it. Enter, or leaving it, names the folder so;
    /// Escape leaves it as it was; a name taken or that cannot be is marked
    /// and kept to be changed. Returns where the line is.
    pub(in crate::tab) fn folder_name_field(
        &mut self,
        ui: &mut egui::Ui,
        kind: Kind,
        item: &list::TreeItem,
    ) -> egui::Rect {
        let Spot::Folder(path) = &item.spot else {
            return egui::Rect::NOTHING;
        };
        let line = self.folder_look(path, item);
        let line = widgets::FolderRow { name: "", ..line }.show(ui);
        let taken = self.section_folders(kind);
        let Some(renaming) = &mut self.editor.renaming else {
            return line.rect;
        };
        let name = renaming.typed.trim().to_owned();
        let target = joined(parent_of(&renaming.path), &name);
        let wrong = name.is_empty()
            || name.contains('/')
            || (target != renaming.path && taken.contains(&target));
        let id = ui.make_persistent_id(("folder name", kind));
        let field =
            widgets::folder_name_field(ui, line.rect, item.depth, &mut renaming.typed, id, wrong);
        ui.ctx()
            .accesskit_node_builder(field.id, |node| node.set_label(tr("Folder name")));
        if renaming.fresh {
            renaming.fresh = false;
            field.request_focus();
            if let Some(mut state) = TextEdit::load_state(ui.ctx(), id) {
                let all = egui::text::CCursorRange::select_all_str(&renaming.typed);
                state.cursor.set_char_range(Some(all));
                state.store(ui.ctx(), id);
            }
        }
        // Why the name cannot be.
        let problem = match () {
            _ if name.is_empty() => None,
            _ if name.contains('/') => Some(tr("A folder's name cannot have a slash.").to_owned()),
            _ if wrong => Some(trf("There is a folder called {0} already.", &[&name])),
            _ => None,
        };
        // The field's edge turns red; what is wrong is said over it, and
        // to a screen reader with the field.
        if let Some(problem) = &problem {
            field.clone().on_hover_text(problem);
            (ui.ctx())
                .accesskit_node_builder(field.id, |node| node.set_description(problem.clone()));
        }
        let entered = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        if ui.input(|input| input.key_pressed(Key::Escape)) {
            self.editor.renaming = None;
        } else if entered && wrong {
            // Kept, to be changed, and why said again.
            field.request_focus();
            if let Some(problem) = &problem {
                self.voice.say_now(problem);
            }
        } else if field.lost_focus()
            && let Some(Renaming { path, choose, .. }) = self.editor.renaming.take()
        {
            let named = match !wrong && target != path && self.move_folder(kind, &path, &target) {
                true => target,
                false => path,
            };
            // A folder made is chosen once named, with the keyboard back on
            // it; one renamed is chosen if it was.
            if choose {
                self.choose_folder(&named);
            }
            if entered {
                let spot = Spot::Folder(named);
                let id = list::item_id(self.editor.section, &spot);
                ui.memory_mut(|memory| memory.request_focus(id));
            }
        }
        line.rect
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
        // What is known by its path goes with it: the folders closed, the
        // one chosen, and the one a menu was opened on.
        let follow = |path: &mut String| match path.strip_prefix(from) {
            Some("") => *path = to.to_owned(),
            Some(rest) if rest.starts_with('/') => *path = format!("{to}{rest}"),
            _ => {}
        };
        for (open_kind, path) in &mut self.editor.opened {
            if *open_kind == kind {
                follow(path);
            }
        }
        if let Some(path) = &mut self.editor.folder {
            follow(path);
        }
        if let Some(Spot::Folder(path)) = &mut self.editor.menu_target {
            follow(path);
        }
        true
    }

    /// The details of a folder: its name, with where it is, its switch and
    /// the way to delete it, as a rule's are; then what it holds, and what
    /// can be done in it.
    pub(in crate::tab) fn folder_detail(&mut self, ui: &mut egui::Ui, kind: Kind, path: &str) {
        let theme = theme::current();
        ui.spacing_mut().item_spacing.y = 10.0;
        let name = name_of(path).to_owned();
        let mut on = self.editor.draft.folder_on(kind, path);
        let state = on;
        let mut delete = false;
        let id = ui.make_persistent_id("folder header");
        widgets::right_last(
            ui,
            id,
            &mut (),
            |_, ui| {
                let heading = RichText::new(&name)
                    .font(theme::font(Weight::Semibold, 20.0))
                    .color(theme.text);
                ui.add(egui::Label::new(heading).selectable(false));
            },
            |_, ui| {
                off_badge(ui, state);
                let switch = widgets::switch(ui, &mut on, tr("Enabled"));
                widgets::name_tip(switch, tr("Enabled"));
                delete = widgets::icon_button(ui, widgets::Icon::Trash, tr("Delete folder"), false)
                    .clicked();
            },
        );
        ui.add(egui::Separator::default().spacing(2.0));
        if on != state {
            self.editor.draft.set_folder_on(kind, path, on);
        }
        let below = format!("{path}/");
        let things = (self.automation_rows().iter())
            .filter(|row| row.folder == path || row.folder.starts_with(&below))
            .count();
        let folders = (self.section_folders(kind).iter())
            .filter(|folder| folder.starts_with(&below))
            .count();
        let (mut add, mut inside, mut rename) = (false, false, false);
        // Where it is, chosen as a rule's folder is: among those it can go
        // into.
        let mut parent = parent_of(path).to_owned();
        let places = self.folders_for(kind, path);
        let mut moved = false;
        widgets::card(
            ui,
            Some(widgets::Glyph::Folder),
            tr("Folder"),
            |_| {},
            |ui| {
                let mut held = trn("{0} item", "{0} items", things as u64);
                if folders > 0 {
                    held = format!(
                        "{held}, {}",
                        trn("{0} folder", "{0} folders", folders as u64)
                    );
                }
                ui.label(RichText::new(held).color(theme.text));
                moved = folder_row(ui, &mut parent, &places, false);
                note(
                    ui,
                    tr(
                        "Switched off, nothing in it runs. Deleted, what it holds moves to the folder above.",
                    ),
                );
                ui.horizontal_wrapped(|ui| {
                    add = widgets::button_with(ui, widgets::Glyph::Plus, tr("Add here")).clicked();
                    inside =
                        widgets::button_with(ui, widgets::Glyph::Folder, tr("New folder inside"))
                            .clicked();
                    rename = widgets::button(ui, tr("Rename…")).clicked();
                });
            },
        );
        if moved {
            self.folder_moved_to(ui, kind, path, &parent);
        }
        if add {
            self.add_rule();
        }
        if inside {
            self.new_folder(kind, path);
        }
        if rename {
            self.start_renaming(path);
        }
        if delete {
            self.delete_folder(kind, path);
        }
    }

    /// Deletes a folder, to be had back for a while: what it held goes to
    /// the folder it was in, and what is chosen is the line before it.
    pub(in crate::tab) fn delete_folder(&mut self, kind: Kind, path: &str) {
        let name = name_of(path).to_owned();
        let what = match kind {
            Kind::Script => {
                let dir = self.own_scripts_dir().map(|dir| dir.join(path));
                let held = (dir.into_iter())
                    .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
                    .map(|entry| {
                        let entry = entry.path();
                        let name = match entry.is_dir() {
                            true => entry.file_name(),
                            false => entry.file_stem(),
                        };
                        name.unwrap_or_default().to_string_lossy().into_owned()
                    })
                    .collect();
                undo::Undone::ScriptFolder(path.to_owned(), held)
            }
            _ => undo::Undone::Step(self.editor.history.len() + 1),
        };
        let before = (self.editor.visible.iter())
            .position(|spot| *spot == Spot::Folder(path.to_owned()))
            .and_then(|at| at.checked_sub(1))
            .and_then(|at| self.editor.visible.get(at).cloned());
        self.remove_folder(kind, path);
        self.editor.undo = Some(undo::Undo {
            title: name,
            section: self.editor.section,
            index: self.editor.selected[self.editor.section],
            what,
            since: Instant::now(),
        });
        // What was chosen stays chosen, unless it was the folder deleted:
        // then the line before it is.
        if self.editor.folder.as_deref() == Some(path) {
            self.editor.folder = None;
            if let Some(Spot::Folder(folder)) = before
                && self.section_folders(kind).contains(&folder)
            {
                self.choose_folder(&folder);
            }
        }
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
            self.refuse(&trf("Could not move it: {0}", &[&error.to_string()]));
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

    /// Whether a folder can go into `parent`: not into itself, nor into a
    /// folder of its own.
    pub(in crate::tab) fn folder_fits(path: &str, parent: &str) -> bool {
        parent != path && !parent.starts_with(&format!("{path}/"))
    }

    /// Moves a folder into `parent`, beside another folder there if given:
    /// before it, or after. Returns where it is now, or None if it cannot
    /// go there.
    pub(in crate::tab) fn put_folder(
        &mut self,
        kind: Kind,
        path: &str,
        parent: &str,
        beside: Option<(&str, bool)>,
    ) -> Option<String> {
        if !Self::folder_fits(path, parent) {
            return None;
        }
        let name = name_of(path);
        let to = joined(parent, name);
        if to != path && !self.move_folder(kind, path, &to) {
            return None;
        }
        if let Some((beside, after)) = beside.filter(|(beside, _)| *beside != to) {
            self.editor.draft.place_folder(kind, &to, beside, after);
        }
        Some(to)
    }

    /// Says where something was moved to by the keys: a screen reader is
    /// not told otherwise.
    pub(in crate::tab) fn say_moved(&mut self, folder: &str) {
        let said = match folder {
            "" => tr("Moved to the top level.").to_owned(),
            folder => trf("Moved into {0}.", &[name_of(folder)]),
        };
        self.voice.say_now(&said);
    }
}
