//! The interface's own controls, drawn to the theme instead of taking
//! the toolkit's defaults.

use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, Response, RichText, Role, Sense,
    Stroke, TextEdit, Ui, Vec2, WidgetInfo, vec2,
};

use crate::theme::{self, Weight};

mod buttons;
mod colours;
mod dialog;
mod edges;
mod fields;
mod glyphs;
mod icons;
mod lists;
mod menus;
mod order;
mod rows;
mod toggles;

pub use buttons::*;
pub use colours::*;
pub use dialog::*;
pub use edges::*;
pub use fields::*;
pub use glyphs::*;
pub use icons::*;
pub use lists::*;
pub use menus::*;
pub use order::*;
pub use rows::*;
pub use toggles::*;

#[cfg(test)]
mod tests {
    /// Every control is one of this module's, so that there is one of
    /// each kind and they all look and behave alike: nothing else makes
    /// a button, a list, a tick or a number field of its own.
    #[test]
    fn controls_are_made_here_and_nowhere_else() {
        let raw = [
            "ui.button(",
            "Button::new(",
            "ui.small_button(",
            ".selectable_label(",
            ".selectable_value(",
            "ComboBox::from_",
            "DragValue::new(",
            "Slider::new(",
            "ui.checkbox(",
            "ui.radio(",
            ".radio_value(",
            ".toggle_value(",
        ];
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        // Every source file, in every folder, but this module's own.
        let mut files = vec![];
        let mut folders = vec![source];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(folder).unwrap().flatten() {
                let path = entry.path();
                let ours = path.ends_with("widgets.rs") || path.ends_with("widgets");
                if ours {
                    continue;
                }
                if path.is_dir() {
                    folders.push(path);
                } else if path.extension().is_some_and(|kind| kind == "rs") {
                    files.push(path);
                }
            }
        }
        assert!(files.len() > 20, "the sources were not found");
        let mut found = vec![];
        for path in files {
            let text = std::fs::read_to_string(&path).unwrap();
            for (number, line) in text.lines().enumerate() {
                if raw.iter().any(|control| line.contains(control)) {
                    found.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
        assert!(
            found.is_empty(),
            "use the controls of widgets.rs:\n{}",
            found.join("\n")
        );
    }
}
