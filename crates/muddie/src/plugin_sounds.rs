//! The sounds of a plugin, which are not part of it: its `plugin.toml` says
//! in which folder of the web they are, and Muddie fetches each as it is
//! needed, or all those a script names at once, and keeps them in the
//! user's cache. One kept is not fetched again: when the plugin points to
//! another version of the folder, each is asked for only if it changed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The largest sound fetched.
const LARGEST: u64 = 25 * 1024 * 1024;
const PATIENCE: Duration = Duration::from_secs(60);

/// What is known of each sound kept: from which folder of the web it was
/// fetched, and the version the web gave of it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Index {
    files: BTreeMap<String, Entry>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Entry {
    source: String,
    version: Option<String>,
}

/// The sounds of one plugin: where they are on the web, and where they
/// are kept.
#[derive(Debug, Clone)]
pub struct Kept {
    source: String,
    dir: PathBuf,
    index: Index,
}

/// A sound that came, or could not.
#[derive(Debug)]
pub enum Came {
    /// Kept: its name, and the version the web gave of it.
    Sound(String, Option<String>),
    /// Could not be had: its name, and why.
    Failed(String, String),
}

impl Kept {
    /// The sounds of the plugin `id`, which are in the folder `source` of
    /// the web: none if the cache cannot be found, or `source` is not an
    /// address of the web that is safe to fetch from.
    pub fn of(id: &str, source: &str) -> Option<Self> {
        let dir = crate::media::cache_dir()?.join("plugins").join(id);
        Self::at(dir, source)
    }

