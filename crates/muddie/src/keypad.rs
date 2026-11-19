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

/// A keypad key: its name for `bind`, the key egui takes it for, and what
/// it types with Num Lock on.
struct KeypadKey {
    code: KeyCode,
    name: &'static str,
    egui_key: Option<Key>,
    types: &'static str,
}

const fn key(
    code: KeyCode,
    name: &'static str,
    egui_key: Option<Key>,
    types: &'static str,
) -> KeypadKey {
    KeypadKey {
        code,
        name,
        egui_key,
        types,
    }
}

const KEYS: [KeypadKey; 16] = [
    key(KeyCode::Numpad0, "numpad0", Some(Key::Num0), "0"),
    key(KeyCode::Numpad1, "numpad1", Some(Key::Num1), "1"),
    key(KeyCode::Numpad2, "numpad2", Some(Key::Num2), "2"),
    key(KeyCode::Numpad3, "numpad3", Some(Key::Num3), "3"),
    key(KeyCode::Numpad4, "numpad4", Some(Key::Num4), "4"),
    key(KeyCode::Numpad5, "numpad5", Some(Key::Num5), "5"),
    key(KeyCode::Numpad6, "numpad6", Some(Key::Num6), "6"),
    key(KeyCode::Numpad7, "numpad7", Some(Key::Num7), "7"),
    key(KeyCode::Numpad8, "numpad8", Some(Key::Num8), "8"),
    key(KeyCode::Numpad9, "numpad9", Some(Key::Num9), "9"),
    key(KeyCode::NumpadAdd, "numpadadd", Some(Key::Plus), "+"),
    key(
        KeyCode::NumpadSubtract,
        "numpadsubtract",
        Some(Key::Minus),
        "-",
    ),
    key(KeyCode::NumpadMultiply, "numpadmultiply", None, "*"),
    key(KeyCode::NumpadDivide, "numpaddivide", Some(Key::Slash), "/"),
    key(
        KeyCode::NumpadDecimal,
        "numpaddecimal",
        Some(Key::Period),
        ".",
    ),
    key(KeyCode::NumpadEnter, "numpadenter", Some(Key::Enter), ""),
];

/// Keypad keys pressed since they were last taken, by name.
pub type Pressed = Arc<Mutex<Vec<&'static str>>>;

/// Whether `event` is what egui made of a press of the keypad key `name`:
/// the key itself, or the character it typed. Such events are dropped when
/// the key is bound, so that it does not also type or move the cursor.
pub fn is_event_of(name: &str, event: &Event) -> bool {
    let Some(keypad_key) = KEYS.iter().find(|key| key.name == name) else {
        return false;
    };
    match event {
        Event::Text(text) => !keypad_key.types.is_empty() && text == keypad_key.types,
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
}

impl ApplicationHandler<UserEvent> for Watcher<'_> {
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::KeyboardInput {
            event: key_event, ..
        } = &event
            && key_event.state == ElementState::Pressed
            && !key_event.repeat
            && let PhysicalKey::Code(code) = key_event.physical_key
            && let Some(keypad_key) = KEYS.iter().find(|key| key.code == code)
        {
            self.pressed
                .lock()
                .expect("keypad lock")
                .push(keypad_key.name);
            if let Some(ctx) = &*self.ctx.lock().expect("context lock") {
                ctx.request_repaint();
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
        // Num Lock off: it arrives as the up arrow.
        assert!(is_event_of(
            "numpad8",
            &key_event(Key::ArrowUp, Some(Key::Num8))
        ));
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
