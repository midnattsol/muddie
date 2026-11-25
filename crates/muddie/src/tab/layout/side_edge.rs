//! The edge of a side of panels, which makes the side larger or smaller.

use super::*;

/// The edge of `side`, whose panels are in `rect`, for the pointer only.
/// Returns how large the side is now.
pub(super) fn drag_edge(ui: &mut egui::Ui, side: Side, rect: egui::Rect, id: egui::Id) -> f32 {
    let line = match side {
        Side::Left => rect.with_min_x(rect.right()),
        Side::Right => rect.with_max_x(rect.left()),
        Side::Top => rect.with_min_y(rect.bottom()),
        Side::Bottom => rect.with_max_y(rect.top()),
    };
    let reach = ui.style().interaction.resize_grab_radius_side;
    let (along, sign) = match side {
        Side::Left => (egui::vec2(reach, 0.0), 1.0),
        Side::Right => (egui::vec2(reach, 0.0), -1.0),
        Side::Top => (egui::vec2(0.0, reach), 1.0),
        Side::Bottom => (egui::vec2(0.0, reach), -1.0),
    };
    let edge = widgets::edge(
        ui,
        line.expand2(along),
        id.with("edge"),
        side.size_label(),
        side.is_beside(),
        false,
    );
    if edge.response.hovered() || edge.response.dragged() {
        let stroke = ui.visuals().widgets.hovered.fg_stroke;
        ui.painter().line_segment([line.min, line.max], stroke);
    }
    let size = match side.is_beside() {
        true => rect.width(),
        false => rect.height(),
    };
    // Whole, unless it is being dragged, not to lose what is less than one.
    match edge.response.dragged() {
        true => size + sign * edge.moved,
        false => size.round(),
    }
}

/// Moves the edge of `side` with the arrows while `handle`, the edge, has
/// the keyboard: away from the output makes its panels smaller, toward it
/// larger. Returns whether it was moved.
pub(super) fn size_by_keys(ui: &egui::Ui, side: Side, handle: egui::Id) -> bool {
    if !ui.memory(|memory| memory.has_focus(handle)) {
        return false;
    }
    let (larger, smaller) = match side {
        Side::Left => (egui::Key::ArrowRight, egui::Key::ArrowLeft),
        Side::Right => (egui::Key::ArrowLeft, egui::Key::ArrowRight),
        Side::Top => (egui::Key::ArrowDown, egui::Key::ArrowUp),
        Side::Bottom => (egui::Key::ArrowUp, egui::Key::ArrowDown),
    };
    ui.input_mut(|input| {
        let action = if input.consume_key(egui::Modifiers::NONE, larger) {
            egui::accesskit::Action::Increment
        } else if input.consume_key(egui::Modifiers::NONE, smaller) {
            egui::accesskit::Action::Decrement
        } else {
            return false;
        };
        // As a screen reader would ask: the panel does it as it does then.
        input.events.push(egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action,
                target_tree: egui::accesskit::TreeId::ROOT,
                target_node: handle.accesskit_id(),
                data: None,
            },
        ));
        true
    })
}
