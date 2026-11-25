//! What is shown when something is pointed at.

use super::*;

/// Shows `name`, what something with no words of its own is called, when it
/// is pointed at. A screen reader has it as its name: as egui's own hint it
/// would have it as its description too, and say it twice.
pub fn name_tip(response: Response, name: &str) -> Response {
    response.on_hover_ui(|ui| {
        ui.set_max_width(ui.spacing().tooltip_width);
        ui.label(name);
    })
}
