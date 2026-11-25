//! The official plugins: downloaded as a zip from a release, kept in a folder
//! of the configuration of their own, and brought up to date by Muddie. What
//! is on disk is never touched unless what came is whole and fit: offline,
//! an error, a damaged zip or a plugin that is not right leave it as it is.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use super::judge::{Verdict, judge};
use super::{store, unpack};

/// Where the official plugins are taken from: each a zip with a folder per
/// plugin, `<id>/plugin.toml`, its scripts, `rules.toml` and, if it brings
/// one, `template.toml`.
pub const SOURCES: [&str; 1] =
    ["https://github.com/midnattsol/muddie-plugins/releases/latest/download/plugins.zip"];

/// How long the web is waited for.
const PATIENCE: Duration = Duration::from_secs(30);

impl crate::profile::Profiles {
    /// Where the official plugins are kept, and the templates that come
    /// with them: a folder of Muddie's own, apart from `plugins_dir`, which
    /// is the player's.
    pub fn official_dir(&self) -> PathBuf {
        self.settings_path().with_file_name("official-plugins")
    }

    /// Where the versions of what the official plugins came in are kept.
    pub fn official_versions(&self) -> PathBuf {
        self.settings_path()
            .with_file_name("official-plugins.versions")
    }
}

/// What looking for updates came to.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// The plugins put on disk, new or changed: their ids and names.
    pub changed: Vec<(String, String)>,
    /// Those made for a newer Muddie, kept as they were: name and version.
    pub newer: Vec<(String, String)>,
    /// Those that were not fit to be kept, with why.
    pub refused: Vec<String>,
    /// Whether the web had anything newer.
    pub fetched: bool,
}

impl Report {
    fn merge(&mut self, other: Report) {
        self.changed.extend(other.changed);
        self.newer.extend(other.newer);
        self.refused.extend(other.refused);
        self.fetched |= other.fetched;
    }
}

/// The name a plugin gives itself, read without the rest of what it says
/// being understood, as a newer plugin may have what this Muddie does not
/// know: its id if it gives none.
fn name_of(files: &unpack::Files, id: &str) -> String {
    let manifest = files
        .get("plugin.toml")
        .and_then(|bytes| std::str::from_utf8(bytes).ok());
    let table = manifest.and_then(|text| toml::from_str::<toml::Table>(text).ok());
    let name = table.as_ref().and_then(|table| table.get("name"));
    let text = match name {
        Some(toml::Value::String(name)) => Some(name.as_str()),
        Some(toml::Value::Table(names)) => names.get("en").and_then(toml::Value::as_str),
        _ => None,
    };
    match text {
        Some(name) if !name.trim().is_empty() => name.to_owned(),
        _ => id.to_owned(),
    }
}

/// Takes the zip at `source` if the web has a newer one than was last
/// taken, and puts each plugin in it fit to be kept in `dir`. `etags` keeps
/// which version of the zip that was: not kept if a plugin could not be
/// written, so that it is tried again.
pub fn update(dir: &Path, etags: &Path, source: &str) -> Result<Report, String> {
    store::recover(dir);
    // Told nothing of what was kept if none is there to go with it.
    let known = store::known_version(etags, source).filter(|_| store::has_plugins(dir));
    let fetched = crate::webfetch::get(source, known.as_deref(), unpack::MAX_PACKED, PATIENCE)?;
    let Some((bytes, version)) = fetched else {
        return Ok(Report::default());
    };
    let mut report = Report {
        fetched: true,
        ..Report::default()
    };
    let mut kept_all = true;
    for (id, files) in unpack::plugins(&bytes)? {
        match judge(&files) {
            Verdict::Refused(why) => report.refused.push(format!("{id}: {why}")),
            Verdict::Newer(asked) => report.newer.push((name_of(&files, &id), asked)),
            Verdict::Ready => match store::put(dir, &id, &files) {
                Ok(true) => report.changed.push((id.clone(), name_of(&files, &id))),
                Ok(false) => {}
                Err(error) => {
                    kept_all = false;
                    report.refused.push(format!("{id}: {error}"));
                }
            },
        }
    }
    if let (true, Some(version)) = (kept_all, version) {
        // Failing to remember only costs fetching the zip again.
        let _ = store::keep_version(etags, source, &version);
    }
    Ok(report)
}

/// Looks at each of `sources`: what was found, or the last error if none
/// could be looked at.
pub fn update_all(dir: &Path, etags: &Path, sources: &[String]) -> Result<Report, String> {
    let (mut report, mut error, mut looked) = (Report::default(), None, false);
    for source in sources {
        match update(dir, etags, source) {
            Ok(found) => {
                report.merge(found);
                looked = true;
            }
            Err(failed) => error = Some(failed),
        }
    }
    match (looked, error) {
        (false, Some(error)) => Err(error),
        _ => Ok(report),
    }
}

/// Where what looking for updates came to arrives.
pub type Looking = Receiver<Result<Report, String>>;

/// Looks at `sources` away from the thread that asked, and calls `wake`
/// when it is done.
pub fn spawn(
    dir: PathBuf,
    etags: PathBuf,
    sources: Vec<String>,
    wake: impl FnOnce() + Send + 'static,
) -> Looking {
    let (done, result) = channel();
    std::thread::spawn(move || {
        let _ = done.send(update_all(&dir, &etags, &sources));
        wake();
    });
    result
}

/// Looks at the official sources, away from the thread that asked. Tests
/// never reach the web: those of plugins use [`spawn`] with an address of
/// their own.
pub fn start(
    dir: PathBuf,
    etags: PathBuf,
    wake: impl FnOnce() + Send + 'static,
) -> Option<Looking> {
    if cfg!(test) {
        return None;
    }
    let sources = SOURCES.iter().map(|source| (*source).to_owned()).collect();
    Some(spawn(dir, etags, sources, wake))
}

/// Whether the official plugins are being looked for, shared by the window
/// and its sessions: one may wait for them before it connects.
#[derive(Debug, Clone, Default)]
pub struct Download(Arc<AtomicBool>);

impl Download {
    pub fn is_running(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn set(&self, running: bool) {
        self.0.store(running, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests;
