//! The keys Muddie's own shortcuts are held with, which are not the same on
//! a Mac: there Option types characters (@ on a Spanish keyboard is
//! Option+2), so that it cannot be held for a letter or a figure, and Cmd
//! does what Ctrl does elsewhere. How each is written comes from here too.

use std::cell::RefCell;
use std::collections::BTreeMap;

use eframe::egui::{Key, KeyboardShortcut, Modifiers};

use crate::i18n::tr;

const MAC: bool = cfg!(target_os = "macos");

/// Held with a letter or a figure (reading the latest lines, the gauges,
/// where one is): Alt; on a Mac, Ctrl.
pub const WITH_KEY: Modifiers = if MAC { Modifiers::CTRL } else { Modifiers::ALT };

/// Held with the arrows (reading line by line, moving in the editor's
/// list): Alt, which is Option on a Mac, where the arrows type nothing and
/// Ctrl and the arrows are the system's.
pub const WITH_ARROWS: Modifiers = Modifiers::ALT;

/// How Command and a key are written: Cmd on a Mac, Ctrl elsewhere.
macro_rules! command {
    ($key:literal) => {
        if cfg!(target_os = "macos") {
            concat!("Cmd+", $key)
        } else {
            concat!("Ctrl+", $key)
        }
    };
}

/// How `WITH_KEY` and a key are written.
macro_rules! with_key {
    ($key:literal) => {
        if cfg!(target_os = "macos") {
            concat!("Ctrl+", $key)
        } else {
            concat!("Alt+", $key)
        }
    };
}

/// How `WITH_ARROWS` and a key are written.
macro_rules! with_arrows {
    ($key:literal) => {
        if cfg!(target_os = "macos") {
            concat!("Option+", $key)
        } else {
            concat!("Alt+", $key)
        }
    };
}

pub(crate) use {with_arrows, with_key};

/// One of Muddie's shortcuts: the keys it is pressed with, and how they
/// are written, everywhere it is shown. Most can be pressed with other
/// keys, chosen in the settings: see [`configure`].
#[derive(Debug, Clone, Copy)]
pub struct Shortcut {
    /// Its name in the settings file.
    pub id: &'static str,
    default_keys: KeyboardShortcut,
    default_written: &'static str,
}

impl Shortcut {
    /// The keys it is pressed with now.
    pub fn keys(&self) -> KeyboardShortcut {
        bound(self.id).map_or(self.default_keys, |(keys, _)| keys)
    }

    /// How they are written.
    pub fn written(&self) -> &'static str {
        bound(self.id).map_or(self.default_written, |(_, written)| written)
    }

    /// Whether it is pressed with keys other than its own.
    pub fn is_changed(&self) -> bool {
        bound(self.id).is_some()
    }

    /// Whether it was pressed, taking the press.
    pub fn pressed(&self, input: &mut eframe::egui::InputState) -> bool {
        input.consume_shortcut(&self.keys())
    }
}

/// Command and a key: Cmd on a Mac, Ctrl elsewhere.
macro_rules! command_shortcut {
    ($id:literal, $key:ident, $written:literal) => {
        Shortcut {
            id: $id,
            default_keys: KeyboardShortcut::new(Modifiers::COMMAND, eframe::egui::Key::$key),
            default_written: command!($written),
        }
    };
}

pub const NEW_SESSION: Shortcut = command_shortcut!("new_session", T, "T");
pub const CLOSE_SESSION: Shortcut = command_shortcut!("close_session", W, "W");
/// Between the game and the editor.
pub const SWITCH_VIEW: Shortcut = command_shortcut!("switch_view", E, "E");
/// The incoming data.
pub const DATA_VIEW: Shortcut = command_shortcut!("data_view", D, "D");
pub const PALETTE: Shortcut = command_shortcut!("palette", K, "K");
pub const SETTINGS: Shortcut = command_shortcut!("settings", Comma, ",");
/// In the output, and on the map while exploring it.
pub const FIND: Shortcut = command_shortcut!("find", F, "F");
/// Shows or hides the map.
pub const MAP: Shortcut = command_shortcut!("map", M, "M");
pub const COPY: Shortcut = command_shortcut!("copy", C, "C");
pub const SELECT_ALL: Shortcut = command_shortcut!("select_all", A, "A");
pub const UNDO: Shortcut = command_shortcut!("undo", Z, "Z");
pub const REDO: Shortcut = command_shortcut!("redo", Y, "Y");
/// While exploring the map: all about the room, what is near, marking it
/// and writing a note on it; and searching, as [`FIND`].
pub const ROOM_INFO: Shortcut = command_shortcut!("room_info", I, "I");
pub const NEAR: Shortcut = command_shortcut!("near", L, "L");
pub const MARK: Shortcut = command_shortcut!("mark", B, "B");
pub const NOTE: Shortcut = command_shortcut!("note", N, "N");

