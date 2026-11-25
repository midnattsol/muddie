//! Speech output, the same on every system: through the screen reader in
//! use when one is running, or with the system's own voice. [`Speech`]
//! talks to it, through prism, from a thread of its own; [`Narrator`]
//! decides what incoming text to speak.

mod narrator;

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    /// Whether it has been looked for yet: until then, nothing is known.
    pub known: bool,
}

/// How often a screen reader or a voice is looked for while there is none:
/// one can start at any time, or be too busy to answer when first asked.
const LOOK_AGAIN: Duration = Duration::from_secs(2);

/// What waited longer than this to be said is not said: the screen reader
/// was not answering, and what the game said then is old news.
const STALE: Duration = Duration::from_secs(3);

/// Called when what speech goes through changes: to look at it again.
pub type Wake = Box<dyn Fn() + Send>;

/// Makes a registry of backends, in the speech thread.
pub type MakeRegistry = Box<dyn FnOnce() -> Registry + Send>;

enum Request {
    Say {
        text: String,
        interrupt: bool,
        at: Instant,
    },
    Stop,
    /// Speech rate from -100 to 100; 0 is the voice's default.
    Rate(i32),
    /// Language to speak in, as a code such as `es`; empty for the
    /// system's default.
    Language(String),
    /// What to say when what speech goes through changes, with `{0}` for
    /// its name; and whether to say it of what it is now, too.
    Announce {
        template: String,
        now: bool,
    },
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
    /// Starts speaking through what `through` says; `wake` is called when
    /// that changes.
    pub fn start(through: Through, wake: Wake) -> Self {
        Self::start_with(through, None, wake)
    }

    /// Starts it with the backends of the registry `registry` makes besides
    /// prism's own: for tests, which speak to one of their own. It is made
    /// in the thread that uses it, as it cannot be passed to another.
    pub fn start_with(through: Through, registry: Option<MakeRegistry>, wake: Wake) -> Self {
        let (requests, receiver) = channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let (changed, shared) = (requests.clone(), Arc::clone(&status));
        let told = Told {
            status: shared,
            wake,
        };
        std::thread::Builder::new()
            .name("speech".into())
            .spawn(move || worker(through, registry, &receiver, &changed, &told))
            .expect("failed to start the speech thread");
        Self { requests, status }
    }

