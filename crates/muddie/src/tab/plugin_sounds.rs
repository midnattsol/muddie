//! The sounds of plugins, which are on the web and not part of them:
//! fetched as a script plays one that is not here yet, or all those a
//! script names at once, in the background, and kept.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use super::*;
use crate::plugin_sounds::{Came, Kept};
use muddie_script::Playing;

/// How long after it was asked a sound fetched for it is still played:
/// later, it would be out of place.
const STILL_WANTED: Duration = Duration::from_secs(10);

/// Sounds of a plugin being fetched.
pub(super) struct SoundJob {
    plugin: String,
    kept: Kept,
    told: Receiver<Came>,
    /// The names being fetched.
    names: Vec<String>,
    /// What to play as it comes: the sound, how, by whom, and when it was
    /// asked.
    waiting: Vec<(String, Playing, (String, u64), Instant)>,
    /// Whether to say when all are here; and those that could not be had,
    /// with why.
    told_of: bool,
    failed: Vec<(String, String)>,
}

impl Tab {
    /// The sounds of the plugin whose script `owner` is, if it has any on
    /// the web, with its name.
    fn plugin_sounds_of(&self, owner: &str) -> Option<(Kept, String)> {
        let folder = std::path::Path::new(owner).parent()?.file_name()?;
        let plugin = (self.plugins.iter()).find(|plugin| folder == plugin.folder())?;
        let source = plugin.manifest.sounds.as_deref()?;
        Some((Kept::of(&plugin.id, source)?, plugin.shown_name()))
    }

    /// Plays a sound a plugin's script named that is not among the
    /// player's own: kept, or fetched and played as it comes. Returns
    /// whether it was one of the plugin's.
    pub(super) fn play_plugin_sound(
        &mut self,
        file: &str,
        how: &Playing,
        by: &(String, u64),
    ) -> bool {
        let Some((kept, name)) = self.plugin_sounds_of(&by.0) else {
            return false;
        };
        if let Some(path) = kept.path(file) {
            // A newer one, if the plugin points to another version, comes
            // for next time.
            if kept.wanted(file) {
                self.fetch_plugin_sounds_of(kept, name, vec![file.to_owned()], None);
            }
            self.play_path_for_script(&path, how, by);
            return true;
        }
        let waiting = (file.to_owned(), how.clone(), by.clone(), Instant::now());
        self.fetch_plugin_sounds_of(kept, name, vec![file.to_owned()], Some(waiting));
        true
    }

    /// Fetches, in the background, the sounds of the plugin whose script
    /// `owner` is that are not kept or are of an older version, saying so
    /// as it starts and when all are here.
    pub(super) fn fetch_plugin_sounds(&mut self, owner: &str, names: Vec<String>) {
        if let Some((kept, name)) = self.plugin_sounds_of(owner) {
            self.fetch_plugin_sounds_of(kept, name, names, None);
        }
    }

    fn fetch_plugin_sounds_of(
        &mut self,
        kept: Kept,
        plugin: String,
        names: Vec<String>,
        then: Option<(String, Playing, (String, u64), Instant)>,
    ) {
        if !self.prefs.media_downloads {
            return;
        }
        // Those being fetched already: what is waiting waits for them.
        let mut names: Vec<String> = (names.into_iter())
            .filter(|name| kept.wanted(name))
            .collect();
        names.sort();
        names.dedup();
        for job in self
            .plugin_sound_jobs
            .iter_mut()
            .filter(|job| job.plugin == plugin)
        {
            if let Some(then) = then.as_ref().filter(|then| job.names.contains(&then.0)) {
                job.waiting.push(then.clone());
            }
            names.retain(|name| !job.names.contains(name));
        }
        if names.is_empty() {
            return;
        }
        let told_of = names.len() > 3;
        if told_of {
            let count = names.len().to_string();
            self.note(&trf(
                "Fetching the sounds of {0} ({1}), in the background.",
                &[&plugin, &count],
            ));
        }
        let told = kept.fetch(names.clone());
        self.plugin_sound_jobs.push(SoundJob {
            plugin,
            kept,
            told,
            names,
            waiting: then.into_iter().collect(),
            told_of,
            failed: Vec::new(),
        });
    }

    /// Keeps the sounds of plugins that came, plays those waited for, and
    /// says when all of a plugin's are here, or how many could not be.
    pub(super) fn plugin_sounds_came(&mut self) {
        let mut jobs = std::mem::take(&mut self.plugin_sound_jobs);
        let mut playing = Vec::new();
        let mut said = Vec::new();
        jobs.retain_mut(|job| {
            loop {
                match job.told.try_recv() {
                    Ok(Came::Sound(name, version)) => {
                        job.kept.came(&name, version);
                        let (now, later) = std::mem::take(&mut job.waiting)
                            .into_iter()
                            .partition(|(waited, ..)| *waited == name);
                        job.waiting = later;
                        for (_, how, by, asked) in now {
                            if let Some(path) = job
                                .kept
                                .path(&name)
                                .filter(|_| asked.elapsed() < STILL_WANTED)
                            {
                                playing.push((path, how, by));
                            }
                        }
                    }
                    Ok(Came::Failed(name, why)) => job.failed.push((name, why)),
                    Err(TryRecvError::Empty) => break true,
                    Err(TryRecvError::Disconnected) => {
                        said.push((
                            job.plugin.clone(),
                            job.told_of,
                            std::mem::take(&mut job.failed),
                        ));
                        break false;
                    }
                }
            }
        });
        self.plugin_sound_jobs = jobs;
        for (path, how, by) in playing {
            self.play_path_for_script(&path, &how, &by);
        }
        for (plugin, told_of, failed) in said {
            match failed.first() {
                None if told_of => self.note(&trf("The sounds of {0} are here.", &[&plugin])),
                None => {}
                // Said once, with the first: the rest are likely alike.
                Some((name, why)) => self.refuse(&trf(
                    "{0} sounds of {1} could not be fetched; {2}: {3}",
                    &[&failed.len().to_string(), &plugin, name, why],
                )),
            }
        }
    }
}
