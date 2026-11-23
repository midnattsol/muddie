//! The plugins of a session: switched on and off, installed in its
//! profile, offered for the MUD connected to, downloaded and removed.

use super::*;
use crate::plugins::{self, Plugin};

impl Tab {
    /// The plugins there are, as last looked for.
    pub fn plugins(&self) -> &[Plugin] {
        &self.plugins
    }

    /// Looks again for the plugins there are: those downloaded may have
    /// changed.
    pub(super) fn find_plugins(&mut self) {
        self.plugins = plugins::all(self.profiles.as_ref());
    }

    /// The plugin of this id or name, if there is one.
    pub(super) fn plugin(&self, id: &str) -> Option<Plugin> {
        let id = id.trim();
        (self.plugins.iter())
            .find(|plugin| plugin.id == id || plugin.name.eq_ignore_ascii_case(id))
            .cloned()
    }

    /// Whether a plugin is on in this session.
    pub(super) fn plugin_on(&self, plugin: &Plugin) -> bool {
        plugin.is_on(&self.plugins_on, &self.plugins_off)
    }

    /// Switches a plugin on or off, at once and without running the other
    /// scripts again, and keeps that with the profile if there is one;
    /// without one, it is run as it comes, for the session.
    pub(super) fn set_plugin(&mut self, plugin: &Plugin, on: bool) {
        plugin.switch(&mut self.plugins_on, &mut self.plugins_off, on);
        self.run_plugin(plugin, on);
        self.keep_plugins();
    }

    /// Starts or stops a plugin in this session: installed in the profile
    /// the first time, and then its folders of scripts and of gauges are
    /// switched on or off.
    pub(super) fn run_plugin(&mut self, plugin: &Plugin, on: bool) {
        if self.plugin_offer.as_ref() == Some(plugin) {
            self.plugin_offer = None;
        }
        if self.own_scripts_dir().is_some() {
            if on && !self.plugins_installed.contains_key(&plugin.id) {
                self.install_plugin_files(plugin, false);
            }
            self.switch_installed(plugin, on);
        } else if on {
            self.run_as_it_comes(plugin);
            self.run_rules();
        } else {
            for (name, _) in &plugin.scripts {
                self.scripts.unload(&as_it_comes(plugin, name));
            }
            self.rules.drop_folder(plugin.folder());
            self.run_rules();
        }
        self.run_actions();
        self.sync_modules();
    }

    /// Installs a plugin in the profile: its scripts in a folder named
    /// after it, its gauges among the profile's. What there is of it
    /// already is kept; installed `again`, what it installed is put back
    /// as it came. What was added is kept either way. Returns whether it
    /// was.
    fn install_plugin_files(&mut self, plugin: &Plugin, again: bool) -> bool {
        let Some(folder) = self.own_scripts_dir().map(|dir| dir.join(plugin.folder())) else {
            return false;
        };
        let written = std::fs::create_dir_all(&folder).and_then(|()| {
            for (name, code) in &plugin.scripts {
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
                &[&plugin.shown_name(), &error.to_string()],
            ));
            return false;
        }
        let Some(mut theirs) = self.plugin_rules(plugin) else {
            return false;
        };
        self.place_gauges(&mut theirs.gauges, again);
        self.change_rules(|rules| rules.bring(&theirs, again));
        (self.plugins_installed).insert(plugin.id.clone(), plugin.fingerprint());
        self.keep_plugins();
        true
    }

    /// What a plugin brings to the rules, in a folder of its name, as its
    /// scripts are, to be switched with them; none, and said, if its rules
    /// cannot be read.
    fn plugin_rules(&mut self, plugin: &Plugin) -> Option<crate::rules::Rules> {
        match plugin.read_rules() {
            Ok(mut theirs) => {
                theirs.put_in(plugin.folder());
                Some(theirs)
            }
            Err(error) => {
                self.refuse(&trf(
                    "The rules of {0} could not be read: {1}",
                    &[&plugin.shown_name(), &error],
                ));
                None
            }
        }
    }

