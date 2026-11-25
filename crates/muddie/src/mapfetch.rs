//! Maps made with Mudlet, imported from a file or from the web, read away
//! from the thread that draws. A zip is opened in memory and only the map
//! in it is read: nothing it holds is written anywhere or run.

use std::io::Read;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use crate::webfetch::{self, Fetched};
use crate::zipread::{self, ZipError};

use muddie_mapper::{MudletMap, read_mudlet_map};

/// The most a map, or the zip it comes in, may take: Mudlet's maps of the
/// largest MUDs are a few tens of megabytes.
const LARGEST: u64 = 128 * 1024 * 1024;

/// What importing a map came to.
pub enum Imported {
    /// The map, and the version of it the web gave, to ask only for a
    /// newer one next time.
    Map {
        map: MudletMap,
        version: Option<String>,
    },
    /// The web has no newer version than the one known.
    Unchanged,
}

/// Where what importing a map came to arrives.
pub type Importing = Receiver<Result<Imported, String>>;

/// Where whether a map has changed arrives.
pub type Checking = Receiver<Result<bool, String>>;

/// Finds out, away from the thread that asked, whether the map at
/// `source` on the web is another than the version `known`, without
/// taking it: asked only for what tells its version. A file, or a web
/// that gives no version, is not said to have changed.
pub fn changed(source: String, known: Option<String>) -> Checking {
    let (done, result) = channel();
    std::thread::spawn(move || {
        let _ = done.send(version_of(&source).map(|version| match (version, known) {
            (Some(version), Some(known)) => version != known,
            (Some(_), None) => true,
            (None, _) => false,
        }));
    });
    result
}

/// The version the web gives of what is at `source`, without taking it;
/// none for a file, or if the web gives none.
fn version_of(source: &str) -> Result<Option<String>, String> {
    if !on_the_web(source) {
        return Ok(None);
    }
    let said = |error: &dyn std::fmt::Display| format!("{source}: {error}");
    // Tests never reach a server: plugins they load offer real maps.
    if cfg!(test) {
        return Err(said(&"the web is not reached while testing"));
    }
    let response = webfetch::agent(PATIENCE)
        .head(source.trim())
        .call()
        .map_err(|error| said(&error))?;
    Ok(webfetch::version_header(&response))
}

/// How long the web is waited for.
const PATIENCE: Duration = Duration::from_secs(120);

/// Whether `source` is an address on the web rather than a file.
pub fn on_the_web(source: &str) -> bool {
    let lower = source.trim().to_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Takes the map at `source`, a file or an address, away from the thread
/// that asked; from the web, only if newer than the version `known`.
pub fn import(source: String, known: Option<String>) -> Importing {
    let (done, result) = channel();
    std::thread::spawn(move || {
        let _ = done.send(fetch(&source, known.as_deref()).and_then(|fetched| {
            let Some((bytes, version)) = fetched else {
                return Ok(Imported::Unchanged);
            };
            let map = read_mudlet_map(&unpacked(bytes)?)?;
            Ok(Imported::Map { map, version })
        }));
    });
    result
}

/// The bytes at `source`, with the version the web gave of them; `None`
/// if the web has no newer version than `known`.
fn fetch(source: &str, known: Option<&str>) -> Result<Fetched, String> {
    let said = |error: &dyn std::fmt::Display| format!("{source}: {error}");
    if !on_the_web(source) {
        let file = std::fs::File::open(source.trim()).map_err(|error| said(&error))?;
        let mut bytes = Vec::new();
        (file.take(LARGEST + 1).read_to_end(&mut bytes)).map_err(|error| said(&error))?;
        if bytes.len() as u64 > LARGEST {
            return Err(said(&"too large to be a map"));
        }
        return Ok(Some((bytes, None)));
    }
    // Tests never reach a server: plugins they load offer real maps.
    if cfg!(test) {
        return Err(said(&"the web is not reached while testing"));
    }
    webfetch::get(source, known, LARGEST, PATIENCE).map_err(|error| said(&error))
}

/// The map in `bytes`: themselves, or the first `.dat` file of a zip.
fn unpacked(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return Ok(bytes);
    }
    let says = |error: ZipError| match error {
        ZipError::TooLarge => "the map in the zip is too large".to_owned(),
        other => other.message().to_owned(),
    };
    for entry in zipread::entries(&bytes, usize::from(u16::MAX)).map_err(says)? {
        let name = String::from_utf8_lossy(&entry.name).to_lowercase();
        if name.ends_with(".dat") && !name.starts_with("__macosx/") {
            return zipread::data(&bytes, &entry, LARGEST).map_err(says);
        }
    }
    Err("there is no map (.dat) in the zip".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zipread::make::zip;

    #[test]
    fn the_map_in_a_zip_is_found_and_nothing_else_is_read() {
        let map = b"the map itself".repeat(50);
        let packed = zip(&[("readme.txt", b"hello"), ("Maps/World.DAT", &map)]);
        assert_eq!(unpacked(packed).unwrap(), map);
        // Not a zip: the map as it is.
        assert_eq!(unpacked(map.clone()).unwrap(), map);
        let none = unpacked(zip(&[("readme.txt", b"hello")])).unwrap_err();
        assert!(none.contains("no map"), "{none}");
        // Cut short, it says it is damaged rather than reading past it.
        let packed = zip(&[("world.dat", &map)]);
        let short = unpacked(packed[..packed.len() / 2].to_vec()).unwrap_err();
        assert!(short.contains("damaged"), "{short}");
    }

    #[test]
    fn a_map_is_taken_from_a_file_and_one_that_is_not_says_why() {
        let path = std::env::temp_dir().join(format!("muddie-take-{}.zip", std::process::id()));
        std::fs::write(&path, zip(&[("map.dat", b"\xff\xff\xff\xff")])).unwrap();
        let source = path.display().to_string();
        assert!(!on_the_web(&source) && on_the_web("HTTPS://example.org/map.zip"));
        let error = match import(source, None).recv().unwrap() {
            Err(error) => error,
            Ok(_) => panic!("not a map"),
        };
        assert!(error.contains("not a map of Mudlet"), "{error}");
        std::fs::remove_file(&path).unwrap();
        let missing = import("/no/such/map.dat".into(), None).recv().unwrap();
        assert!(missing.is_err());
    }
}
