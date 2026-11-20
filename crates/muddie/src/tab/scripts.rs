//! The scripts and rules a session runs: loading them, running them again,
//! and doing what they ask.

use super::*;

/// What the rules made in the interface are run as: one owner, to take
/// them away together when they are run again.
pub(super) const RULES: &str = "rules";

/// Things by their names, each with whether it is switched on.
pub(super) type Switched = Vec<(String, bool)>;

impl Tab {
    /// The part of the settings window that belongs to sessions.
    /// Starts the script runtime afresh and runs the user's scripts: the
    /// general one, the profile's packages and scripts, and those given on
    /// the command line.
    pub(super) fn load_scripts(&mut self) {
        // Groups switched for the session keep how they were left, unless
        // what is saved for them is what changed.
        let (saved_before, session) = self.group_states();
        // The scripts say again what they want announced and are told
        // again how big the panel is.
        self.supports.clear();
        self.completions.clear();
        self.panels_untold();
        // Variables belong to the session. With a saved profile they are
        // kept with it; without one they last as long as the session, and
        // go with it into a profile if it is then saved as one.
        let passing = std::mem::take(&mut self.variables_passing).then(|| self.scripts.variables());
        self.scripts =
            Engine::new(Rc::clone(&self.map)).expect("failed to start the script runtime");
        // The new runtime knows as much of the panels as the old one did.
        self.retell_panel_sizes();
        match (&self.profiles, &self.profile) {
            (Some(profiles), Some(profile)) => {
                (self.scripts).use_memory_file(profiles.memory_path(Some(profile)));
            }
            _ => self.variables_passing = true,
        }
        let known: Vec<String> = self
            .scripts
            .variables()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        for (name, value) in passing.into_iter().flatten() {
            if !known.contains(&name) {
                let _ = self.scripts.set_variable(&name, Some(value));
            }
        }
        let mut files = Vec::new();
        if let Some(profiles) = &self.profiles {
            files.extend(profiles.init_script());
            if let Some(name) = &self.profile {
                let profile = Profile {
                    packages: self.packages.clone(),
                    ..Profile::default()
                };
                files.extend(profiles.scripts(name, &profile));
            }
        }
        files.extend(self.extra_scripts.iter().cloned());
        // The plugins that come with Muddie and are on, before the
        // player's own scripts, which may build on them.
        for plugin in &crate::bundled::ALL {
            if let (Some(code), true) = (plugin.code, self.included(plugin)) {
                let name = format!("{}.luau", plugin.id);
                (self.scripts).exec_owned(&crate::bundled::owner(plugin), &name, code);
            }
        }
        // The rules made in the interface, which also say which of the
        // scripts are switched off.
        if let Some(profiles) = &self.profiles {
            match Rules::load(&profiles.rules_path(self.profile.as_deref())) {
                Ok(rules) => self.rules = rules,
                Err(error) => self.note(&trf("Could not load the rules: {0}", &[&error])),
            }
        }
        // The player's own scripts that are switched off are not run;
        // a plugin has its own way of being switched off.
        let own: Vec<PathBuf> = (self.profiles.as_ref())
            .map(|profiles| profiles.editable_scripts(self.profile.as_deref()))
            .unwrap_or_default();
        files.retain(|file| {
            !own.contains(file)
                || !self
                    .rules
                    .scripts_off
                    .contains(&crate::rules::script_key(file))
        });
        for file in &files {
            self.scripts.load_file(file);
        }
        // The rules run last, as one more script.
        self.run_rules(&saved_before, session);
        self.run_actions();
    }

    /// Whether each group is on: as the rules have it saved, and as it
    /// is in this session, where a script or the player may have
    /// switched it.
    pub(super) fn group_states(&self) -> (Switched, Switched) {
        let saved = (self.rules.group_names().into_iter())
            .map(|name| {
                let on = self.rules.group_enabled(&name);
                (name, on)
            })
            .collect();
        let session = (self.scripts.groups().into_iter())
            .map(|name| {
                let on = self.scripts.group_enabled(&name);
                (name, on)
            })
            .collect();
        (saved, session)
    }

    /// Runs the rules made in the interface, in place of those that were
    /// running. Groups switched for the session keep how they were left,
    /// unless what is saved for them is what changed.
    pub(super) fn run_rules(&mut self, saved_before: &[(String, bool)], session: Switched) {
        self.scripts.unload(RULES);
        // Each on its own, so a mistake in one says which and spares the rest.
        for (name, code) in self.rules.chunks() {
            self.scripts.exec_owned(RULES, &name, &code);
        }
        for (name, on) in session {
            let before = saved_before
                .iter()
                .find(|(known, _)| *known == name)
                .is_none_or(|(_, on)| *on);
            if before == self.rules.group_enabled(&name) {
                self.scripts.set_group(&name, on);
            }
        }
    }

