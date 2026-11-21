//! The scripts of a profile, in the editor: saving, running and checking
//! them.

use super::*;

/// Scripts to start from: what each is for, its file name, its text.
pub(in crate::tab) const EXAMPLES: [(&str, &str, &str); 5] = [
    (
        "A map the MUD draws itself",
        "drawn-map",
        include_str!("../../../../../examples/scripts/drawn-map.luau"),
    ),
    (
        "A map the MUD sends whole",
        "server-map",
        include_str!("../../../../../examples/scripts/server-map.luau"),
    ),
    (
        "A map read from the room descriptions",
        "text-mapper",
        include_str!("../../../../../examples/scripts/text-mapper.luau"),
    ),
    (
        "Words to complete, given by the MUD",
        "completions",
        include_str!("../../../../../examples/scripts/completions.luau"),
    ),
    (
        "Gauges from the MUD's vitals",
        "vitals-gmcp",
        include_str!("../../../../../examples/scripts/vitals-gmcp.luau"),
    ),
];

/// What checking a script said, kept for the scripts last looked at, so
/// that one being shown is not compiled again at every frame.
#[derive(Default)]
pub(in crate::tab) struct Checks(Vec<(u64, Option<(usize, String)>)>);

impl Tab {
    /// Looks again for the script files that can be edited.
    pub(in crate::tab) fn list_scripts(&mut self) {
        (self.editor.script_files, self.editor.script_folders) =
            match (&self.profiles, &self.profile) {
                (Some(profiles), Some(profile)) => (
                    profiles.editable_scripts(Some(profile)),
                    profiles.script_folders(profile),
                ),
                (Some(profiles), None) => (profiles.editable_scripts(None), Vec::new()),
                _ => (Vec::new(), Vec::new()),
            };
    }

    /// Saves the script being edited if it changed, renames its file if
    /// its name was changed, and runs the scripts again.
    pub(in crate::tab) fn flush_script(&mut self) {
        self.editor.script_settle.clear();
        let Some(path) = self.editor.script_loaded.clone() else {
            return;
        };
        let mut changed = false;
        if self.editor.script_text != self.editor.script_saved {
            let written = (path.parent().map_or(Ok(()), std::fs::create_dir_all))
                .and_then(|()| std::fs::write(&path, &self.editor.script_text));
            match written {
                Ok(()) => {
                    self.editor.script_saved = self.editor.script_text.clone();
                    changed = true;
                }
                Err(error) => self.note(&trf(
                    "Could not save the script: {0}",
                    &[&error.to_string()],
                )),
            }
        }
        let name = self
            .editor
            .script_name
            .trim()
            .trim_end_matches(".luau")
            .to_owned();
        let current = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if self.is_own_script(&path) && name != current && crate::profile::valid_name(&name) {
            // In the same folder, under its new name, with how it is kept.
            let place = self.script_place(&path);
            let renamed = crate::rules::joined(&folder_of(&place), &name);
            if self.move_script_files(&place, &renamed) {
                let index = (self.editor.script_files.iter())
                    .position(|file| self.editor.script_loaded.as_ref() == Some(file));
                self.editor.selected[SCRIPTS] = index.unwrap_or(0);
                changed = true;
            }
        }
        if changed {
            self.run_scripts();
        }
    }

    /// Runs the script being edited again, by itself, and keeps whether
    /// that went well, to say so beside it. The other scripts and the
    /// rules go on as they were, with all they keep.
    pub(in crate::tab) fn run_scripts(&mut self) {
        // Saved first, if what is being typed has not been yet.
        if self.editor.script_loaded.is_some()
            && self.editor.script_text != self.editor.script_saved
        {
            return self.flush_script();
        }
        let Some(path) = self.editor.script_loaded.clone() else {
            return;
        };
        self.palette.answer.clear();
        (self.palette.answering, self.palette.answer_failed) = (true, false);
        // Run now whether or not it is switched on: that says whether it
        // is run by itself when a session starts, not whether it can be.
        self.scripts.reload_file(&path);
        // It is told again how big the panels are.
        self.panels_untold();
        self.run_actions();
        self.palette.answering = false;
        let said = std::mem::take(&mut self.palette.answer);
        let at = crate::output::clock();
        // Run by itself, whether it fails is known again.
        let name = list::title_of(&path);
        self.editor.failed.retain(|failed| *failed != name);
        if self.palette.answer_failed {
            self.editor.failed.push(name);
            let error: Vec<String> = said.into_iter().map(|said| said.text).collect();
            self.editor.script_ran = Some((at, Some(error.join("\n"))));
        } else {
            // What it printed is shown where such things are.
            for said in said {
                self.show_local(&said.text);
            }
            self.editor.script_ran = Some((at, None));
        }
    }

