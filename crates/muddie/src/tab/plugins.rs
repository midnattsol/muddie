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
    /// profile if there is one. One installed in the profile needs one.
    pub(super) fn set_included(&mut self, plugin: &'static crate::bundled::Bundled, on: bool) {
        if on && plugin.installs() && self.own_scripts_dir().is_none() {
            self.refuse(tr(
                "This plugin is installed in a profile: save this session as a profile, then switch it on.",
            ));
            self.app_request = Some(AppRequest::SaveAsProfile);
            return;
        }
        plugin.switch(&mut self.included_on, &mut self.included_off, on);
        self.run_included(plugin, on);
        self.keep_included();
    }

    /// Starts or stops a plugin that comes with Muddie in this session:
    /// one installed in the profile is installed the first time, and then
    /// its folder of scripts and its gauges are switched on or off.
    pub(super) fn run_included(&mut self, plugin: &'static crate::bundled::Bundled, on: bool) {
        if self.plugin_offer == Some(plugin) {
            self.plugin_offer = None;
        }
        if plugin.installs() && self.own_scripts_dir().is_some() {
            if on && !self.included_installed.contains_key(plugin.id) {
                self.install_included(plugin, false);
            }
            self.switch_installed(plugin, on);
        }
        self.run_actions();
        // The one that is part of the program changes what is asked for by
        // itself, with no script to say so.
        self.sync_modules();
    }

    /// Installs a plugin that comes with Muddie in the profile: its
    /// scripts in a folder named after it, its gauges among the profile's.
    /// What there is of it already is kept; installed `again`, what it
    /// installed is put back as it came. What was added is kept either way.
    /// Returns whether it was.
    fn install_included(&mut self, plugin: &'static crate::bundled::Bundled, again: bool) -> bool {
        let Some(folder) = self.own_scripts_dir().map(|dir| dir.join(plugin.folder())) else {
            return false;
        };
        let written = std::fs::create_dir_all(&folder).and_then(|()| {
            for (name, code) in plugin.scripts {
                let path = folder.join(name);
                if again || !path.exists() {
                    std::fs::write(&path, code)?;
                }
            }
            Ok(())
        });
        if let Err(error) = written {
            self.refuse(&trf(
                "{0} could not be installed: {1}",
                &[tr(plugin.name), &error.to_string()],
            ));
            return false;
        }
        let theirs = plugin.read_rules();
        self.change_rules(|rules| {
            for gauge in &theirs.gauges {
                let known = (rules.gauges.iter())
                    .position(|known| known.shown_name() == gauge.shown_name());
                match known {
                    Some(at) if again => rules.gauges[at] = gauge.clone(),
                    Some(_) => {}
                    None => rules.gauges.push(gauge.clone()),
                }
            }
        });
        (self.included_installed).insert(plugin.id.to_owned(), plugin.fingerprint());
        self.keep_included();
        true
    }

    /// Installs a plugin that comes with Muddie again, putting back what it
    /// installed as it came, and runs its scripts again.
    pub fn install_included_again(&mut self, plugin: &'static crate::bundled::Bundled) {
        if !self.install_included(plugin, true) {
            return;
        }
        if let Some(folder) = self.own_scripts_dir().map(|dir| dir.join(plugin.folder())) {
            for (name, _) in plugin.scripts {
                let path = folder.join(name);
                if self.scripts.is_loaded(&path) {
                    self.scripts.reload_file(&path);
                }
            }
        }
        self.run_actions();
        self.note(&trf("{0} installed again.", &[tr(plugin.name)]));
    }

    /// Switches the folder of scripts and the gauges a plugin installed on
    /// or off, as saved.
    fn switch_installed(&mut self, plugin: &'static crate::bundled::Bundled, on: bool) {
        let theirs = plugin.read_rules();
        let names: Vec<&str> = theirs
            .gauges
            .iter()
            .map(crate::rules::Gauge::shown_name)
            .collect();
        self.change_rules(|rules| {
            rules.set_folder_on(Kind::Script, plugin.folder(), on);
            for gauge in &mut rules.gauges {
                if names.contains(&gauge.shown_name()) {
                    gauge.enabled = on;
                }
            }
        });
        self.scripts_synced = None;
        self.sync_scripts();
    }

    /// Installs the plugins that come with Muddie that are switched on and
    /// not installed yet, as those switched on in the profile's settings,
    /// and says, once, of those installed that are not as they come now.
    /// Returns whether one was installed.
    pub(super) fn install_switched_on(&mut self) -> bool {
        if self.own_scripts_dir().is_none() {
            return false;
        }
        let on = (crate::bundled::ALL.iter())
            .filter(|plugin| plugin.installs() && self.included(plugin));
        let (mut installed, mut changed) = (false, Vec::new());
        for plugin in on.collect::<Vec<_>>() {
            match self.included_installed.get(plugin.id) {
                None => installed |= self.install_included(plugin, false),
                Some(mark) if *mark != plugin.fingerprint() => changed.push(tr(plugin.name)),
                Some(_) => {}
            }
        }
        if !changed.is_empty() && !self.included_changes_told {
            self.included_changes_told = true;
            self.note(&trf(
                "{0} has changed in this version of Muddie. To have the changes, install it again, from its menu in the profile's plugins: what it installed is put back as it comes, and what you added is kept.",
                &[&changed.join(", ")],
            ));
        }
        installed
    }

    /// Changes the rules as saved, and in the editor if it is open, and
    /// puts them to work.
    pub(super) fn change_rules(&mut self, change: impl Fn(&mut Rules)) {
        let mut rules = self.rules.clone();
        change(&mut rules);
        if self.editor.open {
            change(&mut self.editor.draft);
            change(&mut self.editor.seen);
        }
        if let Some(profiles) = &self.profiles
            && let Err(error) = rules.save(&profiles.rules_path(self.profile.as_deref()))
        {
            self.note(&trf("Could not save the rules: {0}", &[&error]));
        }
        self.reload_rules(Some(rules));
    }

    /// Saves which plugins are on with the profile, if there is one;
    /// without one they are as set for as long as the session lasts.
    pub(super) fn keep_included(&mut self) {
        let (on, off, declined, installed) = (
            self.included_on.clone(),
            self.included_off.clone(),
            self.included_declined.clone(),
            self.included_installed.clone(),
        );
        self.keep_in_profile(move |profile| {
            profile.included_on = on;
            profile.included_off = off;
            profile.included_declined = declined;
            profile.included_installed = installed;
        });
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
                    self.note(&trf("Plugin {0} installed.", &[&name]));
                    self.set_package(&name, true);
                }
                Ok(name) => {
                    self.note(&trf("Plugin {0} updated.", &[&name]));
                    self.load_scripts();
                }
                Err(error) => self.say(
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
