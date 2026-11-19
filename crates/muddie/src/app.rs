//! The window: the menu bar with the session tabs, the active session, the
//! status bar, and the settings shared by every session.

use std::path::PathBuf;

use eframe::egui::{
    self, Color32, CornerRadius, KeyboardShortcut, Modifiers, Panel, RichText, Sense, Stroke, Ui,
};

use crate::i18n::{self, tr, trf};
use crate::keypad;
use crate::profile::Profiles;
use crate::settings::{MudPalette, Settings, ThemeName};
use crate::tab::{Startup, Tab};
use crate::theme::{self, Weight};
use crate::widgets;

const NEW_SESSION: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::T);
const CLOSE_SESSION: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::W);
const SWITCH_VIEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::E);

pub struct App {
    tabs: Vec<Tab>,
    active: usize,
    settings: Settings,
    profiles: Option<Profiles>,
    /// Scripts given on the command line, run in every session.
    extra_scripts: Vec<PathBuf>,
    show_settings: bool,
    /// The section of the settings on screen.
    settings_section: usize,
    show_shortcuts: bool,
    show_about: bool,
    /// A problem with the settings to show in the settings window.
    settings_error: Option<String>,
    /// Keypad keys noted by the window layer, waiting for the next frame.
    keypad: keypad::Pressed,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        startup: Startup,
        keypad: keypad::Pressed,
    ) -> Self {
        let profiles = startup.profiles.clone();
        let loaded = match &profiles {
            Some(profiles) => Settings::load(&profiles.settings_path()),
            None => Ok(Settings::default()),
        };
        let (settings, mut settings_error) = match loaded {
            Ok(settings) => (settings, None),
            Err(error) => (
                Settings::default(),
                Some(trf("Could not load the settings: {0}", &[&error])),
            ),
        };
        if let Some(profiles) = &profiles {
            i18n::set_user_dir(profiles.locale_dir());
        }
        i18n::set_language(&settings.language);
        if let Err(error) = theme::apply(&cc.egui_ctx, &settings) {
            settings_error = Some(error);
        }
        let extra_scripts = startup.scripts.clone();
        let mut first = Tab::new(&cc.egui_ctx, startup, &settings);
        if let Some(error) = &settings_error {
            first.note(error);
        }
        Self {
            tabs: vec![first],
            active: 0,
            settings,
            profiles,
            extra_scripts,
            show_settings: false,
            settings_section: 0,
            show_shortcuts: false,
            show_about: false,
            settings_error,
            keypad,
        }
    }

    #[cfg(test)]
    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    #[cfg(test)]
    pub fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    /// Switches theme as the settings window would, to look at it.
    #[cfg(test)]
    pub fn use_theme(&mut self, ctx: &egui::Context, theme: ThemeName) {
        self.settings.theme = theme;
        self.apply_settings(ctx);
    }

    /// Switches the output's font as the settings window would.
    #[cfg(test)]
    pub fn use_font(&mut self, ctx: &egui::Context, family: &str) {
        self.settings.font_family = family.to_owned();
        self.apply_settings(ctx);
    }

    /// Opens the settings window, to look at it.
    #[cfg(test)]
    pub fn open_settings(&mut self, section: usize) {
        self.show_settings = true;
        self.settings_section = section;
    }

    fn new_tab(&mut self, ctx: &egui::Context) {
        let startup = Startup {
            profiles: self.profiles.clone(),
            profile: None,
            target: None,
            speech: false,
            scripts: self.extra_scripts.clone(),
        };
        self.tabs.push(Tab::new(ctx, startup, &self.settings));
        self.active = self.tabs.len() - 1;
    }

    fn close_tab(&mut self, index: usize, ctx: &egui::Context) {
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.new_tab(ctx);
        }
        if self.active > index || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
    }

    /// The bar on top: the menus.
    fn top_bar(&mut self, ui: &mut Ui) {
        let theme = theme::current();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().button_padding = egui::vec2(9.0, 4.0);
            let plain = theme::font(Weight::Regular, 13.5);
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Button, plain);
            // The menus are plain words until pointed at.
            let visuals = &mut ui.visuals_mut().widgets;
            visuals.inactive.weak_bg_fill = Color32::TRANSPARENT;
            visuals.inactive.bg_stroke = Stroke::NONE;
            visuals.inactive.fg_stroke = Stroke::new(1.0, theme.text_dim);
            visuals.hovered.bg_stroke = Stroke::NONE;
            visuals.open.bg_stroke = Stroke::NONE;
            visuals.active.bg_stroke = Stroke::NONE;
            self.menus(ui);
            // The session's views, at the far end.
            let active = self.active;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let tab = &mut self.tabs[active];
                let current = usize::from(tab.automation_open());
                match widgets::segmented(ui, &[tr("Game"), tr("Editor")], current) {
                    Some(0) => tab.close_automation(),
                    Some(_) => tab.reopen_automation(),
                    None => {}
                }
            });
        });
    }

    /// The bar at the bottom: one tab per session, each with a dot that
    /// says how its connection is doing, then what the session on screen
    /// is up to.
    fn bottom_bar(&mut self, ui: &mut Ui) {
        let theme = theme::current();
        let mut close = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (index, tab) in self.tabs.iter().enumerate() {
                let active = index == self.active;
                let title = tab.title();
                let (dot, state) = tab.status();
                let fill = if active {
                    theme.raised
                } else {
                    Color32::TRANSPARENT
                };
                let frame = egui::Frame::new()
                    .fill(fill)
                    .corner_radius(CornerRadius::same(7))
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 5,
                        top: 3,
                        bottom: 3,
                    });
                let selected = frame
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 7.0;
                        widgets::status_dot(ui, dot);
                        let unread = tab.has_unread() && !active;
                        let colour = match (active, unread) {
                            (true, _) => theme.text,
                            (false, true) => theme.accent,
                            (false, false) => theme.text_dim,
                        };
                        let weight = if active || unread {
                            Weight::Medium
                        } else {
                            Weight::Regular
                        };
                        let label = RichText::new(title.as_str())
                            .font(theme::font(weight, 12.5))
                            .color(colour);
                        let select = ui
                            .add(
                                egui::Label::new(label)
                                    .selectable(false)
                                    .sense(Sense::click()),
                            )
                            .on_hover_text(state.as_str());
                        ui.ctx().accesskit_node_builder(select.id, |node| {
                            node.set_role(egui::accesskit::Role::Tab);
                            node.set_selected(active);
                            node.set_label(format!("{title}, {state}"));
                        });
                        let closing = format!("{} {title}", tr("Close session"));
                        if widgets::close_button(ui, &closing, 18.0).clicked() {
                            close = Some(index);
                        }
                        select.clicked()
                    })
                    .inner;
                if selected {
                    self.active = index;
                }
            }
            let plus = RichText::new("+")
                .font(theme::font(Weight::Regular, 16.0))
                .color(theme.text_dim);
            let add = ui
                .add(
                    egui::Button::new(plus)
                        .frame(false)
                        .min_size(egui::vec2(24.0, 24.0)),
                )
                .on_hover_text(tr("New session"));
            ui.ctx().accesskit_node_builder(add.id, |node| {
                node.set_label(tr("New session"));
            });
            if add.clicked() {
                self.new_tab(ui.ctx());
            }
            let active = self.active;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.tabs[active].status_line(ui);
            });
        });
        if let Some(index) = close {
            self.close_tab(index, ui.ctx());
        }
    }

    fn menus(&mut self, ui: &mut Ui) {
        let active = self.active;
        ui.menu_button(tr("Session"), |ui| {
            widgets::menu_width(ui);
            let shortcut = ui.ctx().format_shortcut(&NEW_SESSION);
            if widgets::menu_item(ui, tr("New session"), &shortcut).clicked() {
                self.new_tab(ui.ctx());
                ui.close();
            }
            let shortcut = ui.ctx().format_shortcut(&CLOSE_SESSION);
            if widgets::menu_item(ui, tr("Close session"), &shortcut).clicked() {
                self.close_tab(self.active, ui.ctx());
                ui.close();
            }
            widgets::menu_separator(ui);
            if let Some(tab) = self.tabs.get_mut(active) {
                tab.session_menu(ui);
            }
        });
        ui.menu_button(tr("Preferences"), |ui| {
            widgets::menu_width(ui);
            self.tabs[active].view_menu(ui);
            widgets::menu_separator(ui);
            if widgets::menu_item(ui, tr("Settings…"), "").clicked() {
                self.show_settings = true;
                ui.close();
            }
        });
        ui.menu_button(tr("Help"), |ui| {
            widgets::menu_width(ui);
            if widgets::menu_item(ui, tr("Keyboard shortcuts"), "").clicked() {
                self.show_shortcuts = true;
                ui.close();
            }
            if widgets::menu_item(ui, tr("About Muddie"), "").clicked() {
                self.show_about = true;
                ui.close();
            }
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        const GENERAL: usize = 0;
        const APPEARANCE: usize = 1;
        const PLUGINS: usize = 5;
        let before = self.settings.clone();
        let mut open = self.show_settings;
        widgets::dialog(ctx, "settings", tr("Settings"), &mut open, 760.0, |ui| {
            ui.horizontal_top(|ui| {
                // The sections on the left; more can be added as settings grow.
                ui.vertical(|ui| {
                    ui.set_width(170.0);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    let sections = [
                        tr("General"),
                        tr("Appearance"),
                        tr("Output"),
                        tr("Sessions"),
                        tr("Speech"),
                        tr("Plugins"),
                    ];
                    for (index, name) in sections.into_iter().enumerate() {
                        let selected = self.settings_section == index;
                        if widgets::nav_item(ui, selected, name, None).clicked() {
                            self.settings_section = index;
                        }
                    }
                });
                ui.add_space(14.0);
                let section = self.settings_section;
                let active = self.active;
                ui.vertical(|ui| {
                    ui.set_min_height(380.0);
                    // On a small window the settings scroll rather than spill.
                    let room = ctx.content_rect().height() - 150.0;
                    egui::ScrollArea::vertical()
                        .max_height(room)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 9.0;
                            let settings = &mut self.settings;
                            match section {
                                GENERAL => general_settings(ui, settings),
                                APPEARANCE => appearance_settings(ui, settings),
                                PLUGINS => self.tabs[active].plugins_ui(ui),
                                // Those of the sessions: output, sessions, speech.
                                section => Tab::settings_ui(ui, settings, section - 1),
                            }
                            if let Some(error) = &self.settings_error {
                                ui.colored_label(theme::current().danger, error);
                            }
                        });
                });
            });
        });
        self.show_settings = open;
        if self.settings != before {
            self.apply_settings(ctx);
        }
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        let theme = theme::current();
        widgets::dialog(
            ctx,
            "keyboard shortcuts",
            tr("Keyboard shortcuts"),
            &mut self.show_shortcuts,
            430.0,
            |ui| {
                let groups: [(&str, &[(&str, &str)]); 3] = [
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
                            ("Alt+Shift+↑ …", tr("The same, for the chat")),
                            ("Alt+0", tr("Read the gauges")),
                        ],
                    ),
                    (
                        tr("Sessions"),
                        &[
                            ("Ctrl+T", tr("New session")),
                            ("Ctrl+W", tr("Close session")),
                            ("Ctrl+E", tr("Switch between the game and the editor")),
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

    fn about_window(&mut self, ctx: &egui::Context) {
        let theme = theme::current();
        widgets::dialog(
            ctx,
            "about muddie",
            tr("About Muddie"),
            &mut self.show_about,
            320.0,
            |ui| {
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

    /// Puts changed settings into effect and saves them.
    fn apply_settings(&mut self, ctx: &egui::Context) {
        i18n::set_language(&self.settings.language);
        self.settings_error = theme::apply(ctx, &self.settings).err();
        for tab in &mut self.tabs {
            tab.settings_changed(&self.settings);
        }
        if let Some(profiles) = &self.profiles
            && let Err(error) = self.settings.save(&profiles.settings_path())
        {
            self.settings_error = Some(trf("Could not save the settings: {0}", &[&error]));
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Every session keeps running; only the active one is drawn.
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            tab.poll(ui.ctx(), &self.settings, index == self.active);
        }
        if ui.input_mut(|input| input.consume_shortcut(&NEW_SESSION)) {
            self.new_tab(ui.ctx());
        }
        if ui.input_mut(|input| input.consume_shortcut(&CLOSE_SESSION)) {
            self.close_tab(self.active, ui.ctx());
        }
        if ui.input_mut(|input| input.consume_shortcut(&SWITCH_VIEW)) {
            let tab = &mut self.tabs[self.active];
            if tab.automation_open() {
                tab.close_automation();
            } else {
                tab.reopen_automation();
            }
        }
        let theme = theme::current();
        ui.painter().rect_filled(ui.max_rect(), 0.0, theme.surface);
        let chrome = |vertical: i8| {
            egui::Frame::new()
                .fill(theme.surface)
                .inner_margin(egui::Margin::symmetric(10, vertical))
        };
        Panel::top("top bar")
            .frame(chrome(5))
            .show_separator_line(false)
            .show(ui, |ui| self.top_bar(ui));
        let active = self.active;
        Panel::bottom("status bar")
            .frame(chrome(4))
            .show_separator_line(false)
            .show(ui, |ui| self.bottom_bar(ui));

        self.settings_window(ui.ctx());
        self.shortcuts_window(ui.ctx());
        self.about_window(ui.ctx());
        // Keypad presses go to the session on screen.
        let pressed = std::mem::take(&mut *self.keypad.lock().expect("keypad lock"));
        self.tabs[active].keypad_pressed.extend(pressed);
        self.tabs[active].show(ui, &self.settings);
        // Panels switched on or off stay that way the next time.
        let panes = self.tabs[active].pane_widths();
        // Not while something is being dragged: once it is let go.
        let settled = !ui.input(|input| input.pointer.any_down());
        if settled && panes != self.settings.automation_panes {
            self.settings.automation_panes = panes;
            if let Some(profiles) = &self.profiles
                && let Err(error) = self.settings.save(&profiles.settings_path())
            {
                self.settings_error = Some(trf("Could not save the settings: {0}", &[&error]));
            }
        }
    }
}

/// The language and the size of the interface.
fn general_settings(ui: &mut Ui, settings: &mut Settings) {
    widgets::group(ui, |ui| {
        widgets::setting_row(ui, tr("Language"), &mut |ui| {
            // The system's, English, and every catalogue there is.
            let mut choices = vec![
                (String::new(), tr("Same as the system").to_owned()),
                ("en".to_owned(), "English".to_owned()),
            ];
            choices.extend(i18n::languages());
            let current = match settings.language.as_str() {
                "auto" => "",
                "english" => "en",
                "spanish" => "es",
                code => code,
            }
            .to_owned();
            let shown = choices.iter().find(|(code, _)| *code == current);
            let shown = shown.map_or(current.clone(), |(_, name)| name.clone());
            egui::ComboBox::from_id_salt("language")
                .selected_text(shown)
                .width(210.0)
                .show_ui(ui, |ui| {
                    for (code, name) in &choices {
                        if ui.selectable_label(*code == current, name).clicked() {
                            settings.language.clone_from(code);
                        }
                    }
                })
                .response
        });
        widgets::setting_row(ui, tr("Interface size"), &mut |ui| {
            let mut percent = (settings.ui_scale * 100.0).round();
            let slider = egui::Slider::new(&mut percent, 75.0..=200.0)
                .step_by(5.0)
                .suffix(" %");
            let response = ui.add(slider);
            // Applied once let go: the slider would move under the pointer.
            if response.drag_stopped() || (response.changed() && !response.dragged()) {
                settings.ui_scale = percent / 100.0;
            }
            response
        });
    });
    let about = tr(
        "More languages can be added as gettext catalogues in the locale folder of the configuration.",
    );
    ui.label(
        RichText::new(about)
            .font(theme::font(Weight::Regular, 12.0))
            .color(theme::current().text_faint),
    );
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
            egui::ComboBox::from_id_salt("theme")
                .selected_text(name(settings.theme))
                .width(210.0)
                .show_ui(ui, |ui| {
                    for theme in themes {
                        ui.selectable_value(&mut settings.theme, theme, name(theme));
                    }
                })
                .response
        });
    });
    ui.add_space(6.0);
    widgets::caption(ui, tr("Text of the MUD"));
    widgets::group(ui, |ui| {
        widgets::setting_row(ui, tr("Colours"), &mut |ui| {
            let name = |palette: MudPalette| match palette {
                MudPalette::Vivid => tr("Vivid"),
                MudPalette::Soft => tr("Soft, to go with the theme"),
                MudPalette::Classic => tr("Classic terminal"),
            };
            egui::ComboBox::from_id_salt("mud palette")
                .selected_text(name(settings.mud_palette))
                .width(260.0)
                .show_ui(ui, |ui| {
                    for palette in [MudPalette::Vivid, MudPalette::Soft, MudPalette::Classic] {
                        ui.selectable_value(&mut settings.mud_palette, palette, name(palette));
                    }
                })
                .response
        });
        widgets::setting_row(ui, tr("Font"), &mut |ui| {
            // What the settings hold is a name of ours or a family's.
            let shown = match settings.font_family.strip_prefix("muddie:") {
                _ if settings.font_family.is_empty() => "JetBrains Mono",
                Some(bundled) => bundled,
                None => settings.font_family.as_str(),
            }
            .to_owned();
            egui::ComboBox::from_id_salt("font")
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
                    heading(ui, tr("With Muddie"));
                    if ui
                        .selectable_label(settings.font_family.is_empty(), "JetBrains Mono")
                        .clicked()
                    {
                        settings.font_family.clear();
                    }
                    for (name, _) in theme::BUNDLED_FONTS {
                        let key = theme::bundled_font(name);
                        if ui
                            .selectable_label(settings.font_family == key, name)
                            .clicked()
                        {
                            settings.font_family = key;
                        }
                    }
                    // The monospaced fonts the system has.
                    ui.separator();
                    heading(ui, tr("Installed on this computer"));
                    for font in theme::installed_fonts() {
                        let chosen = settings.font_family == font.family;
                        if ui.selectable_label(chosen, &font.family).clicked() {
                            settings.font_family.clone_from(&font.family);
                        }
                    }
                })
                .response
        });
        widgets::setting_row(ui, tr("Size"), &mut |ui| {
            ui.add(egui::Slider::new(&mut settings.font_size, 10.0..=28.0).step_by(0.5))
        });
        widgets::setting_row(ui, tr("Line spacing"), &mut |ui| {
            ui.add(egui::Slider::new(&mut settings.line_spacing, 0.9..=1.8).step_by(0.05))
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
