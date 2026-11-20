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
use crate::theme::{self, Weight, size};
use crate::widgets;

const NEW_SESSION: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::T);
const CLOSE_SESSION: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::W);
const SWITCH_VIEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::E);
const COMMANDS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::K);
const SETTINGS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::Comma);

/// The sections of the settings: Muddie's own, the same for every
/// profile, and those of the profile the session plays with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    General,
    Appearance,
    Text,
    Speech,
    Connection,
    Login,
    Plugins,
    Logs,
}

impl Section {
    pub const MUDDIE: [Section; 4] = [
        Section::General,
        Section::Appearance,
        Section::Text,
        Section::Speech,
    ];
    pub const PROFILE: [Section; 4] = [
        Section::Connection,
        Section::Login,
        Section::Plugins,
        Section::Logs,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Section::General => tr("General"),
            Section::Appearance => tr("Appearance"),
            Section::Text => tr("Text and typing"),
            Section::Speech => tr("Speech and screen reader"),
            Section::Connection => tr("Connection"),
            Section::Login => tr("Log in"),
            Section::Plugins => tr("Plugins"),
            Section::Logs => tr("Logs and sounds"),
        }
    }
}
const DEBUG_VIEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, egui::Key::D);

pub struct App {
    tabs: Vec<Tab>,
    active: usize,
    settings: Settings,
    profiles: Option<Profiles>,
    /// Scripts given on the command line, run in every session.
    extra_scripts: Vec<PathBuf>,
    show_settings: bool,
    /// Whether the last key came with Alt alone, so that its letter, if
    /// it comes a frame later, is not typed.
    alt_typing: bool,
    /// The section of the settings on screen.
    settings_section: Section,
    /// Settings changed and not yet written to their file.
    settings_unsaved: bool,
    /// Frames drawn since they were last counted, and when that was.
    frames: u32,
    traced_at: std::time::Instant,
    /// The languages there are, looked up when the settings open.
    languages: Option<Vec<(String, String)>>,
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
        // Not in tests, which read the interface in English and run side
        // by side: the language is one for the whole program.
        #[cfg(not(test))]
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
            alt_typing: false,
            settings_section: Section::General,
            settings_unsaved: false,
            frames: 0,
            traced_at: std::time::Instant::now(),
            languages: None,
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

    #[cfg(test)]
    pub fn app_settings(&self) -> &Settings {
        &self.settings
    }