    /// Says `text`, after anything already being said, or in its place if
    /// `interrupt`.
    pub fn say(&self, text: &str, interrupt: bool) {
        let (text, at) = (text.to_owned(), Instant::now());
        let _ = self.requests.send(Request::Say {
            text,
            interrupt,
            at,
        });
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

    /// Has what speech goes through said when it changes, as `template`
    /// says with `{0}` for its name: a screen reader started, or a voice
    /// came. What it is at first is said too if `now`: when it was chosen
    /// just then, rather than found at start.
    pub fn announce_changes(&self, template: &str, now: bool) {
        let template = template.to_owned();
        let _ = self.requests.send(Request::Announce { template, now });
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
    told: &Told,
) {
    // What goes wrong in prism is told where Muddie's own troubles are.
    prismer::log::set_handler(|level, source, message| {
        eprintln!("muddie: speech: {level:?}: {source}: {message}");
    });
    let _ = prismer::log::set_level(prismer::log::Level::Warn);
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
    let mut settings = Settings::default();
    let mut backend = choose(&prism, through);
    tune(backend.as_ref(), settings.rate, &settings.language);
    report(backend.as_ref(), told);
    loop {
        let Some(request) = next_request(requests, backend.is_some()) else {
            return;
        };
        let Some(chosen) = &backend else {
            match request {
                Request::Changed => switch(
                    &prism,
                    through,
                    &mut backend,
                    (settings.rate, &settings.language, &settings.announce),
                    told,
                ),
                other => settings.keep_unheard(other),
            }
            continue;
        };
        match request {
            Request::Changed => switch(
                &prism,
                through,
                &mut backend,
                (settings.rate, &settings.language, &settings.announce),
                told,
            ),
            other => hear(other, chosen, &mut settings),
        }
    }
}

/// What the user chose for speech, kept to give to what is found.
#[derive(Default)]
struct Settings {
    rate: i32,
    language: String,
    announce: String,
}

impl Settings {
    /// Keeps what was asked while nothing is found to speak through.
    fn keep_unheard(&mut self, request: Request) {
        match request {
            Request::Rate(wanted) => self.rate = wanted,
            Request::Language(wanted) => self.language = wanted,
            // Said of the first found, when it is found.
            Request::Announce { template, .. } => self.announce = template,
            Request::Say { .. } | Request::Stop | Request::Changed => {}
        }
    }
}

/// Waits for the next request: for good if something to speak through
/// is found, otherwise looking again every so often. `None` when there
/// will be no more.
fn next_request(requests: &Receiver<Request>, found: bool) -> Option<Request> {
    match found {
        true => requests.recv().ok(),
        false => match requests.recv_timeout(LOOK_AGAIN) {
            Ok(request) => Some(request),
            Err(RecvTimeoutError::Timeout) => Some(Request::Changed),
            Err(RecvTimeoutError::Disconnected) => None,
        },
    }
}

/// Does a request other than looking again, through `chosen`.
fn hear(request: Request, chosen: &Backend<'_>, settings: &mut Settings) {
    match request {
        Request::Say { at, .. } if at.elapsed() > STALE => {}
        Request::Say {
            text, interrupt, ..
        } => {
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
            settings.rate = wanted;
            tune(Some(chosen), settings.rate, &settings.language);
        }
        Request::Language(wanted) => {
            settings.language = wanted;
            tune(Some(chosen), settings.rate, &settings.language);
        }
        Request::Announce { template, now } => {
            if now {
                let _ = chosen.speak(&template.replace("{0}", &chosen.name()), false);
            }
            settings.announce = template;
        }
        Request::Changed => {}
    }
}

/// Takes what speech now goes through, if it is another, and says so
/// before anything else is said through it.
fn switch<'a>(
    prism: &'a Prism,
    through: Through,
    backend: &mut Option<Backend<'a>>,
    (rate, language, announce): (i32, &str, &str),
    told: &Told,
) {
    let now = choose(prism, through);
    if now.as_ref().map(Backend::name) == backend.as_ref().map(Backend::name) {
        return;
    }
    *backend = now;
    tune(backend.as_ref(), rate, language);
    report(backend.as_ref(), told);
    if let Some(chosen) = backend.as_ref().filter(|_| !announce.is_empty()) {
        let _ = chosen.speak(&announce.replace("{0}", &chosen.name()), false);
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
        // One that says nothing is not a voice.
        if prism.backend_priority(id) == 0 {
            return None;
        }
        let backend = prism.create(id).ok()?;
        // What it is, known before it is started: what is not wanted is
        // not started at all.
        let features = backend.features();
        let usable = features.contains(wanted)
            && features.contains(Features::SPEAK)
            && !features.contains(Features::REQUIRES_EXPLICIT_SELECTION);
        (usable && backend.initialize().is_ok()).then_some(backend)
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

/// The screen readers and voices this build can speak through, by name,
/// whether or not they are running.
pub fn built_in() -> Vec<String> {
    let Ok(prism) = Prism::new() else {
        return Vec::new();
    };
    let mut names: Vec<String> = (prism.backend_ids().into_iter())
        .filter(|id| prism.backend_priority(*id) > 0)
        .filter_map(|id| prism.backend_name(id))
        .collect();
    names.sort();
    names
}

/// Where what speech goes through is told: kept to be read, and the window
/// woken to read it.
struct Told {
    status: Arc<Mutex<Status>>,
    wake: Wake,
}

/// Says what speech now goes through.
fn report(backend: Option<&Backend<'_>>, told: &Told) {
    let now = Status {
        name: backend.map(Backend::name),
        screen_reader: backend.is_some_and(|backend| backend.supports(Features::IS_SCREEN_READER)),
        known: true,
    };
    if let Ok(mut status) = told.status.lock() {
        *status = now;
    }
    (told.wake)();
}

#[cfg(test)]
mod tests;
