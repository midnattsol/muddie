//! The keys of Muddie's shortcuts, in the settings: each with its keys,
//! to choose others, add some, leave it without, or give it its own back.

use super::*;

impl App {
    /// Muddie's shortcuts by where they work, each with the keys it is
    /// pressed with: pressed, the next keys are its; and beside it, to add
    /// other keys, to leave it without, and to give it its own back.
    pub(super) fn keys_settings(&mut self, ui: &mut Ui) {
        widgets::page_note(
            ui,
            tr(
                "Press a shortcut's keys to choose others, then the keys; Escape keeps them. Its menu (Shift+F10) adds more keys, leaves it without, or resets it. A macro on the same keys goes before it, and keys a script chooses go before these.",
            ),
        );
        for place in crate::shortcuts::Place::ALL {
            widgets::caption(ui, place.title());
            widgets::group(ui, |ui| {
                for shortcut in crate::shortcuts::ALL
                    .iter()
                    .filter(|shortcut| shortcut.place == place)
                {
                    self.shortcut_row(ui, *shortcut);
                }
            });
        }
    }

    /// The row of a shortcut in the settings: pressed, other keys are
    /// chosen for it; its menu adds keys, leaves it without, or gives it
    /// its own back.
    fn shortcut_row(&mut self, ui: &mut Ui, shortcut: crate::shortcuts::Shortcut) {
        let does = shortcut.about();
        let recording = self.recording_shortcut.map(|(id, _)| id) == Some(shortcut.id);
        let shown = match recording {
            true => tr("Press the keys…").to_owned(),
            false => shortcut.all_written(),
        };
        let about =
            (shortcut.script()).map(|script| trf("Chosen by {0}, which goes first.", &[&script]));
        let button = widgets::setting_row(ui, does, about.as_deref(), &mut |ui| {
            widgets::button(ui, &shown)
        });
        // Named by what it does and its keys, not by the row alone.
        let label = trf("{0}: {1}", &[does, &shown]);
        (ui.ctx()).accesskit_node_builder(button.id, |node| {
            node.set_label(label);
            node.set_description(tr(
                "Press to choose other keys. Its menu adds keys, leaves it without, or resets it.",
            ));
        });
        let (mut add, mut none, mut reset) = (false, false, false);
        widgets::context_menu_of(&button, |ui| {
            add = widgets::menu_item(ui, tr("Add keys…"), "").clicked();
            if !shortcut.chosen_keys().is_empty() {
                none = widgets::menu_item(ui, tr("No keys"), "").clicked();
            }
            if shortcut.is_changed() {
                reset = widgets::menu_item(ui, tr("Reset"), "").clicked();
            }
        });
        if button.clicked() || add {
            self.recording_shortcut = match recording && !add {
                true => None,
                false => Some((shortcut.id, add)),
            };
            let said = match recording && !add {
                true => trf("{0} kept: {1}.", &[does, &shortcut.all_written()]),
                false => trf("Press the keys for {0}; Escape keeps them.", &[does]),
            };
            self.tabs[self.active].say_again(&said, false);
        }
        if none || reset {
            match none {
                true => (self.settings.shortcuts).insert(shortcut.id.to_owned(), String::new()),
                false => self.settings.shortcuts.remove(shortcut.id),
            };
            self.recording_shortcut = None;
            self.apply_settings(ui.ctx());
            let said = trf("{0}: {1}.", &[does, &shortcut.all_written()]);
            self.tabs[self.active].say_again(&said, false);
        }
    }

    /// While a shortcut's keys are chosen, takes the next pressed: they
    /// are its if they can be and nothing else of Muddie's that works
    /// where it does has them; Escape keeps those it had. Either way, it
    /// is said.
    pub(super) fn record_shortcut(&mut self, ui: &mut Ui) {
        let Some((id, adding)) = self.recording_shortcut else {
            return;
        };
        let Some(shortcut) = crate::shortcuts::named(id) else {
            self.recording_shortcut = None;
            return;
        };
        let Some(keys) = self.next_press(ui) else {
            return;
        };
        let does = shortcut.about();
        let written = keys.written();
        let escape = keys == crate::shortcuts::Press::parse("escape").expect("a key");
        let said = if escape {
            self.recording_shortcut = None;
            trf("{0} kept: {1}.", &[does, &shortcut.all_written()])
        } else if let Err(wrong) = shortcut.check(&keys) {
            wrong.said().to_owned()
        } else if let Some(other) = crate::shortcuts::taken_by(&keys, shortcut.place, Some(id)) {
            trf("{0} is already used: {1}.", &[&written, other.about()])
        } else {
            let mut now = match adding {
                true => shortcut.chosen_keys(),
                false => Vec::new(),
            };
            if !now.contains(&keys) {
                now.push(keys);
            }
            match now == shortcut.defaults() {
                true => self.settings.shortcuts.remove(id),
                false => (self.settings.shortcuts)
                    .insert(id.to_owned(), crate::shortcuts::write_list(&now)),
            };
            self.recording_shortcut = None;
            self.apply_settings(ui.ctx());
            trf("{0}: {1}.", &[does, &shortcut.all_written()])
        };
        self.tabs[self.active].say_again(&said, false);
    }

    /// The next keys pressed, of the keypad or of the main block, taken
    /// so that nothing else acts on them.
    fn next_press(&mut self, ui: &mut Ui) -> Option<crate::shortcuts::Press> {
        let held = ui.input(|input| crate::shortcuts::Held::of(input.modifiers));
        let keypad = crate::shortcuts::keypad_left();
        let pressed = match keypad.first() {
            Some((_, name)) => Some(crate::shortcuts::Press {
                held,
                key: crate::shortcuts::Pressed::Keypad(name),
            }),
            None => ui.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => Some(crate::shortcuts::Press {
                        held: crate::shortcuts::Held::of(*modifiers),
                        key: crate::shortcuts::Pressed::Key(*key),
                    }),
                    _ => None,
                })
            }),
        };
        // Nothing else acts on keys being chosen.
        if pressed.is_some() {
            ui.input_mut(|input| input.events.clear());
        }
        pressed
    }
}
