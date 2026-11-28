//! Keys of the numeric keypad. egui reports them as the digits and symbols
//! they type, the same as the keys of the main block, so they are picked
//! out one layer down, from the physical key of each key press.

use std::sync::{Arc, Mutex};

use eframe::egui::{self, Event, Key};
use eframe::{EframeWinitApplication, UserEvent};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, StartCause, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowId;

/// A keypad key: its name for `bind` with Num Lock on, the one it has
/// with Num Lock off if that is another, the key egui takes it for, and
/// what it types with Num Lock on.
struct KeypadKey {
    code: KeyCode,
    name: &'static str,
    off: &'static str,
    egui_key: Option<Key>,
    types: &'static str,
}

const fn key(
    code: KeyCode,
    name: &'static str,
    off: &'static str,
    egui_key: Option<Key>,
    types: &'static str,
) -> KeypadKey {
    KeypadKey {
        code,
        name,
        off,
        egui_key,
        types,
    }
}

const KEYS: [KeypadKey; 16] = [
    key(
        KeyCode::Numpad0,
        "numpad0",
        "numpadinsert",
        Some(Key::Num0),
        "0",
    ),
    key(
        KeyCode::Numpad1,
        "numpad1",
        "numpadend",
        Some(Key::Num1),
        "1",
    ),
    key(
        KeyCode::Numpad2,
        "numpad2",
        "numpaddown",
        Some(Key::Num2),
        "2",
    ),
    key(
        KeyCode::Numpad3,
        "numpad3",
        "numpadpagedown",
        Some(Key::Num3),
        "3",
    ),
    key(
        KeyCode::Numpad4,
        "numpad4",
        "numpadleft",
        Some(Key::Num4),
        "4",
    ),
    key(
        KeyCode::Numpad5,
        "numpad5",
        "numpadclear",
        Some(Key::Num5),
        "5",
    ),
    key(
        KeyCode::Numpad6,
        "numpad6",
        "numpadright",
        Some(Key::Num6),
        "6",
    ),
    key(
        KeyCode::Numpad7,
        "numpad7",
        "numpadhome",
        Some(Key::Num7),
        "7",
    ),
    key(
        KeyCode::Numpad8,
        "numpad8",
        "numpadup",
        Some(Key::Num8),
        "8",
    ),
    key(
        KeyCode::Numpad9,
        "numpad9",
        "numpadpageup",
        Some(Key::Num9),
        "9",
    ),
    key(KeyCode::NumpadAdd, "numpadadd", "", Some(Key::Plus), "+"),
    key(
        KeyCode::NumpadSubtract,
        "numpadsubtract",
        "",
        Some(Key::Minus),
        "-",
    ),
    key(KeyCode::NumpadMultiply, "numpadmultiply", "", None, "*"),
    key(
        KeyCode::NumpadDivide,
        "numpaddivide",
        "",
        Some(Key::Slash),
        "/",
    ),
    key(
        KeyCode::NumpadDecimal,
        "numpaddecimal",
        "numpaddelete",
        Some(Key::Period),
        ".",
    ),
    key(
        KeyCode::NumpadEnter,
        "numpadenter",
        "",
        Some(Key::Enter),
        "",
    ),
];

/// The name of a press of the keypad key `code`. With Num Lock off the
/// digits and the point are other keys, by what is then written on them:
/// arrows, Home, End and the like; `typing` says whether the press typed
/// a character, which is how Num Lock being on is told.
pub fn name_of(code: KeyCode, typing: bool) -> Option<&'static str> {
    let key = KEYS.iter().find(|key| key.code == code)?;
    Some(if typing || key.off.is_empty() {
        key.name
    } else {
        key.off
    })
}

/// Every name the keypad's keys have.
pub fn names() -> impl Iterator<Item = &'static str> {
    (KEYS.iter())
        .flat_map(|key| [key.name, key.off])
        .filter(|name| !name.is_empty())
}

/// Whether a press of the keypad key `name` types a character.
pub fn types(name: &str) -> bool {
    KEYS.iter()
        .any(|key| key.name == name && !key.types.is_empty())
}

/// Keypad keys pressed since they were last taken, by name.
pub type Pressed = Arc<Mutex<Vec<&'static str>>>;

