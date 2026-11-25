//! Plays files with the `rodio` library, on Windows and macOS.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use rodio::Source;

use super::{Label, Options};

#[derive(Default)]
pub struct Player {
    /// The audio output, opened when the first sound is played.
    device: Option<rodio::MixerDeviceSink>,
    playing: Vec<(rodio::Player, Label)>,
}

impl Player {
    /// Starts playing `file` at `volume` (0 to 1), repeating it until
    /// stopped if `looped`.
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
        super::check_file(file)?;
        if self.device.is_none() {
            let opened = rodio::DeviceSinkBuilder::open_default_sink();
            self.device = Some(opened.map_err(|error| error.to_string())?);
        }
        let Some(device) = &self.device else {
            return Ok(());
        };
        let opened = File::open(file).map_err(|error| error.to_string())?;
        let source =
            rodio::Decoder::new(BufReader::new(opened)).map_err(|error| error.to_string())?;
        let player = rodio::Player::connect_new(device.mixer());
        player.set_volume(volume);
        match times {
            None => player.append(source.buffered().repeat_infinite()),
            Some(times) => {
                let source = source.buffered();
                for _ in 0..times.max(1) {
                    player.append(source.clone());
                }
            }
        }
        self.playing.push((player, label));
        Ok(())
    }

    /// Plays `file` as `options` say, under labels to stop it by.
    /// Its volume and how many times are followed; it is played
    /// whole, from its start, without coming up or dying away.
    pub fn play_with(&mut self, file: &Path, options: Options, label: Label) -> Result<(), String> {
        self.play_as(file, options.volume, options.times, label)
    }

    /// Forgets sounds that have ended.
    pub fn poll(&mut self) {
        self.playing.retain(|(player, _)| !player.empty());
    }

    /// Sounds are played again by themselves here.
    pub fn has_loops(&self) -> bool {
        false
    }

    /// Whether a sound whose labels pass `chosen` is playing.
    pub fn is_playing(&self, chosen: impl Fn(&Label) -> bool) -> bool {
        (self.playing.iter()).any(|(player, label)| !player.empty() && chosen(label))
    }

    /// Stops the sounds whose labels pass `chosen`.
    pub fn stop_where(&mut self, chosen: impl Fn(&Label) -> bool) {
        self.playing.retain(|(player, label)| {
            let stop = chosen(label);
            if stop {
                player.stop();
            }
            !stop
        });
    }

    pub fn stop_all(&mut self) {
        self.stop_where(|_| true);
    }
}
