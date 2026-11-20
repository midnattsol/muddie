//! The keys bound by scripts and macros.

use super::*;

impl Tab {
    /// Runs the binding of any key pressed with Ctrl or Alt, or of a
    /// function key, before anything else gets to use the key.
    pub(super) fn bound_keys(&mut self, ui: &mut egui::Ui) {
        if ui.input_mut(|input| input.consume_key(Modifiers::ALT, Key::R)) {
            self.reconnect(&ui.ctx().clone());
        }
        if ui.input_mut(|input| input.consume_key(Modifiers::COMMAND, Key::M)) {
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
        for name in std::mem::take(&mut self.keypad_pressed) {
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
            full.push_str(name);
            if self.scripts.on_key(&full) {
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
            if !(modifiers.ctrl || modifiers.alt || is_function_key) {
                continue;
            }
            let mut full = String::new();
            for (held, modifier) in [
                (modifiers.ctrl, "ctrl+"),
                (modifiers.alt, "alt+"),
                (modifiers.shift, "shift+"),
            ] {
                if held {
                    full.push_str(modifier);
                }
            }
            full.push_str(&name);
            if self.scripts.on_key(&full) {
                ui.input_mut(|input| input.consume_key(modifiers, key));
                self.run_actions();
            }
        }
    }
}
