//! Muddie's shortcuts: every one there is, the keys each is pressed with,
//! and how they are chosen. Each has keys of its own; the player can
//! choose others in the settings, and a script others still, which go
//! first while it is loaded: so that everything can be set by writing it,
//! and a set of keys shared as a script.
//!
//! The keys are not the same on a Mac: there Option types characters (@
//! on a Spanish keyboard is Option+2), so that it cannot be held for a
//! letter or a figure, and Cmd does what Ctrl does elsewhere.

mod list;
mod press;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::BTreeMap;

use eframe::egui::{self, Key, Modifiers};

pub use list::*;
pub use press::{Held, Press, Pressed, Wrong};

const MAC: bool = cfg!(target_os = "macos");

/// Held with a letter or a figure (reading the latest lines, the gauges,
/// where one is): Alt; on a Mac, Ctrl.
#[cfg(test)]
pub const WITH_KEY: Modifiers = if MAC { Modifiers::CTRL } else { Modifiers::ALT };

/// Held with the arrows (reading line by line, moving in the editor's
/// list): Alt, which is Option on a Mac, where the arrows type nothing and
/// Ctrl and the arrows are the system's.
#[cfg(test)]
pub const WITH_ARROWS: Modifiers = Modifiers::ALT;

/// Takes Tab, if it was pressed alone: with Shift it goes back, which taking
/// it with nothing held would take too.
pub fn take_tab(input: &mut egui::InputState) -> bool {
    !input.modifiers.shift && input.consume_key(Modifiers::NONE, Key::Tab)
}

/// How copying is written: the system's, which stays.
pub fn copy_written() -> &'static str {
    if MAC { "Cmd+C" } else { "Ctrl+C" }
}

/// The keys of every shortcut, as they are now: by name, those that are
/// not their own. Of the window's thread: tests, each in a thread of its
/// own, do not change each other's.
#[derive(Default)]
struct Bound {
    /// Chosen in the settings, as they are kept there.
    chosen: BTreeMap<String, String>,
    /// Chosen by the scripts of the session on screen, in the order they
    /// were: the script, the shortcut and its keys.
    scripted: Vec<(String, String, Vec<String>)>,
    /// What both make of each shortcut's keys.
    now: BTreeMap<&'static str, Vec<Press>>,
    /// The script that chose each shortcut's keys, of those a script did.
    by_script: BTreeMap<&'static str, String>,
}

