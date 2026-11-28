//! The shortcuts that work anywhere: of the window, of reading and of the
//! command line.

use eframe::egui::Key;

use super::super::MAC;
use super::super::press::{Held, Press, Pressed, press};
use super::{COMMAND, NONE, Place, Shortcut, WITH_ARROWS, WITH_KEY};
use crate::i18n::tr;

shortcut!(
    PALETTE,
    "palette",
    Everywhere,
    [press(COMMAND, Key::K)],
    || tr("Palette")
);
shortcut!(
    SETTINGS,
    "settings",
    Everywhere,
    [press(COMMAND, Key::Comma)],
    || tr("Settings")
);
shortcut!(
    NEW_SESSION,
    "new_session",
    Everywhere,
    [press(COMMAND, Key::T)],
    || tr("New session")
);
shortcut!(
    CLOSE_SESSION,
    "close_session",
    Everywhere,
    [press(COMMAND, Key::W)],
    || { tr("Close session") }
);
shortcut!(
    NEXT_SESSION,
    "next_session",
    Everywhere,
    [press(Held::CTRL, Key::Tab)],
    || tr("Next session")
);
shortcut!(
    PREVIOUS_SESSION,
    "previous_session",
    Everywhere,
    [press(Held::CTRL.and_shift(), Key::Tab)],
    || tr("Previous session")
);
// Reloading is Cmd+R in every app on a Mac; Ctrl+R searches what was typed.
shortcut!(
    RECONNECT,
    "reconnect",
    Everywhere,
    [press(if MAC { Held::CMD } else { Held::ALT }, Key::R)],
    || tr("Reconnect")
);
shortcut!(
    SWITCH_VIEW,
    "switch_view",
    Everywhere,
    [press(COMMAND, Key::E)],
    || { tr("Between the game and the editor") }
);
shortcut!(
    DATA_VIEW,
    "data_view",
    Everywhere,
    [press(COMMAND, Key::D)],
    || { tr("Between the game and the incoming data") }
);
shortcut!(FIND, "find", Everywhere, [press(COMMAND, Key::F)], || tr(
    "Find in the output"
));
shortcut!(
    MESSAGES,
    "messages",
    Everywhere,
    [press(WITH_KEY, Key::N)],
    || tr("Messages")
);
shortcut!(
    GAUGES,
    "gauges",
    Everywhere,
    [press(WITH_KEY, Key::Num0)],
    || tr("Read the gauges")
);
shortcut!(
    WHERE,
    "where",
    Everywhere,
    [press(WITH_KEY, Key::M)],
    || tr("Where am I")
);
shortcut!(
    EXPLORE,
    "explore",
    Everywhere,
    [press(WITH_KEY, Key::E)],
    || tr("Explore the map")
);
shortcut!(MAP, "map", Everywhere, [press(COMMAND, Key::M)], || tr(
    "Show or hide the map"
));
shortcut!(
    KEYS_HELP,
    "keys_help",
    Everywhere,
    [press(NONE, Key::F1)],
    || { tr("Say the keys to know") }
);
shortcut!(
    NEXT_ZONE,
    "next_zone",
    Everywhere,
    [press(NONE, Key::F6)],
    || { tr("Go to the output and each panel") }
);
shortcut!(
    PREVIOUS_ZONE,
    "previous_zone",
    Everywhere,
    [press(Held::SHIFT, Key::F6)],
    || { tr("Go back through the panels") }
);
shortcut!(
    HUSH,
    "hush",
    Everywhere,
    [Press {
        held: Held::CTRL,
        key: Pressed::Alone
    }],
    || tr("Silence speech")
);

