//! The keys Muddie's own shortcuts are held with, which are not the same on
//! a Mac: there Option types characters (@ on a Spanish keyboard is
//! Option+2), so that it cannot be held for a letter or a figure, and Cmd
//! does what Ctrl does elsewhere. How each is written comes from here too.

use eframe::egui::Modifiers;

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
/// are written, everywhere it is shown.
#[derive(Debug, Clone, Copy)]
pub struct Shortcut {
    pub keys: eframe::egui::KeyboardShortcut,
    pub written: &'static str,
}

impl Shortcut {
    /// Whether it was pressed, taking the press.
    pub fn pressed(&self, input: &mut eframe::egui::InputState) -> bool {
        input.consume_shortcut(&self.keys)
    }
}

/// Command and a key: Cmd on a Mac, Ctrl elsewhere.
macro_rules! command_shortcut {
    ($key:ident, $written:literal) => {
        Shortcut {
            keys: eframe::egui::KeyboardShortcut::new(Modifiers::COMMAND, eframe::egui::Key::$key),
            written: command!($written),
        }
    };
}

pub const NEW_SESSION: Shortcut = command_shortcut!(T, "T");
pub const CLOSE_SESSION: Shortcut = command_shortcut!(W, "W");
/// Between the game and the editor.
pub const SWITCH_VIEW: Shortcut = command_shortcut!(E, "E");
/// The incoming data.
pub const DATA_VIEW: Shortcut = command_shortcut!(D, "D");
pub const PALETTE: Shortcut = command_shortcut!(K, "K");
pub const SETTINGS: Shortcut = command_shortcut!(Comma, ",");
/// In the output, and on the map while exploring it.
pub const FIND: Shortcut = command_shortcut!(F, "F");
/// Shows or hides the map.
pub const MAP: Shortcut = command_shortcut!(M, "M");
pub const COPY: Shortcut = command_shortcut!(C, "C");
pub const SELECT_ALL: Shortcut = command_shortcut!(A, "A");
pub const UNDO: Shortcut = command_shortcut!(Z, "Z");
pub const REDO: Shortcut = command_shortcut!(Y, "Y");
/// While exploring the map: all about the room, what is near, marking it
/// and writing a note on it; and searching, as [`FIND`].
pub const ROOM_INFO: Shortcut = command_shortcut!(I, "I");
pub const NEAR: Shortcut = command_shortcut!(L, "L");
pub const MARK: Shortcut = command_shortcut!(B, "B");
pub const NOTE: Shortcut = command_shortcut!(N, "N");

/// [`WITH_KEY`] and a key.
macro_rules! with_key_shortcut {
    ($key:ident, $written:literal) => {
        Shortcut {
            keys: eframe::egui::KeyboardShortcut::new(WITH_KEY, eframe::egui::Key::$key),
            written: with_key!($written),
        }
    };
}

/// Says where the player is, by the map.
pub const WHERE: Shortcut = with_key_shortcut!(M, "M");
/// Explores the map with the keys.
pub const EXPLORE: Shortcut = with_key_shortcut!(E, "E");
/// Reads the gauges aloud.
pub const GAUGES: Shortcut = with_key_shortcut!(Num0, "0");

/// Reconnecting: Alt+R; on a Mac Cmd+R, as reloading is in every app
/// there, Ctrl+R being the search of what was typed, as in a terminal.
pub const RECONNECT: Shortcut = Shortcut {
    keys: eframe::egui::KeyboardShortcut::new(
        if MAC {
            Modifiers::COMMAND
        } else {
            Modifiers::ALT
        },
        eframe::egui::Key::R,
    ),
    written: if MAC { "Cmd+R" } else { "Alt+R" },
};

/// Searching the commands typed before, as in a terminal: Ctrl+R, on a
/// Mac too.
pub const HISTORY_SEARCH: Shortcut = Shortcut {
    keys: eframe::egui::KeyboardShortcut::new(Modifiers::CTRL, eframe::egui::Key::R),
    written: "Ctrl+R",
};
