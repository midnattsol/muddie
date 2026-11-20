//! Speech output, the same on every system: through the screen reader in
//! use when one is running, or with the system's own voice. [`Speech`]
//! talks to it, through prism, from a thread of its own; [`Narrator`]
//! decides what incoming text to speak.

mod narrator;

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use prismer::{Backend, Features, Prism, Registry};

pub use narrator::Narrator;

/// Where what is said goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Through {
    /// The screen reader in use, while one is running; nothing otherwise.
    ScreenReader,
    /// The system's own voice, whether or not a screen reader runs.
    Voice,
}

/// What speech goes through now: what it is called, and whether it is a
/// screen reader. Nothing while there is nothing to speak with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub name: Option<String>,
    pub screen_reader: bool,
}

/// Makes a registry of backends, in the speech thread.
pub type MakeRegistry = Box<dyn FnOnce() -> Registry + Send>;

enum Request {
    Say {
        text: String,
        interrupt: bool,
    },
    Stop,
    /// Speech rate from -100 to 100; 0 is the voice's default.
    Rate(i32),
    /// Language to speak in, as a code such as `es`; empty for the
    /// system's default.
    Language(String),
    /// A screen reader or a voice came or went: what is spoken through is
    /// chosen again.
    Changed,
}

/// Handle to the speech thread. Calls never block; dropping it ends the
/// thread.
pub struct Speech {
    requests: Sender<Request>,
    status: Arc<Mutex<Status>>,
}

impl Speech {
    pub fn start(through: Through) -> Self {
        Self::start_with(through, None)
    }

    /// Starts it with the backends of the registry `registry` makes besides
    /// prism's own: for tests, which speak to one of their own. It is made
    /// in the thread that uses it, as it cannot be passed to another.
    pub fn start_with(through: Through, registry: Option<MakeRegistry>) -> Self {
        let (requests, receiver) = channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let (changed, shared) = (requests.clone(), Arc::clone(&status));
        std::thread::Builder::new()
            .name("speech".into())
            .spawn(move || worker(through, registry, &receiver, &changed, &shared))
            .expect("failed to start the speech thread");
        Self { requests, status }
    }

    /// Says `text`, after anything already being said, or in its place if
    /// `interrupt`.
    pub fn say(&self, text: &str, interrupt: bool) {
        let text = text.to_owned();
        let _ = self.requests.send(Request::Say { text, interrupt });
    }

    /// Sets how fast to speak, from -100 (slowest) to 100 (fastest).
    pub fn set_rate(&self, rate: i32) {
        let _ = self.requests.send(Request::Rate(rate.clamp(-100, 100)));
    }

    /// Chooses the language to speak in, by its code (`es`, `en-GB`...).
    /// An empty code goes back to the system's default.
    pub fn set_language(&self, code: &str) {
        let _ = self
            .requests
            .send(Request::Language(code.trim().to_owned()));
    }

    /// Silences what is being said and all that is waiting.
    pub fn stop(&self) {
        let _ = self.requests.send(Request::Stop);
    }

    /// What speech goes through now.
    pub fn status(&self) -> Status {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }
}

/// Speaks what is asked, through what `through` says, choosing it again
/// whenever a screen reader or a voice comes or goes.
fn worker(
    through: Through,
    registry: Option<MakeRegistry>,
    requests: &Receiver<Request>,
    changed: &Sender<Request>,
    status: &Mutex<Status>,
) {
    let changed = changed.clone();
    let mut builder = Prism::builder().on_availability(move |_, _, _| {
        let _ = changed.send(Request::Changed);
    });
    let registry = registry.map(|make| make());
    if let Some(registry) = &registry {
        builder = builder.registry(registry);
    }
    let prism = match builder.build() {
        Ok(prism) => prism,
        Err(error) => return eprintln!("muddie: speech unavailable: {error}"),
    };
    let (mut rate, mut language) = (0, String::new());
    let mut backend = choose(&prism, through);
    tune(backend.as_ref(), rate, &language);
    report(backend.as_ref(), status);
    for request in requests {
        let Some(chosen) = &backend else {
            if let Request::Changed = request {
                backend = choose(&prism, through);
                tune(backend.as_ref(), rate, &language);
                report(backend.as_ref(), status);
            }
            continue;
        };
        match request {
            Request::Say { text, interrupt } => {
                let said = match chosen.supports(Features::OUTPUT) {
                    // Spoken, and on the braille display too.
                    true => chosen.output(&text, interrupt),
                    false => chosen.speak(&text, interrupt),
                };
                if let Err(error) = said {
                    eprintln!("muddie: speech failed: {error}");
                }
            }
            // Not speaking is no error here.
            Request::Stop => drop(chosen.stop()),
            Request::Rate(wanted) => {
                rate = wanted;
                tune(backend.as_ref(), rate, &language);
            }
            Request::Language(wanted) => {
                language = wanted;
                tune(backend.as_ref(), rate, &language);
            }
            Request::Changed => {
                let now = choose(&prism, through);
                let same = now.as_ref().map(Backend::name) == backend.as_ref().map(Backend::name);
                if !same {
                    backend = now;
                    tune(backend.as_ref(), rate, &language);
                    report(backend.as_ref(), status);
                }
            }
        }
    }
}

/// The backend to speak through: the first running screen reader, or the
/// first voice, by prism's order of preference.
fn choose(prism: &Prism, through: Through) -> Option<Backend<'_>> {
    let wanted = match through {
        Through::ScreenReader => Features::IS_SCREEN_READER,
        Through::Voice => Features::IS_SPEECH_ENGINE,
    };
    let mut ids = prism.backend_ids();
    ids.sort_by_key(|id| std::cmp::Reverse(prism.backend_priority(*id)));
    ids.into_iter().find_map(|id| {
        let backend = prism.create(id).ok()?;
        backend.initialize().ok()?;
        let features = backend.features();
        let usable = features.contains(wanted)
            && features.contains(Features::SPEAK)
            && !features.contains(Features::REQUIRES_EXPLICIT_SELECTION)
            // One that says nothing is not a voice.
            && prism.backend_priority(id) > 0;
        usable.then_some(backend)
    })
}

/// Gives a voice the rate and language wanted. A screen reader keeps its
/// own: they are the user's, set in it.
fn tune(backend: Option<&Backend<'_>>, rate: i32, language: &str) {
    let Some(backend) = backend.filter(|backend| !backend.supports(Features::IS_SCREEN_READER))
    else {
        return;
    };
    if backend.supports(Features::SET_RATE) {
        // From -100..100 to prism's 0..1, with 0 at the middle.
        let _ = backend.set_rate((rate as f32 + 100.0) / 200.0);
    }
    if language.is_empty() || !backend.supports(Features::SET_VOICE) {
        return;
    }
    let wanted = language.to_lowercase();
    let voices = backend.voices().unwrap_or_default();
    let voice = (voices.iter()).find(|voice| {
        (voice.language.as_deref())
            .is_some_and(|language| language.to_lowercase().starts_with(&wanted))
    });
    if let Some(voice) = voice {
        let _ = backend.set_voice(voice.id);
    }
}

/// Says what speech now goes through.
fn report(backend: Option<&Backend<'_>>, status: &Mutex<Status>) {
    let now = Status {
        name: backend.map(Backend::name),
        screen_reader: backend.is_some_and(|backend| backend.supports(Features::IS_SCREEN_READER)),
    };
    if let Ok(mut status) = status.lock() {
        *status = now;
    }
}

#[cfg(test)]
mod tests;
