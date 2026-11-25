//! Keeping the plugins installed in a profile up to date with those there
//! are: when a profile is opened, when an official plugin was downloaded
//! anew, and when one from git was updated. What the player changed in a
//! plugin's scripts is kept beside them, never lost.

use super::*;
use crate::plugins::Plugin;

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
            let (made, written) = match mark.split_once(':') {
                Some((made, written)) => (made, Some(written)),
                None => (mark.as_str(), None),
            };
            if made == plugin.fingerprint() || self.is_connected() {
                continue;
            }
            // Another version than the one installed: installed again, as
            // it comes. Scripts of it changed since it was installed, or not
            // known to be as they were, are kept beside, so nothing is lost.
            kept |= self.keep_changed_scripts(&plugin, written);
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
    /// as it was installed (`written`, a mark of them then; none, not
    /// known) and not as the plugin has it now. Returns whether any was.
    fn keep_changed_scripts(&mut self, plugin: &Plugin, written: Option<&str>) -> bool {
        let Some(folder) = self.own_scripts_dir().map(|dir| dir.join(plugin.folder())) else {
            return false;
        };
        if written == Some(scripts_mark(plugin, &folder).as_str()) {
            return false;
        }
        let mut kept = false;
        for (name, code) in &plugin.scripts {
            let path = folder.join(name);
            let Ok(now) = std::fs::read_to_string(&path) else {
                continue;
            };
            if now != *code {
                let copy = folder.join(format!("{name}.before-update"));
                kept |= std::fs::write(copy, now).is_ok();
            }
        }
        kept
    }

    /// A plugin from git was brought up to date: installed again where it
    /// is installed, after what the player changed in its scripts is kept.
    pub(super) fn git_plugin_updated(&mut self, plugin: &Plugin) {
        let Some(mark) = self.plugins_installed.get(&plugin.id).cloned() else {
            return;
        };
        let written = mark.split_once(':').map(|(_, written)| written);
        if self.keep_changed_scripts(plugin, written) {
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

/// A mark of the scripts of `plugin` as they are in `folder`: FNV-1a, the
/// same on every build, as [`Plugin::fingerprint`].
pub(super) fn scripts_mark(plugin: &Plugin, folder: &std::path::Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (name, _) in &plugin.scripts {
        let code = std::fs::read(folder.join(name)).unwrap_or_default();
        for byte in name.bytes().chain([0]).chain(code).chain([0]) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}
