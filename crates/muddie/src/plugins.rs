//! Plugins: what makes Muddie know the ways of a MUD, so that the client
//! itself knows those of none. Some come with Muddie; others are downloaded
//! from a git repository. Either is the same thing: scripts, and the rules
//! they bring, so far gauges.
//!
//! A plugin is installed in a profile when it is first switched on there:
//! its scripts in a folder of the profile's named after it, its gauges
//! among the profile's in a folder of the same name, all in sight in the
//! editor and the profile's to change. Switched off, those folders are; it
//! is not installed again unless asked, and then what it installed is put
//! back as it came. One for a MUD is offered when that MUD is connected to.

use std::path::Path;

use crate::profile::Profiles;

pub mod git;

/// A plugin that comes with the program, as it is written in it.
#[derive(Debug, PartialEq, Eq)]
pub struct Included {
    /// What it is known by in a profile.
    pub id: &'static str,
    /// What it is called and what it does, in English: both are given to
    /// `tr` to be shown.
    pub name: &'static str,
    pub about: &'static str,
    /// Its scripts, each by the name of its file.
    pub scripts: &'static [(&'static str, &'static str)],
    /// Its rules, as in a profile's `rules.toml`: so far, its gauges.
    pub rules: &'static str,
    /// On in a profile until switched off, rather than off until
    /// switched on.
    pub default_on: bool,
    /// The MUDs it is for: part of the address of one, or the name one
    /// gives itself. None for a plugin that is for no MUD in particular.
    pub hosts: &'static [&'static str],
    pub names: &'static [&'static str],
}

/// What many MUDs send the same way, after those that did it first.
pub const COMMON: &str = "common";

pub const INCLUDED: [Included; 2] = [
    Included {
        id: COMMON,
        name: "Standard GMCP and MSDP",
        about: "Gauges from Char.Vitals, the map from Room.Info, channels from Comm.Channel.Text.",
        scripts: &[(
            "Standard GMCP and MSDP.luau",
            include_str!("../plugins/standard/Standard GMCP and MSDP.luau"),
        )],
        rules: include_str!("../plugins/standard/rules.toml"),
        default_on: true,
        hosts: &[],
        names: &[],
    },
    Included {
        id: "reinos-de-leyenda",
        name: "Reinos de Leyenda",
        about: "Its gauges, channels and map. Needs consentir gmcp on, typed in the game.",
        scripts: &[(
            "Reinos de Leyenda.luau",
            include_str!("../plugins/reinos-de-leyenda/Reinos de Leyenda.luau"),
        )],
        rules: include_str!("../plugins/reinos-de-leyenda/rules.toml"),
        default_on: false,
        hosts: &["rlmud.org", "reinosdeleyenda"],
        names: &["reinos de leyenda"],
    },
];

/// A plugin, whether it comes with Muddie or was downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    /// What it is known by in a profile.
    pub id: String,
    /// What it is called, which is the folder it is installed in.
    pub name: String,
    /// Its scripts, each by the name of its file.
    pub scripts: Vec<(String, String)>,
    /// Its rules, as in a profile's `rules.toml`.
    pub rules: String,
    /// The one that comes with Muddie it is; none for one downloaded,
    /// which can be updated and removed.
    pub included: Option<&'static Included>,
}

impl From<&'static Included> for Plugin {
    fn from(included: &'static Included) -> Self {
        Self {
            id: included.id.to_owned(),
            name: included.name.to_owned(),
            scripts: (included.scripts.iter())
                .map(|(name, code)| ((*name).to_owned(), (*code).to_owned()))
                .collect(),
            rules: included.rules.to_owned(),
            included: Some(included),
        }
    }
}

