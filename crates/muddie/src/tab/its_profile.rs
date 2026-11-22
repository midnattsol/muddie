//! What a session takes from its profile, and gives back to it. The
//! profile is edited in the settings, on what is saved of it; a session
//! playing with it takes what changed as it is saved.

use super::*;
use crate::profile::SessionPrefs;

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
            packages: self.packages.clone(),
            included_on: self.included_on.clone(),
            included_off: self.included_off.clone(),
            included_declined: self.included_declined.clone(),
            included_installed: self.included_installed.clone(),
            session: Some(self.prefs.clone()),
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
        self.prefs =
            (profile.session.clone()).unwrap_or_else(|| SessionPrefs::from(&self.settings));
        let switched: Vec<(&'static crate::bundled::Bundled, bool)> = (crate::bundled::ALL.iter())
            .map(|plugin| {
                (
                    plugin,
                    plugin.is_on(&profile.included_on, &profile.included_off),
                )
            })
            .filter(|(plugin, on)| self.included(plugin) != *on)
            .collect();
        self.included_on.clone_from(&profile.included_on);
        self.included_off.clone_from(&profile.included_off);
        self.included_declined
            .clone_from(&profile.included_declined);
        self.included_installed
            .clone_from(&profile.included_installed);
        for (plugin, on) in switched {
            self.run_included(plugin, on);
        }
        if self.packages != profile.packages {
            self.packages.clone_from(&profile.packages);
            self.load_scripts();
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

    /// Runs a command about the installed plugins, whose answers are then
    /// shown where it was asked.
    pub fn plugin_command(&mut self, command: &str) {
        let before = self.output.len();
        self.do_client_command(command);
        self.plugin_said = before;
    }

    /// What the plugin commands asked for last have said, and whether
    /// one is still at work.
    pub fn plugin_answers(&self) -> (Vec<String>, bool) {
        let from = self.plugin_said.min(self.output.len());
        let busy = !self.package_jobs.is_empty();
        if from == 0 && !busy {
            return (Vec::new(), false);
        }
        let said = (from..self.output.len())
            .map(|row| self.output.text(row).to_owned())
            .collect();
        (said, busy)
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
                self.note(&trf(
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
            self.packages = profile.packages;
            self.character = profile.character;
            self.encoding = profile.encoding;
            self.language.chosen = profile.language;
            self.included_on = profile.included_on;
            self.included_off = profile.included_off;
            self.included_declined = profile.included_declined;
            self.included_installed = profile.included_installed;
            self.prefs = (profile.session)
                .unwrap_or_else(|| crate::profile::SessionPrefs::from(&self.settings));
        } else {
            self.prefs = crate::profile::SessionPrefs::from(&self.settings);
            self.packages.clear();
            self.included_on.clear();
            self.included_off.clear();
            self.included_declined.clear();
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
            Err(error) => self.note(&trf("Could not save the profile: {0}", &[&error])),
        }
    }

    /// Saves the connection settings as the profile named in the bar.
    pub(super) fn save_profile(&mut self) {
        let name = self.profile_name.trim().to_owned();
        let Some(profiles) = &self.profiles else {
            return self.note(tr(
                "Profiles cannot be saved: no configuration folder was found.",
            ));
        };
        let Some(mut profile) = self.profile_now() else {
            return self.note(tr("The port must be a number from 1 to 65535."));
        };
        // A profile saved under a new name starts without packages.
        if self.profile.as_deref() != Some(&name) {
            profile.packages.clear();
        }
        match profiles.save(&name, &profile) {
            Ok(()) => {
                let renamed = self.profile.as_deref() != Some(&name);
                self.profile_written = true;
                self.packages = profile.packages;
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
                    self.note(&trf(
                        "The password could not be saved in the keyring: {0}",
                        &[&error],
                    ));
                }
            }
            Err(error) => self.note(&trf(
                "Could not save the profile: {0}",
                &[&error.to_string()],
            )),
        }
    }
}