shortcut!(
    READ_PREVIOUS,
    "read_previous",
    Reading,
    [press(WITH_ARROWS, Key::ArrowUp)],
    || { tr("Read the previous line") }
);
shortcut!(
    READ_NEXT,
    "read_next",
    Reading,
    [press(WITH_ARROWS, Key::ArrowDown)],
    || { tr("Read the next line") }
);
shortcut!(
    READ_FIRST,
    "read_first",
    Reading,
    [press(WITH_ARROWS, Key::Home)],
    || { tr("Read the first line") }
);
shortcut!(
    READ_LAST,
    "read_last",
    Reading,
    [press(WITH_ARROWS, Key::End)],
    || { tr("Read the last line") }
);
shortcut!(
    READ_RECENT_1,
    "read_recent_1",
    Reading,
    [press(WITH_KEY, Key::Num1)],
    || { tr("Read the most recent line") }
);
shortcut!(
    READ_RECENT_2,
    "read_recent_2",
    Reading,
    [press(WITH_KEY, Key::Num2)],
    || { tr("Read the 2nd most recent line") }
);
shortcut!(
    READ_RECENT_3,
    "read_recent_3",
    Reading,
    [press(WITH_KEY, Key::Num3)],
    || { tr("Read the 3rd most recent line") }
);
shortcut!(
    READ_RECENT_4,
    "read_recent_4",
    Reading,
    [press(WITH_KEY, Key::Num4)],
    || { tr("Read the 4th most recent line") }
);
shortcut!(
    READ_RECENT_5,
    "read_recent_5",
    Reading,
    [press(WITH_KEY, Key::Num5)],
    || { tr("Read the 5th most recent line") }
);
shortcut!(
    READ_RECENT_6,
    "read_recent_6",
    Reading,
    [press(WITH_KEY, Key::Num6)],
    || { tr("Read the 6th most recent line") }
);
shortcut!(
    READ_RECENT_7,
    "read_recent_7",
    Reading,
    [press(WITH_KEY, Key::Num7)],
    || { tr("Read the 7th most recent line") }
);
shortcut!(
    READ_RECENT_8,
    "read_recent_8",
    Reading,
    [press(WITH_KEY, Key::Num8)],
    || { tr("Read the 8th most recent line") }
);
shortcut!(
    READ_RECENT_9,
    "read_recent_9",
    Reading,
    [press(WITH_KEY, Key::Num9)],
    || { tr("Read the 9th most recent line") }
);
shortcut!(
    COPY_READ,
    "copy_read",
    Reading,
    [press(WITH_KEY, Key::C)],
    || { tr("Copy the line read last") }
);
shortcut!(
    FOLLOW_LINK,
    "follow_link",
    Reading,
    [press(WITH_KEY, Key::O)],
    || { tr("Follow the link of the line read last") }
);
shortcut!(
    PANEL_PREVIOUS,
    "panel_previous",
    Reading,
    [press(WITH_ARROWS.and_shift(), Key::ArrowLeft)],
    || tr("Choose the previous panel of text to read")
);
shortcut!(
    PANEL_NEXT,
    "panel_next",
    Reading,
    [press(WITH_ARROWS.and_shift(), Key::ArrowRight)],
    || tr("Choose the next panel of text to read")
);
shortcut!(
    PANEL_READ_PREVIOUS,
    "panel_read_previous",
    Reading,
    [press(WITH_ARROWS.and_shift(), Key::ArrowUp)],
    || tr("Read the previous line of the panel")
);
shortcut!(
    PANEL_READ_NEXT,
    "panel_read_next",
    Reading,
    [press(WITH_ARROWS.and_shift(), Key::ArrowDown)],
    || tr("Read the next line of the panel")
);
shortcut!(
    PANEL_READ_FIRST,
    "panel_read_first",
    Reading,
    [press(WITH_ARROWS.and_shift(), Key::Home)],
    || tr("Read the first line of the panel")
);
shortcut!(
    PANEL_READ_LAST,
    "panel_read_last",
    Reading,
    [press(WITH_ARROWS.and_shift(), Key::End)],
    || tr("Read the last line of the panel")
);
shortcut!(
    PANEL_RECENT_1,
    "panel_recent_1",
    Reading,
    [press(WITH_KEY.and_shift(), Key::Num1)],
    || tr("Read the most recent line of the panel")
);
shortcut!(
    PANEL_RECENT_2,
    "panel_recent_2",
    Reading,
    [press(WITH_KEY.and_shift(), Key::Num2)],
    || tr("Read the 2nd most recent line of the panel")
);
shortcut!(
    PANEL_RECENT_3,
    "panel_recent_3",
    Reading,
    [press(WITH_KEY.and_shift(), Key::Num3)],
    || tr("Read the 3rd most recent line of the panel")
);
shortcut!(
    PANEL_RECENT_4,
    "panel_recent_4",
    Reading,
    [press(WITH_KEY.and_shift(), Key::Num4)],
    || tr("Read the 4th most recent line of the panel")
);
shortcut!(
    PANEL_RECENT_5,
    "panel_recent_5",
    Reading,
    [press(WITH_KEY.and_shift(), Key::Num5)],
    || tr("Read the 5th most recent line of the panel")
);
shortcut!(
    PANEL_RECENT_6,
    "panel_recent_6",
    Reading,
    [press(WITH_KEY.and_shift(), Key::Num6)],
    || tr("Read the 6th most recent line of the panel")
);
shortcut!(
    PANEL_RECENT_7,
    "panel_recent_7",
    Reading,
    [press(WITH_KEY.and_shift(), Key::Num7)],
    || tr("Read the 7th most recent line of the panel")
);
shortcut!(
    PANEL_RECENT_8,
    "panel_recent_8",
    Reading,
    [press(WITH_KEY.and_shift(), Key::Num8)],
    || tr("Read the 8th most recent line of the panel")
);
shortcut!(
    PANEL_RECENT_9,
    "panel_recent_9",
    Reading,
    [press(WITH_KEY.and_shift(), Key::Num9)],
    || tr("Read the 9th most recent line of the panel")
);
shortcut!(
    PANEL_COPY_READ,
    "panel_copy_read",
    Reading,
    [press(WITH_KEY.and_shift(), Key::C)],
    || tr("Copy the line of the panel read last")
);

shortcut!(
    HISTORY_PREVIOUS,
    "history_previous",
    Line,
    [press(NONE, Key::ArrowUp)],
    || { tr("Earlier command") }
);
shortcut!(
    HISTORY_NEXT,
    "history_next",
    Line,
    [press(NONE, Key::ArrowDown)],
    || { tr("Later command") }
);
// Ctrl+R, as in a terminal: on a Mac too.
shortcut!(
    HISTORY_SEARCH,
    "history_search",
    Line,
    [press(Held::CTRL, Key::R)],
    || { tr("Search the commands typed before") }
);
