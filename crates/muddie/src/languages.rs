//! The languages MUDs are played in, as far as the map needs them: the
//! words each says directions with. Those that come with Muddie are in its
//! `languages` folder; more are read from the `languages` folder of the
//! configuration, one TOML file each, named by the language's code:
//!
//! ```toml
//! name = "Español"
//! names = ["Spanish", "Español", "es"]
//!
//! [directions]          # the first word is what a walk sends
//! n = ["norte", "n"]
//! ```
//!
//! A profile says which its MUD speaks; else its plugin, or the MUD itself
//! over MSSP; else English.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

/// The language of a MUD.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Language {
    /// Its code, the name of its file: `es`.
    #[serde(skip)]
    pub code: String,
    /// What it calls itself: `Español`.
    pub name: String,
    /// What else it is called, as a MUD may say it of itself.
    #[serde(default)]
    pub names: Vec<String>,
    /// The words for each direction the map knows, by its key; the first
    /// is what a walk sends.
    pub directions: BTreeMap<String, Vec<String>>,
}

/// Those that come with Muddie, by code.
const INCLUDED: [(&str, &str); 2] = [
    ("en", include_str!("../languages/en.toml")),
    ("es", include_str!("../languages/es.toml")),
];

/// The language played in when nothing says otherwise.
pub const DEFAULT: &str = "en";

/// The folder of the configuration more languages are read from, once it
/// is known.
static FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// Says where the configuration keeps more languages.
pub fn set_folder(folder: PathBuf) {
    let _ = FOLDER.set(folder);
}

impl Language {
    /// Reads one from its file's text. Says what is wrong with one that
    /// cannot be: a direction the map does not know is a mistake, as its
    /// words would mean nothing.
    fn read(code: &str, text: &str) -> Result<Self, String> {
        let mut language: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        language.code = code.to_owned();
        let unknown: Vec<&str> = (language.directions.keys())
            .map(String::as_str)
            .filter(|key| muddie_mapper::known_direction(key).is_none())
            .collect();
        if !unknown.is_empty() {
            return Err(format!("directions not known: {}", unknown.join(", ")));
        }
        Ok(language)
    }

    /// Its words for directions, as the map takes them.
    pub fn words(&self) -> Vec<(String, Vec<String>)> {
        (self.directions.iter())
            .map(|(key, words)| (key.clone(), words.clone()))
            .collect()
    }

    /// Whether `said` names it: its code, its name, or another of its
    /// names, whatever the case.
    pub fn is_called(&self, said: &str) -> bool {
        let said = said.trim();
        std::iter::once(&self.code)
            .chain(std::iter::once(&self.name))
            .chain(&self.names)
            .any(|name| name.eq_ignore_ascii_case(said))
    }
}

/// The languages: those that come with Muddie, then those in `folder`;
/// one there of the same code takes the place of one that comes. One that
/// cannot be read is left out, with why, for it to be said.
pub fn all_in(folder: Option<&Path>) -> (Vec<Language>, Vec<String>) {
    let mut languages: Vec<Language> = (INCLUDED.iter())
        .map(|(code, text)| {
            Language::read(code, text).unwrap_or_else(|error| panic!("{code}: {error}"))
        })
        .collect();
    let mut problems = Vec::new();
    let mut files: Vec<PathBuf> = (folder.and_then(|folder| std::fs::read_dir(folder).ok()))
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
        let code =
            (path.file_stem()).map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
        let read = std::fs::read_to_string(&path).map_err(|error| error.to_string());
        match read.and_then(|text| Language::read(&code, &text)) {
            Ok(language) => {
                languages.retain(|known| known.code != language.code);
                languages.push(language);
            }
            Err(error) => problems.push(format!("{}: {error}", path.display())),
        }
    }
    (languages, problems)
}

/// The languages read, kept till they are read again.
static READ: std::sync::Mutex<Option<(Vec<Language>, Vec<String>)>> = std::sync::Mutex::new(None);

/// The languages there are, with those of the configuration: read once,
/// and again after [`read_again`].
pub fn all() -> (Vec<Language>, Vec<String>) {
    let mut read = READ
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    read.get_or_insert_with(|| all_in(FOLDER.get().map(PathBuf::as_path)))
        .clone()
}

/// Reads the languages again when next asked for: one added or changed
/// in the configuration is taken then.
pub fn read_again() {
    *READ
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// The language called `said`, by its code or any of its names.
pub fn find(said: &str) -> Option<Language> {
    all()
        .0
        .into_iter()
        .find(|language| language.is_called(said))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_come_with_muddie_and_from_the_configuration() {
        let (included, problems) = all_in(None);
        assert!(problems.is_empty());
        // Each says every direction the map knows.
        for language in &included {
            for direction in &muddie_mapper::DIRECTIONS {
                assert!(
                    language.directions.contains_key(direction.key),
                    "{} {}",
                    language.code,
                    direction.key
                );
            }
        }
        let folder = std::env::temp_dir().join(format!("muddie-languages-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("fr.toml"),
            "name = \"Français\"\nnames = [\"French\"]\n[directions]\nn = [\"nord\", \"n\"]\n",
        )
        .unwrap();
        std::fs::write(
            folder.join("xx.toml"),
            "name = \"Roto\"\n[directions]\nnorthish = [\"x\"]\n",
        )
        .unwrap();
        let (languages, problems) = all_in(Some(&folder));
        let codes: Vec<&str> = languages
            .iter()
            .map(|language| language.code.as_str())
            .collect();
        assert_eq!(codes, ["en", "es", "fr"]);
        assert!(
            problems.len() == 1 && problems[0].contains("northish"),
            "{problems:?}"
        );
        // Found by its code or any of its names, whatever the case.
        let french = languages
            .iter()
            .find(|language| language.is_called("FRENCH"));
        assert_eq!(
            french.map(|language| language.name.as_str()),
            Some("Français")
        );
        assert!(languages[1].is_called("spanish") && languages[1].is_called("es"));
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
