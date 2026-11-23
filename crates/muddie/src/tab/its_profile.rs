//! What a session takes from its profile, and gives back to it. The
//! profile is edited in the settings, on what is saved of it; a session
//! playing with it takes what changed as it is saved.

use super::*;

impl Tab {
    /// The profile this session plays with, if it has one.
    pub fn profile_name(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    /// Whether this session plays with the profile of this name.
    pub fn uses_profile(&self, name: &str) -> bool {
        self.profile.as_deref() == Some(name)
    }

    /// Whether it is connected.
    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    /// The profile as this session has it.
    pub(super) fn profile_now(&self) -> Option<Profile> {
        let port = self.port.trim().parse::<u16>().ok()?;
        Some(Profile {
            host: self.host.trim().to_owned(),
            port,
            tls: self.tls,
            verify_certificate: self.verify_certificate,
            character: self.character.trim().to_owned(),
            encoding: self.encoding.trim().to_owned(),
            language: self.language.chosen.clone(),
            plugins_on: self.plugins_on.clone(),
            plugins_off: self.plugins_off.clone(),
            plugins_declined: self.plugins_declined.clone(),
            plugins_installed: self.plugins_installed.clone(),
            session: Some(self.prefs.clone()),
            old_packages: Vec::new(),
        })
    }

    /// What a connection is made with, to tell when the profile says
    /// otherwise.
    pub(super) fn connection_key(&self) -> String {
        let fields = [&self.host, &self.port, &self.encoding, &self.character];
        let fields: Vec<&str> = fields.iter().map(|field| field.trim()).collect();
        format!(
            "{}|{}|{}",
            fields.join("|"),
            self.tls,
            self.verify_certificate
        )
    }

    /// Whether it is connected otherwise than its profile now says, so
    /// that connecting again would apply what changed.
    pub fn connection_outdated(&self) -> bool {
        self.connection.is_some() && self.connected_with.as_deref() != Some(&self.connection_key())
    }

    /// Takes what its profile has now, as the settings saved it: where to
    /// connect, how the session goes, and its plugins, run or stopped as
    /// they were switched.
    pub fn adopt_profile(&mut self, profile: &Profile) {
        self.host.clone_from(&profile.host);
        self.port = profile.port.to_string();
        self.tls = profile.tls;
        self.verify_certificate = profile.verify_certificate;
        self.character.clone_from(&profile.character);
        self.encoding.clone_from(&profile.encoding);
        self.language.chosen.clone_from(&profile.language);
        self.speak_the_language();
        self.prefs = profile.session.clone().unwrap_or_default();
        let switched: Vec<(crate::plugins::Plugin, bool)> = (self.plugins.iter())
            .map(|plugin| {
                let on = plugin.is_on(&profile.plugins_on, &profile.plugins_off);
                (plugin.clone(), on)
            })
            .filter(|(plugin, on)| self.plugin_on(plugin) != *on)
            .collect();
        self.plugins_on.clone_from(&profile.plugins_on);
        self.plugins_off.clone_from(&profile.plugins_off);
        self.plugins_declined.clone_from(&profile.plugins_declined);
        self.plugins_installed
            .clone_from(&profile.plugins_installed);
        for (plugin, on) in switched {
            self.run_plugin(&plugin, on);
        }
    }

    /// Its profile was given another name, and went with all it has.
    pub fn profile_renamed(&mut self, to: &str) {
        self.profile = Some(to.to_owned());
        self.profile_name = to.to_owned();
        // Its remembered values are now in the new folder.
        self.load_scripts();
        self.welcome_profiles = None;
    }

    /// The saved profiles changed: the welcome screen lists them again.
    pub fn profiles_changed(&mut self) {
        self.welcome_profiles = None;
    }

    /// Whether the session wrote its profile since this was last asked:
    /// a plugin switched from a command, say.
    pub fn take_profile_written(&mut self) -> bool {
        std::mem::take(&mut self.profile_written)
    }

    /// This session says nothing aloud, ever: one kept out of sight.
    pub fn silence(&mut self) {
        self.voice.silence();
    }

    /// Takes the saved profile of this name, without connecting: for what
    /// is done with the profile from outside its sessions.
    pub fn play_quietly(&mut self, name: &str) {
        self.select_profile(Some(name.to_owned()));
    }

    /// Plays with the saved profile of this name, and connects.
    pub fn play(&mut self, name: &str, ctx: &egui::Context) {
        if self.select_profile(Some(name.to_owned())) {
            self.connect(ctx);
        }
    }

    /// A name to keep this session under: the MUD's, as its address has
    /// it, or a plain one.
    pub fn suggested_profile_name(&self) -> String {
        let host = self.host.trim().trim_start_matches("www.");
        let first = host.split('.').next().unwrap_or_default();
        let named: String = first
            .chars()
            .filter(|ch| ch.is_alphanumeric() || *ch == '-')
            .collect();
        let mut letters = named.chars();
        match letters.next() {
            Some(initial) => initial.to_uppercase().chain(letters).collect(),
            None => tr("New profile").to_owned(),
        }
    }

    /// Saves this session, which has no profile, as one of this name.
    pub fn save_as_profile(&mut self, name: &str) {
        self.profile_name = name.to_owned();
        self.save_profile();
    }

    /// What the client said since the message of this number, to be
    /// said again elsewhere: as that of a session kept for a profile.
    pub fn said_since(&self, since: u64) -> Vec<(String, bool)> {
        (self.messages.list.iter())
            .filter(|message| message.serial > since)
            .map(|message| (message.text.clone(), message.problem))
            .collect()
    }

    /// How many messages the client said in this session.
    pub fn said_count(&self) -> u64 {
        self.messages.said
    }

    /// Says again what another session said.
    pub fn say_again(&mut self, text: &str, problem: bool) {
        self.say(text, problem);
    }
}

impl Tab {
    /// Switches to a saved profile, or to none. Returns false if the
    /// profile could not be loaded.
    pub(super) fn select_profile(&mut self, name: Option<String>) -> bool {
        let loaded = match (&name, &self.profiles) {
            (Some(name), Some(profiles)) => profiles.load(name).map(Some),
            (Some(_), None) => Err("no configuration folder was found".to_owned()),
            (None, _) => Ok(None),
        };
        let profile = match loaded {
            Ok(profile) => profile,
            Err(error) => {
                self.refuse(&trf(
                    "Could not load the profile: {0}",
                    &[&error.to_string()],
                ));
                return false;
            }
        };
        if let Some(profile) = profile {
            self.host = profile.host;
            self.port = profile.port.to_string();
            self.tls = profile.tls;
            self.verify_certificate = profile.verify_certificate;
            self.character = profile.character;
            self.encoding = profile.encoding;
            self.language.chosen = profile.language;
            self.plugins_on = profile.plugins_on;
            self.plugins_off = profile.plugins_off;
            self.plugins_declined = profile.plugins_declined;
            self.plugins_installed = profile.plugins_installed;
            self.prefs = (profile.session).unwrap_or_default();
        } else {
            self.prefs = crate::profile::SessionPrefs::default();
            self.plugins_on.clear();
            self.plugins_off.clear();
            self.plugins_declined.clear();
        }
        self.plugin_offer = None;
        self.profile_name = name.clone().unwrap_or_default();
        self.profile = name;
        self.load_layout();
        self.load_scripts();
        true
    }

