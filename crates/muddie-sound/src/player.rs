//! Plays sounds: each decoded and shaped by Muddie, and mixed into one
//! output, which is opened when a sound is first played and closed a while
//! after the last ends.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::{Duration, Instant};

use rodio::Source;

use super::output::Output;
use super::shape::Shape;
use super::{Label, Options};

/// How long the output is kept open with nothing to play.
const IDLE: Duration = Duration::from_secs(20);
/// The most times a sound is played over before it is taken to be played
/// until stopped: each time is queued apart, and a server may ask for
/// billions.
const MOST_TIMES: u32 = 100;

/// A sound being played: how loud it was asked to be, and its labels.
struct Playing {
    player: rodio::Player,
    volume: f32,
    label: Label,
}

#[derive(Default)]
pub struct Player {
    output: Option<Output>,
    playing: Vec<Playing>,
    /// The volume of each group of sounds, from 0 to 1; 1 for those not
    /// set.
    groups: HashMap<String, f32>,
    /// Since when nothing plays, to close the output a while after.
    idle_since: Option<Instant>,
}

impl Player {
    /// A player whose mix is read back rather than heard: for tests.
    #[cfg(test)]
    pub fn in_memory() -> (Self, rodio::mixer::MixerSource) {
        let (output, mix) = Output::memory();
        let player = Self {
            output: Some(output),
            ..Self::default()
        };
        (player, mix)
    }

    /// Starts playing `file` at `volume` (0 to 1), repeating it until
    /// stopped if `looped`. The error says what went wrong, for instance
    /// that the file is not a sound it can read.
    pub fn play(&mut self, file: &Path, volume: f32, looped: bool) -> Result<(), String> {
        let times = if looped { None } else { Some(1) };
        self.play_as(file, volume, times, Label::default())
    }

    /// The same, so many `times` (`None` for until stopped), under labels
    /// to stop it by.
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
        let source = shaped(file, &options)?;
        if self.output.is_none() {
            self.output = Some(Output::open()?);
        }
        let Some(output) = &self.output else {
            return Ok(());
        };
        let player = rodio::Player::connect_new(output.mixer());
        let volume = audible(options.volume);
        player.set_volume(volume * self.group_volume(&label.group));
        match plays(options.times) {
            None => player.append(source.repeat_infinite()),
            Some(times) => {
                for _ in 0..times {
                    player.append(source.clone());
                }
            }
        }
        self.playing.push(Playing {
            player,
            volume,
            label,
        });
        self.idle_since = None;
        Ok(())
    }

    /// Forgets sounds that have ended, and closes the output a while after
    /// the last.
    pub fn poll(&mut self) {
        self.playing.retain(|sound| !sound.player.empty());
        if !self.playing.is_empty() || self.output.is_none() {
            return;
        }
        let since = *self.idle_since.get_or_insert_with(Instant::now);
        if since.elapsed() >= IDLE {
            self.output = None;
            self.idle_since = None;
        }
    }

    /// Whether it must be polled to close its output.
    pub fn has_loops(&self) -> bool {
        self.output.is_some()
    }

    /// Whether a sound whose labels pass `chosen` is playing.
    pub fn is_playing(&self, chosen: impl Fn(&Label) -> bool) -> bool {
        (self.playing.iter()).any(|sound| !sound.player.empty() && chosen(&sound.label))
    }

    /// Sets how loud the sounds whose labels pass `chosen` are, from 0 to
    /// 1, as they play.
    pub fn set_volume_where(&mut self, chosen: impl Fn(&Label) -> bool, volume: f32) {
        let volume = audible(volume);
        for sound in self.playing.iter_mut().filter(|sound| chosen(&sound.label)) {
            sound.volume = volume;
            let group = self.groups.get(&sound.label.group).copied().unwrap_or(1.0);
            sound.player.set_volume(volume * group);
        }
    }

    /// Sets how loud a group of sounds is, from 0 to 1: those playing at
    /// once, and those played after.
    pub fn set_group_volume(&mut self, group: &str, volume: f32) {
        let volume = audible(volume);
        self.groups.insert(group.to_owned(), volume);
        for sound in self
            .playing
            .iter()
            .filter(|sound| sound.label.group == group)
        {
            sound.player.set_volume(sound.volume * volume);
        }
    }

    /// How loud a group of sounds is.
    pub fn group_volume(&self, group: &str) -> f32 {
        self.groups.get(group).copied().unwrap_or(1.0)
    }

    /// Stops the sounds whose labels pass `chosen`.
    pub fn stop_where(&mut self, chosen: impl Fn(&Label) -> bool) {
        self.playing.retain(|sound| {
            let stop = chosen(&sound.label);
            if stop {
                sound.player.stop();
            }
            !stop
        });
    }

    pub fn stop_all(&mut self) {
        self.stop_where(|_| true);
    }
}

/// A volume as it is played, from 0 to 1: one that is no number, as a
/// server or a script may give, is silence, and never reaches the mix,
/// which it would make no number all through.
fn audible(volume: f32) -> f32 {
    match volume.is_finite() {
        true => volume.clamp(0.0, 1.0),
        false => 0.0,
    }
}

/// How many times a sound asked for `times` (`None` for until stopped) is
/// queued: at least once, and past [`MOST_TIMES`], until stopped.
fn plays(times: Option<u32>) -> Option<u32> {
    times
        .filter(|&times| times <= MOST_TIMES)
        .map(|times| times.max(1))
}

/// The sound in `file`, decoded, cut and shaped as `options` say, kept
/// whole to play again.
fn shaped(
    file: &Path,
    options: &Options,
) -> Result<rodio::source::Buffered<Box<dyn Source + Send>>, String> {
    let opened = File::open(file).map_err(|error| error.to_string())?;
    let decoded = rodio::Decoder::new(BufReader::new(opened))
        .map_err(|error| format!("{}: {error}", file.display()))?;
    let milliseconds = |ms: u32| Duration::from_millis(u64::from(ms));
    let finish = (options.finish).map(|finish| milliseconds(finish.saturating_sub(options.start)));
    // How long it plays, which the fade out ends with: to where it is
    // told to finish, else to its end, if the file says where that is.
    let whole =
        (decoded.total_duration()).map(|total| total.saturating_sub(milliseconds(options.start)));
    let length = finish.or(whole);
    let mut source: Box<dyn Source + Send> =
        Box::new(decoded.skip_duration(milliseconds(options.start)));
    if let Some(finish) = finish {
        source = Box::new(source.take_duration(finish));
    }
    let fades = (
        milliseconds(options.fade_in),
        milliseconds(options.fade_out),
    );
    let shaped: Box<dyn Source + Send> = Box::new(Shape::new(source, length, fades, options.pan));
    Ok(shaped.buffered())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sound_is_queued_no_more_than_so_many_times() {
        assert_eq!(plays(Some(0)), Some(1));
        assert_eq!(plays(Some(3)), Some(3));
        assert_eq!(plays(Some(MOST_TIMES)), Some(MOST_TIMES));
        // Billions are as good as until stopped.
        assert_eq!(plays(Some(2_000_000_000)), None);
        assert_eq!(plays(None), None);
    }
}