    /// Opens the settings window, to look at it.
    pub fn open_settings(&mut self, section: Section) {
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
    /// The bar at the top: the profile at the left, what opens the
    /// palette in the middle, the session's views and the menu at the
    /// right.
    fn top_bar(&mut self, ui: &mut Ui) {
        let theme = theme::current();
        let full = ui.max_rect();
        let active = self.active;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().button_padding = egui::vec2(9.0, 4.0);
            let plain = theme::font(Weight::Regular, 13.5);
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Button, plain);
            let visuals = &mut ui.visuals_mut().widgets;
            visuals.inactive.weak_bg_fill = Color32::TRANSPARENT;
            visuals.inactive.bg_stroke = Stroke::NONE;
            visuals.inactive.fg_stroke = Stroke::new(1.0, theme.text_dim);
            visuals.hovered.bg_stroke = Stroke::NONE;
            visuals.open.bg_stroke = Stroke::NONE;
            visuals.active.bg_stroke = Stroke::NONE;
            // The profile this session plays with: what is saved with it
            // is opened from here. With none, the way to keep one.
            let shown = match self.tabs[active].profile_name() {
                Some(name) => widgets::chip(ui, name, tr("Profile")),
                None => widgets::chip(ui, tr("Save profile"), tr("This session has no profile")),
            };
            if shown.clicked() {
                self.open_settings(Section::Connection);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.menus(ui);
                ui.add_space(size::GAP_S);
                let tab = &mut self.tabs[active];
                let views = [tr("Game"), tr("Editor")];
                if let Some(view) = widgets::segmented(ui, &views, tab.view().min(1)) {
                    tab.set_view(view);
                }
            });
        });
        // Where anything can be found, in the middle, as wide as the
        // ends leave it.
        let width = (full.width() - 2.0 * 330.0).clamp(180.0, 380.0);
        let place = egui::Rect::from_center_size(full.center(), egui::vec2(width, 26.0));
        let keys = ui.ctx().format_shortcut(&COMMANDS);
        let search = ui.scope_builder(egui::UiBuilder::new().max_rect(place), |ui| {
            widgets::palette_button(ui, width, &keys)
        });
        if search.inner.clicked() {
            self.tabs[active].toggle_palette();
        }
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
            let add = widgets::sign_button(ui, "+", tr("New session"), false);
            if add.clicked() {
                self.new_tab(ui.ctx());
            }
            let active = self.active;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                notices(ui, &mut self.tabs[active]);
                self.tabs[active].status_line(ui);
            });
        });
        if let Some(index) = close {
            self.close_tab(index, ui.ctx());
        }
    }

    /// The one menu: what a session can do, what is shown, the
    /// settings, and help. Everything in it is in the palette too.
    fn menus(&mut self, ui: &mut Ui) {
        let active = self.active;
        let button = widgets::icon_button(ui, widgets::Icon::Menu, tr("Menu"), false);
        egui::Popup::menu(&button).show(|ui| {
            widgets::menu_width(ui);
            widgets::menu_heading(ui, tr("Session"));
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
            if let Some(tab) = self.tabs.get_mut(active) {
                tab.session_menu(ui);
                widgets::menu_separator(ui);
                widgets::menu_heading(ui, tr("Panels"));
                tab.view_menu(ui);
            }
            widgets::menu_separator(ui);
            if widgets::menu_item(ui, tr("Settings"), "Ctrl+,").clicked() {
                self.open_settings(Section::General);
                ui.close();
            }
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

    /// Does what a session asked of the window.
    fn carry_out(&mut self, request: crate::tab::AppRequest, ctx: &egui::Context) {
        use crate::tab::AppRequest;
        match request {
            AppRequest::NewSession => self.new_tab(ctx),
            AppRequest::CloseSession => self.close_tab(self.active, ctx),
            AppRequest::Settings(section) => self.open_settings(section),
            AppRequest::CloseSettings => self.show_settings = false,
            AppRequest::Shortcuts => self.show_shortcuts = true,
            AppRequest::About => self.show_about = true,
        }
    }

    /// The settings, as one window: Muddie's own at the top of its list,
    /// the profile's under them, each section with its rows explained.
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return self.tabs[self.active].profile_autosave(ctx, false);
        }
        let before = self.settings.clone();
        let mut open = true;
        let active = self.active;
        // Which there are is looked up as the settings open, not each frame.
        let languages = self.languages.get_or_insert_with(i18n::languages).clone();
        let profile = match self.tabs[active].profile_name() {
            Some(name) => trf("Profile · {0}", &[name]),
            None => tr("This session").to_owned(),
        };
        let height = (ctx.content_rect().height() - 140.0).clamp(300.0, 640.0);
        widgets::dialog(
            ctx,
            "settings",
            tr("Settings"),
            &mut open,
            size::DIALOG_L,
            |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(210.0);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for (heading, sections) in [
                            ("Muddie", Section::MUDDIE),
                            (profile.as_str(), Section::PROFILE),
                        ] {
                            widgets::caption(ui, heading);
                            for section in sections {
                                let chosen = self.settings_section == section;
                                if widgets::nav_item(ui, chosen, section.name(), None).clicked() {
                                    self.settings_section = section;
                                }
                            }
                            ui.add_space(size::GAP_M);
                        }
                    });
                    ui.add_space(size::GAP_L);
                    let section = self.settings_section;
                    ui.vertical(|ui| {
                        ui.set_min_height(height);
                        egui::ScrollArea::vertical()
                            .id_salt(("settings", section))
                            .max_height(height)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = size::GAP_S;
                                let title = RichText::new(section.name())
                                    .font(theme::font(Weight::Semibold, 17.0))
                                    .color(theme::current().text);
                                ui.add(egui::Label::new(title).selectable(false));
                                ui.add_space(size::GAP_S);
                                let tab = &mut self.tabs[active];
                                // A session without a profile has the settings'
                                // own for what a profile would keep.
                                let shared = tab.profile_name().is_none();
                                match section {
                                    Section::General => {
                                        general_settings(ui, &mut self.settings, &languages)
                                    }
                                    Section::Appearance => {
                                        appearance_settings(ui, &mut self.settings)
                                    }
                                    Section::Text => text_settings(ui, &mut self.settings),
                                    Section::Speech => speech_settings(ui, &mut self.settings),
                                    section => tab.profile_section(ui, section),
                                }
                                if shared {
                                    tab.prefs().apply_to(&mut self.settings);
                                }
                                if let Some(error) = &self.settings_error {
                                    ui.colored_label(theme::current().danger, error);
                                }
                            });
                    });
                });
            },
        );
        self.show_settings = open;
        self.tabs[active].profile_autosave(ctx, open);
        if !open {
            self.languages = None;
        }
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
            size::DIALOG_S,
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
                            ("Alt+Shift+↑ …", tr("Same, for the chat panel")),
                            ("Alt+0", tr("Read the gauges")),
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
                            ("Ctrl+D", tr("Show what the MUD sends")),
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
            size::DIALOG_S,
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
        // Saved once whatever is being dragged is let go, not at each step
        // of a slider.
        self.settings_unsaved = true;
    }

    /// Writes the settings to their file if they changed since.
    fn save_settings(&mut self) {
        if !std::mem::take(&mut self.settings_unsaved) {
            return;
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
        // Alt with a key is a shortcut here, never typing.
        let last = &mut self.alt_typing;
        ui.input_mut(|input| drop_alt_typing(&mut input.events, input.modifiers, last));
        // With MUDDIE_TRACE set, how often the window is drawn is said on
        // standard error, to find what keeps it busy when nothing happens.
        self.frames += 1;
        if self.traced_at.elapsed() >= std::time::Duration::from_secs(5) {
            if std::env::var_os("MUDDIE_TRACE").is_some() {
                let causes: Vec<String> = (ui.ctx().repaint_causes().iter())
                    .map(ToString::to_string)
                    .collect();
                let lines = self.tabs[self.active].lines();
                let events: Vec<String> = ui.input(|input| {
                    (input.events.iter())
                        .map(|event| format!("{event:?}").chars().take(60).collect())
                        .collect()
                });
                eprintln!(
                    "muddie: {} frames in 5 s, {lines} lines; last asked for by {causes:?}; input {events:?}",
                    self.frames
                );
            }
            (self.frames, self.traced_at) = (0, std::time::Instant::now());
        }
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
        if ui.input_mut(|input| input.consume_shortcut(&SETTINGS)) {
            self.show_settings = true;
        }
        if ui.input_mut(|input| input.consume_shortcut(&COMMANDS)) {
            self.tabs[self.active].toggle_palette();
        }
        if ui.input_mut(|input| input.consume_shortcut(&SWITCH_VIEW)) {
            let tab = &mut self.tabs[self.active];
            tab.set_view(if tab.view() == 0 { 1 } else { 0 });
        }
        if ui.input_mut(|input| input.consume_shortcut(&DEBUG_VIEW)) {
            let tab = &mut self.tabs[self.active];
            tab.set_view(if tab.view() == 2 { 0 } else { 2 });
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
        if let Some(request) = self.tabs[active].take_app_request() {
            self.carry_out(request, ui.ctx());
        }
        // Panels switched on or off stay that way the next time.
        let panes = self.tabs[active].pane_widths();
        // Not while something is being dragged: once it is let go.
        let settled = !ui.input(|input| input.pointer.any_down());
        if settled {
            self.save_settings();
        }
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
/// The bell of what the client said in a session, marked with how many
/// have not been looked at, and the list of them when pressed: each with
/// when it was said, to read again or copy.
fn notices(ui: &mut Ui, tab: &mut Tab) {
    let theme = theme::current();
    let (said, unread) = tab.notices();
    let label = match unread {
        0 => tr("Messages").to_owned(),
        count => trf("Messages, {0} new", &[&count.to_string()]),
    };
    let bell = widgets::icon_button(ui, widgets::Icon::Bell, &label, false);
    if unread > 0 {
        let at = bell.rect.right_top() + egui::vec2(-7.0, 7.0);
        ui.painter().circle_filled(at, 3.5, theme.accent);
    }
    let said: Vec<_> = said.iter().rev().take(60).cloned().collect();
    let mut clear = false;
    let shown = egui::Popup::menu(&bell).width(420.0).show(|ui| {
        ui.horizontal(|ui| {
            widgets::menu_heading(ui, tr("Messages"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !said.is_empty() {
                    clear = widgets::quiet_button(ui, tr("Clear")).clicked();
                }
            });
        });
        if said.is_empty() {
            widgets::menu_heading(ui, tr("Nothing yet."));
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(360.0)
            .show(ui, |ui| {
                for notice in &said {
                    ui.horizontal_top(|ui| {
                        let time = RichText::new(&notice.time)
                            .font(egui::FontId::monospace(11.5))
                            .color(theme.text_faint);
                        ui.add(egui::Label::new(time).selectable(false));
                        let colour = if notice.problem {
                            theme.danger
                        } else {
                            theme.text
                        };
                        let text = RichText::new(&notice.text).color(colour);
                        ui.add(egui::Label::new(text).wrap().selectable(true));
                    });
                }
            });
    });
    if shown.is_some() {
        tab.notices_read();
    }
    if clear {
        tab.clear_notices();
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

/// Takes out the characters typed while Alt is held without Ctrl: Alt and
/// a key is a shortcut (Alt+R, Alt+0, a macro), and the key must not also
/// land in the command line. AltGr, which types @, # or €, comes as Ctrl
/// and Alt on Windows and as neither elsewhere, so it still types. On a
/// Mac, Option is how many characters are typed, and it is left alone.
fn drop_alt_typing(events: &mut Vec<egui::Event>, held: Modifiers, last: &mut bool) {
    if cfg!(target_os = "macos") {
        return;
    }
    let alt_only = |modifiers: Modifiers| modifiers.alt && !modifiers.ctrl && !modifiers.command;
    // What is held when each character comes: the last key pressed, or
    // the modifiers as they changed, before it.
    // The letter may come a frame after its key: what was last pressed
    // is remembered until another key or a change of modifiers.
    let mut alt = *last || alt_only(held);
    events.retain(|event| match event {
        egui::Event::Key {
            modifiers,
            pressed: true,
            ..
        } => {
            alt = alt_only(*modifiers);
            true
        }
        egui::Event::ModifiersChanged(modifiers) => {
            alt = alt_only(*modifiers);
            true
        }
        egui::Event::Text(_) => !alt,
        _ => true,
    });
    *last = alt;
}

#[cfg(test)]
mod alt_tests {
    use super::*;

    #[test]
    fn alt_and_a_key_does_not_type_but_altgr_does() {
        let key = |key, modifiers| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let text = |text: &str| egui::Event::Text(text.into());
        let mut events = vec![
            key(egui::Key::R, Modifiers::ALT),
            text("r"),
            key(egui::Key::A, Modifiers::NONE),
            text("a"),
            // AltGr+2 on Windows: Ctrl and Alt, typing @.
            key(egui::Key::Num2, Modifiers::ALT | Modifiers::CTRL),
            text("@"),
            egui::Event::ModifiersChanged(Modifiers::ALT),
            text("0"),
        ];
        drop_alt_typing(&mut events, Modifiers::NONE, &mut false);
        let typed: Vec<&str> = (events.iter())
            .filter_map(|event| match event {
                egui::Event::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(typed, ["a", "@"]);
    }
}
