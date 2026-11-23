use super::*;

/// The pages of the settings, as tabs along their top: Muddie's own, the
/// same for every profile. Each profile's are in the Profiles window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    General,
    Appearance,
    Text,
    Speech,
}

impl Section {
    pub const ALL: [Section; 4] = [
        Section::General,
        Section::Appearance,
        Section::Text,
        Section::Speech,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Section::General => tr("General"),
            Section::Appearance => tr("Appearance"),
            Section::Text => tr("Text"),
            Section::Speech => tr("Speech"),
        }
    }
}

impl App {
    /// Opens the settings at a page.
    pub fn open_settings(&mut self, section: Section) {
        self.show_settings = true;
        self.settings_section = section;
    }

    /// The settings: a tab for each page along the top, and the page
    /// under them.
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
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
                        widgets::keep_clear_of_bar(ui);
                        ui.spacing_mut().item_spacing.y = size::GAP_S;
                        self.settings_page(ui);
                        if let Some(error) = &self.settings_error {
                            widgets::problem_note(ui, error);
                        }
                    });
            },
        );
        self.show_settings = open;
        if self.settings != before {
            self.apply_settings(ctx);
        }
    }

    /// The page chosen.
    fn settings_page(&mut self, ui: &mut Ui) {
        match self.settings_section {
            Section::General => {
                let languages = self.languages.get_or_insert_with(i18n::languages).clone();
                general_settings(ui, &mut self.settings, &languages);
            }
            Section::Appearance => appearance_settings(ui, &mut self.settings),
            Section::Text => text_settings(ui, &mut self.settings),
            Section::Speech => {
                let through = self.tabs[self.active].speech_through();
                speech_settings(ui, &mut self.settings, through.as_deref());
            }
        }
    }
}

