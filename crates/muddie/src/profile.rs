//! Profiles: a saved connection with its own scripts and map, kept under
//! `profiles/<name>/` in the configuration folder.
//!
//! ```text
//! muddie/
//!   init.luau                 run for every profile
//!   packages/<package>/init.luau
//!   profiles/<name>/profile.toml
//!   profiles/<name>/scripts/*.luau
//!   profiles/<name>/map.sqlite
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What `profile.toml` holds. Missing fields take their defaults, so files
/// written by older versions keep loading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    /// Turn off for servers with a self-signed certificate.
    pub verify_certificate: bool,
    /// Character name to log in with; its password is in the system's
    /// keyring, never in this file.
    pub character: String,
    /// Label of the server's text encoding; `auto` suits most servers.
    pub encoding: String,
    /// Script packages to load before the profile's own scripts.
    pub packages: Vec<String>,
    /// The plugins that come with Muddie: those switched on that are
    /// off unless asked for, those switched off that are on unless
    /// asked not, and those offered for this MUD and declined.
    pub included_on: Vec<String>,
    pub included_off: Vec<String>,
    pub included_declined: Vec<String>,
    /// Those installed in it, each with the mark of what it was made of
    /// when it was (see [`crate::bundled::Bundled::fingerprint`]).
    pub included_installed: std::collections::BTreeMap<String, String>,
    /// How its sessions go: none in a profile saved before they were
    /// kept with it, which then has those of the settings.
    pub session: Option<SessionPrefs>,
}

/// What depends on the MUD more than on the person: whether to connect
/// again, keep the connection alive, log, and fetch sounds. Kept with a
/// profile; a session without one has those of the settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionPrefs {
    pub reconnect: bool,
    /// Seconds between keep-alive messages while idle; 0 sends none.
    pub keepalive_seconds: u32,
    pub log_sessions: bool,
    pub log_format: crate::settings::LogFormat,
    pub media_downloads: bool,
    /// The MUD's channels not read aloud, by name.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub quiet_channels: Vec<String>,
}

impl Default for SessionPrefs {
    fn default() -> Self {
        Self::from(&crate::settings::Settings::default())
    }
}

impl SessionPrefs {
    pub fn from(settings: &crate::settings::Settings) -> Self {
        Self {
            reconnect: settings.reconnect,
            keepalive_seconds: settings.keepalive_seconds,
            log_sessions: settings.log_sessions,
            log_format: settings.log_format,
            media_downloads: settings.media_downloads,
            quiet_channels: Vec::new(),
        }
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 23,
            tls: false,
            verify_certificate: true,
            character: String::new(),
            encoding: "auto".into(),
            packages: Vec::new(),
            included_on: Vec::new(),
            included_off: Vec::new(),
            included_declined: Vec::new(),
            included_installed: Default::default(),
            session: None,
        }
    }
}

/// Whether `name` can name a profile or package: it becomes a folder name,
/// so only letters, digits, spaces, `-` and `_` are allowed.
pub fn valid_name(name: &str) -> bool {
    let allowed = |ch: char| ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_');
    !name.trim().is_empty() && name == name.trim() && name.chars().all(allowed)
}

/// The configuration folder and what is stored in it.
#[derive(Debug, Clone)]
pub struct Profiles {
    dir: PathBuf,
}

/// The folders of the user that Muddie keeps things in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserDir {
    /// Settings, profiles, scripts: what the user made.
    Config,
    /// What can be fetched or worked out again.
    Cache,
    /// What the program gathered, such as the maps.
    Data,
}

/// Muddie's folder of a kind, if the system says where such things go:
/// where the desktop says, or the usual place under the home folder.
pub fn user_dir(kind: UserDir) -> Option<PathBuf> {
    // Tests keep what they write apart from the user's own: their maps
    // and fetched files were left among the user's.
    if cfg!(test) {
        let run = std::env::temp_dir().join(format!("muddie-tests-{}", std::process::id()));
        return Some(run.join(format!("{kind:?}").to_lowercase()).join("muddie"));
    }
    let (named, windows, usual) = match kind {
        UserDir::Config => ("XDG_CONFIG_HOME", "APPDATA", ".config"),
        UserDir::Cache => ("XDG_CACHE_HOME", "LOCALAPPDATA", ".cache"),
        UserDir::Data => ("XDG_DATA_HOME", "APPDATA", ".local/share"),
    };
    let base = (std::env::var_os(named).or_else(|| std::env::var_os(windows)))
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(usual)))?;
    Some(base.join("muddie"))
}

