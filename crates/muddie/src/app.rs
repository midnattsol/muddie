//! The window: the menu bar with the session tabs, the active session, the
//! status bar, and the settings shared by every session.

use std::path::PathBuf;

use eframe::egui::{
    self, Color32, CornerRadius, KeyboardShortcut, Modifiers, Panel, RichText, Sense, Stroke, Ui,
};

use crate::i18n::{self, tr, trf};
use crate::keypad;
use crate::profile::Profiles;
use crate::settings::{Language, Settings, ThemeName};
use crate::tab::{Startup, Tab};
use crate::theme::{self, Weight};
use crate::widgets;

const NEW_SESSION: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::T);
const CLOSE_SESSION: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::W);

pub struct App {
    tabs: Vec<Tab>,
    active: usize,
    settings: Settings,
    profiles: Option<Profiles>,
    /// Scripts given on the command line, run in every session.
    extra_scripts: Vec<PathBuf>,
    show_settings: bool,
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
        i18n::set_language(settings.language);
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

    /// Opens the settings window, to look at it.
    #[cfg(test)]
    pub fn open_settings(&mut self) {
        self.show_settings = true;
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

    /// The bar on top: the menus, then one tab per session.
    fn top_bar(&mut self, ui: &mut Ui) {
        let theme = theme::current();
        let mut close = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            // The menus are plain words until pointed at.
            ui.scope(|ui| {
                ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
                let widgets = &mut ui.visuals_mut().widgets;
                widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                widgets.inactive.bg_stroke = Stroke::NONE;
                widgets.inactive.fg_stroke = Stroke::new(1.0, theme.text_dim);
                self.menus(ui);
            });
            ui.add_space(10.0);
            let (line, _) = ui.allocate_exact_size(egui::vec2(1.0, 16.0), Sense::hover());
            ui.painter().vline(
                line.center().x,
                line.y_range(),
                Stroke::new(1.0, theme.border),
            );
            ui.add_space(10.0);
            for (index, tab) in self.tabs.iter().enumerate() {
                let active = index == self.active;
                let title = tab.title();
                let (fill, colour) = if active {
                    (theme.raised, theme.text)
                } else {
                    (Color32::TRANSPARENT, theme.text_dim)
                };
                let frame = egui::Frame::new()
                    .fill(fill)
                    .corner_radius(CornerRadius::same(7))
                    .inner_margin(egui::Margin::symmetric(11, 4));
                let selected = frame
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 7.0;
                        if tab.has_unread() && !active {
                            widgets::status_dot(ui, theme.accent);
                        }
                        let weight = if active {
                            Weight::Medium
                        } else {
                            Weight::Regular
                        };
                        let label = RichText::new(title.as_str())
                            .font(theme::font(weight, 13.0))
                            .color(colour);
                        let select = ui.add(
                            egui::Label::new(label)
                                .selectable(false)
                                .sense(Sense::click()),
                        );
                        let cross = RichText::new("×")
                            .font(theme::font(Weight::Regular, 14.0))
                            .color(theme.text_faint);
                        let closing = ui
                            .add(
                                egui::Button::new(cross)
                                    .frame(false)
                                    .min_size(egui::vec2(14.0, 14.0)),
                            )
                            .on_hover_text(tr("Close session"));
                        ui.ctx().accesskit_node_builder(closing.id, |node| {
                            node.set_label(format!("{} {title}", tr("Close session")));
                        });
                        if closing.clicked() {
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
                .add(egui::Button::new(plus).frame(false))
                .on_hover_text(tr("New session"));
            ui.ctx().accesskit_node_builder(add.id, |node| {
                node.set_label(tr("New session"));
            });
            if add.clicked() {
                self.new_tab(ui.ctx());
            }
        });
        if let Some(index) = close {
            self.close_tab(index, ui.ctx());
        }
    }

    fn menus(&mut self, ui: &mut Ui) {
        let active = self.active;
        ui.menu_button(tr("Session"), |ui| {
            if ui
                .add(
                    egui::Button::new(tr("New session"))
                        .shortcut_text(ui.ctx().format_shortcut(&NEW_SESSION)),
                )
                .clicked()
            {
                self.new_tab(ui.ctx());
                ui.close();
            }
            if ui
                .add(
                    egui::Button::new(tr("Close session"))
                        .shortcut_text(ui.ctx().format_shortcut(&CLOSE_SESSION)),
                )
                .clicked()
            {
                self.close_tab(self.active, ui.ctx());
                ui.close();
            }
            ui.separator();
            if let Some(tab) = self.tabs.get_mut(active) {
                tab.session_menu(ui);
            }
        });
        ui.menu_button(tr("View"), |ui| {
            self.tabs[active].view_menu(ui);
            ui.separator();
            if ui.button(tr("Settings…")).clicked() {
                self.show_settings = true;
                ui.close();
            }
        });
        ui.menu_button(tr("Automation"), |ui| self.tabs[active].automation_menu(ui));
        ui.menu_button(tr("Map"), |ui| self.tabs[active].map_menu(ui));
        ui.menu_button(tr("Help"), |ui| {
            if ui.button(tr("Keyboard shortcuts")).clicked() {
                self.show_shortcuts = true;
                ui.close();
            }
            if ui.button(tr("About muddie")).clicked() {
                self.show_about = true;
                ui.close();
            }
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let before = self.settings.clone();
        let mut open = self.show_settings;
        egui::Window::new(tr("Settings"))
            .open(&mut open)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(430.0);
                // On a small window the settings scroll rather than spill.
                let room = ctx.content_rect().height() - 150.0;
                egui::ScrollArea::vertical()
                    .max_height(room)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 9.0;
                        let settings = &mut self.settings;
                        widgets::caption(ui, tr("Appearance"));
                        let row =
                        |ui: &mut Ui,
                         label: &str,
                         control: &mut dyn FnMut(&mut Ui) -> egui::Response| {
                            ui.horizontal(|ui| {
                                let name = ui.label(label);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        control(ui).labelled_by(name.id);
                                    },
                                );
                            });
                        };
                        row(ui, tr("Theme"), &mut |ui| {
                            let name = |theme: ThemeName| match theme {
                                ThemeName::Dark => tr("Dark"),
                                ThemeName::Light => tr("Light"),
                                ThemeName::Contrast => tr("High contrast"),
                            };
                            egui::ComboBox::from_id_salt("theme")
                                .selected_text(name(settings.theme))
                                .width(170.0)
                                .show_ui(ui, |ui| {
                                    for theme in
                                        [ThemeName::Dark, ThemeName::Light, ThemeName::Contrast]
                                    {
                                        ui.selectable_value(
                                            &mut settings.theme,
                                            theme,
                                            name(theme),
                                        );
                                    }
                                })
                                .response
                        });
                        row(ui, tr("Language"), &mut |ui| {
                            let name = |language: Language| match language {
                                Language::Auto => tr("System"),
                                Language::English => "English",
                                Language::Spanish => "Español",
                            };
                            egui::ComboBox::from_id_salt("language")
                                .selected_text(name(settings.language))
                                .width(170.0)
                                .show_ui(ui, |ui| {
                                    for language in
                                        [Language::Auto, Language::English, Language::Spanish]
                                    {
                                        ui.selectable_value(
                                            &mut settings.language,
                                            language,
                                            name(language),
                                        );
                                    }
                                })
                                .response
                        });
                        row(ui, tr("Text size"), &mut |ui| {
                            ui.add(
                                egui::Slider::new(&mut settings.font_size, 10.0..=28.0)
                                    .step_by(0.5),
                            )
                        });
                        row(ui, tr("Font file"), &mut |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut settings.font_file)
                                    .hint_text(tr("Built-in font"))
                                    .margin(egui::Margin::symmetric(9, 6))
                                    .desired_width(230.0),
                            )
                        });
                        Tab::settings_ui(ui, settings);
                        if let Some(error) = &self.settings_error {
                            ui.colored_label(theme::current().danger, error);
                        }
                    });
            });
        self.show_settings = open;
        if self.settings != before {
            self.apply_settings(ctx);
        }
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        let theme = theme::current();
        egui::Window::new(tr("Keyboard shortcuts"))
            .open(&mut self.show_shortcuts)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(430.0);
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
            });
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let theme = theme::current();
        egui::Window::new(tr("About muddie"))
            .open(&mut self.show_about)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_width(320.0);
                ui.label(
                    RichText::new("muddie")
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
            });
    }

    /// Puts changed settings into effect and saves them.
    fn apply_settings(&mut self, ctx: &egui::Context) {
        i18n::set_language(self.settings.language);
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
        let theme = theme::current();
        let hairline = Stroke::new(1.0, theme.border);
        let chrome = |vertical: i8| {
            egui::Frame::new()
                .fill(theme.surface)
                .inner_margin(egui::Margin::symmetric(10, vertical))
        };
        let top = Panel::top("top bar")
            .frame(chrome(5))
            .show_separator_line(false)
            .show(ui, |ui| self.top_bar(ui));
        ui.painter().hline(
            top.response.rect.x_range(),
            top.response.rect.bottom(),
            hairline,
        );
        let active = self.active;
        let bottom = Panel::bottom("status bar")
            .frame(chrome(4))
            .show_separator_line(false)
            .show(ui, |ui| self.tabs[active].status_bar(ui));
        ui.painter().hline(
            bottom.response.rect.x_range(),
            bottom.response.rect.top(),
            hairline,
        );

        self.settings_window(ui.ctx());
        self.shortcuts_window(ui.ctx());
        self.about_window(ui.ctx());
        // Keypad presses go to the session on screen.
        let pressed = std::mem::take(&mut *self.keypad.lock().expect("keypad lock"));
        self.tabs[active].keypad_pressed.extend(pressed);
        self.tabs[active].show(ui, &self.settings);
    }
}