    /// Saves one change of the profile in use, if there is one, as `change`
    /// makes it: what else is saved stays as it is, and what is being
    /// typed in the connection's form is not this one's to save.
    pub(super) fn keep_in_profile(&mut self, change: impl FnOnce(&mut Profile)) {
        let (Some(profiles), Some(name)) = (&self.profiles, &self.profile) else {
            return;
        };
        let saved = profiles.load(name).and_then(|mut profile| {
            change(&mut profile);
            (profiles.save(name, &profile)).map_err(|error| error.to_string())
        });
        match saved {
            Ok(()) => self.profile_written = true,
            Err(error) => self.refuse(&trf("Could not save the profile: {0}", &[&error])),
        }
    }

    /// Saves the connection settings as the profile of the name given for
    /// it.
    pub(super) fn save_profile(&mut self) {
        let name = self.profile_name.trim().to_owned();
        let Some(profiles) = &self.profiles else {
            return self.refuse(tr(
                "Profiles cannot be saved: no configuration folder was found.",
            ));
        };
        let Some(profile) = self.profile_now() else {
            return self.refuse(tr("The port must be a number from 1 to 65535."));
        };
        match profiles.save(&name, &profile) {
            Ok(()) => {
                let renamed = self.profile.as_deref() != Some(&name);
                self.profile_written = true;
                self.profile = Some(name.clone());
                if renamed {
                    self.load_scripts();
                }
                self.welcome_profiles = None;
                self.note(&trf("Profile {0} saved.", &[&name]));
                let password = std::mem::take(&mut self.new_password);
                if !password.is_empty()
                    && let Err(error) = secrets::set_password(&name, Some(&password))
                {
                    self.refuse(&trf(
                        "The password could not be saved in the keyring: {0}",
                        &[&error],
                    ));
                }
            }
            Err(error) => self.refuse(&trf(
                "Could not save the profile: {0}",
                &[&error.to_string()],
            )),
        }
    }
}