    /// Makes an empty script file for the profile and selects it.
    pub(in crate::tab) fn add_script(&mut self, example: Option<(&str, &str)>) {
        self.flush_script();
        let (stem, text) = example.unwrap_or(("script", ""));
        let (Some(profiles), Some(profile)) = (&self.profiles, &self.profile) else {
            return self.note(tr(
                "Scripts of your own are kept per profile: save or choose a profile first.",
            ));
        };
        let dir = profiles.scripts_dir(profile);
        let free = (1..).map(|n| {
            dir.join(if n == 1 {
                format!("{stem}.luau")
            } else {
                format!("{stem}-{n}.luau")
            })
        });
        let path = free.take(500).find(|path| !path.exists());
        let made = path
            .ok_or_else(|| "too many scripts".to_owned())
            .and_then(|path| {
                std::fs::create_dir_all(&dir)
                    .and_then(|()| std::fs::write(&path, text))
                    .map(|()| path)
                    .map_err(|error| error.to_string())
            });
        match made {
            Ok(path) => {
                self.list_scripts();
                let index = self
                    .editor
                    .script_files
                    .iter()
                    .position(|file| *file == path);
                self.editor.selected[SCRIPTS] = index.unwrap_or(0);
                self.editor.filter.clear();
                // Run by itself: the others have nothing to start over for.
                if !text.is_empty() {
                    self.scripts.load_file(&path);
                    self.run_actions();
                }
            }
            Err(error) => self.note(&trf("Could not save the script: {0}", &[&error])),
        }
    }

