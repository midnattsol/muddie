//! Plays each file with one of the system's players, on Linux.

use std::path::Path;
use std::process::{Child, Command, Stdio};

use super::{Label, Options};

/// Builds a player's arguments for a volume from 0 to 1.
type VolumeArguments = fn(f32) -> Vec<String>;

/// The players tried, in order, with how each takes a volume.
const PLAYERS: [(&str, VolumeArguments); 4] = [
    ("pw-play", |volume| vec![format!("--volume={volume:.2}")]),
    ("paplay", |volume| {
        vec![format!("--volume={}", (volume * 65536.0) as u32)]
    }),
    ("ffplay", |volume| {
        let percent = (volume * 100.0) as u32;
        [
            "-nodisp",
            "-autoexit",
            "-loglevel",
            "quiet",
            "-volume",
            &percent.to_string(),
        ]
        .map(String::from)
        .to_vec()
    }),
    ("aplay", |_| vec!["-q".to_owned()]),
];

/// A sound still playing: how to start it again, how many more times
/// it is to be played (`None` for until stopped), and its labels.
struct Playing {
    child: Child,
    file: String,
    options: Options,
    left: Option<u32>,
    label: Label,
}

#[derive(Default)]
pub struct Player {
    playing: Vec<Playing>,
}

impl Player {
    /// Starts playing `file` at `volume` (0 to 1), repeating it until
    /// stopped if `looped`. The error says what went wrong, for
    /// instance that no player is installed.
    pub fn play(&mut self, file: &Path, volume: f32, looped: bool) -> Result<(), String> {
        let times = if looped { None } else { Some(1) };
        self.play_as(file, volume, times, Label::default())
    }

    /// The same, so many `times` (`None` for until stopped), under
    /// labels to stop it by.
    pub fn play_as(
        &mut self,
        file: &Path,
        volume: f32,
        times: Option<u32>,
        label: Label,
    ) -> Result<(), String> {
        let options = Options {
            volume,
            times,
            ..Options::default()
        };
        self.play_with(file, options, label)
    }

    /// Plays `file` as `options` say, under labels to stop it by.
    pub fn play_with(&mut self, file: &Path, options: Options, label: Label) -> Result<(), String> {
        super::check_file(file)?;
        let child = start(file, &options)?;
        self.playing.push(Playing {
            child,
            file: file.display().to_string(),
            left: options.times.map(|times| times.saturating_sub(1)),
            options,
            label,
        });
        Ok(())
    }

    /// Forgets sounds that have ended, and starts again those with
    /// times left to play.
    pub fn poll(&mut self) {
        let mut again = Vec::new();
        let playing = std::mem::take(&mut self.playing);
        for mut sound in playing {
            match sound.child.try_wait() {
                Ok(None) => self.playing.push(sound),
                _ if sound.left != Some(0) => again.push(sound),
                _ => {}
            }
        }
        for mut sound in again {
            if let Ok(child) = start(Path::new(&sound.file), &sound.options) {
                sound.child = child;
                sound.left = sound.left.map(|left| left - 1);
                self.playing.push(sound);
            }
        }
    }

    /// Whether a sound to be played again needs [`Self::poll`] to
    /// keep it going.
    pub fn has_loops(&self) -> bool {
        self.playing.iter().any(|sound| sound.left != Some(0))
    }

    /// Whether a sound whose labels pass `chosen` is playing.
    pub fn is_playing(&self, chosen: impl Fn(&Label) -> bool) -> bool {
        self.playing.iter().any(|sound| chosen(&sound.label))
    }

    /// Stops the sounds whose labels pass `chosen`.
    pub fn stop_where(&mut self, chosen: impl Fn(&Label) -> bool) {
        let playing = std::mem::take(&mut self.playing);
        for mut sound in playing {
            if chosen(&sound.label) {
                let _ = sound.child.kill();
                let _ = sound.child.wait();
            } else {
                self.playing.push(sound);
            }
        }
    }

    pub fn stop_all(&mut self) {
        self.stop_where(|_| true);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// Starts a sound as its options say. Only `ffplay` can start part
/// way in, stop short or fade, so it is tried first for those; the
/// others play the sound whole if it is not installed.
fn start(file: &Path, options: &Options) -> Result<Child, String> {
    if options.shaped() {
        let started = Command::new("ffplay")
            .args(super::ffplay_arguments(options))
            .arg(file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(child) = started {
            return Ok(child);
        }
    }
    spawn(file, options.volume)
}

fn spawn(file: &Path, volume: f32) -> Result<Child, String> {
    for (program, arguments) in PLAYERS {
        let started = Command::new(program)
            .args(arguments(volume))
            .arg(file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(child) = started {
            return Ok(child);
        }
    }
    Err("no sound player found (tried pw-play, paplay, ffplay and aplay)".to_owned())
}