    /// Puts the rules as they are now saved to work, without running the
    /// scripts again: what those keep is not theirs to lose because a
    /// rule changed. A script switched on or off since is run or stopped.
    pub(super) fn reload_rules(&mut self, saved: Option<Rules>) {
        let (saved_before, session) = self.group_states();
        let off_before = self.rules.scripts_off.clone();
        if let Some(rules) = saved {
            self.rules = rules;
        }
        let own: Vec<PathBuf> = (self.profiles.as_ref())
            .map(|profiles| profiles.editable_scripts(self.profile.as_deref()))
            .unwrap_or_default();
        for file in own.iter().filter(|file| file.is_file()) {
            let key = crate::rules::script_key(file);
            match (
                off_before.contains(&key),
                self.rules.scripts_off.contains(&key),
            ) {
                (false, true) => self.scripts.unload_file(file),
                (true, false) => self.scripts.load_file(file),
                _ => {}
            }
        }
        self.run_rules(&saved_before, session);
        self.run_actions();
    }

    /// Saves the rules edited in the rules window and puts them to work.
    pub(super) fn save_rules(&mut self) {
        let draft = self.editor.draft.clone();
        let Some(profiles) = &self.profiles else {
            // Nowhere to keep them: they still work for this session.
            return self.reload_rules(Some(draft));
        };
        let saved = draft.save(&profiles.rules_path(self.profile.as_deref()));
        match saved {
            Ok(()) => self.reload_rules(Some(draft)),
            Err(error) => self.note(&trf("Could not save the rules: {0}", &[&error])),
        }
    }

    /// Carries out what scripts asked for since the last call.
    pub(super) fn run_actions(&mut self) {
        for action in self.scripts.take_actions() {
            match action {
                Action::Send(text) => self.send(text),
                Action::Echo(text) => text.lines().for_each(|line| self.note(line)),
                Action::Speak { text, interrupt } => {
                    if interrupt {
                        self.voice.stop();
                    }
                    self.voice.say(&text);
                }
                Action::Gmcp { package, data } => {
                    if let Some(connection) = &self.connection {
                        let _ = connection.commands.send(Command::Gmcp { package, data });
                    }
                }
                Action::Msdp { variable, values } => {
                    if let Some(connection) = &self.connection {
                        let _ = connection.commands.send(Command::Msdp { variable, values });
                    }
                }
                Action::Gauge { name, level, look } => {
                    let look = (look != Default::default()).then_some(look);
                    self.gauges.set_with(&name, level, look);
                }
                Action::Walk(room) => self.walk_to(room),
                Action::Entered(room) => self.arrived_in(room),
                Action::Panel { name, text } => self.show_in_panel(&name, text.as_deref()),
                Action::PanelAdd { name, text } => {
                    for line in panels::styled_lines(&text) {
                        self.add_to_panel(&name, &line);
                    }
                }
                Action::ToPanel(name) => {
                    if let Some(line) = self.dispatched.clone() {
                        self.add_to_panel(&name, &line);
                    }
                }
                Action::Supports(modules) => self.announce_modules(modules),
                Action::Completions { name, words } => {
                    // A list keeps its place when a newer one replaces it.
                    let known = self.completions.iter().position(|(list, _)| *list == name);
                    match (known, words.is_empty()) {
                        (Some(index), true) => drop(self.completions.remove(index)),
                        (Some(index), false) => self.completions[index].1 = words,
                        (None, false) => self.completions.push((name, words)),
                        (None, true) => {}
                    }
                }
                Action::Channel { name, text, panel } => {
                    let lines = text.as_deref().map(panels::styled_lines);
                    self.channel_said(&name, &panel, lines.unwrap_or_default());
                }
                Action::Chat(text) => {
                    for line in panels::styled_lines(&text) {
                        self.channel_line(&line);
                    }
                }
                Action::PlaySound {
                    file,
                    volume,
                    looped,
                } => {
                    let path = self.sound_file(&file);
                    if let Err(error) = self.sound.play(&path, volume, looped) {
                        self.note(&trf("Cannot play the sound: {0}", &[&error]));
                    }
                }
                Action::StopSounds => self.sound.stop_all(),
                Action::Error(message) => {
                    let message = trf("Script error: {0}", &[&message]);
                    message.lines().for_each(|line| self.refuse(line));
                }
            }
        }
    }
}
