//! Where what Muddie mixes is heard: on Linux, one of the system's
//! players, fed the mix through a pipe; elsewhere, the system's audio.

use rodio::mixer::Mixer;

/// What the sounds are mixed into, and what hears the mix.
pub struct Output {
    mixer: Mixer,
    #[cfg(target_os = "linux")]
    _pipe: pipe::Pipe,
    #[cfg(not(target_os = "linux"))]
    _device: Option<rodio::MixerDeviceSink>,
}

impl Output {
    /// Opens the system's audio, or says why it could not.
    #[cfg(target_os = "linux")]
    pub fn open() -> Result<Self, String> {
        let (mixer, pipe) = pipe::open()?;
        Ok(Self { mixer, _pipe: pipe })
    }

    /// Opens the system's audio, or says why it could not.
    #[cfg(not(target_os = "linux"))]
    pub fn open() -> Result<Self, String> {
        let mut device =
            rodio::DeviceSinkBuilder::open_default_sink().map_err(|error| error.to_string())?;
        // Closed on purpose after a while with nothing to play.
        device.log_on_drop(false);
        Ok(Self {
            mixer: device.mixer().clone(),
            _device: Some(device),
        })
    }

    /// A mix heard by no one, to read it back: for tests.
    #[cfg(test)]
    pub fn memory() -> (Self, rodio::mixer::MixerSource) {
        let (mixer, source) = rodio::mixer::mixer(pipe_format::CHANNELS, pipe_format::RATE);
        let output = Self {
            mixer,
            #[cfg(target_os = "linux")]
            _pipe: pipe::Pipe::none(),
            #[cfg(not(target_os = "linux"))]
            _device: None,
        };
        (output, source)
    }

    pub fn mixer(&self) -> &Mixer {
        &self.mixer
    }
}

/// The mix as it is given to a player.
mod pipe_format {
    use rodio::{ChannelCount, SampleRate};

    /// What PipeWire runs at, so that it does not convert it.
    pub const RATE: SampleRate = SampleRate::new(48_000).unwrap();
    pub const CHANNELS: ChannelCount = ChannelCount::new(2).unwrap();
}

#[cfg(target_os = "linux")]
mod pipe {
    use std::io::Write;
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use rodio::mixer::{Mixer, MixerSource};

    use super::pipe_format::{CHANNELS, RATE};

    /// How far ahead of what is heard the mix is written: a change, as of
    /// a volume, is heard no later than this.
    const AHEAD: Duration = Duration::from_millis(60);
    /// How much is written at a time, in frames: ten milliseconds.
    const CHUNK: usize = 480;

    /// The players tried, in order, with the arguments that have each read
    /// the mix, as 32-bit floats, two channels at 48 kHz, from its input.
    const PLAYERS: [(&str, &[&str]); 3] = [
        (
            "pw-cat",
            &[
                "--playback",
                "--raw",
                "--format=f32",
                "--rate=48000",
                "--channels=2",
                "--latency=40ms",
                "--properties={ application.name = \"Muddie\" media.name = \"Muddie\" }",
                "-",
            ],
        ),
        (
            "pacat",
            &[
                "--playback",
                "--format=float32le",
                "--rate=48000",
                "--channels=2",
                "--latency-msec=40",
                "--client-name=Muddie",
                "--stream-name=Muddie",
            ],
        ),
        (
            "aplay",
            &[
                "-q", "-t", "raw", "-f", "FLOAT_LE", "-r", "48000", "-c", "2", "-",
            ],
        ),
    ];

    /// The player fed the mix, by a thread of its own, until dropped.
    pub struct Pipe {
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<()>>,
    }

    impl Pipe {
        #[cfg(test)]
        pub fn none() -> Self {
            Self {
                stop: Arc::new(AtomicBool::new(true)),
                thread: None,
            }
        }
    }

    impl Drop for Pipe {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// Starts a player, and a thread that feeds it the mix.
    pub fn open() -> Result<(Mixer, Pipe), String> {
        let player = start(0)?;
        let (mixer, source) = rodio::mixer::mixer(CHANNELS, RATE);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("sound".to_owned())
            .spawn(move || feed(source, player, &stopped))
            .map_err(|error| error.to_string())?;
        Ok((
            mixer,
            Pipe {
                stop,
                thread: Some(thread),
            },
        ))
    }

    /// A player started, and what it reads from.
    struct Started {
        child: Child,
        input: ChildStdin,
        /// Which of [`PLAYERS`] it is, and when it started.
        which: usize,
        at: Instant,
    }

    /// The first player there is from the one `from` in the list on,
    /// started, with its input.
    fn start(from: usize) -> Result<Started, String> {
        for (which, (program, arguments)) in PLAYERS.iter().enumerate().skip(from) {
            let started = Command::new(program)
                .args(*arguments)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            if let Ok(mut child) = started
                && let Some(input) = child.stdin.take()
            {
                let at = Instant::now();
                return Ok(Started {
                    child,
                    input,
                    which,
                    at,
                });
            }
        }
        Err("no sound player found (tried pw-cat, pacat and aplay)".to_owned())
    }

    /// Writes the mix to the player as it is heard, a little ahead, until
    /// told to stop. A player that goes is started again; one that goes as
    /// soon as it starts cannot play the mix, and the next is tried.
    fn feed(mut source: MixerSource, player: Started, stop: &AtomicBool) {
        let mut player = player;
        let mut written: u64 = 0;
        let mut bytes = Vec::with_capacity(CHUNK * usize::from(CHANNELS.get()) * 4);
        while !stop.load(Ordering::Relaxed) {
            let due = ((player.at.elapsed() + AHEAD).as_secs_f64() * f64::from(RATE.get())) as u64;
            if written >= due {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            bytes.clear();
            for _ in 0..CHUNK * usize::from(CHANNELS.get()) {
                let sample = source.next().unwrap_or(0.0).clamp(-1.0, 1.0);
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
            if player.input.write_all(&bytes).is_ok() {
                written += CHUNK as u64;
                continue;
            }
            let _ = player.child.kill();
            let _ = player.child.wait();
            let next = match player.at.elapsed() < Duration::from_secs(1) {
                true => player.which + 1,
                false => player.which,
            };
            let Ok(again) = start(next) else {
                return;
            };
            (player, written) = (again, 0);
        }
        drop(player.input);
        let _ = player.child.kill();
        let _ = player.child.wait();
    }
}