impl Profiles {
    /// The user's configuration folder, if the system has one.
    pub fn in_config_dir() -> Option<Self> {
        Some(Self::at(user_dir(UserDir::Config)?))
    }

    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Where translations of the user's own are looked for.
    pub fn locale_dir(&self) -> PathBuf {
        self.dir.join("locale")
    }

    /// Where the settings shared by all profiles are kept.
    pub fn settings_path(&self) -> PathBuf {
        self.dir.join("settings.toml")
    }

    /// The script run for every profile, if the user wrote one.
    pub fn init_script(&self) -> Option<PathBuf> {
        let path = self.dir.join("init.luau");
        path.is_file().then_some(path)
    }

    /// Names of the saved profiles, in alphabetical order.
    pub fn names(&self) -> Vec<String> {
        self.folders_with("profiles", "profile.toml")
    }

    /// Where templates for new profiles are read from, besides those that
    /// come with Muddie.
    pub fn templates_dir(&self) -> PathBuf {
        self.dir.join("templates")
    }

    /// Where script packages are installed.
    pub fn packages_dir(&self) -> PathBuf {
        self.dir.join("packages")
    }

    /// Names of the installed script packages, in alphabetical order.
    pub fn packages(&self) -> Vec<String> {
        self.folders_with("packages", "init.luau")
    }

