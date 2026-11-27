//! The sounds and music a MUD asks for, with MSP or Client.Media: found
//! among the player's, or fetched.

use super::*;

/// A sound being fetched: where the result comes, its address, and what
/// to play once it is here.
pub(super) type MediaJob = (
    Receiver<Result<PathBuf, String>>,
    String,
    Option<crate::media::Play>,
);

impl Tab {
    /// Where a sound a script named is: the path given if it is one, else
    /// the first file of that name in the sound folders.
    pub(super) fn sound_file(&self, name: &str) -> PathBuf {
        let given = PathBuf::from(name);
        if given.is_absolute() {
            return given;
        }
        let folders = (self.profiles.as_ref())
            .map(|profiles| profiles.sound_dirs(self.profile.as_deref()))
            .unwrap_or_default();
        let found = folders
            .iter()
            .map(|dir| dir.join(name))
            .find(|path| path.is_file());
        found.unwrap_or(given)
    }

    /// Carries out a message of `Client.Media`: a sound or music the MUD
    /// asks for, by name, with where to fetch it if it is not here.
    pub(super) fn media(&mut self, message: crate::media::Media) {
        use crate::media::Media;
        match message {
            Media::Default(url) => self.fetching.base = Some(url),
            Media::Load { name, url } => {
                if let Some((address, None)) = self.media_file(&name, url.as_deref()) {
                    self.media_fetch(address, None);
                }
            }
            Media::Play(play) => self.media_play(play),
            Media::Stop(stop) => self
                .sound
                .stop_where(|label| stop.matches(&label.name, &label.kind, &label.tag, &label.key)),
        }
    }

    /// Carries out a request of the MUD Sound Protocol, which says the
    /// same things in its own way: a sound of a kind, with a priority,
    /// or the one music there is at a time.
    pub(super) fn msp(&mut self, request: Msp) {
        let kind_of = |music: bool| if music { "music" } else { "sound" };
        match request {
            Msp::Stop { music, url } => {
                if let Some(url) = url {
                    self.fetching.base = Some(url);
                }
                let kind = kind_of(music);
                self.sound.stop_where(|label| label.kind == kind);
            }
            Msp::Play {
                music,
                file,
                volume,
                times,
                priority,
                keep_going,
                kind,
                url,
            } => {
                // A sound of a kind may be in the folder of that kind.
                let inside = (!kind.is_empty()).then(|| format!("{kind}/{file}"));
                let names = inside.into_iter().chain([file.clone()]);
                // One that stands for several is one of those there are.
                let name = (names.clone())
                    .find_map(|name| self.sound_among(&name))
                    .or_else(|| names.clone().next())
                    .unwrap_or(file);
                // An address may be of the folder, or of the file itself.
                let url = url.map(|url| match url.rsplit_once('/') {
                    Some((folder, last)) if crate::media::safe_name(last) => folder.to_owned(),
                    _ => url,
                });
                self.media_play(crate::media::Play {
                    name,
                    url,
                    kind: kind_of(music).to_owned(),
                    tag: kind,
                    // There is one music at a time.
                    key: if music {
                        "msp music".to_owned()
                    } else {
                        String::new()
                    },
                    volume,
                    loops: times,
                    keep_going,
                    priority,
                    start: 0,
                    finish: None,
                    fade_in: 0,
                    fade_out: 0,
                });
            }
        }
    }