/// [`WITH_KEY`] and a key.
macro_rules! with_key_shortcut {
    ($id:literal, $key:ident, $written:literal) => {
        Shortcut {
            id: $id,
            default_keys: KeyboardShortcut::new(WITH_KEY, eframe::egui::Key::$key),
            default_written: with_key!($written),
        }
    };
}

/// Says where the player is, by the map.
pub const WHERE: Shortcut = with_key_shortcut!("where", M, "M");
/// Explores the map with the keys.
pub const EXPLORE: Shortcut = with_key_shortcut!("explore", E, "E");
/// Reads the gauges aloud.
pub const GAUGES: Shortcut = with_key_shortcut!("gauges", Num0, "0");

/// Reconnecting: Alt+R; on a Mac Cmd+R, as reloading is in every app
/// there, Ctrl+R being the search of what was typed, as in a terminal.
pub const RECONNECT: Shortcut = Shortcut {
    id: "reconnect",
    default_keys: KeyboardShortcut::new(
        if MAC {
            Modifiers::COMMAND
        } else {
            Modifiers::ALT
        },
        eframe::egui::Key::R,
    ),
    default_written: if MAC { "Cmd+R" } else { "Alt+R" },
};

/// Searching the commands typed before, as in a terminal: Ctrl+R, on a
/// Mac too.
pub const HISTORY_SEARCH: Shortcut = Shortcut {
    id: "history_search",
    default_keys: KeyboardShortcut::new(Modifiers::CTRL, eframe::egui::Key::R),
    default_written: "Ctrl+R",
};

/// The shortcuts that can be pressed with other keys, and what each does.
/// Copying, selecting, undoing and redoing are the system's, and stay.
pub fn bindable() -> [(Shortcut, &'static str); 17] {
    [
        (PALETTE, tr("Palette")),
        (SETTINGS, tr("Settings")),
        (NEW_SESSION, tr("New session")),
        (CLOSE_SESSION, tr("Close session")),
        (RECONNECT, tr("Reconnect")),
        (SWITCH_VIEW, tr("Between the game and the editor")),
        (DATA_VIEW, tr("Between the game and the incoming data")),
        (FIND, tr("Find")),
        (HISTORY_SEARCH, tr("Search the commands typed before")),
        (GAUGES, tr("Read the gauges")),
        (WHERE, tr("Where am I")),
        (EXPLORE, tr("Explore the map")),
        (MAP, tr("Show or hide the map")),
        (ROOM_INFO, tr("All about the room, exploring the map")),
        (NEAR, tr("What is near, exploring the map")),
        (MARK, tr("Mark the room, exploring the map")),
        (NOTE, tr("Note on the room, exploring the map")),
    ]
}

thread_local! {
    /// The shortcuts pressed with other keys than their own, by their
    /// name, and how those are written. Of the window's thread: tests,
    /// each in a thread of its own, do not change each other's.
    static BOUND: RefCell<BTreeMap<&'static str, (KeyboardShortcut, &'static str)>> =
        RefCell::default();
}

fn bound(id: &str) -> Option<(KeyboardShortcut, &'static str)> {
    BOUND.with(|bound| bound.borrow().get(id).copied())
}

/// Takes the keys chosen in the settings, by each shortcut's name, as
/// [`stored`] writes them. What cannot be read, or is a shortcut's own
/// keys, leaves it as it is.
pub fn configure(chosen: &BTreeMap<String, String>) {
    let mut now = BTreeMap::new();
    for (shortcut, _) in bindable() {
        let keys = chosen.get(shortcut.id).and_then(|text| parse(text));
        if let Some(keys) = keys.filter(|keys| !is_own(&shortcut, keys)) {
            now.insert(shortcut.id, (keys, written_once(&keys)));
        }
    }
    BOUND.with(|bound| *bound.borrow_mut() = now);
}

