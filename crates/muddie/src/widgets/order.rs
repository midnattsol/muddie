//! Drawing a part of the window after what is above it, though it is laid
//! out first: the keyboard goes through what is drawn in the order it is
//! drawn, and should go through the window as it is read, top to bottom.

use super::*;

/// Draws `above` in `ui`, leaving room at the bottom, and then `below` in
/// that room, which takes the bottom of `ui` as a panel does: what is
/// below is reached last with Tab, where it is seen. The room is as tall
/// as `below` said it was the time before, the height it returns; when
/// that changes, the pass is drawn again, so it is never seen wrong.
pub fn below_last<S, R>(
    ui: &mut Ui,
    id: egui::Id,
    state: &mut S,
    above: impl FnOnce(&mut S, &mut Ui) -> R,
    below: impl FnOnce(&mut S, &mut Ui) -> f32,
) -> R {
    let whole = ui.available_rect_before_wrap();
    let room = (ui.data(|data| data.get_temp::<f32>(id)))
        .unwrap_or(0.0)
        .clamp(0.0, whole.height());
    let upper = egui::Rect::from_min_max(whole.min, egui::pos2(whole.max.x, whole.max.y - room));
    let shown = ui
        .scope_builder(egui::UiBuilder::new().max_rect(upper), |ui| {
            above(state, ui)
        })
        .inner;
    let height = ui
        .scope_builder(egui::UiBuilder::new().max_rect(whole), |ui| {
            below(state, ui)
        })
        .inner;
    if (height - room).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(id, height));
        ui.ctx().request_discard("the room at the bottom changed");
    }
    ui.advance_cursor_after_rect(whole);
    shown
}

/// Notes whether the keyboard or the pointer was used last: call it once a
/// frame, before anything is drawn.
pub fn note_input(ctx: &egui::Context) {
    let (keys, pointer) = ctx.input(|input| {
        let keys = (input.events.iter())
            .any(|event| matches!(event, egui::Event::Key { pressed: true, .. }));
        (keys, input.pointer.any_pressed())
    });
    if keys || pointer {
        ctx.data_mut(|data| data.insert_temp(keyboard_id(), keys && !pointer));
    }
}

/// Whether the keyboard was used last rather than the pointer: what opens
/// then puts the keyboard in itself, where it is seen. Opened with the
/// pointer, nothing is marked until the keyboard is used.
pub fn keyboard_in_use(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp(keyboard_id()))
        .unwrap_or(false)
}

fn keyboard_id() -> egui::Id {
    egui::Id::unique("keyboard in use")
}
