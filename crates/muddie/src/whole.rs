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