    /// The same, kept in `dir`.
    pub fn at(dir: PathBuf, source: &str) -> Option<Self> {
        let source = source.trim().trim_end_matches('/').to_owned();
        if !source.starts_with("https://") {
            return None;
        }
        let index = (std::fs::read_to_string(dir.join("index.json")).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Some(Self { source, dir, index })
    }

    /// Where the sound `name` is, if it is kept: it may be of an older
    /// version of the folder, to play until the newer comes.
    pub fn path(&self, name: &str) -> Option<PathBuf> {
        if !crate::media::safe_name(name) || !self.index.files.contains_key(name) {
            return None;
        }
        let path = self.dir.join(name);
        path.is_file().then_some(path)
    }

    /// Whether `name` is to be asked of the web: it is not kept, or it was
    /// fetched from another version of the folder.
    pub fn wanted(&self, name: &str) -> bool {
        crate::media::safe_name(name)
            && (self.path(name).is_none()
                || self
                    .index
                    .files
                    .get(name)
                    .is_none_or(|entry| entry.source != self.source))
    }

    /// Fetches `names`, away from the thread that asked, each asked for
    /// only if it changed from the version kept: each that comes, or
    /// could not, is told as it does.
    pub fn fetch(&self, names: Vec<String>) -> Receiver<Came> {
        let (tell, told) = channel();
        let known: Vec<(String, Option<String>)> = (names.into_iter())
            .filter(|name| crate::media::safe_name(name))
            .map(|name| {
                let version = (self.path(&name))
                    .and(self.index.files.get(&name))
                    .and_then(|entry| entry.version.clone());
                (name, version)
            })
            .collect();
        let (source, dir) = (self.source.clone(), self.dir.clone());
        std::thread::spawn(move || {
            for (name, version) in known {
                let came = fetch_one(&source, &dir, &name, version.as_deref());
                if tell.send(came).is_err() {
                    return;
                }
            }
        });
        told
    }

    /// Notes that `name` came, of the version `version` of the folder there
    /// is now, and keeps the note.
    pub fn came(&mut self, name: &str, version: Option<String>) {
        let entry = Entry {
            source: self.source.clone(),
            version,
        };
        self.index.files.insert(name.to_owned(), entry);
        if let Ok(text) = serde_json::to_string_pretty(&self.index) {
            let _ = std::fs::create_dir_all(&self.dir);
            let _ = std::fs::write(self.dir.join("index.json"), text);
        }
    }
}

/// Fetches one sound into `dir`, unless the web says the version `known`
/// is the one there is.
fn fetch_one(source: &str, dir: &Path, name: &str, known: Option<&str>) -> Came {
    let address = format!("{source}/{}", encoded(name));
    let fetched = crate::webfetch::get(&address, known, LARGEST, PATIENCE);
    let failed = |error: String| Came::Failed(name.to_owned(), error);
    match fetched {
        // The same as the one kept.
        Ok(None) => Came::Sound(name.to_owned(), known.map(str::to_owned)),
        Ok(Some((bytes, version))) => {
            let path = dir.join(name);
            let Some(folder) = path.parent() else {
                return failed("no folder to keep it in".to_owned());
            };
            if let Err(error) = std::fs::create_dir_all(folder) {
                return failed(error.to_string());
            }
            // Written whole under another name first, so that a file cut
            // short is never taken for the sound.
            let partial = path.with_extension("part");
            let written =
                std::fs::write(&partial, bytes).and_then(|()| std::fs::rename(&partial, &path));
            match written {
                Ok(()) => Came::Sound(name.to_owned(), version),
                Err(error) => failed(error.to_string()),
            }
        }
        Err(error) => failed(error),
    }
}

/// A name of a file as it goes in an address: each part of it with what
/// is not a letter, a figure or `-._~` written as `%` and its bytes, as
/// spaces and accents are.
fn encoded(name: &str) -> String {
    let part = |part: &str| {
        part.bytes()
            .map(|byte| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    (byte as char).to_string()
                }
                other => format!("%{other:02X}"),
            })
            .collect::<String>()
    };
    name.split('/').map(part).collect::<Vec<_>>().join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_written_as_an_address_takes_it() {
        assert_eq!(
            encoded("Combate/Alerta vida 30.wav"),
            "Combate/Alerta%20vida%2030.wav"
        );
        assert_eq!(
            encoded("Sucesos/Abrir caña.wav"),
            "Sucesos/Abrir%20ca%C3%B1a.wav"
        );
    }

    #[test]
    fn a_sound_kept_is_not_asked_for_until_the_folder_is_another() {
        let dir = std::env::temp_dir().join(format!("muddie-kept-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let old = "https://raw.example.org/sounds/abc";
        assert!(Kept::at(dir.clone(), "http://plain.example.org").is_none());
        let mut kept = Kept::at(dir.clone(), old).unwrap();
        let name = "Combate/Alerta vida 30.wav";
        assert!(kept.wanted(name) && kept.path(name).is_none());
        // Climbing out of the folder, or not a sound, is never asked for.
        assert!(!kept.wanted("../secrets.wav") && !kept.wanted("Combate/notes.txt"));
        std::fs::create_dir_all(dir.join("Combate")).unwrap();
        std::fs::write(dir.join(name), b"RIFF").unwrap();
        kept.came(name, Some("\"v1\"".into()));
        assert!(!kept.wanted(name));
        assert_eq!(kept.path(name), Some(dir.join(name)));
        // Read again, it is still kept.
        let again = Kept::at(dir.clone(), old).unwrap();
        assert!(!again.wanted(name));
        // Another version of the folder: asked for, still played meanwhile.
        let newer = Kept::at(dir.clone(), "https://raw.example.org/sounds/def/").unwrap();
        assert!(newer.wanted(name));
        assert_eq!(newer.path(name), Some(dir.join(name)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Fetches a sound of ScriptsRL from the web, and again, which is not
    /// sent again: `cargo test -p muddie fetched_from_the_web -- --ignored`.
    #[test]
    #[ignore = "asks the web"]
    fn fetched_from_the_web_once() {
        let dir = std::env::temp_dir().join(format!("muddie-web-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let source = "https://raw.githubusercontent.com/DPrenC/ScriptsRL/905b41d66ec05b7663b4abf11ff303a7b3493b4a/sounds/RL";
        let mut kept = Kept::at(dir.clone(), source).unwrap();
        let name = "Combate/Alerta vida 30.wav";
        let Ok(Came::Sound(_, version)) = kept.fetch(vec![name.into()]).recv() else {
            panic!("not fetched");
        };
        kept.came(name, version.clone());
        assert_eq!(std::fs::metadata(dir.join(name)).unwrap().len(), 187_290);
        // Another version of the folder, the same sound: not sent again.
        let mut newer = Kept::at(
            dir.clone(),
            "https://raw.githubusercontent.com/DPrenC/ScriptsRL/main/sounds/RL",
        )
        .unwrap();
        assert!(newer.wanted(name));
        let Ok(Came::Sound(_, again)) = newer.fetch(vec![name.into()]).recv() else {
            panic!("not asked");
        };
        assert_eq!(again, version);
        newer.came(name, again);
        assert!(!newer.wanted(name));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