    /// The sound among the player's own that `name` stands for, if it has
    /// `*` or `?` in it: one of those there are, a different one each
    /// time. `None` if it has neither, or there is none.
    pub(super) fn sound_among(&self, name: &str) -> Option<String> {
        if !name.contains(['*', '?']) || !crate::media::safe_name(&name.replace(['*', '?'], "x")) {
            return None;
        }
        let (folder, pattern) = match name.rsplit_once('/') {
            Some((folder, pattern)) => (folder, pattern),
            None => ("", name),
        };
        let dirs = (self.profiles.as_ref())
            .map(|profiles| profiles.sound_dirs(self.profile.as_deref()))
            .unwrap_or_default();
        let mut found: Vec<String> = (dirs.iter())
            .filter_map(|dir| std::fs::read_dir(dir.join(folder)).ok())
            .flat_map(|entries| entries.flatten())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|file| muddie_sound::name_matches(pattern, file))
            .collect();
        found.sort();
        found.dedup();
        if found.is_empty() {
            return None;
        }
        // Whichever the clock says: no two in a row need be alike.
        let turn = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.subsec_nanos() as usize);
        let file = found.swap_remove(turn % found.len());
        Some(if folder.is_empty() {
            file
        } else {
            format!("{folder}/{file}")
        })
    }

    /// Plays a sound or music asked for by name, from the player's own
    /// or the web, as its priority and its key allow.
    pub(super) fn media_play(&mut self, play: crate::media::Play) {
        let same = |label: &muddie_sound::Label| label.name == play.name;
        // Music that is playing goes on, unless told otherwise.
        let music = |label: &muddie_sound::Label| same(label) && label.kind == "music";
        if play.keep_going && self.sound.is_playing(music) {
            return;
        }
        // Under a key there is one thing at a time.
        if !play.key.is_empty() {
            let key = play.key.clone();
            self.sound
                .stop_where(|label| label.key == key && !same(label));
        }
        // What matters more is not played over, and stops what matters
        // less; what was given no priority does neither.
        if play.priority > 0 {
            let priority = play.priority;
            if self.sound.is_playing(|label| label.priority > priority) {
                return;
            }
            self.sound
                .stop_where(|label| label.priority > 0 && label.priority < priority);
        }
        match self.media_file(&play.name, play.url.as_deref()) {
            Some((_, Some(path))) => self.media_start(&path, &play),
            Some((address, None)) => self.media_fetch(address, Some(play)),
            None => {}
        }
    }

    /// Where the sound `name` is: among the player's own sounds, or
    /// already fetched, which gives its path; or on the web, which gives
    /// only its address. `None` if it is neither, or not safe to name.
    pub(super) fn media_file(
        &self,
        name: &str,
        url: Option<&str>,
    ) -> Option<(String, Option<PathBuf>)> {
        if !crate::media::safe_name(name) {
            return None;
        }
        let own = self.sound_file(name);
        if own.is_file() {
            return Some((String::new(), Some(own)));
        }
        let address = crate::media::address(url.or(self.fetching.base.as_deref())?, name)?;
        let kept = crate::media::kept_at(&crate::media::cache_dir()?, &address);
        let here = kept.is_file().then_some(kept);
        Some((address, here))
    }

    /// Starts fetching a sound, to play `then` once it is here, unless
    /// fetching is switched off, it is being fetched, or it failed before.
    pub(super) fn media_fetch(&mut self, address: String, then: Option<crate::media::Play>) {
        let busy = self
            .fetching
            .jobs
            .iter()
            .any(|(_, known, _)| *known == address);
        if !self.prefs.media_downloads || busy || self.fetching.missing.contains(&address) {
            return;
        }
        let Some(cache) = crate::media::cache_dir() else {
            return;
        };
        // No more at once than a MUD could reasonably ask for.
        if self.fetching.jobs.len() < 8 {
            let job = crate::media::fetch(&cache, &address);
            self.fetching.jobs.push((job, address, then));
        }
    }

    /// Plays what has been fetched since the last look, and remembers
    /// what could not be, saying so once.
    pub(super) fn media_fetched(&mut self) {
        let mut ended = Vec::new();
        self.fetching
            .jobs
            .retain_mut(|(job, address, then)| match job.try_recv() {
                Ok(outcome) => {
                    ended.push((outcome, address.clone(), then.take()));
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
            });
        for (outcome, address, then) in ended {
            match (outcome, then) {
                (Ok(path), Some(play)) => self.media_start(&path, &play),
                (Ok(_), None) => {}
                (Err(error), _) => {
                    self.fetching.missing.push(address);
                    self.refuse(&trf("A sound could not be fetched: {0}", &[&error]));
                }
            }
        }
    }

    pub(super) fn media_start(&mut self, path: &std::path::Path, play: &crate::media::Play) {
        let label = muddie_sound::Label {
            name: play.name.clone(),
            kind: play.kind.clone(),
            tag: play.tag.clone(),
            key: play.key.clone(),
            priority: play.priority,
            // What the MUD plays, as a group of its own, to make it all
            // louder or quieter.
            group: "mud".to_owned(),
        };
        let options = muddie_sound::Options {
            volume: play.volume,
            times: play.loops,
            start: play.start,
            finish: play.finish,
            fade_in: play.fade_in,
            fade_out: play.fade_out,
            pan: 0.0,
        };
        // A sound that cannot be played is no error.
        let _ = self.sound.play_with(path, options, label);
    }
}

/// The sounds being fetched for the MUD, where it said they are, and those
/// that could not be had.
#[derive(Default)]
pub(super) struct Fetching {
    /// Where the MUD said its sounds are fetched from, the ones being
    /// fetched, each with what to play once it is here, and the ones
    /// that could not be had, which are not asked for again.
    pub(super) base: Option<String>,
    pub(super) jobs: Vec<MediaJob>,
    pub(super) missing: Vec<String>,
}
