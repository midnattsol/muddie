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
        pub fn play_with(
            &mut self,
            file: &Path,
            options: Options,
            label: Label,
        ) -> Result<(), String> {
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
}

#[cfg(not(target_os = "linux"))]
mod library {
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
        pub fn play_with(
            &mut self,
            file: &Path,
            options: Options,
            label: Label,
        ) -> Result<(), String> {
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
}

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

/// A request in a line of the MUD Sound Protocol, such as
/// `!!SOUND(sword.wav V=80 L=2 P=60 T=combat U=http://mud.example/s/)`.
#[derive(Debug, Clone, PartialEq)]
pub enum Msp {
    Play {
        /// `!!MUSIC` rather than `!!SOUND`.
        music: bool,
        /// The file, which may have `*` and `?` in it to stand for any
        /// of several, and no ending, which is then `.wav` or `.mid`.
        file: String,
        /// From 0 to 1.
        volume: f32,
        /// How many times; `None` for until stopped.
        times: Option<u32>,
        /// From 0 to 100: a sound stops those of less, and is not played
        /// over one of more. Music has none.
        priority: u8,
        /// Music already playing goes on rather than starting again.
        keep_going: bool,
        /// The kind of sound, which is also the folder it may be in.
        kind: String,
        /// Where on the web to fetch it from.
        url: Option<String>,
    },
    /// `Off` as the file name stops the sounds, or the music, and with an
    /// address says where files are fetched from when none is given.
    Stop { music: bool, url: Option<String> },
}

/// Reads an MSP trigger if `line` is one.
pub fn parse_msp(line: &str) -> Option<Msp> {
    let line = line.trim();
    let (music, body) = match (line.strip_prefix("!!SOUND("), line.strip_prefix("!!MUSIC(")) {
        (Some(body), _) => (false, body),
        (_, Some(body)) => (true, body),
        _ => return None,
    };
    let mut parts = body.strip_suffix(')')?.split_whitespace();
    let file = parts.next()?;
    let (mut volume, mut times, mut priority) = (1.0, Some(1), 50);
    let (mut keep_going, mut kind, mut url) = (true, String::new(), None);
    for part in parts {
        let (name, value) = part.split_once('=')?;
        match name.to_ascii_uppercase().as_str() {
            "V" => volume = value.parse::<f32>().ok()?.clamp(0.0, 100.0) / 100.0,
            "L" => {
                times = match value.parse::<i32>().ok()? {
                    times if times < 0 => None,
                    times => Some((times as u32).max(1)),
                }
            }
            "P" => priority = value.parse::<u32>().ok()?.min(100) as u8,
            "C" => keep_going = value.parse::<i32>().ok()? != 0,
            "T" => kind = value.to_owned(),
            "U" => url = Some(value.to_owned()),
            _ => {}
        }
    }
    if file.eq_ignore_ascii_case("off") {
        return Some(Msp::Stop { music, url });
    }
    // With no ending it is the usual one for its kind.
    let named = Path::new(file).extension().is_some();
    let ending = if music { "mid" } else { "wav" };
    Some(Msp::Play {
        music,
        file: if named {
            file.to_owned()
        } else {
            format!("{file}.{ending}")
        },
        volume,
        times,
        priority: if music { 0 } else { priority },
        keep_going: music && keep_going,
        kind,
        url,
    })
}

/// Whether the file name `name` is one `pattern` stands for: `*` for any
/// run of characters and `?` for any one, whatever the case of letters.
pub fn name_matches(pattern: &str, name: &str) -> bool {
    fn matches(pattern: &[char], name: &[char]) -> bool {
        match pattern.split_first() {
            None => name.is_empty(),
            Some(('*', rest)) => (0..=name.len()).any(|skipped| matches(rest, &name[skipped..])),
            Some(('?', rest)) => !name.is_empty() && matches(rest, &name[1..]),
            Some((wanted, rest)) => name.first() == Some(wanted) && matches(rest, &name[1..]),
        }
    }
    let lower = |text: &str| -> Vec<char> { text.to_lowercase().chars().collect() };
    matches(&lower(pattern), &lower(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msp_triggers_are_read() {
        let sound = |file: &str, volume, times, priority, kind: &str, url: Option<&str>| {
            Some(Msp::Play {
                music: false,
                file: file.into(),
                volume,
                times,
                priority,
                keep_going: false,
                kind: kind.into(),
                url: url.map(String::from),
            })
        };
        assert_eq!(
            parse_msp("!!SOUND(sword.wav)"),
            sound("sword.wav", 1.0, Some(1), 50, "", None)
        );
        // With no ending, the usual one; and all it can say.
        assert_eq!(
            parse_msp("  !!SOUND(hit* V=80 L=-1 P=70 T=combat U=http://mud.example/s/)  "),
            sound(
                "hit*.wav",
                0.8,
                None,
                70,
                "combat",
                Some("http://mud.example/s/")
            )
        );
        assert_eq!(
            parse_msp("!!SOUND(a.wav l=3 v=250)"),
            sound("a.wav", 1.0, Some(3), 50, "", None)
        );
        // Music goes on unless told to start again, and has no priority.
        let music = |keep_going| {
            Some(Msp::Play {
                music: true,
                file: "town.mid".into(),
                volume: 0.5,
                times: Some(1),
                priority: 0,
                keep_going,
                kind: String::new(),
                url: None,
            })
        };
        assert_eq!(parse_msp("!!MUSIC(town V=50)"), music(true));
        assert_eq!(parse_msp("!!MUSIC(town V=50 C=0)"), music(false));
        // Off stops the sounds or the music, and may say where files are.
        assert_eq!(
            parse_msp("!!SOUND(Off)"),
            Some(Msp::Stop {
                music: false,
                url: None
            })
        );
        assert_eq!(
            parse_msp("!!MUSIC(off U=http://mud.example/m/)"),
            Some(Msp::Stop {
                music: true,
                url: Some("http://mud.example/m/".into())
            })
        );
        for other in [
            "You hear a !!SOUND(x.wav)",
            "!!SOUND(x.wav",
            "!!SOUND()",
            "!!SOUND(x.wav V=loud)",
        ] {
            assert_eq!(parse_msp(other), None, "{other}");
        }
    }

    #[test]
    fn a_name_with_stars_stands_for_several_files() {
        assert!(name_matches("hit*.wav", "hit3.wav") && name_matches("hit*.wav", "HIT.WAV"));
        assert!(name_matches("step?.wav", "step1.wav") && !name_matches("step?.wav", "step12.wav"));
        assert!(!name_matches("hit*.wav", "miss.wav") && name_matches("*", "anything.ogg"));
    }

    #[test]
    fn ffplay_is_told_where_to_start_and_how_to_fade() {
        let plain = Options {
            volume: 0.5,
            ..Options::default()
        };
        assert!(!plain.shaped());
        assert_eq!(
            ffplay_arguments(&plain).join(" "),
            "-nodisp -autoexit -loglevel quiet -volume 50"
        );
        let shaped = Options {
            volume: 1.0,
            start: 1500,
            finish: Some(11500),
            fade_in: 2000,
            fade_out: 3000,
            ..Options::default()
        };
        assert!(shaped.shaped());
        assert_eq!(
            ffplay_arguments(&shaped).join(" "),
            "-nodisp -autoexit -loglevel quiet -volume 100 -ss 1.500 -t 10.000 \
             -af afade=t=in:st=0:d=2.000,afade=t=out:st=7.000:d=3.000"
        );
        // It can only die away before an end it knows.
        let open = Options {
            fade_out: 3000,
            ..Options::default()
        };
        assert!(!ffplay_arguments(&open).join(" ").contains("afade"));
    }
}
