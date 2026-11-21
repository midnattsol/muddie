//! The scripts and rules a session runs: loading them, running them again,
//! and doing what they ask.

use super::*;

/// What the rules made in the interface are run as: one owner, to take
/// them away together when they are run again.
pub(super) const RULES: &str = "rules";

impl Tab {
    /// The part of the settings window that belongs to sessions.
    /// Starts the script runtime afresh and runs the user's scripts: the
    /// general one, the plugins, the profile's packages and scripts, and
    /// those given on the command line; then the rules.
    pub(super) fn load_scripts(&mut self) {
        // What was switched for the session stays as it was left, unless
        // what is saved for it is what changed.
        let (session, before) = (self.scripts.switches(), self.rules.switched_off());
        // The scripts are told again how big the panel is.
        self.completions.clear();
        self.panels_untold();
        self.fresh_runtime();
        let files = self.script_files_to_run();
        self.scripts.set_switches(session);
        self.rebase_switches(&before);
        // The plugins that come with Muddie and are on, before the
        // player's own scripts, which may build on them.
        for plugin in &crate::bundled::ALL {
            if let (Some(code), true) = (plugin.code, self.included(plugin)) {
                let name = format!("{}.luau", plugin.id);
                (self.scripts).exec_owned(&crate::bundled::owner(plugin), &name, code);
            }
        }
        // The player's own, unless they are switched off.
        let (own, switches) = (self.own_scripts(), self.scripts.switches());
        for file in &files {
            let off = (own.iter())
                .any(|(known, belongs)| known == file && !switches.active(Kind::Script, belongs));
            if !off {
                self.scripts.load_file(file);
            }
        }
        self.scripts_synced = Some(switches);
        // The rules run last, as one more script.
        self.run_rules();
        self.run_actions();
    }

    /// A new runtime, knowing as much of the panels as the old one did,
    /// and the variables. They belong to the session: with a saved profile
    /// they are kept with it; without one they last as long as the session,
    /// and go with it into a profile if it is then saved as one.
    fn fresh_runtime(&mut self) {
        let passing = std::mem::take(&mut self.variables_passing).then(|| self.scripts.variables());
        self.scripts =
            Engine::new(Rc::clone(&self.map)).expect("failed to start the script runtime");
        self.retell_panel_sizes();
        match (&self.profiles, &self.profile) {
            (Some(profiles), Some(profile)) => self
                .scripts
                .use_memory_file(profiles.memory_path(Some(profile))),
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
    }

    /// The script files to run, in order: the general one, the profile's
    /// packages and scripts, those given on the command line; and the
    /// rules are read, as they say which of the player's own are switched
    /// off.
    fn script_files_to_run(&mut self) -> Vec<PathBuf> {
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
        if let Some(profiles) = &self.profiles {
            match Rules::load(&profiles.rules_path(self.profile.as_deref())) {
                Ok(rules) => self.rules = rules,
                Err(error) => self.note(&trf("Could not load the rules: {0}", &[&error])),
            }
        }
        files
    }

    /// Takes what a script said was said on a channel. The same line on the
    /// same channel, in one round of actions, is one line: a plugin and a
    /// script of the player's may both make it of the same message of the
    /// MUD's.
    fn channel_from_script(
        &mut self,
        name: &str,
        text: Option<&str>,
        panel: &str,
        said: &mut Vec<(String, String, String)>,
    ) {
        if let Some(text) = text {
            let key = (
                name.trim().to_lowercase(),
                self.channel_place(panel).to_lowercase(),
                text.to_owned(),
            );
            if said.contains(&key) {
                return;
            }
            said.push(key);
        }
        let lines = text.map(panels::styled_lines).unwrap_or_default();
        self.channel_said(name, panel, lines);
    }

    /// Runs every script and rule again, as when a session starts, keeping
    /// the variables: the same from the editor, the palette and `/reload`.
    /// Says how it went, and keeps when it was and which scripts failed,
    /// for the editor to show.
    pub(super) fn restart_scripts(&mut self) {
        self.flush_script();
        self.script_errors.clear();
        self.load_scripts();
        // What failed, by where the error says it was: a script's file, or
        // a rule.
        let mut failed: Vec<String> = (self.script_errors.iter())
            .map(|error| {
                error
                    .split(':')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned()
            })
            .collect();
        failed.sort();
        failed.dedup();
        let said = match failed.len() {
            0 => tr("Scripts restarted.").to_owned(),
            count => trn(
                "Scripts restarted; {0} failed.",
                "Scripts restarted; {0} failed.",
                count as u64,
            ),
        };
        match failed.is_empty() {
            true => self.note(&said),
            false => self.refuse(&said),
        }
        self.editor.restarted = Some(crate::output::clock());
        self.editor.failed = failed;
    }

    /// Runs the rules made in the interface, in place of those that were
    /// running.
    pub(super) fn run_rules(&mut self) {
        self.scripts.unload(RULES);
        // Each on its own, so a mistake in one says which and spares the rest.
        for (name, code) in self.rules.chunks() {
            self.scripts.exec_owned(RULES, &name, &code);
        }
    }

    /// Puts the rules as they are now saved to work, without running the
    /// scripts again: what those keep is not theirs to lose because a
    /// rule changed. A script switched on or off since is run or stopped.
    pub(super) fn reload_rules(&mut self, saved: Option<Rules>) {
        let before = self.rules.switched_off();
        if let Some(rules) = saved {
            self.rules = rules;
        }
        self.rebase_switches(&before);
        self.run_rules();
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
        // What was said on each channel, in this one round of actions.
        let mut said = Vec::new();
        for action in self.scripts.take_actions() {
            match action {
                Action::Send(text) => self.send(text),
                Action::Echo(text) => text.lines().for_each(|line| self.note(line)),
                Action::Speak { text, interrupt } => match interrupt {
                    true => self.voice.say_now(&text),
                    false => self.voice.say(&text),
                },
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
                    self.set_gauge(&name, level, look);
                }
                Action::Walk(room) => self.walk_to(room),
                Action::Entered(room) => self.arrived_in(room),
                Action::LostTrack => self.lost_track(),
                Action::StopWalk => {
                    if self.walk.take().is_some() {
                        self.note(tr("Walk stopped: the server said the move failed."));
                    }
                }
                Action::ImportMap(address) => self.import_for_script(address),
                Action::Panel { name, text } => self.show_in_panel(&name, text.as_deref()),
                Action::PanelAdd { name, text } => {
                    for line in panels::styled_lines(&text) {
                        self.add_to_panel(&name, &line);
                    }
                }
                Action::ToPanel(name) => {
                    if let Some(line) = self.dispatched.take() {
                        self.add_to_panel(&name, &line);
                        self.dispatched = Some(line);
                    }
                }
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
                    self.channel_from_script(&name, text.as_deref(), &panel, &mut said);
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
                    self.script_errors.push(message.clone());
                    let message = trf("Script error: {0}", &[&message]);
                    message.lines().for_each(|line| self.refuse(line));
                }
            }
        }
        // A script loaded, unloaded or run may have changed what it asks
        // the MUD for, or switched scripts on or off.
        if self.scripts.take_changed() {
            self.sync_modules();
            if self.sync_scripts() {
                self.run_actions();
            }
        }
    }
}
