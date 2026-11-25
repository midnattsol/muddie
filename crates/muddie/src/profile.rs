//! Profiles: a saved connection with its own scripts and map, kept under
//! `profiles/<name>/` in the configuration folder.
//!
//! ```text
//! muddie/
//!   init.luau                 run for every profile
//!   plugins/<plugin>/*.luau    downloaded, to install in profiles
//!   profiles/<name>/profile.toml
//!   profiles/<name>/scripts/*.luau
//!   profiles/<name>/map.sqlite
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

mod store;

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
    /// The language its MUD is played in, by its code; empty for the one
    /// its plugin or the MUD says, else English.
    pub language: String,
    /// The plugins: those switched on that are off unless asked for,
    /// those switched off that are on unless asked not, and those offered
    /// for this MUD and declined.
    #[serde(alias = "included_on")]
    pub plugins_on: Vec<String>,
    #[serde(alias = "included_off")]
    pub plugins_off: Vec<String>,
    #[serde(alias = "included_declined")]
    pub plugins_declined: Vec<String>,
    /// Those installed in it, each with the mark of what it was made of
    /// when it was (see [`crate::plugins::Plugin::fingerprint`]).
    #[serde(alias = "included_installed")]
    pub plugins_installed: std::collections::BTreeMap<String, String>,
    /// The plugins downloaded that a profile of an earlier version ran
    /// from where they were downloaded: on loading, they are switched on,
    /// to be installed in it as the others are.
    #[serde(default, skip_serializing, rename = "packages")]
    pub(crate) old_packages: Vec<String>,
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
    pub log_format: crate::logger::LogFormat,
    pub media_downloads: bool,
    /// The MUD's channels not read aloud, by name.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub quiet_channels: Vec<String>,
}

/// How a session goes, for a profile that says nothing of it and for a
/// session with no profile.
impl Default for SessionPrefs {
    fn default() -> Self {
        Self {
            reconnect: true,
            keepalive_seconds: 60,
            log_sessions: false,
            log_format: crate::logger::LogFormat::default(),
            media_downloads: true,
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
            language: String::new(),
            old_packages: Vec::new(),
            plugins_on: Vec::new(),
            plugins_off: Vec::new(),
            plugins_declined: Vec::new(),
            plugins_installed: Default::default(),
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

/// What a profile's name may have, said.
fn name_rule() -> String {
    crate::i18n::tr("A profile name may only have letters, digits, spaces, - and _.").to_owned()
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

    /// Where more languages of MUDs are kept (see [`crate::languages`]).
    pub fn languages_dir(&self) -> PathBuf {
        self.dir.join("languages")
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
        let Ok(entries) = std::fs::read_dir(self.dir.join("profiles")) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().join("profile.toml").is_file())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        names.sort();
        names
    }

    /// Where templates for new profiles are read from, besides those that
    /// come with Muddie.
    pub fn templates_dir(&self) -> PathBuf {
        self.dir.join("templates")
    }

    /// Where plugins are downloaded to: `packages`, as an earlier version
    /// called it, is called so once it is looked for.
    pub fn plugins_dir(&self) -> PathBuf {
        let (folder, before) = (self.dir.join("plugins"), self.dir.join("packages"));
        if !folder.exists() && before.is_dir() {
            let _ = std::fs::rename(&before, &folder);
        }
        folder
    }

    fn profile_dir(&self, name: &str) -> PathBuf {
        self.dir.join("profiles").join(name)
    }

    /// Where a profile's own scripts are kept.
    pub fn scripts_dir(&self, name: &str) -> PathBuf {
        self.profile_dir(name).join("scripts")
    }

    /// A profile's own scripts, in its folder and the folders in it, in
    /// alphabetical order of where they are.
    pub fn own_scripts(&self, name: &str) -> Vec<PathBuf> {
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
mod tests;