/// How `keys` are written, kept for as long as Muddie runs: the same
/// text is made once however often the settings change.
fn written_once(keys: &KeyboardShortcut) -> &'static str {
    thread_local! {
        static MADE: RefCell<BTreeMap<String, &'static str>> = RefCell::default();
    }
    let text = write(keys);
    MADE.with(|made| {
        *made
            .borrow_mut()
            .entry(text.clone())
            .or_insert_with(|| Box::leak(text.into_boxed_str()))
    })
}

/// How `keys` are kept in the settings file: `ctrl+alt+shift+cmd+K`, those
/// held in that order, and the key's name.
pub fn stored(keys: &KeyboardShortcut) -> String {
    let held = keys.modifiers;
    let mut text = String::new();
    for (down, name) in [
        (held.ctrl || (held.command && !MAC), "ctrl+"),
        (held.alt, "alt+"),
        (held.shift, "shift+"),
        (held.mac_cmd || (held.command && MAC), "cmd+"),
    ] {
        if down {
            text.push_str(name);
        }
    }
    text + keys.logical_key.name()
}

/// Reads keys as [`stored`] writes them.
pub fn parse(text: &str) -> Option<KeyboardShortcut> {
    let mut parts: Vec<&str> = text.split('+').collect();
    // A + alone is the key named so.
    let name = match parts.pop()? {
        "" if parts.last() == Some(&"") => {
            parts.pop();
            "+"
        }
        name => name,
    };
    let key = Key::from_name(name)?;
    let mut held = Modifiers::NONE;
    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" => held.ctrl = true,
            "alt" => held.alt = true,
            "shift" => held.shift = true,
            "cmd" => held.mac_cmd = true,
            _ => return None,
        }
    }
    // Command is what the system takes it to be: Cmd on a Mac, Ctrl
    // elsewhere; held so, egui matches it whichever of the two the
    // window reports.
    if MAC {
        held.command = held.mac_cmd;
    } else if held.ctrl {
        held.ctrl = false;
        held.command = true;
    }
    Some(KeyboardShortcut::new(held, key))
}

/// How keys are written to be read: `Ctrl+Alt+K`, with Option and Cmd on
/// a Mac.
pub fn write(keys: &KeyboardShortcut) -> String {
    let held = keys.modifiers;
    let mut text = String::new();
    let alt = if MAC { "Option+" } else { "Alt+" };
    for (down, name) in [
        (held.ctrl || (held.command && !MAC), "Ctrl+"),
        (held.alt, alt),
        (held.shift, "Shift+"),
        (held.mac_cmd || (held.command && MAC), "Cmd+"),
    ] {
        if down {
            text.push_str(name);
        }
    }
    text + keys.logical_key.symbol_or_name()
}

/// The keys of a macro, as it names them (`alt+shift+f5`), as keys
/// Muddie could have: none for the keypad's, which are not egui's.
pub fn of_macro(name: &str) -> Option<KeyboardShortcut> {
    let mut parts: Vec<&str> = name.trim().split('+').collect();
    let key = parts.pop()?;
    let key = *Key::ALL
        .iter()
        .find(|known| known.name().eq_ignore_ascii_case(key))?;
    let mut held = Modifiers::NONE;
    for part in parts {
        match part {
            "ctrl" => held.ctrl = true,
            "alt" => held.alt = true,
            "shift" => held.shift = true,
            _ => return None,
        }
    }
    Some(KeyboardShortcut::new(held, key))
}

/// Whether `keys` are `shortcut`'s own, those it has when none are chosen.
pub fn is_own(shortcut: &Shortcut, keys: &KeyboardShortcut) -> bool {
    let own = shortcut.default_keys;
    own.logical_key == keys.logical_key && same_held(own.modifiers, keys.modifiers)
}

/// Whether keys pressed can be a shortcut: held with Ctrl, Alt or Cmd, or
/// a function key, so that they do not type.
pub fn can_be_shortcut(keys: &KeyboardShortcut) -> bool {
    let held = keys.modifiers;
    held.ctrl || held.alt || held.mac_cmd || held.command || is_function_key(keys.logical_key)
}

fn is_function_key(key: Key) -> bool {
    let name = key.name();
    name.len() > 1 && name.starts_with('F') && name[1..].parse::<u8>().is_ok()
}

