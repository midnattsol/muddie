//! Opening the zip of official plugins in memory, trusting nothing in it:
//! each file is looked at by its name and kind before it is read, and its
//! size is counted as it is inflated. What comes out is files held in
//! memory, by plugin; nothing is written here.

use std::collections::BTreeMap;

use crate::zipread::{self, Kind};

/// The most a zip may take, packed; and, unpacked, in all and each file.
pub const MAX_PACKED: u64 = 16 * 1024 * 1024;
const MAX_UNPACKED: u64 = 8 * 1024 * 1024;
const MAX_FILE: u64 = 2 * 1024 * 1024;
/// The most files and folders a zip may list.
const MAX_ENTRIES: usize = 4096;
/// The longest name a script may have.
const MAX_NAME: usize = 100;

/// The files of one plugin, by name.
pub type Files = BTreeMap<String, Vec<u8>>;

/// The plugins in a zip, by id.
pub type Plugins = BTreeMap<String, Files>;

/// Whether a folder may be called this: what an id is made of.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && !crate::whole::reserved(id)
        && id.len() <= MAX_NAME
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

/// Whether a file of a plugin may be named so: the three it has of its
/// own, and its scripts.
fn valid_file(name: &str) -> bool {
    if matches!(name, "plugin.toml" | "rules.toml" | "template.toml") {
        return true;
    }
    let stem = name.strip_suffix(".luau").unwrap_or("");
    !stem.is_empty()
        && name.len() <= MAX_NAME
        && !name.starts_with('.')
        && !crate::whole::reserved(name)
        && !name
            .chars()
            .any(|ch| ch.is_control() || "/\\:*?\"<>|".contains(ch))
}

/// Where an entry goes: the plugin, and the file in it.
type Place = (String, String);

/// Where in a plugin an entry is. A name that could reach elsewhere is
/// refused, said why; one that is safe but not of a file this Muddie knows
/// is none, left out: a later one may put more in the zip, which is no
/// reason to take nothing of it.
fn place(name: &str) -> Result<Option<Place>, String> {
    let shown = || name.escape_debug().to_string();
    if name.contains('\\') || name.contains(':') || name.chars().any(char::is_control) {
        return Err(format!("{}: not a name Muddie takes", shown()));
    }
    if name.starts_with('/') {
        return Err(format!("{}: an absolute path", shown()));
    }
    let name = name.strip_suffix('/').unwrap_or(name);
    let parts: Vec<&str> = name.split('/').collect();
    if parts.iter().any(|part| matches!(*part, ".." | "." | "")) {
        return Err(format!("{}: not a name Muddie takes", shown()));
    }
    match parts.as_slice() {
        [id, file] if valid_id(id) && valid_file(file) => {
            Ok(Some(((*id).to_owned(), (*file).to_owned())))
        }
        _ => Ok(None),
    }
}

/// The plugins in `bytes`, a zip with a folder for each. Any entry that is
/// not safe, or a zip that is too large, refuses the whole of it.
pub fn plugins(bytes: &[u8]) -> Result<Plugins, String> {
    if bytes.len() as u64 > MAX_PACKED {
        return Err("the zip is too large".to_owned());
    }
    let listed =
        zipread::entries(bytes, MAX_ENTRIES).map_err(|error| error.message().to_owned())?;
    let (mut found, mut left) = (Plugins::new(), MAX_UNPACKED);
    for entry in &listed {
        let name = std::str::from_utf8(&entry.name)
            .map_err(|_| "a name in the zip is not text".to_owned())?;
        let kind = entry.kind();
        if kind == Kind::Other {
            return Err(format!("{name}: neither a file nor a folder"));
        }
        let placed = place(name)?.filter(|_| kind == Kind::File);
        let Some((id, file)) = placed else {
            continue;
        };
        let files = found.entry(id).or_default();
        if files.contains_key(&file) {
            return Err(format!("{name}: listed twice"));
        }
        let data = zipread::data(bytes, entry, MAX_FILE.min(left))
            .map_err(|error| format!("{name}: {}", error.message()))?;
        left -= data.len() as u64;
        files.insert(file, data);
    }
    Ok(found)
}

#[cfg(test)]
pub mod tests;
