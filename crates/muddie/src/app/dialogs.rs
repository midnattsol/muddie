use super::*;

/// The keys of a place that are not shortcuts to choose: those that go
/// through the window, and the system's.
fn fixed_keys(place: crate::shortcuts::Place) -> Vec<(String, &'static str)> {
    use crate::shortcuts::Place;
    let keys = |pairs: &[(&str, &'static str)]| -> Vec<(String, &'static str)> {
        (pairs.iter())
            .map(|(keys, does)| ((*keys).to_owned(), *does))
            .collect()
    };
    match place {
        Place::Everywhere => keys(&[
            ("Tab  Shift+Tab", tr("Go to the next or previous control")),
            (
                tr("Shift+F10  Menu key"),
                tr("Open the context menu of the focused item"),
            ),
            (crate::shortcuts::copy_written(), tr("Copy")),
        ]),
        Place::Line => keys(&[
            ("Tab", tr("Complete the word being typed")),
            ("Tab", tr("With nothing typed, read the output as text")),
        ]),
        Place::Map => keys(&[
            (
                tr("Shift+F10  Menu key"),
                tr("The room's menu, with each exit to follow"),
            ),
            (tr("Escape"), tr("Back to the command line")),
        ]),
        Place::Editor => keys(&[
            ("↑  ↓", tr("Select the previous or next item")),
            ("←  →", tr("Collapse or expand a folder")),
            (
                if cfg!(target_os = "macos") {
                    tr("Cmd+click  Shift+click")
                } else {
                    tr("Ctrl+click  Shift+click")
                },
                tr("Select several items or a range"),
            ),
        ]),
        Place::Layout => keys(&[(
            tr("← →  ↑ ↓ on an edge"),
            tr("Make the panels larger or smaller"),
        )]),
        Place::Reading => Vec::new(),
    }
}

/// The groups of shortcuts, each under its title, in a grid of the keys
/// and what they do: those that can be chosen as they are now, then
/// those that stay.
fn show_shortcut_groups(ui: &mut Ui, theme: &theme::Theme) {
    for place in crate::shortcuts::Place::ALL {
        let title = place.title();
        widgets::caption(ui, title);
        let mut rows: Vec<(String, &str)> = (crate::shortcuts::ALL.iter())
            .filter(|shortcut| shortcut.place == place)
            .map(|shortcut| (shortcut.all_written(), shortcut.about()))
            .collect();
        rows.extend(fixed_keys(place));
        egui::Grid::new(title)
            .num_columns(2)
            .spacing([24.0, 8.0])
            .show(ui, |ui| {
                for (keys, action) in &rows {
                    ui.label(
                        RichText::new(keys.as_str())
                            .font(theme::font(Weight::Medium, 13.0))
                            .color(theme.text),
                    );
                    ui.label(RichText::new(*action).color(theme.text_dim));
                    ui.end_row();
                }
            });
        ui.add_space(8.0);
    }
}

impl App {
    pub(super) fn shortcuts_window(&mut self, ctx: &egui::Context) {
        let theme = theme::current();
        widgets::dialog(
            ctx,
            "keyboard shortcuts",
            tr("Keyboard shortcuts"),
            &mut self.show_shortcuts,
            size::DIALOG_S,
            |ui| {
                show_shortcut_groups(ui, &theme);
            },
        );
    }

    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        let theme = theme::current();
        widgets::dialog(
            ctx,
            "about muddie",
            tr("About Muddie"),
            &mut self.show_about,
            size::DIALOG_S,
            |ui| {
                widgets::logo(ui, 80.0);
                ui.label(
                    RichText::new("Muddie")
                        .font(theme::font(Weight::Semibold, 26.0))
                        .color(theme.text),
                );
                let version = trf("Version {0}", &[env!("CARGO_PKG_VERSION")]);
                ui.label(RichText::new(version).color(theme.text_dim));
                ui.add_space(6.0);
                ui.label(
                    RichText::new(tr("A modern, accessible MUD client.")).color(theme.text_dim),
                );
                ui.label(
                    RichText::new(tr("Free software, under the MIT or Apache 2.0 licence."))
                        .color(theme.text_faint),
                );
            },
        );
    }
}