    /// Installs a plugin again, putting back what it installed as it came,
    /// and runs its scripts again.
    pub fn install_plugin_again(&mut self, plugin: &Plugin) {
        if !self.install_plugin_files(plugin, true) {
            return;
        }
        if let Some(folder) = self.own_scripts_dir().map(|dir| dir.join(plugin.folder())) {
            for (name, _) in &plugin.scripts {
                let path = folder.join(name);
                if self.scripts.is_loaded(&path) {
                    self.scripts.reload_file(&path);
                }
            }
        }
        self.run_actions();
        self.note(&trf("{0} installed again.", &[&plugin.shown_name()]));
    }

    /// Runs a plugin as it comes, in a session with no profile to install
    /// it in: its scripts as they are, its gauges among the session's
    /// rules, unsaved.
    pub(super) fn run_as_it_comes(&mut self, plugin: &Plugin) {
        for (name, code) in &plugin.scripts {
            let owner = as_it_comes(plugin, name);
            self.scripts.unload(&owner);
            self.scripts.exec_owned(&owner, &owner, code);
        }
        let Some(mut theirs) = self.plugin_rules(plugin) else {
            return;
        };
        self.place_gauges(&mut theirs.gauges, false);
        self.rules.bring(&theirs, false);
    }

    /// Puts the gauges a plugin brings in the panels they name, as they are
    /// installed: those it brings again, or new to the profile.
    fn place_gauges(&mut self, gauges: &mut [crate::rules::Gauge], again: bool) {
        let theirs = |known: &crate::rules::Gauge| {
            gauges
                .iter()
                .any(|gauge| crate::rules::same_gauge(known, gauge))
        };
        let known = |gauge: &crate::rules::Gauge| {
            (self.rules.gauges.iter()).any(|known| crate::rules::same_gauge(known, gauge))
        };
        let placing: Vec<bool> = gauges.iter().map(|gauge| again || !known(gauge)).collect();
        // The panels the profile's own gauges are shown in, not the plugin's.
        let first = (self.layout.panels.iter())
            .find(|panel| panel.kind == layout::Kind::Gauges)
            .map(|panel| panel.id);
        // Another plugin's, as the standard one's, which most MUDs leave
        // unseen, take no panel from it.
        let of_a_plugin = |gauge: &crate::rules::Gauge| {
            (self.plugins.iter()).any(|plugin| brought_by(gauge, plugin))
        };
        let mut used: std::collections::HashSet<u32> = (self.rules.gauges.iter())
            .filter(|gauge| !theirs(gauge) && !of_a_plugin(gauge))
            .filter_map(|gauge| match gauge.panel {
                0 => first,
                panel => Some(panel),
            })
            .collect();
        for (gauge, placing) in gauges.iter_mut().zip(placing) {
            let name = std::mem::take(&mut gauge.shown_in);
            if placing && !name.trim().is_empty() {
                gauge.panel = self.gauges_panel_called(&name, |id| used.contains(&id));
                // Taken: the next name is not given it.
                used.insert(gauge.panel);
            }
        }
    }

    /// Switches the folders of the scripts and the gauges a plugin
    /// installed on or off, as saved: what is switched off in them stays
    /// so.
    fn switch_installed(&mut self, plugin: &Plugin, on: bool) {
        self.change_rules(|rules| rules.set_folder_on_everywhere(plugin.folder(), on));
        self.scripts_synced = None;
        self.sync_scripts();
    }