    /// A script's details: its heading, where it is, how it last ran, and
    /// the editor, which is the whole of the view.
    pub(in crate::tab) fn script_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let path = self.editor.script_files[index].clone();
        self.load_script(&path);
        let (delete, run) = self.script_heading(ui, &path);
        file_chip(ui, &path);
        if run {
            self.run_scripts();
        }
        self.script_status(ui);
        let rows = script_rows(self.editor.detail_height, 118.0);
        let hint = "-- Luau\non_gmcp(\"Room.Info\", function(room)\n\tmap.visit({id = room.num, name = room.name, exits = room.exits})\nend)";
        // What keeps it from being read, or else what went wrong when it
        // last ran: shown at its line, in the editor.
        let mistake = (self
            .editor
            .checks
            .of(&self.scripts, &self.editor.script_text))
        .or_else(|| self.run_mistake(&path));
        widgets::code_editor(
            ui,
            tr("Script"),
            &mut self.editor.script_text,
            hint,
            widgets::Language::Luau,
            widgets::CodeHeight::Fill(rows),
            mistake.as_ref(),
        );
        delete
    }

    /// Opens a script to be edited, saving the one that was; a file that
    /// is not there yet is an empty script.
    fn load_script(&mut self, path: &std::path::Path) {
        if self.editor.script_loaded.as_deref() == Some(path) {
            return;
        }
        self.flush_script();
        self.editor.script_text = std::fs::read_to_string(path).unwrap_or_default();
        self.editor.script_saved = self.editor.script_text.clone();
        self.editor.script_seen = self.editor.script_text.clone();
        self.editor.script_name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.editor.script_loaded = Some(path.to_owned());
    }

    /// Its name, to change if it is the profile's own, its switch, and the
    /// buttons that run it and delete it. Switched off, it is kept and not
    /// run. Returns whether deleting and running were asked.
    fn script_heading(&mut self, ui: &mut egui::Ui, path: &std::path::Path) -> (bool, bool) {
        let own = self.is_own_script(path);
        let place = self.script_place(path);
        let mut kept = self.editor.draft.script(&place);
        let (mut delete, mut run) = (false, false);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if own {
                    delete = widgets::icon_button(ui, widgets::Icon::Trash, tr("Delete"), false)
                        .clicked();
                }
                run = widgets::icon_button(ui, widgets::Icon::Play, tr("Run"), false).clicked();
                ui.add_space(6.0);
                widgets::switch(ui, &mut kept.enabled, tr("Enabled")).on_hover_text(tr("Enabled"));
                off_badge(ui, kept.enabled);
                let font = theme::font(Weight::Semibold, 20.0);
                let (name, rename) = (&mut self.editor.script_name, &mut self.editor.rename);
                name_field(ui, name, font, "", rename, own);
            });
        });
        ui.add(egui::Separator::default().spacing(2.0));
        if own {
            self.script_filing(ui, &place, &mut kept);
        }
        if kept != self.editor.draft.script(&place) {
            self.editor.draft.set_script(kept);
        }
        (delete, run)
    }

    /// Where a script of the profile's is filed: its folder, which is the
    /// folder on the disk it is in, and its tags; and why it does not run
    /// though switched on, if it does not.
    fn script_filing(
        &mut self,
        ui: &mut egui::Ui,
        place: &str,
        kept: &mut crate::rules::ScriptEntry,
    ) {
        let folders = self.section_folders(Kind::Script);
        let known = self.editor.draft.tag_names();
        let mut folder = folder_of(place);
        folder_row(ui, &mut folder, &folders, false);
        tags_row(ui, ("script tags", place), &mut kept.tags, &known);
        if folder != folder_of(place) {
            let name = name_of(place);
            let to = crate::rules::joined(&folder, name);
            if self.move_script_files(place, &to) {
                kept.path = to;
            }
        }
        if kept.enabled
            && let Some(reason) = self.off_reason(Kind::Script, &folder_of(&kept.path), &kept.tags)
        {
            note(ui, &reason);
        }
    }

    /// Whether it ran, and when. What kept it from running is shown at its
    /// line, in the editor.
    fn script_status(&self, ui: &mut egui::Ui) {
        match &self.editor.script_ran {
            Some((_, Some(_))) => {}
            Some((at, None)) => note(ui, &trf("Last run: {0}", &[at])),
            None => note(ui, tr("Runs when saved, shortly after you stop typing.")),
        }
    }

    /// What went wrong when the script at `path` last ran, at its line, if
    /// the error names one of this file.
    fn run_mistake(&self, path: &std::path::Path) -> Option<(usize, String)> {
        let Some((_, Some(error))) = &self.editor.script_ran else {
            return None;
        };
        let file = path.file_name()?.to_str()?;
        let after = error.split(&format!("{file}:")).nth(1)?;
        let (line, message) = after.split_once(':')?;
        Some((line.trim().parse().ok()?, message.trim().to_owned()))
    }
}

impl Checks {
    /// The mistake in `code`, if it has one.
    pub fn of(&mut self, engine: &Engine, code: &str) -> Option<(usize, String)> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        code.hash(&mut hasher);
        let key = hasher.finish();
        if let Some((_, known)) = self.0.iter().find(|(known, _)| *known == key) {
            return known.clone();
        }
        let found = engine.check(code);
        if self.0.len() >= 8 {
            self.0.remove(0);
        }
        self.0.push((key, found.clone()));
        found
    }
}

/// Where a script is kept, as a chip, with the way to open its folder.
fn file_chip(ui: &mut egui::Ui, path: &std::path::Path) {
    let theme = theme::current();
    ui.horizontal(|ui| {
        crate::widgets::inset(theme.field)
            .inner_margin(egui::Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    widgets::glyph(ui, widgets::Glyph::Script, 11.0, theme.text_faint);
                    let shown = RichText::new(path.display().to_string())
                        .font(theme::font(Weight::Regular, 12.0))
                        .color(theme.text_dim);
                    ui.add(Label::new(shown).truncate());
                });
            });
        let folder = path.parent().map(std::path::Path::to_path_buf);
        if let Some(folder) = folder
            && widgets::glyph_button(ui, widgets::Glyph::Folder, tr("Open the folder")).clicked()
        {
            crate::widgets::open_path(&folder);
        }
    });
}
