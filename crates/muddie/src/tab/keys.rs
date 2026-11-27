//! The keys bound by scripts and macros.

use super::*;

impl Tab {
    /// The shortcuts of the session that go before the rest of it.
    pub(super) fn session_keys(&mut self, ui: &mut egui::Ui) {
        if ui.input_mut(|input| crate::shortcuts::RECONNECT.pressed(input)) {
            self.reconnect(&ui.ctx().clone());
        }
        if ui.input_mut(|input| crate::shortcuts::MAP.pressed(input)) {
            let shown = self.map_shown();
            self.set_map_shown(!shown);
        }
    }

    /// Runs the macro of any key pressed with Ctrl or Alt, of a function
    /// key or of the keypad, before anything else gets to use the key,
    /// Muddie's own shortcuts too; but while the map is explored, the keys
    /// it takes are its own, and while a macro's key is recorded, none.
    pub fn macro_keys(&mut self, ui: &mut egui::Ui) {
        if self.editor.recording_key {
            return;
        }
        self.keypad_macros(ui);
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
        for (key, modifiers) in pressed {
            let name = key.name().to_lowercase();
            let is_function_key =
                name.len() > 1 && name.starts_with('f') && name[1..].parse::<u8>().is_ok();
            if !(modifiers.ctrl || modifiers.alt || is_function_key) {
                continue;
            }
            let macro_name = key_name(modifiers, &name);
            // On the map explored, a key that walks moves there.
            if let Some(way) = self.walking_way(&macro_name) {
                ui.input_mut(|input| input.consume_key(modifiers, key));
                self.map_view.walk_key(way);
                continue;
            }
            let keys = crate::shortcuts::Press {
                held: crate::shortcuts::Held::of(modifiers),
                key: crate::shortcuts::Pressed::Key(key),
            };
            if self.map_view.takes(&keys) {
                continue;
            }
            if self.scripts.on_key(&macro_name) {
                ui.input_mut(|input| input.consume_key(modifiers, key));
                self.run_actions();
            }
        }
    }

    /// The macros of the keypad's keys, which come from the window layer,
    /// as egui cannot tell them from the main block's: a bound one does not
    /// also type its digit. While the map is explored, a key that walks
    /// moves there, and nothing else bound to the keypad is sent.
    fn keypad_macros(&mut self, ui: &mut egui::Ui) {
        let exploring = self.map_view.exploring();
        let mut left = Vec::new();
        for (held, name) in crate::shortcuts::keypad_left() {
            let macro_name = key_name(held.modifiers(), name);
            let way = self.walking_way(&macro_name);
            let taken = match way {
                Some(way) => {
                    self.map_view.walk_key(way);
                    true
                }
                None => !exploring && self.scripts.on_key(&macro_name),
            };
            if taken {
                ui.input_mut(|input| crate::shortcuts::take_keypad_events(input, name));
                self.run_actions();
            } else {
                left.push((held, name));
            }
        }
        crate::shortcuts::keypad_give_back(left);
    }

    /// The way the macro of `key` walks, while the map is explored: when
    /// all it sends is one direction, the map moves there instead, so that
    /// the keys that walk in the game walk on the map too.
    fn walking_way(&self, key: &str) -> Option<&'static str> {
        if !self.map_view.exploring() {
            return None;
        }
        let commands = self.scripts.macro_commands(key)?;
        let [command] = commands.as_slice() else {
            return None;
        };
        let way = self.map.borrow().direction(command.trim());
        (crate::shortcuts::MAP_MOVES.iter())
            .find(|(_, known)| *known == way)
            .map(|(_, way)| *way)
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
