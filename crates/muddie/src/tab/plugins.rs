//! The plugins of a session: those that come with Muddie, offered for the
//! MUD connected to, and those installed.

use super::*;

impl Tab {
    /// Whether a plugin that comes with Muddie is on in this session.
    pub(super) fn included(&self, plugin: &crate::bundled::Bundled) -> bool {
        plugin.is_on(&self.included_on, &self.included_off)
    }

    /// Whether what many MUDs send the same way is made something of.
    pub(super) fn conventions(&self) -> bool {
        crate::bundled::find(crate::bundled::COMMON).is_some_and(|plugin| self.included(plugin))
    }

    /// Switches a plugin that comes with Muddie on or off, at once and
    /// without running the other scripts again, and keeps that with the
    /// profile if there is one.
    pub(super) fn set_included(&mut self, plugin: &'static crate::bundled::Bundled, on: bool) {
        plugin.switch(&mut self.included_on, &mut self.included_off, on);
        self.run_included(plugin, on);
        self.keep_included();
    }

    /// Starts or stops a plugin that comes with Muddie in this session.
    pub(super) fn run_included(&mut self, plugin: &'static crate::bundled::Bundled, on: bool) {
        if self.plugin_offer == Some(plugin) {
            self.plugin_offer = None;
        }
        let owner = crate::bundled::owner(plugin);
        self.scripts.unload(&owner);
        if let (Some(code), true) = (plugin.code, on) {
            let name = format!("{}.luau", plugin.id);
            self.scripts.exec_owned(&owner, &name, code);
        }
        self.run_actions();
        // The one that is part of the program changes what is asked for by
        // itself, with no script to say so.
        self.sync_modules();
    }

    /// Saves which plugins are on with the profile, if there is one;
    /// without one they are as set for as long as the session lasts.
    pub(super) fn keep_included(&mut self) {
        let (Some(profiles), Some(name)) = (&self.profiles, &self.profile) else {
            return;
        };
        // What is saved of the profile, with only this changed: what is
        // being typed in the connection's form is not this one's to save.
        let saved = profiles.load(name).and_then(|mut profile| {
            profile.included_on.clone_from(&self.included_on);
            profile.included_off.clone_from(&self.included_off);
            profile
                .included_declined
                .clone_from(&self.included_declined);
            (profiles.save(name, &profile)).map_err(|error| error.to_string())
        });
        match saved {
            Ok(()) => self.profile_written = true,
            Err(error) => self.note(&trf("Could not save the profile: {0}", &[&error])),
        }
    }

    /// Offers the plugin made for the MUD at `host`, or that gives itself
    /// `name`, unless it is on already or was declined for this profile.
    pub(super) fn offer_plugin_for(&mut self, host: &str, name: &str) {
        let Some(plugin) = crate::bundled::for_mud(host, name) else {
            return;
        };
        let declined = self.included_declined.iter().any(|id| id == plugin.id);
        if self.included(plugin) || declined || self.plugin_offer == Some(plugin) {
            return;
        }
        self.plugin_offer = Some(plugin);
        self.voice.say(&trf(
            "There is a plugin for {0}: {1} It can be enabled from the notice at the bottom right.",
            &[tr(plugin.name), tr(plugin.about)],
        ));
    }

    /// The plugin offered for this MUD was declined: it is not offered
    /// again in this profile, and stays in the settings.
    pub(super) fn decline_plugin(&mut self) {
        if let Some(plugin) = self.plugin_offer.take() {
            self.included_declined.push(plugin.id.to_owned());
            self.keep_included();
        }
    }

    /// What the plugin offered for this MUD is and does, if one is.
    pub fn plugin_offered(&self) -> Option<(String, String)> {
        let plugin = self.plugin_offer?;
        Some((
            trf("There is a plugin for {0}.", &[tr(plugin.name)]),
            tr(plugin.about).to_owned(),
        ))
    }

    /// Answers the offer of a plugin: switched on, or not offered again.
    pub fn answer_plugin_offer(&mut self, enable: bool) {
        match (self.plugin_offer, enable) {
            (Some(plugin), true) => self.set_included(plugin, true),
            (Some(_), false) => self.decline_plugin(),
            (None, _) => {}
        }
    }

    /// The GMCP modules to ask the MUD for now: the client's own; those
    /// many MUDs send the same way, while the plugin that makes something
    /// of them is on; and those the scripts loaded ask for.
    pub(super) fn wanted_modules(&self) -> Vec<String> {
        let ours = |module: &String| module.starts_with("Core ") || module.starts_with("Client.");
        let mut modules = muddie_core::Config::default().gmcp_supports;
        if !self.conventions() {
            modules.retain(ours);
        }
        for module in self.scripts.supports() {
            if !modules.contains(&module) {
                modules.push(module);
            }
        }
        modules
    }

    /// Tells the connection which GMCP modules are wanted, when that
    /// changed: it tells the MUD what was added and what was removed.
    pub(super) fn sync_modules(&mut self) {
        let wanted = self.wanted_modules();
        if wanted == self.supports {
            return;
        }
        if let Some(connection) = &self.connection {
            let _ = (connection.commands).send(Command::GmcpSupports(wanted.clone()));
        }
        self.supports = wanted;
    }

