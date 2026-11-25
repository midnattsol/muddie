//! Reading, writing, renaming, copying and deleting saved profiles.

use super::*;

impl Profiles {
    pub fn load(&self, name: &str) -> Result<Profile, String> {
        let path = self.profile_dir(name).join("profile.toml");
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let mut profile: Profile =
            toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        // The standard plugin was known as "common".
        for list in [
            &mut profile.plugins_on,
            &mut profile.plugins_off,
            &mut profile.plugins_declined,
        ] {
            for id in list.iter_mut().filter(|id| *id == "common") {
                *id = crate::plugins::STANDARD.to_owned();
            }
        }
        if let Some(mark) = profile.plugins_installed.remove("common") {
            (profile.plugins_installed).insert(crate::plugins::STANDARD.to_owned(), mark);
        }
        for package in std::mem::take(&mut profile.old_packages) {
            if !profile.plugins_on.contains(&package) {
                profile.plugins_on.push(package);
            }
        }
        Ok(profile)
    }

    /// Writes the profile, creating its folder and an empty `scripts`
    /// folder for the user to fill.
    pub fn save(&self, name: &str, profile: &Profile) -> Result<(), String> {
        if !valid_name(name) {
            return Err(name_rule());
        }
        let dir = self.profile_dir(name);
        let text = toml::to_string_pretty(profile).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(dir.join("scripts"))
            .and_then(|()| std::fs::write(dir.join("profile.toml"), text))
            .map_err(|error| {
                crate::i18n::trf(
                    "Could not save the profile in {0}: {1}",
                    &[&dir.display().to_string(), &error.to_string()],
                )
            })
    }

    /// Whether a profile of this name is saved.
    pub fn exists(&self, name: &str) -> bool {
        self.profile_dir(name).join("profile.toml").is_file()
    }

    /// Gives a profile another name, with all it has: its scripts, rules,
    /// layout, map and remembered values. Its password goes with it.
    pub fn rename(&self, from: &str, to: &str) -> Result<(), String> {
        self.free_name(to)?;
        std::fs::rename(self.profile_dir(from), self.profile_dir(to)).map_err(|error| {
            crate::i18n::trf("Could not rename the profile: {0}", &[&error.to_string()])
        })?;
        move_password(from, to, true);
        Ok(())
    }

    /// Makes a copy of a profile under another name, with all it has.
    pub fn duplicate(&self, from: &str, to: &str) -> Result<(), String> {
        self.free_name(to)?;
        copy_dir(&self.profile_dir(from), &self.profile_dir(to)).map_err(|error| {
            crate::i18n::trf("Could not copy the profile: {0}", &[&error.to_string()])
        })?;
        move_password(from, to, false);
        Ok(())
    }

    /// Deletes a profile, with all it has, and forgets its password.
    pub fn delete(&self, name: &str) -> Result<(), String> {
        std::fs::remove_dir_all(self.profile_dir(name)).map_err(|error| {
            crate::i18n::trf("Could not delete the profile: {0}", &[&error.to_string()])
        })?;
        let _ = crate::secrets::set_password(name, None);
        Ok(())
    }

    /// A name a new profile can take: allowed, and no other's.
    pub fn free_name(&self, name: &str) -> Result<(), String> {
        if !valid_name(name) {
            return Err(name_rule());
        }
        if self.profile_dir(name).exists() {
            return Err(crate::i18n::trf(
                "There is a profile called {0} already.",
                &[name],
            ));
        }
        Ok(())
    }

    /// A name not taken, made from `base`: itself, then numbered.
    pub fn unused_name(&self, base: &str) -> String {
        (1..)
            .map(|number| match number {
                1 => base.to_owned(),
                number => format!("{base} {number}"),
            })
            .find(|name| !self.profile_dir(name).exists())
            .unwrap_or_else(|| base.to_owned())
    }
}

/// Copies a profile's password to another name, and with `moving` forgets
/// it under the first. A keyring that cannot be reached leaves it be.
fn move_password(from: &str, to: &str, moving: bool) {
    if let Ok(Some(password)) = crate::secrets::password(from) {
        let _ = crate::secrets::set_password(to, Some(&password));
        if moving {
            let _ = crate::secrets::set_password(from, None);
        }
    }
}

/// Copies a folder and everything in it.
fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
