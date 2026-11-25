//! Keeping the plugins installed in a profile up to date with those there
//! are: when a profile is opened, when an official plugin was downloaded
//! anew, and when one from git was updated. What the player changed in a
//! plugin's scripts is kept beside them, never lost.

use super::*;
use crate::plugins::Plugin;
use crate::rules::Rules;
use std::collections::BTreeMap;
use std::path::Path;

/// What a plugin installed in a profile, to tell later what of it was
/// changed since, and to take away what it no longer has.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Installed {
    /// A mark of each of its scripts as it was written, by its file's name.
    scripts: BTreeMap<String, String>,
    /// Its rules, each by its kind and its place.
    rules: Vec<(Kind, String)>,
}

impl Installed {
    /// What `plugin` installs: its scripts as they are in `folder`, and the
    /// rules it brings.
    pub(super) fn of(plugin: &Plugin, folder: &Path, rules: &Rules) -> Self {
        Self {
            scripts: (plugin.scripts.iter())
                .map(|(name, _)| (name.clone(), mark_of(&folder.join(name))))
                .collect(),
            rules: rules.brought(),
        }
    }

    /// The mark a profile keeps of it, with what the plugin was then.
    pub(super) fn mark(&self, plugin: &Plugin) -> String {
        let said = serde_json::to_string(self).unwrap_or_default();
        format!("{}:{said}", plugin.fingerprint())
    }

    /// What a mark says: what the plugin was then, and what it installed,
    /// if that was noted.
    fn read(mark: &str) -> (&str, Option<Self>) {
        match mark.split_once(':') {
            Some((made, said)) => (made, serde_json::from_str(said).ok()),
            None => (mark, None),
        }
    }
}

impl Tab {
    /// Installs the plugins that are switched on and not installed yet, as
    /// those switched on in the profile's settings, and brings up to date,
    /// and says once, those installed that are not as they are now: not
    /// while connected, for a session is not changed in the middle of a
    /// game, but the next time the profile is opened. Returns whether one
    /// was installed.
    pub(super) fn install_switched_on(&mut self) -> bool {
        if self.own_scripts_dir().is_none() {
            return false;
        }
        let on: Vec<Plugin> = (self.plugins.iter())
            .filter(|plugin| self.plugin_on(plugin))
            .cloned()
            .collect();
        let (mut installed, mut updated, mut kept) = (false, Vec::new(), false);
        for plugin in on {
            let Some(mark) = self.plugins_installed.get(&plugin.id).cloned() else {
                installed |= self.install_plugin_files(&plugin, false);
                continue;
            };
            let (made, before) = Installed::read(&mark);
            if made == plugin.fingerprint() || self.is_connected() {
                continue;
            }
            // Another version than the one installed: installed again, as
            // it comes. Scripts of it changed since it was installed, or not
            // known to be as they were, are kept beside, so nothing is lost.
            kept |= self.keep_changed_scripts(&plugin, before.as_ref());
            if self.install_plugin_files(&plugin, true) {
                installed = true;
                updated.push(plugin.shown_name());
            }
        }
        if !updated.is_empty() {
            self.note(&trf("{0} brought up to date.", &[&updated.join(", ")]));
        }
        if kept {
            self.say_changes_kept();
        }
        installed
    }

    fn say_changes_kept(&mut self) {
        self.note(tr(
            "Changes made to a plugin's scripts are kept beside them, in files ending in .before-update.",
        ));
    }

    /// Keeps a copy of each script of `plugin` in the profile that is not
    /// as it was installed `before`, or not known to be, and not as the
    /// plugin has it now. Returns whether any was.
    fn keep_changed_scripts(&mut self, plugin: &Plugin, before: Option<&Installed>) -> bool {
        let Some(folder) = self.own_scripts_dir().map(|dir| dir.join(plugin.folder())) else {
            return false;
        };
        let mut kept = false;
        for (name, code) in &plugin.scripts {
            let path = folder.join(name);
            let as_it_was = before
                .and_then(|before| before.scripts.get(name))
                .is_some_and(|mark| *mark == mark_of(&path));
            match std::fs::read_to_string(&path) {
                Ok(now) if !as_it_was && now != *code => {
                    kept |= std::fs::write(put_aside(&path), now).is_ok();
                }
                _ => {}
            }
        }
        kept
    }

    /// Takes away what `plugin` installed before and no longer has, of its
    /// scripts and of `rules`. A script changed since, or not known to be
    /// as it was, is put aside: of what was installed with nothing noted,
    /// all that is in its folder. What the player added stays.
    pub(super) fn clear_before(&mut self, plugin: &Plugin, folder: &Path, rules: &Rules) {
        let Some(mark) = self.plugins_installed.get(&plugin.id).cloned() else {
            return;
        };
        let before = Installed::read(&mark).1;
        let names: Vec<String> = match &before {
            Some(before) => before.scripts.keys().cloned().collect(),
            None => scripts_in(folder),
        };
        let mut kept = false;
        for name in names {
            let path = folder.join(&name);
            let now = plugin.scripts.iter().any(|(script, _)| *script == name);
            if now || !path.is_file() {
                continue;
            }
            self.scripts.unload_file(&path);
            let as_it_was = before
                .as_ref()
                .is_some_and(|before| before.scripts.get(&name) == Some(&mark_of(&path)));
            let gone = match as_it_was {
                true => std::fs::remove_file(&path),
                false => std::fs::rename(&path, put_aside(&path)),
            };
            kept |= gone.is_ok() && !as_it_was;
            if let Err(error) = gone {
                self.refuse(&trf(
                    "{0} could not be installed: {1}",
                    &[&plugin.shown_name(), &error.to_string()],
                ));
            }
        }
        if kept {
            self.say_changes_kept();
        }
        let now = rules.brought();
        let mut gone = before.map(|before| before.rules).unwrap_or_default();
        gone.retain(|rule| !now.contains(rule));
        if !gone.is_empty() {
            self.change_rules(|rules| rules.take_out(&gone));
        }
    }

    /// A plugin from git was brought up to date: installed again where it
    /// is installed, after what the player changed in its scripts is kept.
    pub(super) fn git_plugin_updated(&mut self, plugin: &Plugin) {
        let Some(mark) = self.plugins_installed.get(&plugin.id).cloned() else {
            return;
        };
        let before = Installed::read(&mark).1;
        if self.keep_changed_scripts(plugin, before.as_ref()) {
            self.say_changes_kept();
        }
        self.install_plugin_again(plugin);
    }

    /// Official plugins were downloaded anew: what there is is looked for
    /// again. A session not connected takes them at once, as a profile
    /// opened does; one connected keeps its game as it is until the profile
    /// is opened again, but is offered a plugin it did not have for its MUD.
    pub fn official_changed(&mut self) {
        self.find_plugins();
        if self.is_connected() {
            let host = self.host.clone();
            self.offer_plugin_for(&host, "");
        } else if self.profile.is_some() {
            self.load_scripts();
        }
    }
}

/// A mark of a script as it is written: FNV-1a, the same on every build, as
/// [`Plugin::fingerprint`].
fn mark_of(path: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in std::fs::read(path).unwrap_or_default() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Where a script of a plugin is kept when the plugin's takes its place: by
/// a name that is no script's.
fn put_aside(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".before-update");
    path.with_file_name(name)
}

/// The scripts in a folder, by the names of their files.
fn scripts_in(folder: &Path) -> Vec<String> {
    (std::fs::read_dir(folder).into_iter().flatten().flatten())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".luau"))
        .collect()
}