fn general_settings(ui: &mut Ui, settings: &mut Settings, languages: &[(String, String)]) {
    widgets::group(ui, |ui| {
        widgets::setting_row(ui, tr("Language"), None, &mut |ui| {
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
                .show_ui(ui, |ui| {
                    for (code, name) in &choices {
                        if crate::widgets::choice(ui, *code == current, name).clicked() {
                            settings.language.clone_from(code);
                        }
                    }
                })
                .response
        });
        widgets::setting_row(ui, tr("Interface scale"), None, &mut |ui| {
            let percent = (settings.ui_scale * 100.0).round() as u32;
            let steps = [75, 90, 100, 110, 125, 150, 175, 200];
            crate::widgets::dropdown("interface scale")
                .selected_text(format!("{percent} %"))
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
    });
}

/// The theme of the interface; and the text of the MUD: its colours, its
/// font, its size and its spacing, with a sample of it.
fn appearance_settings(ui: &mut Ui, settings: &mut Settings) {
    widgets::section(ui, tr("Interface"), |ui| {
        widgets::setting_row(ui, tr("Theme"), None, &mut |ui| {
            theme_choice(ui, &mut settings.theme)
        });
    });
    widgets::section(ui, tr("MUD text"), |ui| {
        widgets::setting_row(ui, tr("Palette"), None, &mut |ui| {
            palette_choice(ui, &mut settings.mud_palette)
        });
        widgets::setting_row(ui, tr("Font"), None, &mut |ui| font_choice(ui, settings));
        widgets::setting_row(ui, tr("Size"), None, &mut |ui| {
            let sizes = (10..=28).map(|size| size as f32);
            pick(ui, "font size", &mut settings.font_size, sizes, |size| {
                format!("{size} pt")
            })
        });
        widgets::setting_row(ui, tr("Line spacing"), None, &mut |ui| {
            let steps = [0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.8];
            pick(
                ui,
                "line spacing",
                &mut settings.line_spacing,
                steps,
                |step| format!("{step:.1}"),
            )
        });
        widgets::row_divider(ui);
        ui.add_space(size::GAP_M);
        sample(ui);
        ui.add_space(size::GAP_M);
    });
}

/// The theme, among those there are.
fn theme_choice(ui: &mut Ui, chosen: &mut ThemeName) -> egui::Response {
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
        .selected_text(name(*chosen))
        .show_ui(ui, |ui| {
            for theme in themes {
                crate::widgets::choice_value(ui, chosen, theme, name(theme));
            }
        })
        .response
}

/// The colours of the MUD's text, whatever the theme.
fn palette_choice(ui: &mut Ui, chosen: &mut MudPalette) -> egui::Response {
    let name = |palette: MudPalette| match palette {
        MudPalette::Vivid => tr("Vivid"),
        MudPalette::Soft => tr("Soft (follows the theme)"),
        MudPalette::Classic => tr("Classic terminal"),
    };
    crate::widgets::dropdown("mud palette")
        .selected_text(name(*chosen))
        .show_ui(ui, |ui| {
            for palette in [MudPalette::Vivid, MudPalette::Soft, MudPalette::Classic] {
                crate::widgets::choice_value(ui, chosen, palette, name(palette));
            }
        })
        .response
}

/// The font of the MUD's text: one that comes with Muddie, or one of the
/// system's monospaced fonts.
fn font_choice(ui: &mut Ui, settings: &mut Settings) -> egui::Response {
    // What the settings hold is a name of ours or a family's.
    let shown = match settings.font_family.strip_prefix("muddie:") {
        _ if settings.font_family.is_empty() => theme::DEFAULT_FONT,
        Some(bundled) => bundled,
        None => settings.font_family.as_str(),
    }
    .to_owned();
    crate::widgets::dropdown("font")
        .selected_text(shown)
        .height(340.0)
        .show_ui(ui, |ui| {
            bundled_fonts(ui, settings);
            ui.separator();
            system_fonts(ui, settings);
        })
        .response
}

/// A heading among the fonts.
fn font_heading(ui: &mut Ui, text: &str) {
    let small = RichText::new(text)
        .font(theme::font(Weight::Regular, 11.5))
        .color(theme::current().text_faint);
    ui.add(egui::Label::new(small).selectable(false));
}

/// The fonts that come with Muddie.
fn bundled_fonts(ui: &mut Ui, settings: &mut Settings) {
    font_heading(ui, tr("Bundled"));
    if crate::widgets::choice(ui, settings.font_family.is_empty(), theme::DEFAULT_FONT).clicked() {
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
}

/// The monospaced fonts the system has, once they have been looked for.
fn system_fonts(ui: &mut Ui, settings: &mut Settings) {
    font_heading(ui, tr("System fonts"));
    let Some(installed) = theme::installed_fonts() else {
        font_heading(ui, tr("Loading…"));
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(150));
        return;
    };
    for font in installed {
        if crate::widgets::choice(ui, settings.font_family == font.family, &font.family).clicked() {
            // Where it is goes with its name, so that it is found at once
            // the next time.
            settings.font_family.clone_from(&font.family);
            (settings.font_file, settings.font_index) = font.place();
        }
    }
}

/// What the output will look like.
fn sample(ui: &mut Ui) {
    let sample = "\x1b[1;32mPlaza Mayor\x1b[0m\nUna plaza amplia con una fuente. Al norte, la \x1b[33mtorre\x1b[0m.\n\x1b[36mSalidas: norte, este, sur\x1b[0m  0O 1lI {}[] <=>\n\x1b[31mrojo \x1b[32mverde \x1b[33mamarillo \x1b[34mazul \x1b[35mmagenta \x1b[36mcian \x1b[37mblanco \x1b[1;30mgris\x1b[0m\n\x1b[1;31mrojo \x1b[1;32mverde \x1b[1;33mamarillo \x1b[1;34mazul \x1b[1;35mmagenta \x1b[1;36mcian \x1b[1;37mblanco\x1b[0m";
    let well = egui::Frame::new()
        .fill(theme::current().background)
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
        widgets::setting_row(
            ui,
            tr("Scrollback"),
            Some(tr("Lines kept to scroll back through and search.")),
            &mut |ui| {
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
            },
        );
        widgets::switch_setting(ui, &mut settings.timestamps, tr("Timestamps"), None);
        widgets::switch_setting(
            ui,
            &mut settings.keep_command,
            tr("Keep the sent command"),
            Some(tr("Enter sends it again; typing replaces it.")),
        );
    });
}

