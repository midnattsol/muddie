//! What is drawn for who looks, and is not there for who listens: not a
//! stop of the keyboard, nor told to a screen reader. The pointer presses
//! it as ever.

use super::*;

/// What marks a part of the window as not there for who listens.
const TAG: &str = "unreached";

/// A part of the window that is not there for who listens.
pub fn unreached<R>(
    ui: &mut Ui,
    builder: egui::UiBuilder,
    add: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let tagged = builder.ui_stack_info(egui::UiStackInfo::default().with_tag(TAG));
    ui.scope_builder(tagged, |ui| {
        // All in it goes with it.
        let id = ui.unique_id();
        (ui.ctx()).accesskit_node_builder(id, |node| node.set_hidden());
        add(ui)
    })
}

/// Whether what is drawn here is there for who listens.
pub fn is_reached(ui: &Ui) -> bool {
    !(ui.stack().iter()).any(|part| part.tags().contains(TAG))
}

/// How something to press is pressed here: with the pointer and the
/// keyboard, or with the pointer alone.
pub fn press_sense(ui: &Ui) -> Sense {
    match is_reached(ui) {
        true => Sense::click(),
        false => Sense::CLICK,
    }
}
