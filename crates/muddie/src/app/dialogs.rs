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
                let groups: [(&str, &[(&str, &str)]); 7] = [
                    (
                        tr("Typing"),
                        &[
                            ("↑  ↓", tr("Earlier and later commands")),
                            (
                                crate::shortcuts::HISTORY_SEARCH.written(),
                                tr("Find a command typed before"),
                            ),
                            ("Tab", tr("Complete the word being typed")),
                            ("Tab", tr("With nothing typed, read the output as text")),
                            ("F6  Shift+F6", tr("Go to the output and each panel")),
                            (crate::shortcuts::MESSAGES.written(), tr("Open the messages")),
                            (crate::shortcuts::FIND.written(), tr("Find in the output")),
                        ],
                    ),
                    (
                        tr("Speech"),
                        &[
                            ("Ctrl", tr("Silence speech")),
                            (
                                crate::shortcuts::with_arrows!("↑  ↓"),
                                tr("Read the previous and next line"),
                            ),
                            (
                                crate::shortcuts::with_arrows!("Home  End"),
                                tr("Read the first and last line"),
                            ),
                            (
                                crate::shortcuts::with_key!("1 … 9"),
                                tr("Read the most recent lines"),
                            ),
                            (
                                crate::shortcuts::with_arrows!("Shift+↑ …"),
                                tr("Same, for a panel of text"),
                            ),
                            (
                                crate::shortcuts::with_arrows!("Shift+←  →"),
                                tr("Choose the panel of text to read"),
                            ),
                            (
                                crate::shortcuts::with_key!("C"),
                                tr("Copy the line read last"),
                            ),
                            (
                                crate::shortcuts::with_key!("O"),
                                tr("Follow the link of the line read last"),
                            ),
                            (crate::shortcuts::GAUGES.written(), tr("Read the gauges")),
                            (crate::shortcuts::WHERE.written(), tr("Where am I")),
                            (
                                crate::shortcuts::EXPLORE.written(),
                                tr("Explore the map with the keys"),
                            ),
                        ],
                    ),
                    (
                        tr("Exploring the map"),
                        &[
                            ("↑ ↓ ← →  PgUp PgDn", tr("Follow the exits")),
                            (
                                tr("The keypad"),
                                tr(
                                    "Follow the exits by where its keys are; slash and star, in and out",
                                ),
                            ),
                            (tr("Home"), tr("Back to your room")),
                            (tr("Backspace"), tr("Back to the room before")),
                            (tr("Space"), tr("Say the room again")),
                            (
                                crate::shortcuts::ROOM_INFO.written(),
                                tr("All about the room"),
                            ),
                            (crate::shortcuts::NEAR.written(), tr("What is near")),
                            (crate::shortcuts::FIND.written(), tr("Find on the map")),
                            (crate::shortcuts::MARK.written(), tr("Mark the room")),
                            (
                                crate::shortcuts::NOTE.written(),
                                tr("Write a note on the room"),
                            ),
                            (tr("Enter  Shift+Enter"), tr("Go there, or say the way")),
                            (
                                tr("Shift+F10  Menu key"),
                                tr("The room's menu, with each exit to follow"),
                            ),
                            ("F1", tr("Say all the keys")),
                            (tr("Escape"), tr("Back to the command line")),
                        ],
                    ),
                    (
                        tr("Sessions"),
                        &[
                            (crate::shortcuts::NEW_SESSION.written(), tr("New session")),
                            (
                                crate::shortcuts::CLOSE_SESSION.written(),
                                tr("Close session"),
                            ),
                            (crate::shortcuts::RECONNECT.written(), tr("Reconnect")),
                            (crate::shortcuts::PALETTE.written(), tr("Palette")),
                            (
                                crate::shortcuts::SWITCH_VIEW.written(),
                                tr("Switch between the game and the editor"),
                            ),
                            (
                                crate::shortcuts::DATA_VIEW.written(),
                                tr("Show incoming data"),
                            ),
                            (crate::shortcuts::SETTINGS.written(), tr("Settings")),
                        ],
                    ),
                    (
                        tr("Editor"),
                        &[
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
                            (
                                crate::shortcuts::SELECT_ALL.written(),
                                tr("Select all visible items"),
                            ),
                            (
                                crate::shortcuts::with_arrows!("↑  ↓"),
                                tr("Move up or down"),
                            ),
                            (
                                crate::shortcuts::with_arrows!("←"),
                                tr("Move out of its folder"),
                            ),
                            ("F2", tr("Rename")),
                            (tr("Del"), tr("Delete")),
                            (crate::shortcuts::UNDO.written(), tr("Undo")),
                            (crate::shortcuts::REDO.written(), tr("Redo")),
                        ],
                    ),
                    (
                        tr("Panels"),
                        &[
                            (crate::shortcuts::MAP.written(), tr("Show or hide the map")),
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