/// The speed of the voice along a line, slow at the left and fast at the
/// right, as wide as any field with its two words.
fn speed_control(ui: &mut Ui, rate: &mut i32) -> egui::Response {
    let theme = theme::current();
    let font = theme::font(Weight::Regular, 12.0);
    let word = |ui: &mut Ui, text: &str| {
        ui.label(RichText::new(text).font(font.clone()).color(theme.text_dim))
            .rect
            .width()
    };
    let gap = ui.spacing().item_spacing.x;
    let fast = word(ui, tr("Fast"));
    let slow = ui
        .painter()
        .layout_no_wrap(tr("Slow").to_owned(), font.clone(), theme.text_dim)
        .size()
        .x;
    ui.spacing_mut().slider_width = (size::FIELD_L - fast - slow - 2.0 * gap).max(60.0);
    let response = ui.add(crate::widgets::slider(rate, -100..=100).show_value(false));
    word(ui, tr("Slow"));
    response
}

/// What speaks, and how: through the screen reader in use, which also
/// reaches a braille display, or with the system's voice, whose speed and
/// language are set here; and what is said.
fn speech_settings(ui: &mut Ui, settings: &mut Settings, through: Option<&str>) {
    use crate::settings::SpeechOutput;
    widgets::section(ui, tr("Speaking"), |ui| {
        // What speaks now; a screen reader speaks as it is set up to,
        // which is said here, where the keyboard reaches it.
        let now = match (through, settings.speech) {
            (Some(name), SpeechOutput::Reader) => {
                trf("Active voice: {0}, with its own speed and voice.", &[name])
            }
            (Some(name), _) => trf("Active voice: {0}.", &[name]),
            (None, _) => tr("No voice is active.").to_owned(),
        };
        widgets::setting_row(ui, tr("Speak with"), Some(&now), &mut |ui| {
            let choices = [
                (SpeechOutput::Reader, tr("The screen reader").to_owned()),
                (SpeechOutput::Voice, tr("The system's voice").to_owned()),
                (SpeechOutput::Off, tr("Nothing").to_owned()),
            ];
            widgets::choose(ui, "speech output", &mut settings.speech, &choices, |_| {
                String::new()
            })
        });
        widgets::switch_setting(
            ui,
            &mut settings.speech_interrupts,
            tr("Interrupt with new lines"),
            Some(tr(
                "Each new line interrupts what is being read. Useful in fast games.",
            )),
        );
        widgets::switch_setting(
            ui,
            &mut settings.speech_channels,
            tr("Read the channels"),
            Some(tr(
                "What is said on the MUD's channels, with the channel's name.",
            )),
        );
    });
    // These are for the system's voice alone.
    let voice = settings.speech == SpeechOutput::Voice;
    widgets::section(ui, tr("The system's voice"), |ui| {
        ui.add_enabled_ui(voice, |ui| {
            widgets::setting_row(ui, tr("Speed"), None, &mut |ui| {
                speed_control(ui, &mut settings.speech_rate)
            });
            widgets::setting_row(ui, tr("Voice language"), None, &mut |ui| {
                language_choice(ui, &mut settings.speech_language)
            });
        });
    });
}

/// The language the voice speaks in, of those most asked for, or the
/// system's.
fn language_choice(ui: &mut Ui, language: &mut String) -> egui::Response {
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
    let current = language.trim().to_lowercase();
    let shown = (languages.iter())
        .find(|(code, _)| *code == current)
        .map_or(current.clone(), |(_, name)| (*name).to_owned());
    crate::widgets::dropdown("speech language")
        .selected_text(shown)
        .height(320.0)
        .show_ui(ui, |ui| {
            for (code, name) in languages {
                if crate::widgets::choice(ui, code == current, name).clicked() {
                    *language = code.to_owned();
                }
            }
        })
        .response
}
