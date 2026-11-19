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

#[cfg(target_os = "linux")]
mod system {
    use std::path::Path;
    use std::process::{Child, Command, Stdio};

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

    #[derive(Default)]
    pub struct Player {
        /// Sounds still playing, with the request to repeat them if looped.
        playing: Vec<(Child, Option<(String, f32)>)>,
    }

    impl Player {
        /// Starts playing `file` at `volume` (0 to 1), repeating it until
        /// stopped if `looped`. The error says what went wrong, for
        /// instance that no player is installed.
        pub fn play(&mut self, file: &Path, volume: f32, looped: bool) -> Result<(), String> {
            super::check_file(file)?;
            let child = spawn(file, volume)?;
            let again = looped.then(|| (file.display().to_string(), volume));
            self.playing.push((child, again));
            Ok(())
        }

        /// Forgets sounds that have ended, and starts looped ones again.
        pub fn poll(&mut self) {
            let mut restart = Vec::new();
            self.playing
                .retain_mut(|(child, again)| match child.try_wait() {
                    Ok(None) => true,
                    _ => {
                        restart.extend(again.take());
                        false
                    }
                });
            for (file, volume) in restart {
                if let Ok(child) = spawn(Path::new(&file), volume) {
                    self.playing.push((child, Some((file, volume))));
                }
            }
        }

        /// Whether a looped sound needs [`Self::poll`] to keep it going.
        pub fn has_loops(&self) -> bool {
            self.playing.iter().any(|(_, again)| again.is_some())
        }

        pub fn stop_all(&mut self) {
            for (mut child, _) in self.playing.drain(..) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    impl Drop for Player {
        fn drop(&mut self) {
            self.stop_all();
        }
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
}

#[cfg(not(target_os = "linux"))]
mod library {
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;

    use rodio::Source;

    #[derive(Default)]
    pub struct Player {
        /// The audio output, opened when the first sound is played.
        device: Option<rodio::MixerDeviceSink>,
        playing: Vec<rodio::Player>,
    }

    impl Player {
        /// Starts playing `file` at `volume` (0 to 1), repeating it until
        /// stopped if `looped`.
        pub fn play(&mut self, file: &Path, volume: f32, looped: bool) -> Result<(), String> {
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
            if looped {
                player.append(source.buffered().repeat_infinite());
            } else {
                player.append(source);
            }
            self.playing.push(player);
            Ok(())
        }

        /// Forgets sounds that have ended.
        pub fn poll(&mut self) {
            self.playing.retain(|player| !player.empty());
        }

        /// Looped sounds repeat by themselves here.
        pub fn has_loops(&self) -> bool {
            false
        }

        pub fn stop_all(&mut self) {
            for player in self.playing.drain(..) {
                player.stop();
            }
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

/// A request in a line of the MUD Sound Protocol, such as
/// `!!SOUND(sword.wav V=80 L=2)`.
#[derive(Debug, Clone, PartialEq)]
pub enum Msp {
    Play {
        file: String,
        volume: f32,
        looped: bool,
    },
    /// `Off` as the file name stops what is playing.
    Stop,
}

/// Reads an MSP trigger if `line` is one. Only the volume and the repeat
/// count are honoured; a count other than 1 loops until stopped.
pub fn parse_msp(line: &str) -> Option<Msp> {
    let line = line.trim();
    let body = (line.strip_prefix("!!SOUND(")).or_else(|| line.strip_prefix("!!MUSIC("))?;
    let mut parts = body.strip_suffix(')')?.split_whitespace();
    let file = parts.next()?;
    if file.eq_ignore_ascii_case("off") {
        return Some(Msp::Stop);
    }
    let (mut volume, mut looped) = (1.0, false);
    for part in parts {
        match part.split_once('=') {
            Some(("V" | "v", value)) => {
                volume = value.parse::<f32>().ok()?.clamp(0.0, 100.0) / 100.0
            }
            Some(("L" | "l", value)) => looped = value.parse::<i32>().ok()? != 1,
            _ => {}
        }
    }
    Some(Msp::Play {
        file: file.to_owned(),
        volume,
        looped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msp_triggers_are_read() {
        let play = |file: &str, volume, looped| {
            Some(Msp::Play {
                file: file.into(),
                volume,
                looped,
            })
        };
        assert_eq!(
            parse_msp("!!SOUND(sword.wav)"),
            play("sword.wav", 1.0, false)
        );
        assert_eq!(
            parse_msp(" !!MUSIC(tavern.ogg V=40 L=-1 T=ambience) "),
            play("tavern.ogg", 0.4, true)
        );
        assert_eq!(
            parse_msp("!!SOUND(hit.wav L=1 P=50)"),
            play("hit.wav", 1.0, false)
        );
        assert_eq!(parse_msp("!!SOUND(Off)"), Some(Msp::Stop));
        for not_msp in [
            "!!SOUND(",
            "!!SOUND()",
            "say !!SOUND(x.wav)",
            "!!SOUND(x.wav V=loud)",
        ] {
            assert_eq!(parse_msp(not_msp), None, "{not_msp}");
        }
    }

    #[test]
    fn missing_file_is_reported() {
        let mut player = Player::default();
        let error = player
            .play(Path::new("/nonexistent/alert.ogg"), 1.0, false)
            .unwrap_err();
        assert!(error.contains("no such sound file"));
        assert!(!player.has_loops());
    }
}
