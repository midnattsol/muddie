//! Speech output. [`Speech`] talks to the platform's speech service from a
//! background thread; [`Narrator`] decides what incoming text to speak.
//!
//! On Linux this talks to speech-dispatcher directly. On Windows and
//! macOS it uses the system's own speech through the `tts` crate.

mod narrator;
#[cfg(target_os = "linux")]
mod ssip;

use std::sync::mpsc::{Receiver, Sender, channel};

pub use narrator::Narrator;

enum Request {
    Say(String),
    Stop,
    /// Speech rate from -100 to 100; 0 is the voice's default.
    Rate(i32),
    /// Language to speak in, as a code such as `es`; empty for the
    /// system's default.
    Language(String),
}

/// Handle to the speech thread. Calls never block; dropping it ends the
/// thread.
pub struct Speech {
    requests: Sender<Request>,
}

impl Speech {
    pub fn start() -> Self {
        let (requests, receiver) = channel();
        std::thread::Builder::new()
            .name("speech".into())
            .spawn(move || worker(receiver))
            .expect("failed to start the speech thread");
        Self { requests }
    }

    /// Queues `text` after anything already being spoken.
    pub fn say(&self, text: &str) {
        let _ = self.requests.send(Request::Say(text.to_owned()));
    }

    /// Sets how fast to speak, from -100 (slowest) to 100 (fastest).
    pub fn set_rate(&self, rate: i32) {
        let _ = self.requests.send(Request::Rate(rate.clamp(-100, 100)));
    }

    /// Chooses the language to speak in, by its code (`es`, `en-GB`...).
    /// An empty code goes back to the system's default.
    pub fn set_language(&self, code: &str) {
        // Only what a language code can hold, as it goes into a command.
        let code: String = (code.trim().chars())
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
            .collect();
        let _ = self.requests.send(Request::Language(code));
    }

    /// Silences the current and all queued speech.
    pub fn stop(&self) {
        let _ = self.requests.send(Request::Stop);
    }
}

#[cfg(target_os = "linux")]
fn worker(requests: Receiver<Request>) {
    // The connection is opened on first use and reopened after a failure,
    // so a restarted speech service is picked up again.
    let mut connection: Option<ssip::Connection> = None;
    for request in requests {
        let result = match connection.take() {
            Some(open) => Ok(open),
            None => ssip::Connection::open(),
        }
        .and_then(|mut open| {
            match &request {
                Request::Say(text) => open.speak(text)?,
                Request::Stop => open.cancel()?,
                Request::Rate(rate) => open.set_rate(*rate)?,
                Request::Language(code) if code.is_empty() => {}
                Request::Language(code) => open.set_language(code)?,
            }
            Ok(open)
        });
        match result {
            Ok(open) => connection = Some(open),
            Err(error) => eprintln!("muddie: speech unavailable: {error}"),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn worker(requests: Receiver<Request>) {
    // The voice is opened on first use and reopened after a failure.
    let mut voice: Option<tts::Tts> = None;
    for request in requests {
        if voice.is_none() {
            match tts::Tts::default() {
                Ok(opened) => voice = Some(opened),
                Err(error) => eprintln!("muddie: speech unavailable: {error}"),
            }
        }
        let Some(tts) = &mut voice else {
            continue;
        };
        let result = match request {
            Request::Say(text) => tts.speak(text, false).map(drop),
            Request::Stop => tts.stop().map(drop),
            Request::Rate(rate) => {
                // Each system has its own scale: map ours onto it, with 0
                // at the voice's normal rate.
                let (slowest, normal, fastest) =
                    (tts.min_rate(), tts.normal_rate(), tts.max_rate());
                let share = rate as f32 / 100.0;
                let span = if share >= 0.0 {
                    fastest - normal
                } else {
                    normal - slowest
                };
                tts.set_rate(normal + span * share).map(drop)
            }
            Request::Language(code) if code.is_empty() => Ok(()),
            // The first installed voice of that language, if there is one.
            Request::Language(code) => tts.voices().and_then(|voices| {
                let wanted = code.to_lowercase();
                let voice = (voices.iter()).find(|voice| {
                    voice
                        .language()
                        .to_string()
                        .to_lowercase()
                        .starts_with(&wanted)
                });
                match voice {
                    Some(voice) => tts.set_voice(voice),
                    None => Ok(()),
                }
            }),
        };
        if let Err(error) = result {
            eprintln!("muddie: speech unavailable: {error}");
            voice = None;
        }
    }
}
