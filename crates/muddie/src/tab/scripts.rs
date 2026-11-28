//! The scripts and rules a session runs: loading them, running them again,
//! and doing what they ask.

use super::*;
use muddie_script::{Aloud, Playing, Saying};

/// What the rules made in the interface are run as: one owner, to take
/// them away together when they are run again.
pub(super) const RULES: &str = "rules";

impl Tab {
    /// Starts the script runtime afresh and runs the user's scripts: the
    /// general one, the profile's plugins and scripts, and those given on
    /// the command line; then the rules.
    pub fn load_scripts(&mut self) {
        // What was switched for the session stays as it was left, unless
        // what is saved for it is what changed.
        let (session, before) = (self.scripts.switches(), self.rules.switched_off());
        // The scripts are told again how big the panel is.
        self.panels_untold();
        self.fresh_runtime();
        self.tell_session(self.connection.is_some());
        // The scripts teach their words for directions again, over those
        // of the language, and a plugin may say the language anew.
        self.language.declared = None;
        crate::languages::read_again();
        for problem in crate::languages::all().1 {
            self.refuse(&trf("Could not read a language: {0}", &[&problem]));
        }
        self.map.borrow_mut().forget_taught_directions();
        self.speak_the_language();
        self.find_plugins();
        let mut files = self.script_files_to_run();
        // The plugins that come with Muddie are among the player's own once
        // installed; one switched on and not installed yet is installed now.
        if self.install_switched_on() {
            files = self.script_files_to_run();
        }
        self.scripts.set_switches(session);
        self.rebase_switches(&before);
        // The player's own, unless they are switched off.
        let (own, switches) = (self.own_scripts(), self.scripts.switches());
        for file in &files {
            let off = (own.iter())
                .any(|(known, belongs)| known == file && !switches.active(Kind::Script, belongs));
            match off {
                // What it showed while it ran goes with it.
                true => (self.gauges).drop_owned_by(&muddie_script::owner_of(file)),
                false => self.scripts.load_file(file),
            }
        }
        self.scripts_synced = Some(switches);
        // With no profile to install them in, the plugins that come with
        // Muddie are run as they come.
        if self.own_scripts_dir().is_none() {
            let on: Vec<_> = (self.plugins.iter())
                .filter(|plugin| self.plugin_on(plugin))
                .cloned()
                .collect();
            for plugin in &on {
                self.run_as_it_comes(plugin);
            }
        }
        // The rules run last, as one more script.
        self.run_rules();
    }