impl Plugin {
    /// One downloaded into `folder`: its `.luau` files, and its
    /// `rules.toml` if it has one. `None` if it has no script.
    pub fn downloaded(folder: &Path) -> Option<Self> {
        let name = folder.file_name()?.to_str()?.to_owned();
        let mut scripts: Vec<(String, String)> = (std::fs::read_dir(folder).ok()?.flatten())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "luau")
            })
            .filter_map(|path| {
                let file = path.file_name()?.to_str()?.to_owned();
                Some((file, std::fs::read_to_string(&path).ok()?))
            })
            .collect();
        if scripts.is_empty() {
            return None;
        }
        scripts.sort();
        let rules = std::fs::read_to_string(folder.join("rules.toml")).unwrap_or_default();
        Some(Self {
            id: name.clone(),
            name,
            scripts,
            rules,
            included: None,
        })
    }

    /// What it is called, as shown.
    pub fn shown_name(&self) -> String {
        match self.included {
            Some(included) => crate::i18n::tr(included.name).to_owned(),
            None => self.name.clone(),
        }
    }

    /// What it does, as shown; for one downloaded, where it came from.
    pub fn about(&self) -> String {
        match self.included {
            Some(included) => crate::i18n::tr(included.about).to_owned(),
            None => crate::i18n::tr("Downloaded from a git repository.").to_owned(),
        }
    }

    /// The folder its scripts and gauges are put in, which is what it is
    /// called.
    pub fn folder(&self) -> &str {
        &self.name
    }

    /// Its rules, read; or what is wrong with them.
    pub fn read_rules(&self) -> Result<crate::rules::Rules, String> {
        toml::from_str(&self.rules).map_err(|error| error.to_string())
    }

    /// Whether it is on until switched off.
    pub fn default_on(&self) -> bool {
        self.included.is_some_and(|included| included.default_on)
    }

    /// A mark of what it is made of, to tell when the one there is is not
    /// the one installed: the same on every build.
    pub fn fingerprint(&self) -> String {
        // FNV-1a, which does not change between builds as the standard
        // hasher may.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let parts = (self.scripts.iter()).flat_map(|(name, code)| [name.as_str(), code.as_str()]);
        for part in parts.chain([self.rules.as_str()]) {
            for byte in part.bytes().chain([0]) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        format!("{hash:016x}")
    }

    /// Whether it is on in a profile that lists these as switched on and
    /// off: one on unless asked not is on unless in `off`, and the other
    /// way round.
    pub fn is_on(&self, on: &[String], off: &[String]) -> bool {
        let listed = |list: &[String]| list.contains(&self.id);
        if self.default_on() {
            !listed(off)
        } else {
            listed(on)
        }
    }

    /// Switches it in those lists, writing down only what differs from
    /// how it is to begin with.
    pub fn switch(&self, on: &mut Vec<String>, off: &mut Vec<String>, wanted: bool) {
        on.retain(|id| *id != self.id);
        off.retain(|id| *id != self.id);
        match (self.default_on(), wanted) {
            (false, true) => on.push(self.id.clone()),
            (true, false) => off.push(self.id.clone()),
            _ => {}
        }
    }

    /// Whether it is made for a MUD, by its address or by the name it
    /// gives itself; either may be empty if it is not known.
    fn is_for(&self, host: &str, name: &str) -> bool {
        let Some(included) = self.included else {
            return false;
        };
        let (host, name) = (host.trim().to_lowercase(), name.trim().to_lowercase());
        let by_host = !host.is_empty() && included.hosts.iter().any(|part| host.contains(part));
        let by_name = !name.is_empty() && included.names.iter().any(|known| name == *known);
        by_host || by_name
    }
}

/// Every plugin there is: those that come with Muddie, then those
/// downloaded, in the order of their names.
pub fn all(profiles: Option<&Profiles>) -> Vec<Plugin> {
    let mut plugins: Vec<Plugin> = INCLUDED.iter().map(Plugin::from).collect();
    if let Some(profiles) = profiles {
        let folder = profiles.plugins_dir();
        let mut downloaded: Vec<Plugin> =
            (std::fs::read_dir(&folder).into_iter().flatten().flatten())
                .filter_map(|entry| Plugin::downloaded(&entry.path()))
                .filter(|plugin| !plugins.iter().any(|known| known.id == plugin.id))
                .collect();
        downloaded.sort_by(|a, b| a.name.cmp(&b.name));
        plugins.extend(downloaded);
    }
    plugins
}

/// The one that comes with Muddie of this id.
pub fn included(id: &str) -> Option<Plugin> {
    INCLUDED
        .iter()
        .find(|plugin| plugin.id == id)
        .map(Plugin::from)
}

/// The plugin made for a MUD, among `plugins`.
pub fn for_mud<'a>(plugins: &'a [Plugin], host: &str, name: &str) -> Option<&'a Plugin> {
    plugins.iter().find(|plugin| plugin.is_for(host, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mud_is_known_by_its_address_or_its_name() {
        let plugins = all(None);
        let found = |host, name| for_mud(&plugins, host, name).map(|plugin| plugin.id.as_str());
        assert_eq!(found("rlmud.org", ""), Some("reinos-de-leyenda"));
        assert_eq!(found("", "Reinos de Leyenda"), Some("reinos-de-leyenda"));
        assert_eq!(
            found("MUD.ReinosDeLeyenda.es", ""),
            Some("reinos-de-leyenda")
        );
        assert_eq!(found("mud.example.org", "Another MUD"), None);
        assert_eq!(found("", ""), None);
        // Every one has an id of its own, its scripts can be run and its
        // rules read.
        let mut ids: Vec<&str> = INCLUDED.iter().map(|plugin| plugin.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), INCLUDED.len());
        let engine = muddie_script::Engine::new(Default::default()).unwrap();
        for plugin in &INCLUDED {
            for (name, code) in plugin.scripts {
                assert_eq!(engine.check(code), None, "{} {name}", plugin.id);
            }
            let rules: crate::rules::Rules = toml::from_str(plugin.rules).unwrap();
            assert!(plugin.rules.is_empty() || !rules.gauges.is_empty());
        }
        // What they are made of marks each.
        let reinos = included("reinos-de-leyenda").unwrap();
        assert_ne!(
            reinos.fingerprint(),
            included(COMMON).unwrap().fingerprint()
        );
        assert_eq!(reinos.fingerprint(), reinos.fingerprint());
    }
}
