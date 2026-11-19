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
        };
        if let Err(error) = result {
            eprintln!("muddie: speech unavailable: {error}");
            voice = None;
        }
    }
}
