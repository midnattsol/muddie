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

use std::path::PathBuf;

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
    pub speech: bool,
    /// Character name to log in with; its password is in the system's
    /// keyring, never in this file.
    pub character: String,
    /// Label of the server's text encoding; `auto` suits most servers.
    pub encoding: String,
    /// Script packages to load before the profile's own scripts.
    pub packages: Vec<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 23,
            tls: false,
            verify_certificate: true,
            speech: false,
            character: String::new(),
            encoding: "auto".into(),
            packages: Vec::new(),
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

impl Profiles {
    /// The user's configuration folder, if the system has one.
    pub fn in_config_dir() -> Option<Self> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .or_else(|| std::env::var_os("APPDATA"))
            .map(PathBuf::from)
            .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".config")))?;
        Some(Self::at(base.join("muddie")))
    }

    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
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
        let mut own: Vec<PathBuf> = std::fs::read_dir(self.profile_dir(name).join("scripts"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "luau")
            })
            .collect();
        own.sort();
        packages.chain(own).collect()
    }

    /// Where scripts' remembered values are kept: with the profile, or
    /// with the general configuration when no profile is in use.
    pub fn memory_path(&self, name: Option<&str>) -> PathBuf {
        match name {
            Some(name) => self.profile_dir(name).join("data.json"),
            None => self.dir.join("data.json"),
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
    fn profile_round_trips_and_lists() {
        let profiles = scratch("roundtrip");
        assert_eq!(profiles.names(), Vec::<String>::new());
        let profile = Profile {
            host: "mud.example.org".into(),
            port: 4000,
            tls: true,
            verify_certificate: false,
            speech: true,
            character: "Gandalf".into(),
            encoding: "koi8-r".into(),
            packages: vec!["combat".into()],
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
                path.strip_prefix(&profiles.dir)
                    .unwrap()
                    .display()
                    .to_string()
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