    /// A new runtime, knowing as much of the panels as the old one did,
    /// and the variables. They belong to the session: with a saved profile
    /// they are kept with it; without one they last as long as the session,
    /// and go with it into a profile if it is then saved as one.
    fn fresh_runtime(&mut self) {
        let passing = std::mem::take(&mut self.variables_passing).then(|| self.scripts.variables());
        self.scripts =
            Engine::new(Rc::clone(&self.map)).expect("failed to start the script runtime");
        // Known as the scripts run, not a frame later.
        let _ = self.scripts.set_screen_reader(self.voice.screen_reader());
        self.tell_shortcuts();
        self.scripts.set_reading(self.voice.reads_game());
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
    /// scripts, its plugins' among them, those given on the command line; and the
    /// rules are read, as they say which of the player's own are switched
    /// off.
    fn script_files_to_run(&mut self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        if let Some(profiles) = &self.profiles {
            files.extend(profiles.init_script());
            if let Some(name) = &self.profile {
                files.extend(profiles.own_scripts(name));
            }
        }
        files.extend(self.extra_scripts.iter().cloned());
        if let Some(profiles) = &self.profiles {
            match Rules::load(&profiles.rules_path(self.profile.as_deref())) {
                Ok(rules) => {
                    self.rules = rules;
                    self.name_gauge_panels();
                }
                Err(error) => self.refuse(&trf("Could not load the rules: {0}", &[&error])),
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
        (panel, aloud): (&str, Aloud),
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
        self.channel_said(name, panel, lines, aloud);
    }

    /// Runs every script and rule again, as when a session starts, keeping
    /// the variables: the same from the editor and the palette.
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
                // After what kind of error it is, if it says.
                let error = ["syntax error: ", "runtime error: "]
                    .iter()
                    .find_map(|kind| error.strip_prefix(kind))
                    .unwrap_or(error);
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
    /// running. Their gauges are shown at once, as they are now; those no
    /// rule shows now, renamed, deleted or switched off, go.
    pub(super) fn run_rules(&mut self) {
        self.scripts.unload(RULES);
        self.gauges.running_again(RULES);
        // Each on its own, so a mistake in one says which and spares the rest.
        for (name, code) in self.rules.chunks() {
            self.scripts.exec_owned(RULES, &name, &code);
        }
        self.run_actions();
        self.gauges.drop_unset();
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
            Err(error) => self.refuse(&trf("Could not save the rules: {0}", &[&error])),
        }
    }

    fn echo_lines(&mut self, said: &Saying) {
        (said.text.lines()).for_each(|line| self.echo(line, said.aloud));
    }

    /// Carries out what scripts asked for since the last call.
    pub(super) fn run_actions(&mut self) {
        // What was said on each channel, in this one round of actions.
        let mut said = Vec::new();
        for action in self.scripts.take_actions() {
            match action {
                Action::Send(text) => self.send(text),
                Action::SendAsIs(text) => self.send_as_is(text),
                Action::Echo(said) => self.echo_lines(&said),
                Action::Speak(said) => self.voice.say_of_game(&said),
                Action::Gmcp { package, data } => self.tell_server(Command::Gmcp { package, data }),
                Action::Msdp { variable, values } => {
                    self.tell_server(Command::Msdp { variable, values })
                }
                Action::Gauge {
                    name,
                    level,
                    look,
                    owner,
                    belongs,
                    panel,
                } => {
                    let look = (look != Default::default()).then_some(look);
                    self.set_gauge(&name, level, look, Some(&owner), belongs, panel);
                }
                Action::Unloaded(owner) => self.drop_unloaded(&owner),
                Action::Language(language) => self.declare_language(language),
                Action::Walk(room) => self.walk_to(room),
                Action::Entered(room) => {
                    self.record_whereabouts();
                    self.arrived_in(room)
                }
                Action::LostTrack => {
                    self.record_whereabouts();
                    self.lost_track()
                }
                Action::MoveFailed => self.stop_for_failed_move(),
                // A script stops just in case: it is told nothing if there
                // was no walk.
                Action::StopWalk if self.walk.is_some() => self.stop_walking(),
                Action::StopWalk => {}
                Action::ShowArea(area) => self.show_area(&area, false),
                Action::ImportMap(address) => self.import_for_script(address),
                Action::MapNewer { address, answer } => self.check_newer_map(address, answer),
                Action::Choose { id, title, items } => self.start_choice(id, &title, items),
                Action::SoundVolume { id, volume } => self.sound_volume_for_script(id, volume),
                Action::GroupVolume { group, volume } => {
                    self.sound.set_group_volume(&group, volume)
                }
                Action::FetchSounds { owner, names } => self.fetch_plugin_sounds(&owner, names),
                Action::Copy(text) => self.copying = Some(text),
                Action::Reading(reading) => self.voice.set_muted(!reading),
                // The palette lists them: worked out again.
                Action::MapActions => self.palette.cache = None,
                Action::Panel { name, text } => self.show_in_panel(&name, text.as_deref()),
                Action::PanelAdd { name, text } => self.add_text_to_panel(&name, &text),
                Action::ToPanel(name) => self.add_dispatched_to_panel(&name),
                Action::Channel {
                    name,
                    text,
                    panel,
                    aloud,
                } => self.channel_from_script(&name, text.as_deref(), (&panel, aloud), &mut said),
                Action::ChannelClosed(name) => self.close_channel(&name),
                Action::Notify(said) => self.tell(&said.text, false, said.aloud.how),
                Action::PlaySound {
                    id,
                    file,
                    how,
                    owner,
                } => self.play_for_script(&file, how, (owner, id)),
                Action::StopSounds => self.sound.stop_all(),
                Action::Hush => self.voice.hush(),
                Action::Input(text) => self.input_from_script(text),
                Action::Feed(text) => self.fed.push(text),
                Action::StopSound(id) => self.stop_for_script(id),
                Action::Log(text) => crate::debug::show(&self.title(), "LOG", &text),
                Action::Error(message) => self.script_failed(message),
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

    /// Sends the server a command over the connection, if there is one.
    fn tell_server(&self, command: Command) {
        if let Some(connection) = &self.connection {
            let _ = connection.commands.send(command);
        }
    }

    /// What a script took with it when it was unloaded: its gauges and
    /// its sounds.
    pub(super) fn drop_unloaded(&mut self, owner: &str) {
        self.gauges.drop_owned_by(owner);
        self.sound.stop_where(|label| label.key == owner);
    }

    /// Takes what a script said of a move that failed: the map is told, and
    /// a walk under way stops.
    fn stop_for_failed_move(&mut self) {
        self.map.borrow_mut().moves_failed();
        if self.walk.take().is_some() {
            self.note(tr("Stopped: the server said the move failed."));
        }
    }

    /// Adds the lines of a text, styled, to the end of a panel.
    fn add_text_to_panel(&mut self, name: &str, said: &Saying) {
        for line in panels::styled_lines(&said.text) {
            self.add_to_panel(name, &line, said.aloud);
        }
    }

    /// Adds the line the triggers were last given to a panel, if there is
    /// one.
    fn add_dispatched_to_panel(&mut self, name: &str) {
        if let Some(line) = self.dispatched.take() {
            self.add_to_panel(name, &line, Aloud::default());
            self.dispatched = Some(line);
        }
    }

    /// Plays a sound as a script asked.
    fn play_for_script(&mut self, file: &str, how: Playing, by: (String, u64)) {
        let path = self.sound_file(file);
        // One of a plugin's that is not among the player's own: kept, or
        // fetched.
        if !path.is_file() && self.play_plugin_sound(file, &how, &by) {
            return;
        }
        self.play_path_for_script(&path, &how, &by);
    }

    /// Plays the sound at `path` as a script asked, known by the script
    /// that asked, to stop with it.
    pub(super) fn play_path_for_script(
        &mut self,
        path: &std::path::Path,
        how: &Playing,
        by: &(String, u64),
    ) {
        let (owner, id) = by.clone();
        let label = muddie_sound::Label {
            key: owner,
            tag: script_sound(id),
            group: how.group.clone(),
            ..Default::default()
        };
        let milliseconds = |seconds: f32| (seconds * 1000.0) as u32;
        let options = muddie_sound::Options {
            volume: how.volume,
            times: (!how.looped).then_some(1),
            start: milliseconds(how.start),
            finish: how.finish.map(milliseconds),
            fade_in: milliseconds(how.fade_in),
            fade_out: milliseconds(how.fade_out),
            pan: how.pan,
        };
        if let Err(error) = self.sound.play_with(path, options, label) {
            self.refuse(&trf("Could not play the sound: {0}", &[&error]));
        }
    }

    /// Sets how loud the sound a script played with this id is, as it plays.
    fn sound_volume_for_script(&mut self, id: u64, volume: f32) {
        let tag = script_sound(id);
        self.sound
            .set_volume_where(|label| label.tag == tag, volume);
    }

    /// Stops the sound a script played with this id.
    fn stop_for_script(&mut self, id: u64) {
        let tag = script_sound(id);
        self.sound.stop_where(|label| label.tag == tag);
    }

    /// Puts what a script says in the command line, unless a password is
    /// being typed there.
    fn input_from_script(&mut self, text: String) {
        if !self.typing_secret() {
            self.input = text;
            self.kept_input = None;
            self.cursor_to_end = true;
        }
    }

    /// Takes what scripts fed as if the MUD had sent it: what is fed
    /// meanwhile, the next time, so that one that feeds itself does not
    /// stop the rest.
    pub(super) fn take_fed(&mut self) {
        for text in std::mem::take(&mut self.fed) {
            for line in panels::styled_lines(&text) {
                self.on_line(line, false, false);
            }
        }
    }

    /// Tells the scripts what changes with no event of its own: what is
    /// typed, who has the session in front, and which of their sounds are
    /// playing.
    pub(super) fn tell_scripts(&mut self, focused: bool) {
        let typed = (!self.typing_secret()).then_some(self.input.as_str());
        self.scripts.know_input(typed);
        self.scripts.set_focused(focused);
        if self.scripts.has_sounds() {
            let sound = &self.sound;
            (self.scripts).know_sounds(|id| {
                let tag = script_sound(id);
                sound.is_playing(|label| label.tag == tag)
            });
        }
    }

    /// Tells the scripts of the session, as it is now.
    pub(super) fn tell_session(&mut self, connected: bool) {
        self.scripts.know_session(muddie_script::Session {
            connected,
            host: self.host.clone(),
            profile: self.profile.clone(),
        });
    }

    /// Shows an error a script gave, and keeps it for the editor.
    fn script_failed(&mut self, message: String) {
        crate::debug::show(&self.title(), "SCRIPT", &message);
        self.script_errors.push(message.clone());
        let message = trf("Script error: {0}", &[&message]);
        message.lines().for_each(|line| self.refuse(line));
    }
}

/// What a sound a script played is stopped by: no tag of the MUD's is so.
fn script_sound(id: u64) -> String {
    format!("script {id}")
}
