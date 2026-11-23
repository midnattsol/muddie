//! The keys bound by scripts and macros.

use super::*;

impl Tab {
    /// Runs the binding of any key pressed with Ctrl or Alt, or of a
    /// function key, before anything else gets to use the key.
    pub(super) fn bound_keys(&mut self, ui: &mut egui::Ui) {
        if ui.input_mut(|input| crate::shortcuts::RECONNECT.pressed(input)) {
            self.reconnect(&ui.ctx().clone());
        }
        if ui.input_mut(|input| crate::shortcuts::MAP.pressed(input)) {
            let shown = self.map_shown();
            self.set_map_shown(!shown);
        }
        let pressed: Vec<(Key, Modifiers)> = ui.input(|input| {
            let keys = input.events.iter().filter_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            });
            keys.collect()
        });
        // Keypad keys come from the window layer, as egui cannot tell them
        // from the main block's. A bound one must not also type its digit.
        let held = ui.input(|input| input.modifiers);
        // While the map is explored, the keypad moves on it: nothing bound
        // to it is sent to the MUD.
        let mut keypad = std::mem::take(&mut self.keypad_pressed);
        let exploring = self.map_view.exploring();
        if self.map_view.has_keyboard() {
            // On the map, by where the keys are: 7 is north-west, not the
            // Home egui makes of it with Num Lock off.
            for name in &keypad {
                ui.input_mut(|input| {
                    input
                        .events
                        .retain(|event| !keypad::is_event_of(name, event))
                });
            }
            self.map_view.keypad_pressed(std::mem::take(&mut keypad));
        }
        for name in keypad.into_iter().filter(|_| !exploring) {
            if self.scripts.on_key(&key_name(held, name)) {
                ui.input_mut(|input| {
                    input
                        .events
                        .retain(|event| !keypad::is_event_of(name, event))
                });
                self.run_actions();
            }
        }
        for (key, modifiers) in pressed {
            let name = key.name().to_lowercase();
            let is_function_key =
                name.len() > 1 && name.starts_with('f') && name[1..].parse::<u8>().is_ok();
            if !(modifiers.ctrl || modifiers.alt || is_function_key)
                || self.map_view.takes(key, modifiers)
            {
                continue;
            }
            if self.scripts.on_key(&key_name(modifiers, &name)) {
                ui.input_mut(|input| input.consume_key(modifiers, key));
                self.run_actions();
            }
        }
    }
}

/// A key as macros name it: `ctrl+`, `alt+` and `shift+`, those held, in
/// that order, before its name.
pub(super) fn key_name(held: egui::Modifiers, name: &str) -> String {
    let mut full = String::new();
    for (down, modifier) in [
        (held.ctrl, "ctrl+"),
        (held.alt, "alt+"),
        (held.shift, "shift+"),
    ] {
        if down {
            full.push_str(modifier);
        }
    }
    full + name
}

impl Tab {
    /// Opens the bar to find in the output, with the keyboard in it.
    pub(super) fn open_find(&mut self) {
        self.find.open = true;
        self.find.wants_focus = true;
    }
}
