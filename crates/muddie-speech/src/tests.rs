//! Speaking through a reader of the tests' own, which notes what it is told.

use std::time::{Duration, Instant};

use prismer::{CustomBackend, RegistryBuilder};

use super::*;

/// What the reader was told, in order: what to say and whether it was to
/// interrupt, or to be quiet.
type Heard = Arc<Mutex<Vec<String>>>;

struct Reader(Heard);

impl CustomBackend for Reader {
    fn speak(&mut self, text: &str, interrupt: bool) -> prismer::Result<()> {
        let mark = if interrupt { "!" } else { "" };
        self.0.lock().unwrap().push(format!("{mark}{text}"));
        Ok(())
    }

    fn stop(&mut self) -> prismer::Result<()> {
        self.0.lock().unwrap().push("(quiet)".into());
        Ok(())
    }
}

/// A registry with a reader that is a screen reader, or a voice, above
/// every other there is, and what it hears.
fn registry_of(kind: Features) -> (MakeRegistry, Heard) {
    let heard = Heard::default();
    let noted = Arc::clone(&heard);
    let make: MakeRegistry = Box::new(move || {
        let mut registry = RegistryBuilder::new().unwrap();
        let features = kind | Features::SPEAK | Features::STOP;
        registry
            .add_backend("Test reader", 10_000, features, move || {
                Some(Reader(Arc::clone(&noted)))
            })
            .unwrap();
        registry.freeze().unwrap()
    });
    (make, heard)
}

/// Speech through that reader.
fn reader(through: Through, kind: Features) -> (Speech, Heard) {
    let (make, heard) = registry_of(kind);
    (
        Speech::start_with(through, Some(make), Box::new(|| {})),
        heard,
    )
}

/// Waits for what is heard to be `wanted`, however busy the machine.
fn hears(heard: &Heard, wanted: &[&str]) {
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        if *heard.lock().unwrap() == wanted {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(*heard.lock().unwrap(), wanted);
}

#[test]
fn what_is_said_reaches_the_screen_reader_which_is_interrupted_and_quieted() {
    let (speech, heard) = reader(Through::ScreenReader, Features::IS_SCREEN_READER);
    speech.say("A goblin arrives.", false);
    speech.say("It attacks you!", true);
    speech.stop();
    hears(
        &heard,
        &["A goblin arrives.", "!It attacks you!", "(quiet)"],
    );
    let status = speech.status();
    assert_eq!(status.name.as_deref(), Some("Test reader"));
    assert!(status.screen_reader);
}

#[test]
fn a_voice_is_not_taken_for_a_screen_reader() {
    // Speaking through a screen reader, a voice does not do.
    let (speech, heard) = reader(Through::ScreenReader, Features::IS_SPEECH_ENGINE);
    speech.say("Nothing to hear.", false);
    std::thread::sleep(Duration::from_millis(200));
    assert!(heard.lock().unwrap().is_empty());
    // Asked for a voice, it is the one.
    let (speech, heard) = reader(Through::Voice, Features::IS_SPEECH_ENGINE);
    speech.say("Heard.", false);
    hears(&heard, &["Heard."]);
    assert!(!speech.status().screen_reader);
}

/// Built without what they need, the backends of a system are left out with
/// no word said, and the program is mute: this says so instead.
#[test]
#[cfg(target_os = "linux")]
fn the_screen_reader_and_the_voice_of_the_system_are_built_in() {
    let prism = Prism::new().unwrap();
    for name in ["Orca", "Speech Dispatcher"] {
        assert!(
            prism.backend_id_by_name(name).is_ok(),
            "{name} was left out of the build: install the development files of dbus-1 and speech-dispatcher"
        );
    }
}

/// A reader that takes four seconds to answer the first time, as Orca does
/// when it is stuck.
struct Stuck(Heard, bool);

impl CustomBackend for Stuck {
    fn speak(&mut self, text: &str, _interrupt: bool) -> prismer::Result<()> {
        if !std::mem::replace(&mut self.1, true) {
            std::thread::sleep(Duration::from_secs(4));
        }
        self.0.lock().unwrap().push(text.to_owned());
        Ok(())
    }
}

#[test]
fn what_waited_while_the_reader_was_stuck_is_not_said() {
    let heard = Heard::default();
    let noted = Arc::clone(&heard);
    let make: MakeRegistry = Box::new(move || {
        let mut registry = RegistryBuilder::new().unwrap();
        let features = Features::IS_SCREEN_READER | Features::SPEAK;
        registry
            .add_backend("Stuck reader", 10_000, features, move || {
                Some(Stuck(Arc::clone(&noted), false))
            })
            .unwrap();
        registry.freeze().unwrap()
    });
    let speech = Speech::start_with(Through::ScreenReader, Some(make), Box::new(|| {}));
    speech.say("First.", false);
    speech.say("Old news.", false);
    std::thread::sleep(Duration::from_secs(5));
    speech.say("Now.", false);
    hears(&heard, &["First.", "Now."]);
}

#[test]
fn the_window_is_woken_when_what_speaks_is_known() {
    let woken = Arc::new(Mutex::new(0));
    let count = Arc::clone(&woken);
    let wake: Wake = Box::new(move || *count.lock().unwrap() += 1);
    let (registry, _heard) = registry_of(Features::IS_SCREEN_READER);
    let speech = Speech::start_with(Through::ScreenReader, Some(registry), wake);
    let until = Instant::now() + Duration::from_secs(10);
    while *woken.lock().unwrap() == 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(*woken.lock().unwrap() > 0);
    assert!(speech.status().known);
}
