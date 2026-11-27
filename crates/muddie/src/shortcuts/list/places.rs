//! The shortcuts of the places that have the keyboard to themselves: the
//! map explored, the editor's list and Layout's.

use eframe::egui::Key;

use super::super::press::{Held, keypad, press};
use super::{COMMAND, NONE, Place, Shortcut, WITH_ARROWS};
use crate::i18n::tr;

shortcut!(
    MAP_NORTH,
    "map_north",
    Map,
    [
        press(NONE, Key::ArrowUp),
        press(NONE, Key::Num8),
        keypad("numpad8"),
        keypad("numpadup")
    ],
    || tr("Follow the exit north")
);
shortcut!(
    MAP_SOUTH,
    "map_south",
    Map,
    [
        press(NONE, Key::ArrowDown),
        press(NONE, Key::Num2),
        keypad("numpad2"),
        keypad("numpaddown")
    ],
    || tr("Follow the exit south")
);
shortcut!(
    MAP_WEST,
    "map_west",
    Map,
    [
        press(NONE, Key::ArrowLeft),
        press(NONE, Key::Num4),
        keypad("numpad4"),
        keypad("numpadleft")
    ],
    || tr("Follow the exit west")
);
shortcut!(
    MAP_EAST,
    "map_east",
    Map,
    [
        press(NONE, Key::ArrowRight),
        press(NONE, Key::Num6),
        keypad("numpad6"),
        keypad("numpadright")
    ],
    || tr("Follow the exit east")
);
shortcut!(
    MAP_NORTHWEST,
    "map_northwest",
    Map,
    [
        press(NONE, Key::Num7),
        keypad("numpad7"),
        keypad("numpadhome")
    ],
    || tr("Follow the exit north-west")
);
shortcut!(
    MAP_NORTHEAST,
    "map_northeast",
    Map,
    [
        press(NONE, Key::Num9),
        keypad("numpad9"),
        keypad("numpadpageup")
    ],
    || tr("Follow the exit north-east")
);
shortcut!(
    MAP_SOUTHWEST,
    "map_southwest",
    Map,
    [
        press(NONE, Key::Num1),
        keypad("numpad1"),
        keypad("numpadend")
    ],
    || tr("Follow the exit south-west")
);
shortcut!(
    MAP_SOUTHEAST,
    "map_southeast",
    Map,
    [
        press(NONE, Key::Num3),
        keypad("numpad3"),
        keypad("numpadpagedown")
    ],
    || tr("Follow the exit south-east")
);
shortcut!(
    MAP_UP,
    "map_up",
    Map,
    [press(NONE, Key::PageUp), keypad("numpadsubtract")],
    || tr("Follow the exit up")
);
shortcut!(
    MAP_DOWN,
    "map_down",
    Map,
    [press(NONE, Key::PageDown), keypad("numpadadd")],
    || tr("Follow the exit down")
);
shortcut!(MAP_IN, "map_in", Map, [keypad("numpaddivide")], || tr(
    "Follow the exit in"
));
shortcut!(MAP_OUT, "map_out", Map, [keypad("numpadmultiply")], || tr(
    "Follow the exit out"
));
shortcut!(
    MAP_AGAIN,
    "map_again",
    Map,
    [
        press(NONE, Key::Space),
        keypad("numpad5"),
        keypad("numpadclear")
    ],
    || tr("Say the room again")
);
shortcut!(MAP_HOME, "map_home", Map, [press(NONE, Key::Home)], || tr(
    "Back to your room"
));
shortcut!(
    MAP_BACK,
    "map_back",
    Map,
    [press(NONE, Key::Backspace)],
    || { tr("Back to the room before") }
);
shortcut!(
    MAP_GO,
    "map_go",
    Map,
    [press(NONE, Key::Enter), keypad("numpadenter")],
    || tr("Go to the room chosen")
);
shortcut!(
    MAP_WAY,
    "map_way",
    Map,
    [press(Held::SHIFT, Key::Enter)],
    || { tr("Say the way to the room chosen") }
);
shortcut!(MAP_HELP, "map_help", Map, [press(NONE, Key::F1)], || tr(
    "Say all the map's keys"
));
shortcut!(MAP_INFO, "map_info", Map, [press(COMMAND, Key::I)], || tr(
    "All about the room"
));
shortcut!(MAP_NEAR, "map_near", Map, [press(COMMAND, Key::L)], || tr(
    "What is near"
));
shortcut!(MAP_FIND, "map_find", Map, [press(COMMAND, Key::F)], || tr(
    "Find on the map"
));
shortcut!(MAP_MARK, "map_mark", Map, [press(COMMAND, Key::B)], || tr(
    "Mark the room"
));
shortcut!(MAP_NOTE, "map_note", Map, [press(COMMAND, Key::N)], || {
    tr("Write a note on the room")
});

shortcut!(
    MOVE_UP,
    "move_up",
    Editor,
    [press(WITH_ARROWS, Key::ArrowUp)],
    || { tr("Move the rule up") }
);
shortcut!(
    MOVE_DOWN,
    "move_down",
    Editor,
    [press(WITH_ARROWS, Key::ArrowDown)],
    || { tr("Move the rule down") }
);
shortcut!(
    MOVE_OUT,
    "move_out",
    Editor,
    [press(WITH_ARROWS, Key::ArrowLeft)],
    || { tr("Move the rule out of its folder") }
);
shortcut!(RENAME, "rename", Editor, [press(NONE, Key::F2)], || tr(
    "Rename"
));
shortcut!(DELETE, "delete", Editor, [press(NONE, Key::Delete)], || tr(
    "Delete"
));
shortcut!(
    SELECT_ALL,
    "select_all",
    Editor,
    [press(COMMAND, Key::A)],
    || { tr("Select all visible items") }
);
shortcut!(UNDO, "undo", Editor, [press(COMMAND, Key::Z)], || tr(
    "Undo"
));
shortcut!(
    REDO,
    "redo",
    Editor,
    [press(COMMAND, Key::Y), press(COMMAND.and_shift(), Key::Z)],
    || tr("Redo")
);

shortcut!(
    PANEL_UP,
    "panel_up",
    Layout,
    [press(WITH_ARROWS, Key::ArrowUp)],
    || { tr("Move the panel up") }
);
shortcut!(
    PANEL_DOWN,
    "panel_down",
    Layout,
    [press(WITH_ARROWS, Key::ArrowDown)],
    || { tr("Move the panel down") }
);
shortcut!(
    PANEL_SIDE_BACK,
    "panel_side_back",
    Layout,
    [press(WITH_ARROWS, Key::ArrowLeft)],
    || tr("Move the panel to the side before")
);
shortcut!(
    PANEL_SIDE_ON,
    "panel_side_on",
    Layout,
    [press(WITH_ARROWS, Key::ArrowRight)],
    || tr("Move the panel to the next side")
);