/// What Muddie does with keys, of its own: a shortcut but `except`, or the
/// keys that read the output and the panels, which are not chosen. None
/// if they are free.
pub fn taken_by(keys: &KeyboardShortcut, except: Option<&str>) -> Option<&'static str> {
    let same = |other: &KeyboardShortcut| {
        other.logical_key == keys.logical_key && same_held(other.modifiers, keys.modifiers)
    };
    for (shortcut, does) in bindable() {
        if Some(shortcut.id) != except && same(&shortcut.keys()) {
            return Some(does);
        }
    }
    for shortcut in [COPY, SELECT_ALL, UNDO, REDO] {
        if same(&shortcut.keys()) {
            return Some(match shortcut.id {
                "copy" => tr("Copy"),
                "select_all" => tr("Select all"),
                "undo" => tr("Undo"),
                _ => tr("Redo"),
            });
        }
    }
    reading_keys(keys)
}

/// The keys that read the output and the panels, as they are: what each
/// does, if `keys` are one.
fn reading_keys(keys: &KeyboardShortcut) -> Option<&'static str> {
    let held = keys.modifiers;
    let key = keys.logical_key;
    let shift = held.shift;
    let plain = Modifiers {
        shift: false,
        ..held
    };
    let arrows = same_held(plain, WITH_ARROWS);
    let figures = same_held(plain, WITH_KEY);
    let digit = matches!(
        key,
        Key::Num1
            | Key::Num2
            | Key::Num3
            | Key::Num4
            | Key::Num5
            | Key::Num6
            | Key::Num7
            | Key::Num8
            | Key::Num9
    );
    match () {
        _ if arrows && matches!(key, Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End) => {
            Some(match shift {
                false => tr("Reads the output line by line"),
                true => tr("Reads a panel of text line by line"),
            })
        }
        _ if arrows && shift && matches!(key, Key::ArrowLeft | Key::ArrowRight) => {
            Some(tr("Chooses the panel of text to read"))
        }
        _ if figures && digit => Some(match shift {
            false => tr("Reads the most recent lines"),
            true => tr("Reads the most recent lines of a panel"),
        }),
        _ if figures && key == Key::C => Some(tr("Copies the line read last")),
        _ if figures && !shift && key == Key::O => {
            Some(tr("Does what the link of the line read last does"))
        }
        _ => None,
    }
}

/// The same keys held, Command being Ctrl or Cmd as the system has it.
fn same_held(one: Modifiers, other: Modifiers) -> bool {
    let ctrl = |held: Modifiers| held.ctrl || (held.command && !MAC);
    let cmd = |held: Modifiers| held.mac_cmd || (held.command && MAC);
    ctrl(one) == ctrl(other)
        && cmd(one) == cmd(other)
        && one.alt == other.alt
        && one.shift == other.shift
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_chosen_are_kept_read_and_told_apart_from_muddies_own() {
        let keys = KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::P);
        assert_eq!(stored(&keys), "ctrl+shift+P");
        assert_eq!(parse("ctrl+shift+P"), parse(&stored(&keys)));
        assert!(same_held(
            parse("ctrl+shift+P").unwrap().modifiers,
            keys.modifiers
        ));
        assert_eq!(parse("ctrl+F5").map(|keys| keys.logical_key), Some(Key::F5));
        assert!(parse("ctrl+nothing").is_none() && parse("hyper+K").is_none());
        // Chosen, the palette is opened with them and written so.
        configure(&BTreeMap::from([("palette".into(), "ctrl+shift+P".into())]));
        assert!(PALETTE.is_changed());
        assert_eq!(PALETTE.keys().logical_key, Key::P);
        assert_eq!(PALETTE.written(), "Ctrl+Shift+P");
        assert_eq!(taken_by(&keys, None), Some(tr("Palette")));
        assert_eq!(taken_by(&keys, Some("palette")), None);
        // What reads the output is not chosen, but it is taken.
        let review = KeyboardShortcut::new(WITH_KEY, Key::Num1);
        assert_eq!(
            taken_by(&review, None),
            Some(tr("Reads the most recent lines"))
        );
        configure(&BTreeMap::new());
        assert!(!PALETTE.is_changed());
        assert_eq!(PALETTE.written(), command!("K"));
    }
}
