//! Templates for new profiles: a game, where it is played, and the
//! plugins it is played with. Those that come with Muddie are in its
//! `templates` folder; more are read from the `templates` folder of the
//! configuration, one TOML file each, named as the one that comes:
//!
//! ```toml
//! name = "Reinos de Leyenda"
//! host = "reinosdeleyenda.es"
//! port = 5001
//! tls = false
//! encoding = "auto"
//! language = "es"
//! plugins = ["reinos-de-leyenda"]
//! ```

use std::path::Path;

use serde::Deserialize;

use crate::profile::Profile;

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

/// Those that come with Muddie, by file.
const INCLUDED: [(&str, &str); 1] = [(
    "Reinos de Leyenda.toml",
    include_str!("../templates/Reinos de Leyenda.toml"),
)];

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
        // one downloaded switched on, to be installed once there.
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

/// The templates: those that come with Muddie, then those in `folder` in
/// order of name; one of these of the same name takes the place of one that
/// comes. One that cannot be read is left out, with why, for it to be said.
pub fn all(folder: Option<&Path>) -> (Vec<Template>, Vec<String>) {
    let mut templates: Vec<Template> = (INCLUDED.iter())
        .map(|(file, text)| toml::from_str(text).unwrap_or_else(|error| panic!("{file}: {error}")))
        .collect();
    let mut problems = Vec::new();
    let mut files: Vec<_> = (folder.and_then(|folder| std::fs::read_dir(folder).ok()))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect();
    files.sort();
    for path in files {
        let read = std::fs::read_to_string(&path).map_err(|error| error.to_string());
        match read
            .and_then(|text| toml::from_str::<Template>(&text).map_err(|error| error.to_string()))
        {
            Ok(template) => {
                templates.retain(|known| known.name != template.name);
                templates.push(template);
            }
            Err(error) => problems.push(format!("{}: {error}", path.display())),
        }
    }
    (templates, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_come_with_muddie_and_from_the_configuration() {
        let (included, problems) = all(None);
        assert!(problems.is_empty());
        // Each names plugins that come with Muddie.
        for template in &included {
            for id in &template.plugins {
                assert!(
                    crate::plugins::included(id).is_some(),
                    "{} {id}",
                    template.name
                );
            }
        }
        let folder = std::env::temp_dir().join(format!("muddie-templates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("a.toml"),
            "name = \"Otro\"\nhost = \"otro.example\"\nport = 4000\n",
        )
        .unwrap();
        std::fs::write(
            folder.join("b.toml"),
            "name = \"Reinos de Leyenda\"\nhost = \"pruebas.example\"\nport = 6000\n",
        )
        .unwrap();
        std::fs::write(folder.join("c.toml"), "name = \"Roto\"\n").unwrap();
        let (templates, problems) = all(Some(&folder));
        let names: Vec<&str> = templates
            .iter()
            .map(|template| template.name.as_str())
            .collect();
        assert_eq!(names, ["Otro", "Reinos de Leyenda"]);
        assert_eq!(templates[1].host, "pruebas.example");
        assert!(
            problems.len() == 1 && problems[0].contains("c.toml"),
            "{problems:?}"
        );
        let mut profile = Profile::default();
        included[0].fill(&mut profile);
        assert_eq!(
            (profile.host.as_str(), profile.port),
            ("reinosdeleyenda.es", 5001)
        );
        assert_eq!(profile.plugins_on, ["reinos-de-leyenda"]);
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
