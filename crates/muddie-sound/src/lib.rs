//! Plays sound files, and reads the sound requests of the MUD Sound
//! Protocol.
//!
//! On Linux each file is handed to one of the system's players, which
//! works with PipeWire, PulseAudio and ALSA alike without linking an audio
//! library or needing its headers to build. On Windows and macOS the
//! `rodio` library plays through the system's own audio.

use std::path::Path;

#[cfg(not(target_os = "linux"))]
pub use library::Player;
#[cfg(target_os = "linux")]
pub use system::Player;

#[cfg(not(target_os = "linux"))]
mod library;
mod msp;
#[cfg(target_os = "linux")]
mod system;
#[cfg(test)]
mod tests;

pub use msp::{Msp, name_matches, parse_msp};

/// What a sound is played under, to tell it from the others when some
/// are to be stopped: its name, whether it is a sound or music, and a tag
/// and a key that whoever asked for it gave it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Label {
    pub name: String,
    pub kind: String,
    pub tag: String,
    pub key: String,
    /// How much it matters, from 1 to 100; 0 for what was given none.
    pub priority: u8,
}

/// How a sound is to be played.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// From 0 to 1.
    pub volume: f32,
    /// How many times; `None` for until stopped.
    pub times: Option<u32>,
    /// Where in it to start, and where to end if before its own end, in
    /// milliseconds.
    pub start: u32,
    pub finish: Option<u32>,
    /// How long it takes to come up to its volume, and to die away
    /// before where it ends, in milliseconds.
    pub fade_in: u32,
    pub fade_out: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            volume: 1.0,
            times: Some(1),
            start: 0,
            finish: None,
            fade_in: 0,
            fade_out: 0,
        }
    }
}

impl Options {
    /// Whether it asks for more than a volume and a number of times.
    pub fn shaped(&self) -> bool {
        self.start > 0 || self.finish.is_some() || self.fade_in > 0 || self.fade_out > 0
    }
}

/// The arguments that make `ffplay` play a sound as `options` say: from
/// where, how long, and coming up and dying away. Dying away needs to
/// know where the sound ends, so it is only done when that is given.
pub fn ffplay_arguments(options: &Options) -> Vec<String> {
    let seconds = |milliseconds: u32| format!("{:.3}", f64::from(milliseconds) / 1000.0);
    let percent = (options.volume.clamp(0.0, 1.0) * 100.0) as u32;
    let mut arguments: Vec<String> = ["-nodisp", "-autoexit", "-loglevel", "quiet", "-volume"]
        .map(String::from)
        .to_vec();
    arguments.push(percent.to_string());
    if options.start > 0 {
        arguments.extend(["-ss".to_owned(), seconds(options.start)]);
    }
    let length = options
        .finish
        .map(|finish| finish.saturating_sub(options.start));
    if let Some(length) = length {
        arguments.extend(["-t".to_owned(), seconds(length)]);
    }
    let mut filters = Vec::new();
    if options.fade_in > 0 {
        filters.push(format!("afade=t=in:st=0:d={}", seconds(options.fade_in)));
    }
    if let (Some(length), true) = (length, options.fade_out > 0) {
        let from = length.saturating_sub(options.fade_out);
        let lasting = length - from;
        filters.push(format!(
            "afade=t=out:st={}:d={}",
            seconds(from),
            seconds(lasting)
        ));
    }
    if !filters.is_empty() {
        arguments.extend(["-af".to_owned(), filters.join(",")]);
    }
    arguments
}

fn check_file(file: &Path) -> Result<(), String> {
    if file.is_file() {
        Ok(())
    } else {
        Err(format!("no such sound file: {}", file.display()))
    }
}
