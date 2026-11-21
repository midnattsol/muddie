//! An edge between two parts of the window, dragged to trade their sizes.

use super::*;

/// How far the arrows move an edge at a time, in points.
const STEP: f32 = 16.0;

/// What was done with an edge this frame.
pub struct Moved {
    pub response: Response,
    /// How far it was moved, toward the right or the bottom.
    pub moved: f32,
    /// It was moved with the arrows: what it sizes is to be said.
    pub by_keys: bool,
}

/// An edge over `rect`, named `label` for what it sizes, dragged with the
/// pointer or moved with the arrows while it has the keyboard: left and
/// right when it stands, up and down when it lies.
pub fn edge(ui: &mut Ui, rect: egui::Rect, id: egui::Id, label: &str, standing: bool) -> Moved {
    let cursor = match standing {
        true => egui::CursorIcon::ResizeHorizontal,
        false => egui::CursorIcon::ResizeVertical,
    };
    let response = ui.interact(rect, id, Sense::drag()).on_hover_cursor(cursor);
    response.widget_info(|| WidgetInfo::labeled(Role::Splitter, true, label));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_orientation(match standing {
            true => egui::accesskit::Orientation::Vertical,
            false => egui::accesskit::Orientation::Horizontal,
        });
    });
    let mut moved = if response.dragged() {
        ui.ctx().set_cursor_icon(cursor);
        match standing {
            true => response.drag_delta().x,
            false => response.drag_delta().y,
        }
    } else {
        0.0
    };
    let mut by_keys = false;
    if response.has_focus() {
        // Its arrows move it, not the keyboard to what is beside it.
        let filter = egui::EventFilter {
            horizontal_arrows: standing,
            vertical_arrows: !standing,
            ..Default::default()
        };
        ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, filter));
        let (back, on) = match standing {
            true => (egui::Key::ArrowLeft, egui::Key::ArrowRight),
            false => (egui::Key::ArrowUp, egui::Key::ArrowDown),
        };
        ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, back) {
                (moved, by_keys) = (-STEP, true);
            } else if input.consume_key(egui::Modifiers::NONE, on) {
                (moved, by_keys) = (STEP, true);
            }
        });
    }
    Moved {
        response,
        moved,
        by_keys,
    }
}
