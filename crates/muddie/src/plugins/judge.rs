//! Whether the files of an official plugin, as they came in the zip, are
//! fit to be kept: they are read as Muddie reads them, before anything of
//! the plugin on disk is touched.

use super::unpack::Files;
use super::{Manifest, version};

/// What came of looking at a plugin.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Fit to be kept.
    Ready,
    /// Made for a newer Muddie than this one, which says which.
    Newer(String),
    /// Not fit, said why.
    Refused(String),
}

/// The text of a file of the plugin, empty if it has none.
fn text(files: &Files, name: &str) -> Result<String, String> {
    match files.get(name) {
        None => Ok(String::new()),
        Some(bytes) => String::from_utf8(bytes.clone()).map_err(|_| format!("{name} is not text")),
    }
}

/// Looks at a plugin: its scripts, and its `plugin.toml`, `rules.toml`
/// and `template.toml`. The version of Muddie it asks for is read before
/// anything else of the manifest is, as a newer plugin may have what this
/// Muddie does not know.
pub fn judge(files: &Files) -> Verdict {
    match look(files) {
        Ok(verdict) => verdict,
        Err(why) => Verdict::Refused(why),
    }
}

fn look(files: &Files) -> Result<Verdict, String> {
    if !files.keys().any(|name| name.ends_with(".luau")) {
        return Err("it has no .luau script".to_owned());
    }
    let manifest = text(files, "plugin.toml")?;
    toml::from_str::<toml::Table>(&manifest).map_err(|error| format!("plugin.toml: {error}"))?;
    if let Some(asked) = version::required(&manifest)
        && version::is_newer(&asked, version::CURRENT)
    {
        return Ok(Verdict::Newer(asked));
    }
    toml::from_str::<Manifest>(&manifest).map_err(|error| format!("plugin.toml: {error}"))?;
    toml::from_str::<crate::rules::Rules>(&text(files, "rules.toml")?)
        .map_err(|error| format!("rules.toml: {error}"))?;
    let template = text(files, "template.toml")?;
    if !template.is_empty() {
        toml::from_str::<crate::templates::Template>(&template)
            .map_err(|error| format!("template.toml: {error}"))?;
    }
    Ok(Verdict::Ready)
}
