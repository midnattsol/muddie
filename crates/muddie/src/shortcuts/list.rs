//! Every shortcut of Muddie: its name, where it works, the keys it has
//! until other keys are chosen, and what it does.

use super::MAC;
use super::press::{Held, Press, Typing};
use crate::i18n::tr;

/// Where a shortcut works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Anywhere in the window.
    Everywhere,
    /// Anywhere, to read the output and the panels.
    Reading,
    /// In the command line.
    Line,
    /// On the map, while it is explored with the keys.
    Map,
    /// In the editor's list of rules.
    Editor,
    /// In the list of panels of Layout.
    Layout,
}

impl Place {
    pub const ALL: [Place; 6] = [
        Place::Everywhere,
        Place::Reading,
        Place::Line,
        Place::Map,
        Place::Editor,
        Place::Layout,
    ];

    /// Its name, for scripts.
    pub fn id(self) -> &'static str {
        match self {
            Place::Everywhere => "everywhere",
            Place::Reading => "reading",
            Place::Line => "line",
            Place::Map => "map",
            Place::Editor => "editor",
            Place::Layout => "layout",
        }
    }

    /// Its title, in the settings and the window of shortcuts.
    pub fn title(self) -> &'static str {
        match self {
            Place::Everywhere => tr("Everywhere"),
            Place::Reading => tr("Reading"),
            Place::Line => tr("Command line"),
            Place::Map => tr("Exploring the map"),
            Place::Editor => tr("Editor"),
            Place::Layout => tr("Layout"),
        }
    }

    pub(super) fn typing(self) -> Typing {
        match self {
            Place::Everywhere | Place::Reading => Typing::Anywhere,
            Place::Line => Typing::Line,
            Place::Map => Typing::Own,
            Place::Editor | Place::Layout => Typing::List,
        }
    }

    /// Whether keys of the two can be pressed at once and do both: those
    /// that work anywhere meet everything but the places that have the
    /// keyboard to themselves, which go first there.
    pub fn meets(self, other: Place) -> bool {
        let anywhere = |place| matches!(place, Place::Everywhere | Place::Reading);
        self == other || (anywhere(self) && anywhere(other))
    }
}

/// One of Muddie's shortcuts.
#[derive(Debug, Clone, Copy)]
pub struct Shortcut {
    /// Its name, in the settings file and for scripts.
    pub id: &'static str,
    pub place: Place,
    pub(super) defaults: &'static [Press],
    about: fn() -> &'static str,
}

impl Shortcut {
    /// What it does, in the language of the interface.
    pub fn about(&self) -> &'static str {
        (self.about)()
    }

    /// The keys it has until others are chosen.
    pub fn defaults(&self) -> &'static [Press] {
        self.defaults
    }

    /// Whether it can be pressed with keys held alone, as Ctrl silences.
    pub fn can_be_alone(&self) -> bool {
        self.id == HUSH.id
    }
}

/// Command and a key: Cmd on a Mac, Ctrl elsewhere.
const COMMAND: Held = if MAC { Held::CMD } else { Held::CTRL };
/// Held with a letter or a figure: Alt; on a Mac, where Option types,
/// Ctrl.
const WITH_KEY: Held = if MAC { Held::CTRL } else { Held::ALT };
/// Held with the arrows: Alt, Option on a Mac.
const WITH_ARROWS: Held = Held::ALT;
const NONE: Held = Held::NONE;

macro_rules! shortcut {
    ($name:ident, $id:literal, $place:ident, [$($keys:expr),* $(,)?], $about:expr) => {
        pub const $name: Shortcut = Shortcut {
            id: $id,
            place: Place::$place,
            defaults: &[$($keys),*],
            about: $about,
        };
    };
}

mod game;
mod places;

pub use game::*;
pub use places::*;

/// The keys that read the most recent lines, the last first.
pub const READ_RECENT: [Shortcut; 9] = [
    READ_RECENT_1,
    READ_RECENT_2,
    READ_RECENT_3,
    READ_RECENT_4,
    READ_RECENT_5,
    READ_RECENT_6,
    READ_RECENT_7,
    READ_RECENT_8,
    READ_RECENT_9,
];

