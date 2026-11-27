//! The keys of the editor's list, taken from the frame's input.

use super::*;

/// The keys of the list, taken from the frame's input.
pub(super) struct ListKeys {
    /// Shift and the arrows up and down.
    pub(super) wider_up: bool,
    pub(super) wider_down: bool,
    pub(super) up: bool,
    pub(super) down: bool,
    pub(super) home: bool,
    pub(super) end: bool,
    pub(super) left: bool,
    pub(super) right: bool,
    pub(super) delete: bool,
    pub(super) rename: bool,
    /// Ctrl and A.
    pub(super) all: bool,
}

impl ListKeys {
    /// Takes the keys of the list out of the input, those with Shift
    /// before the plain ones that would take them too.
    pub(super) fn take(ui: &mut egui::Ui) -> Self {
        // Those with Shift first: without it, the plain ones would take
        // them too.
        let (wider_up, wider_down) = ui.input_mut(|input| {
            (
                input.consume_key(Modifiers::SHIFT, Key::ArrowUp),
                input.consume_key(Modifiers::SHIFT, Key::ArrowDown),
            )
        });
        let pressed = |key: Key| ui.input_mut(|input| input.consume_key(Modifiers::NONE, key));
        let (up, down, home, end) = (
            pressed(Key::ArrowUp),
            pressed(Key::ArrowDown),
            pressed(Key::Home),
            pressed(Key::End),
        );
        let (left, right) = (pressed(Key::ArrowLeft), pressed(Key::ArrowRight));
        let shortcut =
            |shortcut: crate::shortcuts::Shortcut| ui.input_mut(|input| shortcut.pressed(input));
        let (delete, rename) = (
            shortcut(crate::shortcuts::DELETE),
            shortcut(crate::shortcuts::RENAME),
        );
        let all = shortcut(crate::shortcuts::SELECT_ALL);
        Self {
            wider_up,
            wider_down,
            up,
            down,
            home,
            end,
            left,
            right,
            delete,
            rename,
            all,
        }
    }
}
