//! Templates for new profiles: a game, where it is played, and the
//! plugins it is played with. The official plugins bring theirs, as a
//! `template.toml` in their folder; more are read from the `templates`
//! folder of the configuration, one TOML file each, named as the one that
//! comes:
//!
//! ```toml
//! name = "Some Game"
//! host = "mud.example.org"
//! port = 4000
//! tls = false
//! encoding = "auto"
//! language = "en"
//! plugins = ["some-game"]
//! ```

use std::path::Path;

use serde::Deserialize;

use crate::profile::{Profile, Profiles};

/// What a profile made from a template starts with.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    /// The game, which the profile is named after.
    pub name: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub tls: bool,
    /// The server's text encoding; `auto` if not given.
    #[serde(default)]
    pub encoding: Option<String>,
    /// The language the game is played in, by its code; as the plugin or
    /// the MUD says if not given.
    #[serde(default)]
    pub language: Option<String>,
    /// The plugins switched on, by their ids.
    #[serde(default)]
    pub plugins: Vec<String>,
}

impl Template {
    /// Puts what it starts with into a new profile.
    pub fn fill(&self, profile: &mut Profile) {
        profile.host.clone_from(&self.host);
        profile.port = self.port;
        profile.tls = self.tls;
        if let Some(encoding) = &self.encoding {
            profile.encoding.clone_from(encoding);
        }
        if let Some(language) = &self.language {
            profile.language.clone_from(language);
        }
        // Any plugin, by its id: one that comes with Muddie as it says,
        // any other switched on, to be installed once there.
        for id in &self.plugins {
            match crate::plugins::included(id) {
                Some(plugin) => {
                    plugin.switch(&mut profile.plugins_on, &mut profile.plugins_off, true)
                }
                None if !profile.plugins_on.contains(id) => profile.plugins_on.push(id.clone()),
                None => {}
            }
        }
    }
}

/// The templates in `files`, in order of name, and why those that cannot be
/// read are left out.
fn read(files: Vec<std::path::PathBuf>) -> (Vec<Template>, Vec<String>) {
    let (mut templates, mut problems) = (Vec::new(), Vec::new());
    for path in files {
        let read = std::fs::read_to_string(&path).map_err(|error| error.to_string());
        match read
            .and_then(|text| toml::from_str::<Template>(&text).map_err(|error| error.to_string()))
        {
            Ok(template) => templates.push(template),
            Err(error) => problems.push(format!("{}: {error}", path.display())),
        }
    }
    (templates, problems)
}

/// The `.toml` files in `folder`, in order of name.
fn toml_files(folder: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = (std::fs::read_dir(folder).into_iter().flatten().flatten())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect();
    files.sort();
    files
}

/// The `template.toml` of each official plugin, in order of their folders.
fn official_files(folder: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = crate::plugins::folders(folder)
        .map(|plugin| plugin.join("template.toml"))
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    files
}

/// The templates: those of the official plugins, then those in the
/// configuration's folder in order of name; one of these of the same name
/// takes the place of one before. One that cannot be read is left out, with
/// why, for it to be said.
pub fn all(profiles: Option<&Profiles>) -> (Vec<Template>, Vec<String>) {
    let Some(profiles) = profiles else {
        return (Vec::new(), Vec::new());
    };
    let mut files = official_files(&profiles.official_dir());
    files.extend(toml_files(&profiles.templates_dir()));
    let (found, problems) = read(files);
    let mut templates: Vec<Template> = Vec::new();
    for template in found {
        templates.retain(|known| known.name != template.name);
        templates.push(template);
    }
    (templates, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let folder =
            std::env::temp_dir().join(format!("muddie-templates-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn templates_come_from_the_official_plugins_and_the_configuration() {
        let dir = folder("all");
        let profiles = Profiles::at(dir.clone());
        assert_eq!(all(None), (Vec::new(), Vec::new()));
        // Nothing downloaded, nothing of the user's: none.
        assert_eq!(all(Some(&profiles)), (Vec::new(), Vec::new()));
        let plugin = profiles.official_dir().join("reinos-de-leyenda");
        std::fs::create_dir_all(&plugin).unwrap();
        std::fs::write(
            plugin.join("template.toml"),
            crate::plugins::fixture::TEMPLATE,
        )
        .unwrap();
        // A plugin with no template of its own brings none.
        std::fs::create_dir_all(profiles.official_dir().join("other")).unwrap();
        let templates = profiles.templates_dir();
        std::fs::create_dir_all(&templates).unwrap();
        std::fs::write(
            templates.join("a.toml"),
            "name = \"Otro\"\nhost = \"otro.example\"\nport = 4000\n",
        )
        .unwrap();
        std::fs::write(
            templates.join("b.toml"),
            "name = \"Reinos de Leyenda\"\nhost = \"pruebas.example\"\nport = 6000\n",
        )
        .unwrap();
        std::fs::write(templates.join("c.toml"), "name = \"Roto\"\n").unwrap();
        let (found, problems) = all(Some(&profiles));
        let names: Vec<&str> = found
            .iter()
            .map(|template| template.name.as_str())
            .collect();
        assert_eq!(names, ["Otro", "Reinos de Leyenda"]);
        assert_eq!(found[1].host, "pruebas.example");
        assert!(
            problems.len() == 1 && problems[0].contains("c.toml"),
            "{problems:?}"
        );
        // Without the user's, the official one is as it came.
        std::fs::remove_dir_all(&templates).unwrap();
        let (found, _) = all(Some(&profiles));
        let mut profile = Profile::default();
        found[0].fill(&mut profile);
        assert_eq!(
            (profile.host.as_str(), profile.port),
            ("reinosdeleyenda.es", 5001)
        );
        assert_eq!(profile.plugins_on, ["reinos-de-leyenda"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
