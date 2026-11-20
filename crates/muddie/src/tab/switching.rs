//! What is switched on and off in a session: the player's own scripts, by
//! where they are, and the switches as saved and as changed since.

use super::*;

use std::path::Path;

use muddie_script::Belongs;

impl Tab {
    /// Where the profile's own scripts are kept, if there is a profile.
    pub(super) fn own_scripts_dir(&self) -> Option<PathBuf> {
        let (profiles, profile) = (self.profiles.as_ref()?, self.profile.as_deref()?);
        Some(profiles.scripts_dir(profile))
    }

    /// Whether a script is one of the profile's own, which can be renamed,
    /// moved and deleted, rather than the one of every session.
    pub(super) fn is_own_script(&self, file: &Path) -> bool {
        self.own_scripts_dir()
            .is_some_and(|dir| file.starts_with(dir))
    }

    /// Where a script of the player's is among them, which is what it is
    /// switched by: `Folder/name`, or its name for the one of every
    /// session, which is not among them.
    pub(super) fn script_place(&self, file: &Path) -> String {
        let dir = self.own_scripts_dir();
        let inner = dir.as_deref().and_then(|dir| file.strip_prefix(dir).ok());
        let inner = inner.unwrap_or_else(|| Path::new(file.file_name().unwrap_or_default()));
        inner
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// The player's own scripts, each with what it is switched by.
    pub(super) fn own_scripts(&self) -> Vec<(PathBuf, Belongs)> {
        let files = (self.profiles.as_ref())
            .map(|profiles| profiles.editable_scripts(self.profile.as_deref()))
            .unwrap_or_default();
        (files.into_iter())
            .filter(|file| file.is_file())
            .map(|file| {
                let place = self.script_place(&file);
                let tags = self.rules.script(&place).tags;
                let belongs = Belongs {
                    place: Some(place),
                    tags,
                };
                (file, belongs)
            })
            .collect()
    }

    /// Puts what is saved switched off now to work: what was switched for
    /// the session stays as it was, unless what is saved for it changed
    /// from `before`.
    pub(super) fn rebase_switches(&mut self, before: &Switches) {
        let now = self.scripts.switches();
        let switches = now.rebased(before, &self.rules.switched_off());
        self.scripts.set_switches(switches);
        let known = self.own_scripts().into_iter().map(|(_, belongs)| belongs);
        self.scripts.know_scripts(known.collect());
    }

    /// Runs the player's scripts switched on since they were last looked
    /// at, and stops those switched off. Returns whether any was.
    pub(super) fn sync_scripts(&mut self) -> bool {
        let switches = self.scripts.switches();
        if self.scripts_synced.as_ref() == Some(&switches) {
            return false;
        }
        let mut changed = false;
        for (file, belongs) in self.own_scripts() {
            match (
                switches.active(Kind::Script, &belongs),
                self.scripts.is_loaded(&file),
            ) {
                (true, false) => self.scripts.load_file(&file),
                (false, true) => self.scripts.unload_file(&file),
                _ => continue,
            }
            changed = true;
        }
        self.scripts_synced = Some(switches);
        changed
    }
}
