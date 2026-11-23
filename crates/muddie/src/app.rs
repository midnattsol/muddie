//! The window: the sessions, each in a tab, the bars around the one on
//! screen, and the settings shared by every session.

use std::path::PathBuf;

use eframe::egui::{
    self, Color32, CornerRadius, KeyboardShortcut, Modifiers, Panel, RichText, Sense, Ui,
};

use crate::i18n::{self, tr, trf};
use crate::keypad;
use crate::profile::Profiles;
use crate::settings::{MudColours, Settings, ThemeName};
use crate::tab::{Startup, Tab, View};
use crate::theme::{self, Weight, size};
use crate::widgets;

mod asks;
mod bars;
mod dialogs;
mod keys;
mod profiles;
mod settings_page;
mod toasts;

use asks::Ask;
use keys::drop_alt_typing;
use profiles::ProfileEdit;
pub use profiles::{ProfileAction, ProfileTab, profile_menu};
pub use settings_page::Section;

use crate::shortcuts::{CLOSE_SESSION, DATA_VIEW, NEW_SESSION, PALETTE, SETTINGS, SWITCH_VIEW};

pub struct App {
    tabs: Vec<Tab>,
    active: usize,
    settings: Settings,
    profiles: Option<Profiles>,
    /// Scripts given on the command line, run in every session.
    extra_scripts: Vec<PathBuf>,
    show_settings: bool,
    /// The page of the settings on screen.
    settings_section: Section,
    /// The shortcut whose keys are being chosen, by its name: the next
    /// keys pressed are its.
    recording_shortcut: Option<&'static str>,
    /// The Profiles window, the profile chosen in it, and its tab.
    show_profiles: bool,
    editing: Option<ProfileEdit>,
    profile_tab: ProfileTab,
    /// What is being asked about a profile, and what was wrong with the
    /// answer.
    asking: Option<Ask>,
    ask_error: Option<String>,
    /// Whether the last key came with Alt alone, so that its letter, if
    /// it comes a frame later, is not typed.
    alt_typing: bool,
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
    /// A problem with a profile to show in the Profiles window.
    profile_error: Option<String>,
    /// A session of a profile that no session on screen plays, kept for
    /// what is done with that profile from the Profiles window: its
    /// plugins installed and removed. It never connects.
    profile_session: Option<Tab>,
    /// How many of its messages were said again in the session on screen.
    profile_session_said: u64,
    ctx: egui::Context,
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
            crate::languages::set_folder(profiles.languages_dir());
        }
        // Not in tests, which read the interface in English and run side
        // by side: the language is one for the whole program.
        #[cfg(not(test))]
        i18n::set_language(&settings.language);
        crate::shortcuts::configure(&settings.shortcuts);
        if let Err(error) = theme::apply(&cc.egui_ctx, &settings) {
            settings_error = Some(error);
        }
        let extra_scripts = startup.scripts.clone();
        let mut first = Tab::new(&cc.egui_ctx, startup, &settings);
        if let Some(error) = &settings_error {
            first.refuse(error);
        }
        Self {
            tabs: vec![first],
            active: 0,
            settings,
            profiles,
            extra_scripts,
            show_settings: false,
            settings_section: Section::General,
            recording_shortcut: None,
            show_profiles: false,
            editing: None,
            profile_tab: ProfileTab::default(),
            asking: None,
            ask_error: None,
            alt_typing: false,
            settings_unsaved: false,
            frames: 0,
            traced_at: std::time::Instant::now(),
            languages: None,
            show_shortcuts: false,
            show_about: false,
            settings_error,
            profile_error: None,
            profile_session: None,
            profile_session_said: 0,
            ctx: cc.egui_ctx.clone(),
            keypad,
        }
    }

    #[cfg(test)]
    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    /// Every session, the ones out of sight too.
    #[cfg(test)]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    #[cfg(test)]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Whether the Profiles window is open, and closing it.
    #[cfg(test)]
    pub fn profiles_shown(&self) -> bool {
        self.show_profiles
    }

    #[cfg(test)]
    pub fn close_settings_for_tests(&mut self) {
        self.show_settings = false;
    }

    #[cfg(test)]
    pub fn close_profiles_for_tests(&mut self) {
        self.show_profiles = false;
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

    /// Switches the interface's language as the settings window would.
    #[cfg(test)]
    pub fn use_language(&mut self, ctx: &egui::Context, code: &str) {
        self.settings.language = code.to_owned();
        i18n::set_language(code);
        self.apply_settings(ctx);
    }

    /// Switches the output's font as the settings window would.
    #[cfg(test)]
    pub fn use_font(&mut self, ctx: &egui::Context, family: &str) {
        self.settings.font_family = family.to_owned();
        self.apply_settings(ctx);
    }

    /// The session of a profile: one on screen playing it, else the one
    /// kept for it, made now if need be; with none named, the one on
    /// screen.
    pub(crate) fn session_of(&mut self, profile: Option<&str>) -> &mut Tab {
        let Some(name) = profile else {
            return &mut self.tabs[self.active];
        };
        if let Some(index) = self.tabs.iter().position(|tab| tab.uses_profile(name)) {
            return &mut self.tabs[index];
        }
        let kept = (self.profile_session.as_ref()).is_some_and(|tab| tab.uses_profile(name));
        if !kept {
            let startup = Startup {
                profiles: self.profiles.clone(),
                profile: None,
                target: None,
                speech: false,
                scripts: Vec::new(),
            };
            let mut tab = Tab::new(&self.ctx, startup, &self.settings);
            tab.play_quietly(name);
            // What it says is said by the session on screen, once.
            tab.silence();
            self.profile_session_said = tab.said_count();
            self.profile_session = Some(tab);
        }
        self.profile_session.as_mut().expect("made above")
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

    /// Does what a session asked of the window.
    fn carry_out(&mut self, request: crate::tab::AppRequest, ctx: &egui::Context) {
        use crate::tab::AppRequest;
        match request {
            AppRequest::NewSession => self.new_tab(ctx),
            AppRequest::CloseSession => self.close_tab(self.active, ctx),
            AppRequest::Settings(section) => self.open_settings(section),
            AppRequest::Profiles => self.open_profiles(None),
            AppRequest::InstallPlugin { address, profile } => {
                self.asking = Some(asks::Ask::Install { address, profile });
                self.ask_error = None;
            }
            AppRequest::RemovePlugin { name, profile } => {
                self.asking = Some(asks::Ask::RemovePlugin { name, profile });
                self.ask_error = None;
            }
            AppRequest::Play(name) => self.play(&name, ctx),
            AppRequest::Shortcuts => self.show_shortcuts = true,
            AppRequest::About => self.show_about = true,
            AppRequest::Profile(action, name) => self.profile_action(action, &name),
            AppRequest::SaveAsProfile => self.ask_save_session(),
        }
    }

    /// Plays with the saved profile of this name: in the session on
    /// screen if it is not connected, else in a new one.
    pub(super) fn play(&mut self, name: &str, ctx: &egui::Context) {
        if self.tabs[self.active].is_connected() {
            self.new_tab(ctx);
        }
        self.tabs[self.active].play(name, ctx);
    }

    /// Puts changed settings into effect and saves them.
    fn apply_settings(&mut self, ctx: &egui::Context) {
        // Not in tests, which read the interface in English side by side:
        // the language is one for the whole program.
        #[cfg(not(test))]
        i18n::set_language(&self.settings.language);
        crate::shortcuts::configure(&self.settings.shortcuts);
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

impl App {
    /// With MUDDIE_TRACE set, says on standard error how often the window
    /// is drawn and why, to find what keeps it busy when nothing happens.
    fn trace_frames(&mut self, ui: &Ui) {
        self.frames += 1;
        if self.traced_at.elapsed() < std::time::Duration::from_secs(5) {
            return;
        }
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

    /// The window's own keys, before a session gets them.
    fn window_keys(&mut self, ui: &mut Ui) {
        let pressed =
            |shortcut: &KeyboardShortcut| ui.input_mut(|input| input.consume_shortcut(shortcut));
        if pressed(&NEW_SESSION.keys()) {
            self.new_tab(ui.ctx());
        }
        if pressed(&CLOSE_SESSION.keys()) {
            self.close_tab(self.active, ui.ctx());
        }
        if pressed(&SETTINGS.keys()) {
            self.open_settings(Section::General);
        }
        let tab = &mut self.tabs[self.active];
        if pressed(&PALETTE.keys()) {
            tab.toggle_palette();
        }
        if pressed(&SWITCH_VIEW.keys()) {
            tab.set_view(if tab.view() == View::Game {
                View::Editor
            } else {
                View::Game
            });
        }
        if pressed(&DATA_VIEW.keys()) {
            tab.set_view(if tab.view() == View::Data {
                View::Game
            } else {
                View::Data
            });
        }
    }

    /// The bar at the top, then the session, then the bar at the bottom,
    /// in the room it had: the keyboard goes through them as they are
    /// read, top to bottom.
    fn bars_and_session(&mut self, ui: &mut Ui) {
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
        let id = ui.make_persistent_id("status bar");
        crate::widgets::below_last(ui, id, self, Self::session, |this, ui| {
            Panel::bottom("status bar")
                .frame(chrome(4))
                .show_separator_line(false)
                .show(ui, |ui| this.bottom_bar(ui))
                .response
                .rect
                .height()
        });
    }

    /// The windows over the rest: the settings, what they ask, the keys
    /// and what Muddie is.
    fn windows(&mut self, ctx: &egui::Context) {
        self.settings_window(ctx);
        self.profiles_window(ctx);
        self.ask_window(ctx);
        self.shortcuts_window(ctx);
        self.about_window(ctx);
    }

    /// The session on screen, with the keypad keys pressed for it, and
    /// what it asked of the window.
    fn session(&mut self, ui: &mut Ui) {
        let pressed = std::mem::take(&mut *self.keypad.lock().expect("keypad lock"));
        let tab = &mut self.tabs[self.active];
        tab.keypad_pressed.extend(pressed);
        tab.show(ui);
        if let Some(request) = tab.take_app_request() {
            self.carry_out(request, ui.ctx());
        }
    }

    /// Saves the settings, and the widths of the editor's panes, once
    /// nothing is being dragged.
    fn keep_settings(&mut self, ui: &Ui) {
        if ui.input(|input| input.pointer.any_down()) {
            return;
        }
        let (panes, heights) = self.tabs[self.active].layout();
        if (panes, heights) != (self.settings.automation_panes, self.settings.code_heights) {
            self.settings.automation_panes = panes;
            self.settings.code_heights = heights;
            self.settings_unsaved = true;
            // The same in every session, as it was made in this one.
            for tab in &mut self.tabs {
                tab.share_layout(&self.settings);
            }
        }
        // The folders open, kept for the profile: those of a session whose
        // profile changed are its new profile's.
        let tab = &mut self.tabs[self.active];
        match tab.open_folders() {
            None => tab.share_open_folders(&self.settings),
            Some((key, open))
                if self.settings.open_folders.get(&key).map(Vec::as_slice) != Some(open) =>
            {
                let open = open.to_vec();
                self.settings.open_folders.insert(key.clone(), open);
                self.settings_unsaved = true;
                for tab in &mut self.tabs {
                    if tab.open_folders().is_some_and(|(of, _)| of == key) {
                        tab.share_open_folders(&self.settings);
                    }
                }
            }
            Some(_) => {}
        }
        self.save_settings();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Alt with a key is a shortcut here, never typing.
        let last = &mut self.alt_typing;
        ui.input_mut(|input| drop_alt_typing(&mut input.events, input.modifiers, last));
        self.trace_frames(ui);
        crate::widgets::note_input(ui.ctx());
        // Keys being chosen for a shortcut are for nothing else.
        self.record_shortcut(ui);
        // Every session keeps running; only the one on screen is drawn.
        let mut asked = Vec::new();
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            tab.poll(ui.ctx(), index == self.active);
            // What one not on screen asked, as of a plugin removed from
            // its profile from the Profiles window.
            if index != self.active {
                asked.extend(tab.take_app_request());
            }
        }
        for request in asked {
            self.carry_out(request, ui.ctx());
        }
        let kept = self.profile_session.as_mut().and_then(|tab| {
            tab.poll(ui.ctx(), false);
            tab.take_app_request()
        });
        // What it says is said where it is heard: in the session on screen.
        if let Some(tab) = &self.profile_session {
            let said = tab.said_since(self.profile_session_said);
            self.profile_session_said = tab.said_count();
            for (text, problem) in said {
                self.tabs[self.active].say_again(&text, problem);
            }
        }
        if let Some(request) = kept {
            self.carry_out(request, ui.ctx());
        }
        self.window_keys(ui);
        self.windows(ui.ctx());
        self.bars_and_session(ui);
        self.toasts(ui.ctx());
        self.keep_settings(ui);
    }
}
