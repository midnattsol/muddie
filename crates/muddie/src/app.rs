//! The window: a strip of session tabs, the active session, and the
//! settings shared by all of them.

use std::path::PathBuf;

use eframe::egui::{
    self, Align, Color32, CornerRadius, Layout, Panel, RichText, Sense, Stroke, Ui,
};

use crate::i18n::{self, tr, trf};
use crate::profile::Profiles;
use crate::settings::{Language, Settings, ThemeName};
use crate::tab::{Startup, Tab};
use crate::theme;

pub struct App {
    tabs: Vec<Tab>,
    active: usize,
    settings: Settings,
    profiles: Option<Profiles>,
    /// Scripts given on the command line, run in every session.
    extra_scripts: Vec<PathBuf>,
    show_settings: bool,
    /// A problem with the settings to show in the settings window.
    settings_error: Option<String>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup) -> Self {
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
            settings_error,
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

    /// The strip on top: one tab per session, a button for a new one, and
    /// the settings button at the far end.
    fn tab_strip(&mut self, ui: &mut Ui) {
        let theme = theme::current();
        let mut close = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (index, tab) in self.tabs.iter().enumerate() {
                let active = index == self.active;
                let title = tab.title();
                let colour = if active { theme.text } else { theme.text_dim };
                let mut label = RichText::new(title.as_str()).color(colour);
                if tab.has_unread() && !active {
                    label = RichText::new(format!("{title} •")).color(theme.accent);
                }
                let fill = if active {
                    theme.background
                } else {
                    Color32::TRANSPARENT
                };
                let frame = egui::Frame::new()
                    .fill(fill)
                    .stroke(Stroke::new(
                        1.0,
                        if active {
                            theme.border
                        } else {
                            Color32::TRANSPARENT
                        },
                    ))
                    .corner_radius(CornerRadius::same(7))
                    .inner_margin(egui::Margin::symmetric(10, 4));
                let response = frame
                    .show(ui, |ui| {
                        let select = ui.add(
                            egui::Label::new(label)
                                .selectable(false)
                                .sense(Sense::click()),
                        );
                        let closing = ui
                            .add(
                                egui::Button::new(RichText::new("×").color(theme.text_dim))
                                    .frame(false),
                            )
                            .on_hover_text(tr("Close"));
                        ui.ctx().accesskit_node_builder(closing.id, |node| {
                            node.set_label(format!("{} {title}", tr("Close")));
                        });
                        if closing.clicked() {
                            close = Some(index);
                        }
                        select.clicked()
                    })
                    .inner;
                if response {
                    self.active = index;
                }
            }
            let add = ui
                .add(egui::Button::new("+").frame(false))
                .on_hover_text(tr("New session"));
            ui.ctx().accesskit_node_builder(add.id, |node| {
                node.set_label(tr("New session"));
            });
            if add.clicked() {
                self.new_tab(ui.ctx());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(tr("Settings")).frame(false))
                    .clicked()
                {
                    self.show_settings = !self.show_settings;
                }
            });
        });
        if let Some(index) = close {
            self.close_tab(index, ui.ctx());
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let before = self.settings.clone();
        let mut open = self.show_settings;
        egui::Window::new(tr("Settings"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                let settings = &mut self.settings;
                ui.label(RichText::new(tr("Appearance")).strong());
                egui::Grid::new("appearance")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        let label = ui.label(tr("Theme"));
                        let name = |theme: ThemeName| match theme {
                            ThemeName::Dark => tr("Dark"),
                            ThemeName::Light => tr("Light"),
                            ThemeName::Contrast => tr("High contrast"),
                        };
                        egui::ComboBox::from_id_salt("theme")
                            .selected_text(name(settings.theme))
                            .show_ui(ui, |ui| {
                                for theme in
                                    [ThemeName::Dark, ThemeName::Light, ThemeName::Contrast]
                                {
                                    ui.selectable_value(&mut settings.theme, theme, name(theme));
                                }
                            })
                            .response
                            .labelled_by(label.id);
                        ui.end_row();

                        let label = ui.label(tr("Language"));
                        let name = |language: Language| match language {
                            Language::Auto => tr("System"),
                            Language::English => "English",
                            Language::Spanish => "Español",
                        };
                        egui::ComboBox::from_id_salt("language")
                            .selected_text(name(settings.language))
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
                            .labelled_by(label.id);
                        ui.end_row();

                        let label = ui.label(tr("Text size"));
                        ui.add(
                            egui::Slider::new(&mut settings.font_size, 10.0..=28.0).step_by(0.5),
                        )
                        .labelled_by(label.id);
                        ui.end_row();

                        let label = ui.label(tr("Font file"));
                        ui.add(
                            egui::TextEdit::singleline(&mut settings.font_file)
                                .hint_text(tr("Built-in font"))
                                .desired_width(220.0),
                        )
                        .labelled_by(label.id);
                        ui.end_row();
                    });
                Tab::settings_ui(ui, settings);
                if let Some(error) = &self.settings_error {
                    ui.colored_label(theme::current().terminal[1], error);
                }
            });
        self.show_settings = open;
        if self.settings != before {
            self.apply_settings(ctx);
        }
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
        let theme = theme::current();
        let strip = egui::Frame::new()
            .fill(theme.surface)
            .inner_margin(egui::Margin::symmetric(8, 5));
        Panel::top("tabs")
            .frame(strip)
            .show_separator_line(false)
            .show(ui, |ui| self.tab_strip(ui));
        self.settings_window(ui.ctx());
        let active = self.active;
        self.tabs[active].show(ui, &self.settings);
    }
}