/// The same, of a panel of text.
pub const PANEL_RECENT: [Shortcut; 9] = [
    PANEL_RECENT_1,
    PANEL_RECENT_2,
    PANEL_RECENT_3,
    PANEL_RECENT_4,
    PANEL_RECENT_5,
    PANEL_RECENT_6,
    PANEL_RECENT_7,
    PANEL_RECENT_8,
    PANEL_RECENT_9,
];

/// The keys that move the room chosen on the map, and the way each goes.
pub const MAP_MOVES: [(Shortcut, &str); 12] = [
    (MAP_NORTH, "n"),
    (MAP_SOUTH, "s"),
    (MAP_WEST, "w"),
    (MAP_EAST, "e"),
    (MAP_NORTHWEST, "nw"),
    (MAP_NORTHEAST, "ne"),
    (MAP_SOUTHWEST, "sw"),
    (MAP_SOUTHEAST, "se"),
    (MAP_UP, "u"),
    (MAP_DOWN, "d"),
    (MAP_IN, "in"),
    (MAP_OUT, "out"),
];

/// Every shortcut, by place, in the order they are shown.
pub const ALL: [Shortcut; 86] = [
    PALETTE,
    SETTINGS,
    NEW_SESSION,
    CLOSE_SESSION,
    RECONNECT,
    SWITCH_VIEW,
    DATA_VIEW,
    FIND,
    MESSAGES,
    GAUGES,
    WHERE,
    EXPLORE,
    MAP,
    KEYS_HELP,
    NEXT_ZONE,
    PREVIOUS_ZONE,
    HUSH,
    READ_PREVIOUS,
    READ_NEXT,
    READ_FIRST,
    READ_LAST,
    READ_RECENT_1,
    READ_RECENT_2,
    READ_RECENT_3,
    READ_RECENT_4,
    READ_RECENT_5,
    READ_RECENT_6,
    READ_RECENT_7,
    READ_RECENT_8,
    READ_RECENT_9,
    COPY_READ,
    FOLLOW_LINK,
    PANEL_PREVIOUS,
    PANEL_NEXT,
    PANEL_READ_PREVIOUS,
    PANEL_READ_NEXT,
    PANEL_READ_FIRST,
    PANEL_READ_LAST,
    PANEL_RECENT_1,
    PANEL_RECENT_2,
    PANEL_RECENT_3,
    PANEL_RECENT_4,
    PANEL_RECENT_5,
    PANEL_RECENT_6,
    PANEL_RECENT_7,
    PANEL_RECENT_8,
    PANEL_RECENT_9,
    PANEL_COPY_READ,
    HISTORY_PREVIOUS,
    HISTORY_NEXT,
    HISTORY_SEARCH,
    MAP_NORTH,
    MAP_SOUTH,
    MAP_WEST,
    MAP_EAST,
    MAP_NORTHWEST,
    MAP_NORTHEAST,
    MAP_SOUTHWEST,
    MAP_SOUTHEAST,
    MAP_UP,
    MAP_DOWN,
    MAP_IN,
    MAP_OUT,
    MAP_AGAIN,
    MAP_HOME,
    MAP_BACK,
    MAP_GO,
    MAP_WAY,
    MAP_HELP,
    MAP_INFO,
    MAP_NEAR,
    MAP_FIND,
    MAP_MARK,
    MAP_NOTE,
    MOVE_UP,
    MOVE_DOWN,
    MOVE_OUT,
    RENAME,
    DELETE,
    SELECT_ALL,
    UNDO,
    REDO,
    PANEL_UP,
    PANEL_DOWN,
    PANEL_SIDE_BACK,
    PANEL_SIDE_ON,
];

/// The shortcut named so.
pub fn named(id: &str) -> Option<Shortcut> {
    ALL.iter().find(|shortcut| shortcut.id == id).copied()
}
