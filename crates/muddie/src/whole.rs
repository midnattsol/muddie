//! Files written whole or not at all: first under another name beside
//! them, then put in their place, so that Muddie closed halfway through,
//! or a full disk, never leaves one cut short.

use std::io;
use std::path::{Path, PathBuf};

/// Writes `contents` to `path`, in place of what was there only once all
/// of it is written.
pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let partial = partial(path);
    let written = std::fs::write(&partial, contents).and_then(|()| std::fs::rename(&partial, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    written
}

/// Whether `name`, a file's or a folder's, is one Windows keeps for a
/// device, whatever its ending: `CON`, `nul.wav`. There it is no file, and
/// opening it waits on the device; profiles and plugins go from one
/// system to another, so it is refused on every one.
pub fn reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.as_bytes()[3].is_ascii_digit()
            && upper.as_bytes()[3] != b'0')
}

/// Where `path` is written before it is put in its place: beside it, with
/// `.part` after its whole name, so that two files that differ only in
/// their ending are not written to the same place.
fn partial(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".part");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_written_whole_in_place_of_the_old() {
        let dir = std::env::temp_dir().join(format!("muddie-whole-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        std::fs::write(&path, "old").unwrap();
        write(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(partial(&path), dir.join("settings.toml.part"));
        // Two that differ only in their ending are not written to one place.
        assert_ne!(partial(&dir.join("hit.wav")), partial(&dir.join("hit.ogg")));
        assert!(!partial(&path).exists());
        // Where it cannot be written, what was there is left.
        assert!(write(&dir.join("none").join("x.toml"), "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod reserved_tests {
    /// Names Windows keeps for devices are no file there, and opening one
    /// waits on a device: they are names of nothing, on every system, as
    /// profiles and plugins go from one to another.
    #[test]
    fn names_windows_keeps_for_devices_are_refused() {
        for name in ["CON", "con", "Nul", "aux", "COM1", "lpt9"] {
            assert!(!crate::profile::valid_name(name), "profile {name}");
        }
        assert!(!crate::media::safe_name("con.wav"), "sound con.wav");
        assert!(!crate::media::safe_name("Combate/NUL.ogg"), "sound NUL.ogg");
        assert!(!crate::plugins::unpack::valid_id("prn"), "plugin id prn");
        for name in ["Reinos", "Console", "Nulo", "com", "COM10"] {
            assert!(crate::profile::valid_name(name), "{name}");
        }
    }
}
