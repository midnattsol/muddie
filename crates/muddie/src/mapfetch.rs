//! Maps made with Mudlet, taken from a file or from the web, read away
//! from the thread that draws. A zip is opened in memory and only the map
//! in it is read: nothing it holds is written anywhere or run.

use std::io::Read;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use muddie_mapper::{MudletMap, read_mudlet_map};

/// The most a map, or the zip it comes in, may take: Mudlet's maps of the
/// largest MUDs are a few tens of megabytes.
const LARGEST: u64 = 128 * 1024 * 1024;

/// What taking a map came to.
pub enum Taken {
    /// The map, and the version of it the web gave, to ask only for a
    /// newer one next time.
    Map {
        map: MudletMap,
        version: Option<String>,
    },
    /// The web has no newer version than the one known.
    Unchanged,
}

/// Where what taking a map came to arrives.
pub type Taking = Receiver<Result<Taken, String>>;

/// Bytes fetched, with the version the web gave of them; none when the
/// web had nothing newer.
type Fetched = Option<(Vec<u8>, Option<String>)>;

/// Whether `source` is an address on the web rather than a file.
pub fn on_the_web(source: &str) -> bool {
    let lower = source.trim().to_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Takes the map at `source`, a file or an address, away from the thread
/// that asked; from the web, only if newer than the version `known`.
pub fn take(source: String, known: Option<String>) -> Taking {
    let (done, result) = channel();
    std::thread::spawn(move || {
        let _ = done.send(fetch(&source, known.as_deref()).and_then(|fetched| {
            let Some((bytes, version)) = fetched else {
                return Ok(Taken::Unchanged);
            };
            let map = read_mudlet_map(&unpacked(bytes)?)?;
            Ok(Taken::Map { map, version })
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
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .max_redirects(3)
        .user_agent("muddie")
        .build()
        .into();
    let mut request = agent.get(source.trim());
    if let Some(known) = known {
        request = request.header("If-None-Match", known);
    }
    let mut response = match request.call() {
        Err(ureq::Error::StatusCode(304)) => return Ok(None),
        other => other.map_err(|error| said(&error))?,
    };
    if response.status().as_u16() == 304 {
        return Ok(None);
    }
    // What tells this version of the file from the next: its tag, or
    // failing that when it last changed.
    let header = |name: &str| {
        (response.headers().get(name))
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let version = header("etag").or_else(|| header("last-modified"));
    let bytes = (response
        .body_mut()
        .with_config()
        .limit(LARGEST)
        .read_to_vec())
    .map_err(|error| said(&error))?;
    Ok(Some((bytes, version)))
}

/// The map in `bytes`: themselves, or the first `.dat` file of a zip.
fn unpacked(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return Ok(bytes);
    }
    let damaged = || "the zip is damaged".to_owned();
    let u16_at = |at: usize| -> Option<usize> {
        Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?) as usize)
    };
    let u32_at = |at: usize| -> Option<usize> {
        Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize)
    };
    // The end of the zip says where its list of files is; it is followed
    // only by a comment, of at most 64 KiB.
    let end = (bytes.len().saturating_sub(22 + 0xffff)..=bytes.len().saturating_sub(22))
        .rev()
        .find(|at| bytes[*at..].starts_with(b"PK\x05\x06"))
        .ok_or_else(damaged)?;
    let count = u16_at(end + 10).ok_or_else(damaged)?;
    let mut at = u32_at(end + 16).ok_or_else(damaged)?;
    for _ in 0..count {
        if !bytes
            .get(at..)
            .is_some_and(|rest| rest.starts_with(b"PK\x01\x02"))
        {
            return Err(damaged());
        }
        let method = u16_at(at + 10).ok_or_else(damaged)?;
        let packed = u32_at(at + 20).ok_or_else(damaged)?;
        let size = u32_at(at + 24).ok_or_else(damaged)?;
        let name_length = u16_at(at + 28).ok_or_else(damaged)?;
        let extra = u16_at(at + 30).ok_or_else(damaged)? + u16_at(at + 32).ok_or_else(damaged)?;
        let local = u32_at(at + 42).ok_or_else(damaged)?;
        let name = bytes
            .get(at + 46..at + 46 + name_length)
            .ok_or_else(damaged)?;
        at += 46 + name_length + extra;
        let name = String::from_utf8_lossy(name).to_lowercase();
        if !name.ends_with(".dat") || name.starts_with("__macosx/") {
            continue;
        }
        if size as u64 > LARGEST {
            return Err("the map in the zip is too large".to_owned());
        }
        // The file itself, after its own header.
        if !bytes
            .get(local..)
            .is_some_and(|rest| rest.starts_with(b"PK\x03\x04"))
        {
            return Err(damaged());
        }
        let start = local
            + 30
            + u16_at(local + 26).ok_or_else(damaged)?
            + u16_at(local + 28).ok_or_else(damaged)?;
        let data = bytes.get(start..start + packed).ok_or_else(damaged)?;
        return match method {
            0 => Ok(data.to_vec()),
            8 => {
                let mut map = Vec::with_capacity(size);
                let inflater = flate2::read::DeflateDecoder::new(data);
                (inflater.take(LARGEST + 1).read_to_end(&mut map)).map_err(|_| damaged())?;
                if map.len() != size {
                    return Err(damaged());
                }
                Ok(map)
            }
            _ => Err("the zip is packed in a way that cannot be read".to_owned()),
        };
    }
    Err("there is no map (.dat) in the zip".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A zip of `files`, each packed as Deflate does, as most zips are.
    fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let (mut bytes, mut list) = (Vec::new(), Vec::new());
        for (name, data) in files {
            let mut packer = flate2::write::DeflateEncoder::new(Vec::new(), Default::default());
            packer.write_all(data).unwrap();
            let packed = packer.finish().unwrap();
            let local = bytes.len() as u32;
            let sizes = |out: &mut Vec<u8>| {
                out.extend(8u16.to_le_bytes());
                out.extend([0; 8]);
                out.extend((packed.len() as u32).to_le_bytes());
                out.extend((data.len() as u32).to_le_bytes());
                out.extend((name.len() as u16).to_le_bytes());
                out.extend(0u16.to_le_bytes());
            };
            bytes.extend(b"PK\x03\x04");
            bytes.extend([20, 0, 0, 0]);
            sizes(&mut bytes);
            bytes.extend(name.as_bytes());
            bytes.extend(&packed);
            list.extend(b"PK\x01\x02");
            list.extend([20, 0, 20, 0, 0, 0]);
            sizes(&mut list);
            list.extend([0; 10]);
            list.extend(local.to_le_bytes());
            list.extend(name.as_bytes());
        }
        let start = bytes.len() as u32;
        bytes.extend(&list);
        bytes.extend(b"PK\x05\x06");
        bytes.extend([0; 4]);
        bytes.extend((files.len() as u16).to_le_bytes());
        bytes.extend((files.len() as u16).to_le_bytes());
        bytes.extend((list.len() as u32).to_le_bytes());
        bytes.extend(start.to_le_bytes());
        bytes.extend([0; 2]);
        bytes
    }

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
        let error = match take(source, None).recv().unwrap() {
            Err(error) => error,
            Ok(_) => panic!("not a map"),
        };
        assert!(error.contains("not a map of Mudlet"), "{error}");
        std::fs::remove_file(&path).unwrap();
        let missing = take("/no/such/map.dat".into(), None).recv().unwrap();
        assert!(missing.is_err());
    }
}
