//! Plays sound files, and reads the sound requests of the MUD Sound
//! Protocol.
//!
//! Muddie decodes, shapes and mixes the sounds itself, the same on every
//! system, so that each can be made louder or quieter as it plays, put to
//! one side, or come up and die away. What it mixes is heard on Linux
//! through one of the system's players, which works with PipeWire,
//! PulseAudio and ALSA alike without linking an audio library or needing
//! its headers to build; elsewhere through the system's own audio.

use std::path::Path;

pub use player::Player;

mod msp;
mod output;
mod player;
mod shape;
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
    /// The group it is in, whose volume it follows: none for the rest.
    pub group: String,
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
    /// Where it is heard, from -1, all left, to 1, all right.
    pub pan: f32,
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
            pan: 0.0,
        }
    }
}

fn check_file(file: &Path) -> Result<(), String> {
    if file.is_file() {
        Ok(())
    } else {
        Err(format!("no such sound file: {}", file.display()))
    }
}