    /// Adds a package to the profile's list, or takes it off, saves the
    /// profile and runs the scripts again.
    pub(super) fn set_package(&mut self, name: &str, enable: bool) {
        let Some(profile) = self.profile.clone() else {
            return self.note(tr(
                "Plugins are enabled per profile: save or choose a profile first.",
            ));
        };
        self.packages.retain(|package| package != name);
        if enable {
            self.packages.push(name.to_owned());
        }
        self.profile_name = profile;
        self.save_profile();
        self.load_scripts();
        self.note(&if enable {
            trf("Plugin {0} enabled.", &[name])
        } else {
            trf("Plugin {0} disabled.", &[name])
        });
    }

    /// Deletes an installed plugin, in every profile's eyes.
    pub(super) fn remove_plugin(&mut self, name: &str) {
        let folder = self.profiles.as_ref().map(Profiles::packages_dir);
        let installed = self
            .profiles
            .as_ref()
            .map(Profiles::packages)
            .unwrap_or_default();
        let Some(folder) = folder.filter(|_| installed.iter().any(|plugin| plugin == name)) else {
            return self.note(tr("No such plugin is installed; see /plugin list."));
        };
        // Its name came from the list of folders there, so this is one.
        match std::fs::remove_dir_all(folder.join(name)) {
            Ok(()) => {
                self.note(&trf("Plugin {0} removed.", &[name]));
                if self.packages.iter().any(|plugin| plugin == name) {
                    self.set_package(name, false);
                }
            }
            Err(error) => self.note(&trf(
                "The plugin could not be removed: {0}",
                &[&error.to_string()],
            )),
        }
    }

    /// Reports package installs and updates that have ended.
    pub(super) fn check_packages(&mut self, ctx: &egui::Context) {
        let mut ended = Vec::new();
        self.package_jobs
            .retain(|(job, enable)| match job.try_recv() {
                Ok(outcome) => {
                    ended.push((outcome, *enable));
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
            });
        if !self.package_jobs.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        for (outcome, enable) in ended {
            match outcome {
                Ok(name) if enable => {
                    self.bar(&trf("Plugin {0} installed.", &[&name]), false);
                    self.set_package(&name, true);
                }
                Ok(name) => {
                    self.bar(&trf("Plugin {0} updated.", &[&name]), false);
                    self.load_scripts();
                }
                Err(error) => self.bar(
                    &trf(
                        "The plugin could not be installed or updated: {0}",
                        &[&error],
                    ),
                    true,
                ),
            }
        }
    }

    /// Lists the installed plugins, each with whether it is on in this
    /// profile and the command that switches it.
    pub(super) fn list_plugins(&mut self) {
        let installed = self
            .profiles
            .as_ref()
            .map(Profiles::packages)
            .unwrap_or_default();
        if installed.is_empty() {
            self.refuse(tr(
                "No plugins are installed. Use /plugin install with a git address.",
            ));
        }
        for package in installed {
            let (state, switch) = match self.packages.contains(&package) {
                true => (tr("enabled in this profile"), "disable"),
                false => (tr("not enabled; use /plugin enable"), "enable"),
            };
            self.offer(&package, state, &format!("/plugin {switch} {package}"));
        }
    }

    /// Downloads a plugin from a git repository and enables it: the
    /// address, then a name for it if the address does not give one.
    pub(super) fn install_plugin(&mut self, argument: &str) {
        let (address, given) = argument.split_once(' ').unwrap_or((argument, ""));
        let name = match given.trim() {
            "" => packages::name_from_address(address),
            given => crate::profile::valid_name(given).then(|| given.to_owned()),
        };
        let folder = self.profiles.as_ref().map(Profiles::packages_dir);
        match (folder, name) {
            (Some(folder), Some(name)) => {
                self.note(&trf("Installing {0}…", &[&name]));
                self.package_jobs
                    .push((packages::install(&folder, &name, address), true));
            }
            (None, _) => self.note(tr(
                "Plugins need a configuration folder, and none was found.",
            )),
            (_, None) => self.note(tr(
                "Give the plugin a name after the address: letters, digits, - and _.",
            )),
        }
    }

    /// Brings an installed plugin up to date.
    pub(super) fn update_plugin(&mut self, name: &str) {
        match self.profiles.as_ref().map(Profiles::packages_dir) {
            Some(folder) => {
                self.note(&trf("Updating {0}…", &[name]));
                self.package_jobs
                    .push((packages::update(&folder, name), false));
            }
            None => self.note(tr(
                "Plugins need a configuration folder, and none was found.",
            )),
        }
    }

    /// Enables an installed plugin in this profile, or disables one.
    pub(super) fn enable_plugin(&mut self, name: &str, enable: bool) {
        let installed = self
            .profiles
            .as_ref()
            .map(Profiles::packages)
            .unwrap_or_default();
        if enable && !installed.iter().any(|package| package == name) {
            return self.note(tr("No such plugin is installed; see /plugin list."));
        }
        self.set_package(name, enable);
    }
}
