use super::*;

/// The pages of the settings, as tabs along their top: Muddie's own,
/// the same for every profile, and the profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    General,
    Appearance,
    Text,
    Speech,
    Profiles,
}

impl Section {
    pub const ALL: [Section; 5] = [
        Section::General,
        Section::Appearance,
        Section::Text,
        Section::Speech,
        Section::Profiles,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Section::General => tr("General"),
            Section::Appearance => tr("Appearance"),
            Section::Text => tr("Text"),
            Section::Speech => tr("Speech"),
            Section::Profiles => tr("Profiles"),
        }
    }
}

impl App {
    /// Opens the settings at a page.
    pub fn open_settings(&mut self, section: Section) {
        self.show_settings = true;
        self.settings_section = section;
        if section != Section::Profiles {
            self.editing = None;
        }
    }

    /// The settings: a tab for each page along the top, and the page
    /// under them.
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return self.keep_profile(ctx, false);
        }
        let before = self.settings.clone();
        let mut open = true;
        let height = (ctx.content_rect().height() - 160.0).clamp(300.0, 620.0);
        widgets::dialog(
            ctx,
            "settings",
            tr("Settings"),
            &mut open,
            size::DIALOG_L,
            |ui| {
                let names = Section::ALL.map(Section::name);
                let current = Section::ALL
                    .iter()
                    .position(|section| *section == self.settings_section);
                if let Some(chosen) = widgets::segmented(ui, &names, current.unwrap_or(0)) {
                    self.open_settings(Section::ALL[chosen]);
                }
                ui.add_space(size::GAP_M);
                egui::ScrollArea::vertical()
                    .id_salt(("settings", self.settings_section))
                    .max_height(height)
                    .min_scrolled_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = size::GAP_S;
                        self.settings_page(ui, ctx);
                        if let Some(error) = &self.settings_error {
                            ui.colored_label(theme::current().danger, error);
                        }
                    });
            },
        );
        self.show_settings = open;
        self.keep_profile(ctx, open);
        if self.settings != before {
            self.apply_settings(ctx);
        }
    }

    /// The page chosen.
    fn settings_page(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        match self.settings_section {
            Section::General => {
                let languages = self.languages.get_or_insert_with(i18n::languages).clone();
                general_settings(ui, &mut self.settings, &languages);
            }
            Section::Appearance => appearance_settings(ui, &mut self.settings),
            Section::Text => text_settings(ui, &mut self.settings),
            Section::Speech => speech_settings(ui, &mut self.settings),
            Section::Profiles if self.editing.is_some() => self.profile_page(ui, ctx),
            Section::Profiles => self.profiles_list(ui),
        }
    }
}

fn general_settings(ui: &mut Ui, settings: &mut Settings, languages: &[(String, String)]) {
    widgets::group(ui, |ui| {
        widgets::setting_row(ui, tr("Language"), &mut |ui| {
            // The system's, English, and every catalogue there is.
            let mut choices = vec![
                (String::new(), tr("Same as the system").to_owned()),
                ("en".to_owned(), "English".to_owned()),
            ];
            choices.extend(languages.iter().cloned());
            let current = match settings.language.as_str() {
                "auto" => "",
                "english" => "en",
                "spanish" => "es",
                code => code,
            }
            .to_owned();
            let shown = choices.iter().find(|(code, _)| *code == current);
            let shown = shown.map_or(current.clone(), |(_, name)| name.clone());
            crate::widgets::dropdown("language")
                .selected_text(shown)
                .width(210.0)
                .show_ui(ui, |ui| {
                    for (code, name) in &choices {
                        if crate::widgets::choice(ui, *code == current, name).clicked() {
                            settings.language.clone_from(code);
                        }
                    }
                })
                .response
        });
        widgets::setting_row(ui, tr("Interface scale"), &mut |ui| {
            let percent = (settings.ui_scale * 100.0).round() as u32;
            let steps = [75, 90, 100, 110, 125, 150, 175, 200];
            crate::widgets::dropdown("interface scale")
                .selected_text(format!("{percent} %"))
                .width(size::FIELD_M)
                .show_ui(ui, |ui| {
                    for step in steps {
                        if crate::widgets::choice(ui, step == percent, &format!("{step} %"))
                            .clicked()
                        {
                            settings.ui_scale = step as f32 / 100.0;
                        }
                    }
                })
                .response
        });
        widgets::row_note(ui, tr("How large the whole window is drawn."));
    });
}