    /// Installs the plugins that are switched on and not installed yet, as
    /// those switched on in the profile's settings, and says, once, of
    /// those installed that are not as they are now. Returns whether one
    /// was installed.
    pub(super) fn install_switched_on(&mut self) -> bool {
        if self.own_scripts_dir().is_none() {
            return false;
        }
        let on: Vec<Plugin> = (self.plugins.iter())
            .filter(|plugin| self.plugin_on(plugin))
            .cloned()
            .collect();
        let (mut installed, mut changed) = (false, Vec::new());
        for plugin in on {
            match self.plugins_installed.get(&plugin.id) {
                None => installed |= self.install_plugin_files(&plugin, false),
                Some(mark) if *mark != plugin.fingerprint() => changed.push(plugin.shown_name()),
                Some(_) => {}
            }
        }
        if !changed.is_empty() && !self.plugin_changes_told {
            self.plugin_changes_told = true;
            self.note(&trf(
                "{0} has changed since it was installed. To have the changes, install it again, from its menu in the profile's plugins: what it installed is put back as it comes, and what you added is kept.",
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
            self.refuse(&trf("Could not save the rules: {0}", &[&error]));
        }
        self.reload_rules(Some(rules));
    }

    /// Saves which plugins are on with the profile, if there is one;
    /// without one they are as set for as long as the session lasts.
    pub(super) fn keep_plugins(&mut self) {
        let (on, off, declined, installed) = (
            self.plugins_on.clone(),
            self.plugins_off.clone(),
            self.plugins_declined.clone(),
            self.plugins_installed.clone(),
        );
        self.keep_in_profile(move |profile| {
            profile.plugins_on = on;
            profile.plugins_off = off;
            profile.plugins_declined = declined;
            profile.plugins_installed = installed;
        });
    }

    /// Offers the plugin made for the MUD at `host`, or that gives itself
    /// `name`, unless it is on already or was declined for this profile.
    pub(super) fn offer_plugin_for(&mut self, host: &str, name: &str) {
        let Some(plugin) = plugins::for_mud(&self.plugins, host, name).cloned() else {
            return;
        };
        let declined = self.plugins_declined.contains(&plugin.id);
        if self.plugin_on(&plugin) || declined || self.plugin_offer.as_ref() == Some(&plugin) {
            return;
        }
        self.voice.say(&trf(
            "There is a plugin for {0}: {1} It can be enabled from its message, at the bottom right.",
            &[&plugin.shown_name(), &plugin.about()],
        ));
        self.plugin_offer = Some(plugin);
    }

    /// The plugin offered for this MUD was declined: it is not offered
    /// again in this profile, and stays in the settings.
    pub(super) fn decline_plugin(&mut self) {
        if let Some(plugin) = self.plugin_offer.take() {
            self.plugins_declined.push(plugin.id);
            self.keep_plugins();
        }
    }

    /// What the plugin offered for this MUD is and does, if one is.
    pub fn plugin_offered(&self) -> Option<(String, String)> {
        let plugin = self.plugin_offer.as_ref()?;
        Some((
            trf("There is a plugin for {0}.", &[&plugin.shown_name()]),
            plugin.about(),
        ))
    }

    /// Answers the offer of a plugin: switched on, or not offered again.
    pub fn answer_plugin_offer(&mut self, enable: bool) {
        match (self.plugin_offer.clone(), enable) {
            (Some(plugin), true) => self.set_plugin(&plugin, true),
            (Some(_), false) => self.decline_plugin(),
            (None, _) => {}
        }
    }

    /// The GMCP modules to ask the MUD for now: the client's own, and
    /// those the scripts loaded ask for, the plugins' among them.
    pub(super) fn wanted_modules(&self) -> Vec<String> {
        let mut modules = muddie_core::Config::default().gmcp_supports;
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

    /// Removes a plugin from this profile: what it installed goes, its
    /// scripts and its gauges, and one downloaded is deleted too once no
    /// profile has it installed.
    pub(super) fn remove_plugin(&mut self, name: &str) {
        let Some(plugin) = self.plugin(name).filter(|plugin| plugin.included.is_none()) else {
            return self.refuse(tr("No such plugin is installed."));
        };
        plugin.switch(&mut self.plugins_on, &mut self.plugins_off, false);
        self.plugins_installed.remove(&plugin.id);
        self.keep_plugins();
        if let Some(folder) = self.own_scripts_dir().map(|dir| dir.join(plugin.folder())) {
            let _ = std::fs::remove_dir_all(folder);
        }
        self.change_rules(|rules| rules.drop_folder(plugin.folder()));
        let elsewhere = self.profiles.as_ref().is_some_and(|profiles| {
            (profiles.names().iter())
                .filter(|other| Some(other.as_str()) != self.profile.as_deref())
                .filter_map(|other| profiles.load(other).ok())
                .any(|other| other.plugins_installed.contains_key(&plugin.id))
        });
        let download = self
            .profiles
            .as_ref()
            .map(|profiles| profiles.plugins_dir());
        if let (false, Some(download)) = (elsewhere, download) {
            // Its name came from the list of folders there, so this is one.
            if let Err(error) = std::fs::remove_dir_all(download.join(plugin.folder())) {
                return self.refuse(&trf(
                    "The plugin could not be removed: {0}",
                    &[&error.to_string()],
                ));
            }
        }
        self.find_plugins();
        self.load_scripts();
        self.note(&trf("Plugin {0} removed.", &[&plugin.shown_name()]));
    }

    /// Reports downloads and updates of plugins that have ended: one
    /// downloaded is switched on, one updated installed again where it is.
    pub(super) fn check_plugin_jobs(&mut self, ctx: &egui::Context) {
        let mut ended = Vec::new();
        self.plugin_jobs
            .retain(|(job, enable)| match job.try_recv() {
                Ok(outcome) => {
                    ended.push((outcome, *enable));
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
            });
        if !self.plugin_jobs.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        for (outcome, downloaded) in ended {
            self.find_plugins();
            let found = outcome.map(|name| (self.plugin(&name), name));
            match found {
                Ok((Some(plugin), _)) if downloaded => {
                    self.note(&trf("Plugin {0} installed.", &[&plugin.shown_name()]));
                    self.set_plugin(&plugin, true);
                }
                Ok((Some(plugin), _)) => {
                    self.note(&trf("Plugin {0} updated.", &[&plugin.shown_name()]));
                    if self.plugins_installed.contains_key(&plugin.id) {
                        self.install_plugin_again(&plugin);
                    }
                }
                Ok((None, name)) => self.refuse(&trf(
                    "The plugin could not be installed or updated: {0}",
                    &[&name],
                )),
                Err(error) => self.refuse(&trf(
                    "The plugin could not be installed or updated: {0}",
                    &[&error],
                )),
            }
        }
    }

    /// Downloads a plugin from a git repository and switches it on: the
    /// address, then a name for it if the address does not give one.
    pub(super) fn install_plugin(&mut self, argument: &str) {
        let (address, given) = argument.split_once(' ').unwrap_or((argument, ""));
        let name = match given.trim() {
            "" => plugins::git::name_from_address(address),
            given => crate::profile::valid_name(given).then(|| given.to_owned()),
        };
        let folder = self.profiles.as_ref().map(Profiles::plugins_dir);
        match (folder, name) {
            (Some(folder), Some(name)) => {
                self.note(&trf("Installing {0}…", &[&name]));
                self.plugin_jobs
                    .push((plugins::git::download(&folder, &name, address), true));
            }
            (None, _) => self.refuse(tr(
                "Plugins need a configuration folder, and none was found.",
            )),
            (_, None) => self.refuse(tr(
                "Give the plugin a name after the address: letters, digits, - and _.",
            )),
        }
    }

    /// Brings a plugin downloaded up to date, and installs it again where
    /// it is installed.
    pub(super) fn update_plugin(&mut self, name: &str) {
        let Some(plugin) = self.plugin(name).filter(|plugin| plugin.included.is_none()) else {
            return self.refuse(tr("No such plugin is installed."));
        };
        match self.profiles.as_ref().map(Profiles::plugins_dir) {
            Some(folder) => {
                self.note(&trf("Updating {0}…", &[&plugin.name]));
                self.plugin_jobs
                    .push((plugins::git::update(&folder, &plugin.name), false));
            }
            None => self.refuse(tr(
                "Plugins need a configuration folder, and none was found.",
            )),
        }
    }

    /// Switches a plugin on in this profile, or off.
    pub(super) fn enable_plugin(&mut self, name: &str, enable: bool) {
        let Some(plugin) = self.plugin(name) else {
            return self.refuse(tr("No such plugin is installed."));
        };
        self.set_plugin(&plugin, enable);
        self.note(&if enable {
            trf("Plugin {0} enabled.", &[&plugin.shown_name()])
        } else {
            trf("Plugin {0} disabled.", &[&plugin.shown_name()])
        });
    }
}

/// What a script of a plugin run as it comes is known by.
fn as_it_comes(plugin: &Plugin, script: &str) -> String {
    format!("{}/{script}", plugin.folder())
}

/// Whether a gauge is in the folder of a plugin, or in one inside it.
fn brought_by(gauge: &crate::rules::Gauge, plugin: &Plugin) -> bool {
    gauge.folder.split('/').next() == Some(plugin.folder())
}
