use super::*;

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
                let groups: [(&str, &[(&str, &str)]); 6] = [
                    (
                        tr("Typing"),
                        &[
                            ("↑  ↓", tr("Earlier and later commands")),
                            ("Ctrl+R", tr("Search the commands typed before")),
                            ("Tab", tr("Complete the word being typed")),
                            ("Ctrl+F", tr("Search the output")),
                        ],
                    ),
                    (
                        tr("Speech"),
                        &[
                            ("Ctrl", tr("Silence speech")),
                            ("Alt+↑  Alt+↓", tr("Read the previous and next line")),
                            ("Alt+1 … Alt+9", tr("Read the most recent lines")),
                            (tr("Alt+Shift+↑ …"), tr("Same, for the panel of channels")),
                            ("Alt+0", tr("Read the gauges")),
                            ("Alt+M", tr("Say where you are, by the map")),
                        ],
                    ),
                    (
                        tr("Sessions"),
                        &[
                            ("Ctrl+T", tr("New session")),
                            ("Ctrl+W", tr("Close session")),
                            ("Alt+R", tr("Reconnect")),
                            ("Ctrl+K", tr("Commands")),
                            ("Ctrl+E", tr("Switch between the game and the editor")),
                            ("Ctrl+D", tr("Show incoming data")),
                        ],
                    ),
                    (
                        tr("Editor"),
                        &[
                            ("↑  ↓", tr("Select the previous or next item")),
                            ("←  →", tr("Collapse or expand a folder")),
                            (
                                tr("Ctrl+click  Shift+click"),
                                tr("Select several items or a range"),
                            ),
                            ("Ctrl+A", tr("Select all visible items")),
                            ("Alt+↑  Alt+↓", tr("Move up or down")),
                            ("F2", tr("Rename")),
                            (tr("Del"), tr("Delete")),
                            ("Ctrl+Z", tr("Undo")),
                            (tr("Ctrl+Y  Ctrl+Shift+Z"), tr("Redo")),
                        ],
                    ),
                    (
                        tr("Panels"),
                        &[
                            ("Ctrl+M", tr("Show or hide the map")),
                            (
                                tr("← →  ↑ ↓ on an edge"),
                                tr("Make the panels larger or smaller"),
                            ),
                        ],
                    ),
                    (
                        tr("Everywhere"),
                        &[
                            ("Tab  Shift+Tab", tr("Go to the next or previous control")),
                            (
                                tr("Shift+F10  Menu key"),
                                tr("Open the context menu of the focused item"),
                            ),
                        ],
                    ),
                ];
                for (title, shortcuts) in groups {
                    widgets::caption(ui, title);
                    egui::Grid::new(title)
                        .num_columns(2)
                        .spacing([24.0, 8.0])
                        .show(ui, |ui| {
                            for (keys, action) in shortcuts {
                                ui.label(
                                    RichText::new(*keys)
                                        .font(theme::font(Weight::Medium, 13.0))
                                        .color(theme.text),
                                );
                                ui.label(RichText::new(*action).color(theme.text_dim));
                                ui.end_row();
                            }
                        });
                    ui.add_space(8.0);
                }
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