thread_local! {
    static BOUND: RefCell<Bound> = RefCell::default();
    /// The keypad's keys pressed in this frame, with what was held, not
    /// yet taken by a shortcut or a macro.
    static KEYPAD: RefCell<Vec<(Held, &'static str)>> = const { RefCell::new(Vec::new()) };
    /// Whether the map is explored in this frame: its keys are its own.
    static EXPLORING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Whether keys that type were taken, and what they typed has not come
    /// yet: it may come in the next frame.
    static TYPED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Takes what keys a shortcut took typed, if it came in a frame after
/// them, before another key was pressed: run as a frame begins.
pub fn drop_typed(input: &mut egui::InputState) {
    if !TYPED.with(std::cell::Cell::get) {
        return;
    }
    let came = (input.events.iter()).position(|event| {
        matches!(
            event,
            egui::Event::Text(_) | egui::Event::Key { pressed: true, .. }
        )
    });
    if let Some(at) = came {
        TYPED.with(|waiting| waiting.set(false));
        if matches!(input.events[at], egui::Event::Text(_)) {
            input.events.remove(at);
        }
    }
}

/// Says whether the map is explored in this frame: while it is, the keys
/// of its shortcuts do nothing else.
pub fn exploring(now: bool) {
    EXPLORING.with(|exploring| exploring.set(now));
}

/// Whether the map, explored, has these keys for one of its shortcuts.
fn the_maps(keys: &Press) -> bool {
    EXPLORING.with(std::cell::Cell::get)
        && (ALL.iter()).any(|shortcut| shortcut.place == Place::Map && shortcut.has(keys))
}

/// Takes the keys chosen in the settings, by each shortcut's name: one or
/// more, separated by commas, or none, written as [`Press::stored`] writes
/// them.
pub fn configure(chosen: &BTreeMap<String, String>) {
    BOUND.with(|bound| {
        let mut bound = bound.borrow_mut();
        bound.chosen = chosen.clone();
        work_out(&mut bound);
    });
}

/// Takes the keys the scripts of the session on screen chose, in the order
/// they did: the script, the shortcut and its keys, written as
/// [`Press::stored`] writes them.
pub fn script(scripted: Vec<(String, String, Vec<String>)>) {
    BOUND.with(|bound| {
        let mut bound = bound.borrow_mut();
        if bound.scripted != scripted {
            bound.scripted = scripted;
            work_out(&mut bound);
        }
    });
}

/// Works out the keys of every shortcut: those chosen in the settings in
/// place of its own, then those the scripts chose, each taking its keys
/// from the shortcuts they would meet.
fn work_out(bound: &mut Bound) {
    let mut now = BTreeMap::new();
    for shortcut in ALL {
        if let Some(text) = bound.chosen.get(shortcut.id) {
            now.insert(shortcut.id, read_list(text));
        }
    }
    let mut by_script = BTreeMap::new();
    for (script, id, keys) in &bound.scripted {
        let Some(shortcut) = named(id) else {
            continue;
        };
        let keys: Vec<Press> = keys.iter().filter_map(|key| Press::parse(key)).collect();
        for other in ALL {
            if other.id != shortcut.id && other.place.meets(shortcut.place) {
                let had = now
                    .get(other.id)
                    .cloned()
                    .unwrap_or_else(|| other.defaults.to_vec());
                let kept: Vec<Press> = (had.iter())
                    .filter(|key| !keys.contains(key))
                    .copied()
                    .collect();
                if kept.len() != had.len() {
                    now.insert(other.id, kept);
                }
            }
        }
        now.insert(shortcut.id, keys);
        by_script.insert(shortcut.id, script.clone());
    }
    bound.now = now;
    bound.by_script = by_script;
}

/// Keys as the settings keep them: several, separated by commas.
fn read_list(text: &str) -> Vec<Press> {
    (text.split(',').map(str::trim))
        .filter(|key| !key.is_empty())
        .filter_map(Press::parse)
        .collect()
}

/// Keys as the settings keep them.
pub fn write_list(keys: &[Press]) -> String {
    (keys.iter().map(Press::stored))
        .collect::<Vec<_>>()
        .join(", ")
}

impl Shortcut {
    /// The keys it is pressed with now: none, if it was left without.
    pub fn keys(&self) -> Vec<Press> {
        BOUND
            .with(|bound| bound.borrow().now.get(self.id).cloned())
            .unwrap_or_else(|| self.defaults.to_vec())
    }

    /// The keys it has without the scripts: its own, or those chosen in
    /// the settings.
    pub fn chosen_keys(&self) -> Vec<Press> {
        let chosen = BOUND.with(|bound| bound.borrow().chosen.get(self.id).cloned());
        chosen.map_or_else(|| self.defaults.to_vec(), |text| read_list(&text))
    }

    /// How its first keys are written, as said in a sentence: none, if it
    /// has none.
    pub fn written(&self) -> &'static str {
        match self.keys().first() {
            Some(keys) => written_once(keys.written()),
            None => crate::i18n::tr("no keys"),
        }
    }

    /// How all its keys are written, one after another.
    pub fn all_written(&self) -> String {
        let keys = self.keys();
        match keys.is_empty() {
            true => crate::i18n::tr("No keys").to_owned(),
            false => (keys.iter().map(Press::written))
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    /// Whether the settings give it keys other than its own.
    pub fn is_changed(&self) -> bool {
        BOUND.with(|bound| bound.borrow().chosen.contains_key(self.id))
    }

    /// The script that chose its keys, if one did: those go first.
    pub fn script(&self) -> Option<String> {
        BOUND.with(|bound| bound.borrow().by_script.get(self.id).cloned())
    }

    /// Whether it was pressed, taking the press, of the main block or of
    /// the keypad.
    /// While the map is explored, the keys of its shortcuts are its own.
    pub fn pressed(&self, input: &mut egui::InputState) -> bool {
        let others = self.place != Place::Map;
        (self.keys().iter()).any(|keys| !(others && the_maps(keys)) && take(input, keys))
    }

    /// What egui is told to press its first keys, of the main block: for
    /// tests.
    #[cfg(test)]
    pub fn first(&self) -> (Modifiers, Key) {
        Self::pressed_as(self, self.keys()[0])
    }

    /// The same of its own first keys, whatever keys it has now: for tests.
    #[cfg(test)]
    pub fn first_own(&self) -> (Modifiers, Key) {
        Self::pressed_as(self, self.defaults[0])
    }

    #[cfg(test)]
    fn pressed_as(&self, keys: Press) -> (Modifiers, Key) {
        match keys.key {
            Pressed::Key(key) => (keys.held.modifiers(), key),
            _ => panic!("{} is not pressed with a key of the main block", self.id),
        }
    }

    /// Whether `keys` are among those it is pressed with now.
    pub fn has(&self, keys: &Press) -> bool {
        self.keys().contains(keys)
    }

    /// Whether `keys` can be its keys: what is wrong with them if not.
    pub fn check(&self, keys: &Press) -> Result<(), Wrong> {
        keys.check(self.place.typing(), self.can_be_alone())
    }
}

/// How keys are written, kept for as long as Muddie runs: the same text
/// is made once however often it is asked for.
fn written_once(text: String) -> &'static str {
    thread_local! {
        static MADE: RefCell<BTreeMap<String, &'static str>> = RefCell::default();
    }
    MADE.with(|made| {
        *made
            .borrow_mut()
            .entry(text.clone())
            .or_insert_with(|| Box::leak(text.into_boxed_str()))
    })
}

/// Notes the keypad's keys pressed in this frame, by name, with what was
/// held: shortcuts take them, and what is left goes to the macros.
pub fn keypad_pressed(held: Modifiers, names: impl IntoIterator<Item = &'static str>) {
    let held = Held::of(held);
    KEYPAD.with(|keypad| {
        let mut keypad = keypad.borrow_mut();
        keypad.clear();
        keypad.extend(names.into_iter().map(|name| (held, name)));
    });
}

/// The keypad's keys pressed in this frame that nothing took yet, taken.
pub fn keypad_left() -> Vec<(Held, &'static str)> {
    KEYPAD.with(|keypad| std::mem::take(&mut *keypad.borrow_mut()))
}

/// Puts back keys of the keypad taken but not used, for the shortcuts.
pub fn keypad_give_back(left: Vec<(Held, &'static str)>) {
    KEYPAD.with(|keypad| keypad.borrow_mut().extend(left));
}

/// Whether keys of the map's shortcuts that type were pressed in this
/// frame: of the main block, or of the keypad.
pub fn maps_keys_pressed(input: &egui::InputState) -> bool {
    let the_maps = |keys: &Press| {
        (ALL.iter()).any(|shortcut| shortcut.place == Place::Map && shortcut.has(keys))
    };
    let keypad = KEYPAD.with(|keypad| {
        (keypad.borrow().iter()).any(|(held, name)| {
            the_maps(&Press {
                held: *held,
                key: Pressed::Keypad(name),
            })
        })
    });
    keypad
        || input.events.iter().any(|event| match event {
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let keys = Press {
                    held: Held::of(*modifiers),
                    key: Pressed::Key(*key),
                };
                keys.types() && the_maps(&keys)
            }
            _ => false,
        })
}

/// Takes what egui made of a press of the keypad key `name`: the key, and
/// what it typed, now or in a frame after.
pub fn take_keypad_events(input: &mut egui::InputState, name: &str) {
    let typed = (input.events.iter()).any(|event| {
        matches!(event, egui::Event::Text(_)) && crate::keypad::is_event_of(name, event)
    });
    input
        .events
        .retain(|event| !crate::keypad::is_event_of(name, event));
    if crate::keypad::types(name) && !typed {
        TYPED.with(|waiting| waiting.set(true));
    }
}

/// Takes a press of `keys`, if there was one in this frame: of the main
/// block, not what egui made of a key of the keypad; or of the keypad,
/// with what egui made of it, so that it does not also type.
pub fn take(input: &mut egui::InputState, keys: &Press) -> bool {
    match keys.key {
        Pressed::Key(key) => {
            let waiting: Vec<&'static str> =
                KEYPAD.with(|keypad| keypad.borrow().iter().map(|(_, name)| *name).collect());
            let at = input.events.iter().position(|event| match event {
                egui::Event::Key {
                    key: pressed,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    *pressed == key
                        && Held::of(*modifiers) == keys.held
                        && !(waiting.iter()).any(|name| crate::keypad::is_event_of(name, event))
                }
                _ => false,
            });
            let Some(at) = at else {
                return false;
            };
            input.events.remove(at);
            // Keys that type, taken, type nothing.
            if keys.types() {
                let typed = (input.events[at..].iter())
                    .position(|event| matches!(event, egui::Event::Text(_)));
                match typed {
                    Some(typed) => drop(input.events.remove(at + typed)),
                    None => TYPED.with(|waiting| waiting.set(true)),
                }
            }
            true
        }
        Pressed::Keypad(name) => {
            let taken = KEYPAD.with(|keypad| {
                let mut keypad = keypad.borrow_mut();
                let at =
                    (keypad.iter()).position(|(held, known)| *known == name && *held == keys.held);
                at.map(|at| keypad.remove(at)).is_some()
            });
            if taken {
                take_keypad_events(input, name);
            }
            taken
        }
        Pressed::Alone => false,
    }
}

/// Every shortcut, as scripts are told of them.
pub fn for_scripts() -> Vec<muddie_script::KnownShortcut> {
    (ALL.iter())
        .map(|shortcut| muddie_script::KnownShortcut {
            name: shortcut.id.to_owned(),
            place: shortcut.place.id().to_owned(),
            about: shortcut.about().to_owned(),
            keys: shortcut.chosen_keys().iter().map(Press::stored).collect(),
        })
        .collect()
}

/// Reads keys a script gives the shortcut named `name`: as they are kept,
/// or what is wrong with them, in English, as scripts are told.
pub fn check_for_scripts(name: &str, key: &str) -> Result<String, String> {
    let shortcut = named(name).ok_or_else(|| "is for no shortcut".to_owned())?;
    let keys = Press::parse(key).ok_or_else(|| Wrong::Unknown.for_scripts().to_owned())?;
    shortcut
        .check(&keys)
        .map_err(|wrong| wrong.for_scripts().to_owned())?;
    Ok(keys.stored())
}

/// What has `keys` among Muddie's shortcuts that would meet one working
/// in `place`, but `except`: what it does, if one has.
pub fn taken_by(keys: &Press, place: Place, except: Option<&str>) -> Option<Shortcut> {
    (ALL.iter())
        .filter(|shortcut| Some(shortcut.id) != except && shortcut.place.meets(place))
        .find(|shortcut| shortcut.has(keys))
        .copied()
}

/// What of Muddie's has the keys of a macro, as it names them
/// (`alt+shift+f5`): the macro goes before it.
pub fn taken_by_macro(key: &str) -> Option<Shortcut> {
    let keys = Press::parse(key)?;
    taken_by(&keys, Place::Everywhere, None)
}