/// The theme, and the typeface of the output with a sample of it.
fn appearance_settings(ui: &mut Ui, settings: &mut Settings) {
    let theme = theme::current();
    widgets::group(ui, |ui| {
        widgets::setting_row(ui, tr("Theme"), &mut |ui| {
            let name = |theme: ThemeName| match theme {
                ThemeName::Dark => tr("Dark"),
                ThemeName::Graphite => tr("Graphite"),
                ThemeName::Mocha => "Mocha",
                ThemeName::Dusk => tr("Dusk"),
                ThemeName::TokyoNight => "Tokyo Night",
                ThemeName::Nord => "Nord",
                ThemeName::Light => tr("Light"),
                ThemeName::Contrast => tr("High contrast"),
            };
            let themes = [
                ThemeName::Dark,
                ThemeName::Graphite,
                ThemeName::Mocha,
                ThemeName::Dusk,
                ThemeName::TokyoNight,
                ThemeName::Nord,
                ThemeName::Light,
                ThemeName::Contrast,
            ];
            crate::widgets::dropdown("theme")
                .selected_text(name(settings.theme))
                .width(210.0)
                .show_ui(ui, |ui| {
                    for theme in themes {
                        crate::widgets::choice_value(ui, &mut settings.theme, theme, name(theme));
                    }
                })
                .response
        });
    });
    ui.add_space(6.0);
    widgets::caption(ui, tr("MUD text"));
    widgets::group(ui, |ui| {
        widgets::setting_row(ui, tr("Colours"), &mut |ui| {
            let name = |palette: MudPalette| match palette {
                MudPalette::Vivid => tr("Vivid"),
                MudPalette::Soft => tr("Soft, to go with the theme"),
                MudPalette::Classic => tr("Classic terminal"),
            };
            crate::widgets::dropdown("mud palette")
                .selected_text(name(settings.mud_palette))
                .width(260.0)
                .show_ui(ui, |ui| {
                    for palette in [MudPalette::Vivid, MudPalette::Soft, MudPalette::Classic] {
                        crate::widgets::choice_value(
                            ui,
                            &mut settings.mud_palette,
                            palette,
                            name(palette),
                        );
                    }
                })
                .response
        });
        widgets::setting_row(ui, tr("Font"), &mut |ui| {
            // What the settings hold is a name of ours or a family's.
            let shown = match settings.font_family.strip_prefix("muddie:") {
                _ if settings.font_family.is_empty() => theme::DEFAULT_FONT,
                Some(bundled) => bundled,
                None => settings.font_family.as_str(),
            }
            .to_owned();
            crate::widgets::dropdown("font")
                .selected_text(shown)
                .width(260.0)
                .height(340.0)
                .show_ui(ui, |ui| {
                    let heading = |ui: &mut Ui, text: &str| {
                        let small = RichText::new(text)
                            .font(theme::font(Weight::Regular, 11.5))
                            .color(theme.text_faint);
                        ui.add(egui::Label::new(small).selectable(false));
                    };
                    heading(ui, tr("Bundled"));
                    if crate::widgets::choice(
                        ui,
                        settings.font_family.is_empty(),
                        theme::DEFAULT_FONT,
                    )
                    .clicked()
                    {
                        settings.font_family.clear();
                        settings.font_file.clear();
                    }
                    for (name, _) in theme::BUNDLED_FONTS {
                        let key = theme::bundled_font(name);
                        if crate::widgets::choice(ui, settings.font_family == key, name).clicked() {
                            settings.font_family = key;
                            settings.font_file.clear();
                        }
                    }
                    // The monospaced fonts the system has.
                    ui.separator();
                    heading(ui, tr("System fonts"));
                    let Some(installed) = theme::installed_fonts() else {
                        // Still going through the system's fonts.
                        heading(ui, tr("Loading…"));
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_millis(150));
                        return;
                    };
                    for font in installed {
                        let chosen = settings.font_family == font.family;
                        if crate::widgets::choice(ui, chosen, &font.family).clicked() {
                            // Where it is goes with its name, so that it is
                            // found at once the next time.
                            settings.font_family.clone_from(&font.family);
                            (settings.font_file, settings.font_index) = font.place();
                        }
                    }
                })
                .response
        });
        widgets::setting_row(ui, tr("Size"), &mut |ui| {
            let sizes = (10..=28).map(|size| size as f32);
            pick(ui, "font size", &mut settings.font_size, sizes, |size| {
                format!("{size} pt")
            })
        });
        widgets::setting_row(ui, tr("Line spacing"), &mut |ui| {
            let steps = [0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.8];
            pick(
                ui,
                "line spacing",
                &mut settings.line_spacing,
                steps,
                |step| format!("{step:.1}"),
            )
        });
    });
    // What the output will look like.
    let sample = "\x1b[1;32mPlaza Mayor\x1b[0m\nUna plaza amplia con una fuente. Al norte, la \x1b[33mtorre\x1b[0m.\n\x1b[36mSalidas: norte, este, sur\x1b[0m  0O 1lI {}[] <=>\n\x1b[31mrojo \x1b[32mverde \x1b[33mamarillo \x1b[34mazul \x1b[35mmagenta \x1b[36mcian \x1b[37mblanco \x1b[1;30mgris\x1b[0m\n\x1b[1;31mrojo \x1b[1;32mverde \x1b[1;33mamarillo \x1b[1;34mazul \x1b[1;35mmagenta \x1b[1;36mcian \x1b[1;37mblanco\x1b[0m";
    let well = egui::Frame::new()
        .fill(theme.background)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(14, 10));
    well.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        for line in crate::panels::styled_lines(sample) {
            ui.add(egui::Label::new(crate::output::server_job(&line)).selectable(false));
        }
    });
}

