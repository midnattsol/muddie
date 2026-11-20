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
        self.editor.script_files = match &self.profiles {
            Some(profiles) => profiles.editable_scripts(self.profile.as_deref()),
            None => Vec::new(),
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
        let own = path.parent().is_some_and(|dir| dir.ends_with("scripts"));
        if own && name != current && crate::profile::valid_name(&name) {
            let renamed = path.with_file_name(format!("{name}.luau"));
            if !renamed.exists() && std::fs::rename(&path, &renamed).is_ok() {
                // What it made under its old name goes with that name.
                self.scripts.unload_file(&path);
                // Switched off, it stays so under its new name.
                let (was, now) = (
                    crate::rules::script_key(&path),
                    crate::rules::script_key(&renamed),
                );
                for rules in [&mut self.editor.draft, &mut self.rules] {
                    for known in &mut rules.scripts_off {
                        if *known == was {
                            known.clone_from(&now);
                        }
                    }
                }
                self.editor.script_loaded = Some(renamed.clone());
                self.list_scripts();
                let index = self
                    .editor
                    .script_files
                    .iter()
                    .position(|file| *file == renamed);
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
        if self.palette.answer_failed {
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

    pub(in crate::tab) fn script_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let theme = theme::current();
        let path = self.editor.script_files[index].clone();
        if self.editor.script_loaded.as_ref() != Some(&path) {
            self.flush_script();
            // A file that is not there yet is an empty script.
            self.editor.script_text = std::fs::read_to_string(&path).unwrap_or_default();
            self.editor.script_saved = self.editor.script_text.clone();
            self.editor.script_seen = self.editor.script_text.clone();
            self.editor.script_name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            self.editor.script_loaded = Some(path.clone());
        }
        let own = path.parent().is_some_and(|dir| dir.ends_with("scripts"));
        let (mut delete, mut run) = (false, false);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if own {
                    delete = widgets::icon_button(ui, widgets::Icon::Trash, tr("Delete"), false)
                        .clicked();
                }
                run = widgets::icon_button(ui, widgets::Icon::Play, tr("Run"), false).clicked();
                ui.add_space(6.0);
                // Switched off, it is kept and not run.
                let key = crate::rules::script_key(&path);
                let mut on = !self.editor.draft.scripts_off.contains(&key);
                if widgets::switch(ui, &mut on, tr("Enabled"))
                    .on_hover_text(tr("Enabled"))
                    .changed()
                {
                    self.editor.draft.scripts_off.retain(|known| *known != key);
                    if !on {
                        self.editor.draft.scripts_off.push(key);
                    }
                }
                off_badge(ui, on);
                let title = TextEdit::singleline(&mut self.editor.script_name)
                    .font(theme::font(Weight::Semibold, 20.0))
                    .frame(egui::Frame::NONE)
                    .interactive(own)
                    .desired_width(f32::INFINITY);
                let field = ui.add(title);
                ui.ctx().accesskit_node_builder(field.id, |node| {
                    node.set_label(tr("Name"));
                });
            });
        });
        note(ui, &path.display().to_string());
        if run {
            self.run_scripts();
        }
        // Whether it ran, and when; or what kept it from running.
        match &self.editor.script_ran {
            Some((_, Some(error))) => {
                let said = RichText::new(error)
                    .font(theme::font(Weight::Regular, 11.5))
                    .color(theme.danger);
                ui.add(Label::new(said).wrap());
            }
            Some((at, None)) => note(ui, &trf("Last run: {0}", &[at])),
            None => note(ui, tr("Runs when saved, shortly after you stop typing.")),
        }
        // The editor is the whole of the view.
        let rows = script_rows(self.editor.detail_height, 118.0);
        let hint = "-- Luau\non_gmcp(\"Room.Info\", function(room)\n\tmap.visit({id = room.num, name = room.name, exits = room.exits})\nend)";
        let mistake = self
            .editor
            .checks
            .of(&self.scripts, &self.editor.script_text);
        widgets::code_editor(
            ui,
            tr("Script"),
            &mut self.editor.script_text,
            hint,
            rows,
            mistake.as_ref(),
        );
        delete
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