/// Whether `event` is what egui made of a press of the keypad key `name`:
/// the key itself, or the character it typed. Such events are dropped when
/// the key is bound, so that it does not also type or move the cursor.
pub fn is_event_of(name: &str, event: &Event) -> bool {
    let named = |key: &&KeypadKey| key.name == name || (!key.off.is_empty() && key.off == name);
    let Some(keypad_key) = KEYS.iter().find(named) else {
        return false;
    };
    match event {
        // Only with Num Lock on does it type.
        Event::Text(text) => {
            keypad_key.name == name && !keypad_key.types.is_empty() && text == keypad_key.types
        }
        // With Num Lock off the key arrives as an arrow or the like, but
        // its physical key is still the keypad's.
        Event::Key { physical_key, .. } => {
            keypad_key.egui_key.is_some() && *physical_key == keypad_key.egui_key
        }
        _ => false,
    }
}

/// Runs the application, noting every keypad key pressed on the way.
pub struct Watcher<'a> {
    pub app: EframeWinitApplication<'a>,
    pub pressed: Pressed,
    /// Asked to redraw when a keypad key arrives, in case nothing else does.
    pub ctx: Arc<Mutex<Option<egui::Context>>>,
    /// Whether text is being composed with an input method.
    pub composing: bool,
}

impl ApplicationHandler<UserEvent> for Watcher<'_> {
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // Held down, a key is pressed again and again, as a key of the
        // main block is: what it is bound to is done again, and it never
        // types what egui makes of it.
        if let WindowEvent::KeyboardInput {
            event: key_event, ..
        } = &event
            && key_event.state == ElementState::Pressed
            && let PhysicalKey::Code(code) = key_event.physical_key
            && let Some(name) = name_of(
                code,
                matches!(key_event.logical_key, winit::keyboard::Key::Character(_)),
            )
        {
            self.pressed.lock().expect("keypad lock").push(name);
            if let Some(ctx) = &*self.ctx.lock().expect("context lock") {
                ctx.request_repaint();
            }
        }
        // Some input methods answer every frame, with nothing being
        // composed, that nothing is: passed on, each answer asks for a
        // frame, which gets another answer, and the window is drawn sixty
        // times a second for as long as the command line has the focus.
        if let WindowEvent::Ime(winit::event::Ime::Preedit(text, _)) = &event {
            let idle = text.is_empty() && !self.composing;
            self.composing = !text.is_empty();
            if idle {
                return;
            }
        }
        self.app.window_event(event_loop, window_id, event);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.app.resumed(event_loop);
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        self.app.new_events(event_loop, cause);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        self.app.user_event(event_loop, event);
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        self.app.device_event(event_loop, device_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.app.about_to_wait(event_loop);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.app.suspended(event_loop);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.app.exiting(event_loop);
    }

    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        self.app.memory_warning(event_loop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_of_a_keypad_key_are_recognised() {
        let key_event = |key, physical_key| Event::Key {
            key,
            physical_key,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        assert!(is_event_of("numpad8", &Event::Text("8".into())));
        assert!(is_event_of(
            "numpad8",
            &key_event(Key::Num8, Some(Key::Num8))
        ));
        // Num Lock off: it arrives as the up arrow, and is another key
        // to bind, which types nothing.
        assert!(is_event_of(
            "numpadup",
            &key_event(Key::ArrowUp, Some(Key::Num8))
        ));
        assert!(!is_event_of("numpadup", &Event::Text("8".into())));
        assert_eq!(name_of(KeyCode::Numpad8, true), Some("numpad8"));
        assert_eq!(name_of(KeyCode::Numpad8, false), Some("numpadup"));
        assert_eq!(name_of(KeyCode::Numpad5, false), Some("numpadclear"));
        assert_eq!(name_of(KeyCode::NumpadDecimal, false), Some("numpaddelete"));
        // The signs and Enter are the same keys either way.
        assert_eq!(name_of(KeyCode::NumpadAdd, false), Some("numpadadd"));
        assert_eq!(name_of(KeyCode::NumpadEnter, false), Some("numpadenter"));
        assert_eq!(name_of(KeyCode::KeyA, true), None);
        // The real up arrow is another key.
        assert!(!is_event_of(
            "numpad8",
            &key_event(Key::ArrowUp, Some(Key::ArrowUp))
        ));
        assert!(!is_event_of("numpad8", &Event::Text("9".into())));
        assert!(is_event_of(
            "numpadenter",
            &key_event(Key::Enter, Some(Key::Enter))
        ));
        assert!(!is_event_of("numpadenter", &Event::Text(String::new())));
        assert!(!is_event_of("f5", &Event::Text("8".into())));
    }
}
