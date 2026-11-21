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

/// Reconnecting: Alt+R; on a Mac Cmd+R, as reloading is in every app
/// there, Ctrl+R being the search of what was typed, as in a terminal.
pub const RECONNECT: Modifiers = if MAC {
    Modifiers::COMMAND
} else {
    Modifiers::ALT
};

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

pub(crate) use {command, with_arrows, with_key};

/// How reconnecting is written.
pub const RECONNECT_KEYS: &str = if MAC { "Cmd+R" } else { "Alt+R" };
