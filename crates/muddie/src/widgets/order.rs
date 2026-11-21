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

/// Draws `left` in a row of `ui`, leaving room at its right end, and then
/// `right` there, left to right too: everything is reached with Tab as it
/// is read. The room, and the
/// height of the row, are what the two took the time before; when that
/// changes, the pass is drawn again.
pub fn right_last<S>(
    ui: &mut Ui,
    id: egui::Id,
    state: &mut S,
    left: impl FnOnce(&mut S, &mut Ui),
    right: impl FnOnce(&mut S, &mut Ui),
) {
    let known = ui.data(|data| data.get_temp::<(f32, f32)>(id));
    let (room, height) = known.unwrap_or((0.0, 0.0));
    let row = egui::Rect::from_min_size(
        ui.available_rect_before_wrap().min,
        egui::vec2(ui.available_width(), height),
    );
    let room = room.clamp(0.0, row.width());
    let gap = ui.spacing().item_spacing.x;
    let start = egui::Rect::from_min_max(row.min, egui::pos2(row.max.x - room - gap, row.max.y));
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    // What is at the left goes on to another line rather than widen the row.
    let wrapping = layout.with_main_wrap(true);
    let tall = ui
        .scope_builder(
            egui::UiBuilder::new().max_rect(start).layout(wrapping),
            |ui| {
                left(state, ui);
                ui.min_rect().height()
            },
        )
        .inner;
    // Until how wide it is is known, it is laid out from the right edge, so
    // that it cannot stick out of the row, and widen what holds it, while
    // it is measured.
    let (end, end_layout) = match known {
        Some(_) => (
            egui::Rect::from_min_max(egui::pos2(row.max.x - room, row.min.y), row.max),
            layout,
        ),
        None => (row, egui::Layout::right_to_left(egui::Align::Center)),
    };
    let (wide, tall_right) = ui
        .scope_builder(
            egui::UiBuilder::new().max_rect(end).layout(end_layout),
            |ui| {
                right(state, ui);
                (ui.min_rect().width(), ui.min_rect().height())
            },
        )
        .inner;
    let now = (wide, tall.max(tall_right));
    if (now.0 - room).abs() > 0.5 || (now.1 - height).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(id, now));
        ui.ctx().request_discard("a row's parts changed size");
    }
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(
        row.min,
        egui::vec2(row.width(), now.1),
    ));
}
