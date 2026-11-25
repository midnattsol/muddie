//! Keeping the official plugins on disk: each in a folder of its own,
//! replaced whole or not at all, and the version of the zip they came in
//! kept beside them.

use std::io;
use std::path::Path;

use super::unpack::Files;

/// The files of the folder, if it is one and all are readable.
fn read(folder: &Path) -> Option<Files> {
    let mut files = Files::new();
    for entry in std::fs::read_dir(folder).ok()?.flatten() {
        let name = entry.file_name().into_string().ok()?;
        files.insert(name, std::fs::read(entry.path()).ok()?);
    }
    Some(files)
}

/// Whether any plugin is kept in `dir`.
pub fn has_plugins(dir: &Path) -> bool {
    super::folders(dir).next().is_some()
}

/// Puts the plugin `id` in `dir` made of `files`, unless it is so already.
/// Written beside, then put in place: a crash leaves the plugin as it was or
/// as it is to be, never half of either. Returns whether it changed.
pub fn put(dir: &Path, id: &str, files: &Files) -> io::Result<bool> {
    let place = dir.join(id);
    if read(&place).as_ref() == Some(files) {
        return Ok(false);
    }
    std::fs::create_dir_all(dir)?;
    let process = std::process::id();
    let (new, old) = (
        dir.join(format!(".new-{id}-{process}")),
        dir.join(format!(".old-{id}-{process}")),
    );
    let _ = std::fs::remove_dir_all(&new);
    let written = std::fs::create_dir(&new).and_then(|()| {
        for (name, data) in files {
            std::fs::write(new.join(name), data)?;
        }
        Ok(())
    });
    if let Err(error) = written {
        let _ = std::fs::remove_dir_all(&new);
        return Err(error);
    }
    // A folder is not renamed over another on every system: the one there
    // is put aside, and put back if the new one cannot be put in place.
    let _ = std::fs::remove_dir_all(&old);
    let aside = place.exists();
    if aside && let Err(error) = std::fs::rename(&place, &old) {
        let _ = std::fs::remove_dir_all(&new);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&new, &place) {
        if aside {
            let _ = std::fs::rename(&old, &place);
        }
        let _ = std::fs::remove_dir_all(&new);
        return Err(error);
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(true)
}

/// Puts right what a crash left in `dir`: a plugin that was put aside to be
/// replaced is put back, and what was being written is thrown away.
pub fn recover(dir: &Path) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if let Some(rest) = name.strip_prefix(".old-") {
            let id = rest.rsplit_once('-').map_or(rest, |(id, _)| id);
            if dir.join(id).exists() {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::rename(&path, dir.join(id));
            }
        } else if name.starts_with(".new-") {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// The version the web gave of the zip at `source`, if it was kept by this
/// version of Muddie: a newer Muddie may take what an older one left aside.
/// One line each: the address, the version of Muddie, the version of the zip.
pub fn known_version(etags: &Path, source: &str) -> Option<String> {
    let text = std::fs::read_to_string(etags).ok()?;
    text.lines().find_map(|line| {
        let mut parts = line.splitn(3, '\t');
        let (address, muddie, version) = (parts.next()?, parts.next()?, parts.next()?);
        (address == source && muddie == super::version::CURRENT).then(|| version.to_owned())
    })
}

/// Keeps the version of the zip at `source`, in place of the one kept.
pub fn keep_version(etags: &Path, source: &str, version: &str) -> io::Result<()> {
    let text = std::fs::read_to_string(etags).unwrap_or_default();
    let mut lines: Vec<String> = (text.lines())
        .filter(|line| line.split('\t').next() != Some(source))
        .map(str::to_owned)
        .collect();
    if !version.contains(['\t', '\n', '\r']) {
        let muddie = super::version::CURRENT;
        lines.push(format!("{source}\t{muddie}\t{version}"));
    }
    let temporary = etags.with_extension("new");
    std::fs::write(&temporary, lines.join("\n") + "\n")?;
    std::fs::rename(&temporary, etags)
}