    fn folders_with(&self, parent: &str, file: &str) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.dir.join(parent)) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().join(file).is_file())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        names.sort();
        names
    }

    fn profile_dir(&self, name: &str) -> PathBuf {
        self.dir.join("profiles").join(name)
    }

    pub fn load(&self, name: &str) -> Result<Profile, String> {
        let path = self.profile_dir(name).join("profile.toml");
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Writes the profile, creating its folder and an empty `scripts`
    /// folder for the user to fill.
    pub fn save(&self, name: &str, profile: &Profile) -> Result<(), String> {
        if !valid_name(name) {
            return Err("a profile name may only have letters, digits, spaces, - and _".into());
        }
        let dir = self.profile_dir(name);
        let text = toml::to_string_pretty(profile).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(dir.join("scripts"))
            .and_then(|()| std::fs::write(dir.join("profile.toml"), text))
            .map_err(|error| format!("cannot save the profile in {}: {error}", dir.display()))
    }

    /// The scripts a profile runs, in order: its packages, then the files
    /// in its `scripts` folder by name. A package that is not installed is
    /// still listed, so loading it reports the problem.
    pub fn scripts(&self, name: &str, profile: &Profile) -> Vec<PathBuf> {
        let packages = (profile.packages.iter())
            .map(|package| self.dir.join("packages").join(package).join("init.luau"));
        packages.chain(self.own_scripts(name)).collect()
    }

    /// Where a profile's own scripts are kept.
    pub fn scripts_dir(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join("scripts")
    }

    /// A profile's own scripts, in its folder and the folders in it, in
    /// alphabetical order of where they are.
    fn own_scripts(&self, name: &str) -> Vec<PathBuf> {
        let mut files = Vec::new();
        walk(&self.scripts_dir(name), &mut |path| {
            if path
                .extension()
                .is_some_and(|extension| extension == "luau")
            {
                files.push(path.to_owned());
            }
        });
        files.sort();
        files
    }

    /// The folders among a profile's own scripts, as places such as
    /// `Combat/Healing`, in alphabetical order.
    pub fn script_folders(&self, name: &str) -> Vec<String> {
        let dir = self.scripts_dir(name);
        let mut folders = Vec::new();
        walk(&dir, &mut |path| {
            if path.is_dir()
                && let Ok(inner) = path.strip_prefix(&dir)
            {
                folders.push(inner.to_string_lossy().replace('\\', "/"));
            }
        });
        folders.sort();
        folders
    }

    /// The scripts the user can edit: the one run in every session, there
    /// or not, then the profile's own.
    pub fn editable_scripts(&self, name: Option<&str>) -> Vec<PathBuf> {
        let mut files = vec![self.dir.join("init.luau")];
        files.extend(
            name.map(|name| self.own_scripts(name))
                .into_iter()
                .flatten(),
        );
        files
    }

    /// Where scripts' remembered values are kept: with the profile, or
    /// with the general configuration when no profile is in use.
    pub fn memory_path(&self, name: Option<&str>) -> PathBuf {
        match name {
            Some(name) => self.profile_dir(name).join("data.json"),
            None => self.dir.join("data.json"),
        }
    }

    /// Where the panels and their places are kept.
    pub fn layout_path(&self, name: Option<&str>) -> PathBuf {
        match name {
            Some(name) => self.profile_dir(name).join("layout.toml"),
            None => self.dir.join("layout.toml"),
        }
    }

    /// Where the triggers, aliases and keys made in the interface are kept.
    pub fn rules_path(&self, name: Option<&str>) -> PathBuf {
        match name {
            Some(name) => self.profile_dir(name).join("rules.toml"),
            None => self.dir.join("rules.toml"),
        }
    }

    /// Where sound files are looked for by name: the profile's `sounds`
    /// folder, then the general one.
    pub fn sound_dirs(&self, name: Option<&str>) -> Vec<PathBuf> {
        let own = name.map(|name| self.profile_dir(name).join("sounds"));
        own.into_iter().chain([self.dir.join("sounds")]).collect()
    }

    /// Where session logs are written.
    pub fn logs_dir(&self) -> PathBuf {
        self.dir.join("logs")
    }

    /// Where a profile's map is stored.
    pub fn map_path(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join("map.sqlite")
    }

    /// Whether a profile of this name is saved.
    pub fn exists(&self, name: &str) -> bool {
        self.profile_dir(name).join("profile.toml").is_file()
    }

    /// Gives a profile another name, with all it has: its scripts, rules,
    /// layout, map and remembered values. Its password goes with it.
    pub fn rename(&self, from: &str, to: &str) -> Result<(), String> {
        self.free_name(to)?;
        std::fs::rename(self.profile_dir(from), self.profile_dir(to))
            .map_err(|error| format!("cannot rename the profile: {error}"))?;
        move_password(from, to, true);
        Ok(())
    }

    /// Makes a copy of a profile under another name, with all it has.
    pub fn duplicate(&self, from: &str, to: &str) -> Result<(), String> {
        self.free_name(to)?;
        copy_dir(&self.profile_dir(from), &self.profile_dir(to))
            .map_err(|error| format!("cannot copy the profile: {error}"))?;
        move_password(from, to, false);
        Ok(())
    }

    /// Deletes a profile, with all it has, and forgets its password.
    pub fn delete(&self, name: &str) -> Result<(), String> {
        std::fs::remove_dir_all(self.profile_dir(name))
            .map_err(|error| format!("cannot delete the profile: {error}"))?;
        let _ = crate::secrets::set_password(name, None);
        Ok(())
    }

    /// A name a new profile can take: allowed, and no other's.
    fn free_name(&self, name: &str) -> Result<(), String> {
        if !valid_name(name) {
            return Err("a profile name may only have letters, digits, spaces, - and _".into());
        }
        if self.profile_dir(name).exists() {
            return Err(format!("there is a profile called {name} already"));
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

/// Goes through what is in a folder and in the folders in it, the folders
/// too.
fn walk(dir: &Path, found: &mut impl FnMut(&Path)) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        found(&path);
        if path.is_dir() {
            walk(&path, found);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(test: &str) -> Profiles {
        let dir =
            std::env::temp_dir().join(format!("muddie-profile-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Profiles::at(dir)
    }

    #[test]
    fn a_profile_is_renamed_copied_and_deleted_with_all_it_has() {
        let profiles = scratch("folders");
        profiles
            .save(
                "Reinos",
                &Profile {
                    port: 5001,
                    ..Profile::default()
                },
            )
            .unwrap();
        std::fs::write(profiles.scripts_dir("Reinos").join("a.luau"), "x = 1").unwrap();
        std::fs::write(profiles.map_path("Reinos"), "map").unwrap();
        profiles.duplicate("Reinos", "Reinos 2").unwrap();
        assert_eq!(profiles.load("Reinos 2").unwrap().port, 5001);
        assert!(profiles.scripts_dir("Reinos 2").join("a.luau").is_file());
        assert!(profiles.map_path("Reinos 2").is_file());
        // Not over another, nor under a name that cannot be a folder's.
        assert!(profiles.duplicate("Reinos", "Reinos 2").is_err());
        assert!(profiles.rename("Reinos", "a/b").is_err());
        profiles.rename("Reinos 2", "Naylhena").unwrap();
        assert_eq!(profiles.names(), ["Naylhena", "Reinos"]);
        assert_eq!(profiles.unused_name("Reinos"), "Reinos 2");
        assert_eq!(profiles.unused_name("Nuevo"), "Nuevo");
        profiles.delete("Naylhena").unwrap();
        assert_eq!(profiles.names(), ["Reinos"]);
        std::fs::remove_dir_all(profiles.dir).unwrap();
    }

    #[test]
    fn profile_round_trips_and_lists() {
        let profiles = scratch("roundtrip");
        assert_eq!(profiles.names(), Vec::<String>::new());
        let profile = Profile {
            host: "mud.example.org".into(),
            port: 4000,
            tls: true,
            verify_certificate: false,
            character: "Gandalf".into(),
            encoding: "koi8-r".into(),
            packages: vec!["combat".into()],
            included_on: vec!["reinos-de-leyenda".into()],
            included_off: vec!["common".into()],
            included_declined: Vec::new(),
            included_installed: Default::default(),
            session: Some(SessionPrefs {
                reconnect: false,
                ..SessionPrefs::default()
            }),
        };
        profiles.save("Reinos", &profile).unwrap();
        profiles.save("Aardwolf", &Profile::default()).unwrap();
        assert_eq!(profiles.names(), ["Aardwolf", "Reinos"]);
        assert_eq!(profiles.load("Reinos").unwrap(), profile);
        assert!(profiles.load("Nope").is_err());
        assert_eq!(
            profiles.map_path("Reinos"),
            profiles.dir.join("profiles/Reinos/map.sqlite")
        );
        std::fs::remove_dir_all(&profiles.dir).unwrap();
    }

    #[test]
    fn file_with_missing_fields_takes_defaults() {
        let profiles = scratch("defaults");
        let dir = profiles.dir.join("profiles/Old");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("profile.toml"), "host = \"old.example.org\"\n").unwrap();
        let profile = profiles.load("Old").unwrap();
        assert_eq!(
            (profile.host.as_str(), profile.port),
            ("old.example.org", 23)
        );
        assert!(profile.verify_certificate);
        std::fs::write(dir.join("profile.toml"), "port = \"many\"\n").unwrap();
        assert!(profiles.load("Old").unwrap_err().contains("profile.toml"));
        std::fs::remove_dir_all(&profiles.dir).unwrap();
    }

    #[test]
    fn names_that_are_not_plain_are_refused() {
        let profiles = scratch("names");
        for name in ["", " ", "../evil", "a/b", " padded", "dot.dot"] {
            assert!(!valid_name(name), "{name:?}");
            assert!(profiles.save(name, &Profile::default()).is_err());
        }
        assert!(valid_name("Reinos de Leyenda-2_b"));
        assert!(!profiles.dir.exists());
    }

    #[test]
    fn scripts_are_packages_then_own_files_in_order() {
        let profiles = scratch("scripts");
        let profile = Profile {
            packages: vec!["combat".into(), "missing".into()],
            ..Profile::default()
        };
        profiles.save("Main", &profile).unwrap();
        let own = profiles.dir.join("profiles/Main/scripts");
        for file in ["20-triggers.luau", "10-setup.luau", "notes.txt"] {
            std::fs::write(own.join(file), "").unwrap();
        }
        let package = profiles.dir.join("packages/combat");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("init.luau"), "").unwrap();
        std::fs::create_dir_all(profiles.dir.join("packages/empty")).unwrap();
        assert_eq!(profiles.packages(), ["combat"]);
        let relative: Vec<String> = (profiles.scripts("Main", &profile).iter())
            .map(|path| {
                // Each part, joined with / on every system.
                let inner = path.strip_prefix(&profiles.dir).unwrap();
                let parts: Vec<_> = inner.iter().map(|part| part.to_string_lossy()).collect();
                parts.join("/")
            })
            .collect();
        assert_eq!(
            relative,
            [
                "packages/combat/init.luau",
                "packages/missing/init.luau",
                "profiles/Main/scripts/10-setup.luau",
                "profiles/Main/scripts/20-triggers.luau",
            ]
        );
        assert_eq!(profiles.init_script(), None);
        std::fs::write(profiles.dir.join("init.luau"), "").unwrap();
        assert!(profiles.init_script().is_some());
        std::fs::remove_dir_all(&profiles.dir).unwrap();
    }
}
