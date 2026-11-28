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

/// The mix as it is given to a player, on Linux, and read back in tests.
#[cfg(any(target_os = "linux", test))]
mod pipe_format {
    use rodio::{ChannelCount, SampleRate};

    /// What PipeWire runs at, so that it does not convert it.
    pub const RATE: SampleRate = SampleRate::new(48_000).unwrap();
    pub const CHANNELS: ChannelCount = ChannelCount::new(2).unwrap();
}

#[cfg(target_os = "linux")]
mod pipe {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use rodio::mixer::{Mixer, MixerSource};

    use super::pipe_format::{CHANNELS, RATE};

    /// How far ahead of what is heard the mix is written: a change, as of
    /// a volume, is heard no later than this.
    const AHEAD: Duration = Duration::from_millis(60);
    /// How much is written at a time, in frames: ten milliseconds.
    const CHUNK: usize = 480;

    /// A program that plays the mix, with its arguments.
    type Player = (&'static str, &'static [&'static str]);

    /// The players tried, in order, with the arguments that have each read
    /// the mix, as 32-bit floats, two channels at 48 kHz, from its input.
    const PLAYERS: [Player; 3] = [
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
        /// The player's process, to end it: one stuck would keep the
        /// thread that feeds it waiting for good.
        playing: Arc<AtomicU32>,
        thread: Option<JoinHandle<()>>,
    }

    impl Pipe {
        #[cfg(test)]
        pub fn none() -> Self {
            Self {
                stop: Arc::new(AtomicBool::new(true)),
                playing: Arc::default(),
                thread: None,
            }
        }
    }

    impl Drop for Pipe {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            // Ended, a player waited on lets the thread go.
            let pid = self.playing.load(Ordering::Relaxed);
            if pid != 0 {
                // SAFETY: a signal to a process of ours, by its id.
                unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
            }
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// Starts a player, and a thread that feeds it the mix.
    pub fn open() -> Result<(Mixer, Pipe), String> {
        open_with(&PLAYERS)
    }

    /// The same, with the first of `players` there is.
    pub(super) fn open_with(players: &'static [Player]) -> Result<(Mixer, Pipe), String> {
        let player = start(players, 0)?;
        let (mixer, source) = rodio::mixer::mixer(CHANNELS, RATE);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let playing = Arc::new(AtomicU32::new(player.child.id()));
        let fed = Arc::clone(&playing);
        let thread = std::thread::Builder::new()
            .name("sound".to_owned())
            .spawn(move || feed(source, players, player, &stopped, &fed))
            .map_err(|error| error.to_string())?;
        Ok((
            mixer,
            Pipe {
                stop,
                playing,
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
    fn start(players: &[Player], from: usize) -> Result<Started, String> {
        for (which, (program, arguments)) in players.iter().enumerate().skip(from) {
            let started = Command::new(program)
                .args(*arguments)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            if let Ok(mut child) = started
                && let Some(input) = child.stdin.take()
            {
                // Little waits in the pipe, two pages, some 20 ms: what is
                // played is heard soon, however late the player started or
                // however long it stopped reading, and the player still has
                // the next to read while it plays.
                // SAFETY: a pipe of ours, open, by its descriptor.
                unsafe { libc::fcntl(input.as_raw_fd(), libc::F_SETPIPE_SZ, 8192) };
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
    fn feed(
        mut source: MixerSource,
        players: &[Player],
        player: Started,
        stop: &AtomicBool,
        playing: &AtomicU32,
    ) {
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
            // Ended because it was let go: not started again.
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let next = match player.at.elapsed() < Duration::from_secs(1) {
                true => player.which + 1,
                false => player.which,
            };
            let Ok(again) = start(players, next) else {
                return;
            };
            playing.store(again.child.id(), Ordering::Relaxed);
            (player, written) = (again, 0);
        }
        drop(player.input);
        let _ = player.child.kill();
        let _ = player.child.wait();
    }
}

#[cfg(all(test, target_os = "linux"))]
mod pipe_tests {
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use super::pipe::open_with;

    fn leak(text: String) -> &'static str {
        Box::leak(text.into_boxed_str())
    }

    /// A player that never reads, as one stuck: letting the sound go,
    /// as it does after a while with nothing to play, does not wait on it.
    #[test]
    fn a_player_that_does_not_read_is_let_go_at_once() {
        let (mixer, pipe) = open_with(&[("sleep", &["30"])]).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let started = Instant::now();
        drop(pipe);
        drop(mixer);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "letting it go took {:?}",
            started.elapsed()
        );
    }

    /// A player that takes a while to start, as one does, hears sounds
    /// soon once it plays: what was mixed while it was starting does not
    /// wait in the pipe ahead of them for good.
    #[test]
    fn a_player_slow_to_start_hears_sounds_soon_after() {
        let Ok(_) = std::process::Command::new("python3").arg("-V").output() else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("muddie-pipe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (ready, heard) = (dir.join("ready"), dir.join("heard"));
        // Starts reading a while after it is started, then reads the mix
        // as fast as it is played; says once it reads, and when it first
        // meets what is not silence.
        let script = format!(
            "import sys,time\n\
             time.sleep(0.5)\n\
             start=time.time(); n=0; f=sys.stdin.buffer; told=False\n\
             open({ready:?},'w').write('1')\n\
             while True:\n\
             \x20   chunk=f.read(3840)\n\
             \x20   if not chunk: break\n\
             \x20   n+=len(chunk)\n\
             \x20   wait=start+n/384000-time.time()\n\
             \x20   if wait>0: time.sleep(wait)\n\
             \x20   if any(chunk) and not told:\n\
             \x20       open({heard:?},'w').write(repr(time.time())); told=True\n",
            ready = ready.display().to_string(),
            heard = heard.display().to_string(),
        );
        let args: &'static [&'static str] = Box::leak(Box::new(["-c", leak(script)]));
        let players: &'static [(&str, &[&str])] = Box::leak(Box::new([("python3", args)]));
        let (mixer, pipe) = open_with(players).unwrap();
        let waited = Instant::now();
        while !ready.exists() && waited.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(10));
        }
        // Playing steadily for a while.
        std::thread::sleep(Duration::from_millis(500));
        let played = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        mixer.add(rodio::source::SineWave::new(440.0));
        std::thread::sleep(Duration::from_millis(1000));
        let at: f64 = (std::fs::read_to_string(&heard).unwrap_or_default().trim())
            .parse()
            .unwrap_or(f64::MAX);
        drop(pipe);
        let _ = std::fs::remove_dir_all(&dir);
        let late = at - played.as_secs_f64();
        assert!(late < 0.12, "heard {late:.3} s after it was played");
    }
}