/// A value chosen among steps, in a list: one control for one value.
/// One set by hand between steps is shown as it is.
fn pick<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: &str,
    value: &mut T,
    steps: impl IntoIterator<Item = T>,
    shown: impl Fn(T) -> String,
) -> egui::Response {
    crate::widgets::dropdown(id)
        .selected_text(shown(*value))
        .width(size::FIELD_M)
        .height(320.0)
        .show_ui(ui, |ui| {
            for step in steps {
                if crate::widgets::choice(ui, step == *value, &shown(step)).clicked() {
                    *value = step;
                }
            }
        })
        .response
}

/// A number with its thousands apart, to be read at a glance.
fn grouped(number: usize) -> String {
    let digits = number.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(digit);
    }
    out
}

/// The output kept, how each line is shown, and the command line.
fn text_settings(ui: &mut Ui, settings: &mut Settings) {
    widgets::group(ui, |ui| {
        widgets::setting_row(ui, tr("Lines kept"), &mut |ui| {
            let steps = [
                1_000, 5_000, 10_000, 20_000, 50_000, 100_000, 500_000, 1_000_000,
            ];
            pick(
                ui,
                "scrollback",
                &mut settings.scrollback_lines,
                steps,
                grouped,
            )
        });
        widgets::row_note(
            ui,
            tr("The lines kept to scroll back to and search; older ones are let go."),
        );
        widgets::switch_setting(
            ui,
            &mut settings.timestamps,
            tr("Show the time of each line"),
        );
        widgets::row_note(ui, tr("The time each line arrived, at its left."));
        widgets::switch_setting(
            ui,
            &mut settings.keep_command,
            tr("Keep the command after sending it"),
        );
        widgets::row_note(
            ui,
            tr("It stays selected: Enter sends it again, and typing replaces it."),
        );
    });
}

/// How the voice speaks, and whether the screen reader does instead.
fn speech_settings(ui: &mut Ui, settings: &mut Settings) {
    let theme = theme::current();
    widgets::group(ui, |ui| {
        widgets::setting_row(ui, tr("Speed"), &mut |ui| {
            let small = |text: &str| {
                RichText::new(text)
                    .font(theme::font(Weight::Regular, 12.0))
                    .color(theme.text_dim)
            };
            ui.label(small(tr("Fast")));
            let slider =
                crate::widgets::slider(&mut settings.speech_rate, -100..=100).show_value(false);
            let response = ui.add_sized([size::FIELD_M, 20.0], slider);
            ui.label(small(tr("Slow")));
            response
        });
        widgets::setting_row(ui, tr("Language of the voice"), &mut |ui| {
            let languages = [
                ("", tr("The system's")),
                ("es", "Español"),
                ("en", "English"),
                ("ca", "Català"),
                ("gl", "Galego"),
                ("eu", "Euskara"),
                ("pt", "Português"),
                ("fr", "Français"),
                ("it", "Italiano"),
                ("de", "Deutsch"),
                ("nl", "Nederlands"),
                ("pl", "Polski"),
                ("ru", "Русский"),
            ];
            let current = settings.speech_language.trim().to_lowercase();
            let shown = (languages.iter())
                .find(|(code, _)| *code == current)
                .map_or(current.clone(), |(_, name)| (*name).to_owned());
            crate::widgets::dropdown("speech language")
                .selected_text(shown)
                .width(size::FIELD_M)
                .height(320.0)
                .show_ui(ui, |ui| {
                    for (code, name) in languages {
                        if crate::widgets::choice(ui, code == current, name).clicked() {
                            settings.speech_language = code.to_owned();
                        }
                    }
                })
                .response
        });
        widgets::row_note(ui, tr("The voice the text of the MUD is read with."));
        widgets::switch_setting(
            ui,
            &mut settings.announce_with_screen_reader,
            tr("Speak through the screen reader"),
        );
        widgets::row_note(
            ui,
            tr(
                "Instead of its own voice, Muddie hands each line to the screen reader, which also shows it in braille.",
            ),
        );
    });
}
