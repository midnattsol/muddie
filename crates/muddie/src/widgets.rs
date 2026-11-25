//! The interface's own controls, drawn to the theme instead of taking
//! the toolkit's defaults.

use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, Response, RichText, Role, Sense,
    Stroke, TextEdit, Ui, Vec2, WidgetInfo, vec2,
};

use crate::theme::{self, Weight};

mod buttons;
mod code;
mod colours;
mod dialog;
mod edges;
mod fields;
mod folders;
mod glyphs;
mod icons;
mod lists;
mod menus;
mod order;
mod profiles;
mod rows;
mod tips;
mod toggles;
mod unreached;

pub use buttons::*;
pub use code::*;
pub use colours::*;
pub use dialog::*;
pub use edges::*;
pub use fields::*;
pub use folders::*;
pub use glyphs::*;
pub use icons::*;
pub use lists::*;
pub use menus::*;
pub use order::*;
pub use profiles::*;
pub use rows::*;
pub use tips::*;
pub use toggles::*;
pub use unreached::*;

#[cfg(test)]
mod tests {
    /// A long script is coloured and laid out once, and kept while it is
    /// not changed: not again every frame it is shown.
    #[test]
    fn code_is_laid_out_again_only_when_it_changes() {
        let ctx = eframe::egui::Context::default();
        let mut built = 0;
        let mut script = "local x = 1\n".repeat(400);
        let frame = |script: &str, built: &mut usize| {
            let mut galley = None;
            let mut output = ctx.run_ui(Default::default(), |ui| {
                let id = eframe::egui::Id::unique("test editor");
                galley = Some(super::cached_layout(ui, id, 500u32, script, || {
                    *built += 1;
                    let font = eframe::egui::FontId::monospace(13.0);
                    crate::luau::highlight(script, &font, &crate::theme::current())
                }));
            });
            output.textures_delta.clear();
            galley.unwrap()
        };
        // The theme is the whole program's, and tests run at the same time
        // may change it, which lays it out again rightly: what is looked
        // at is a frame after another with the same theme.
        let stable = || {
            let generation = crate::theme::generation();
            move || crate::theme::generation() == generation
        };
        let same = stable();
        let first = frame(&script, &mut built);
        let once = built;
        let second = frame(&script, &mut built);
        if same() {
            assert_eq!(built, once);
            assert!(std::sync::Arc::ptr_eq(&first, &second));
        }
        let once = built;
        script.push('x');
        frame(&script, &mut built);
        assert!(built > once);
    }

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
