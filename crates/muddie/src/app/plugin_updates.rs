//! Looking for newer official plugins: once as Muddie starts, and when
//! asked. What comes is put on disk by a thread of its own; here it is told
//! to the sessions, which take it as far as they may, and said to the user.

use std::collections::HashSet;
use std::sync::mpsc::TryRecvError;

use super::*;
use crate::plugins::official::{self, Download, Looking, Report};
use crate::plugins::store;

/// A look for updates that is going on, and whether it was asked for.
struct Going {
    result: Looking,
    asked: bool,
}

/// What the window knows of the official plugins' downloading.
#[derive(Default)]
pub(super) struct Updates {
    /// Whether one is going on, shared with the sessions.
    pub download: Download,
    going: Option<Going>,
    /// The plugins said to need a newer Muddie, not to say it twice.
    said_newer: HashSet<String>,
}

impl Updates {
    /// Puts right what a crash left of the plugins and, if the settings say
    /// so, starts looking for newer ones, as Muddie starts: before any
    /// session is made, so that one that connects at once can wait.
    pub(super) fn at_start(
        profiles: Option<&Profiles>,
        settings: &Settings,
        ctx: &egui::Context,
    ) -> Self {
        let mut updates = Self::default();
        let Some(profiles) = profiles else {
            return updates;
        };
        store::recover(&profiles.official_dir());
        if settings.update_plugins {
            updates.start(profiles, false, ctx);
        }
        updates
    }

    fn start(&mut self, profiles: &Profiles, asked: bool, ctx: &egui::Context) {
        let ctx = ctx.clone();
        let result = official::start(
            profiles.official_dir(),
            profiles.official_versions(),
            move || ctx.request_repaint(),
        );
        if let Some(result) = result {
            self.download.set(true);
            self.going = Some(Going { result, asked });
        }
    }
}

impl App {
    /// Looks for newer official plugins now, as asked: what is found or
    /// what went wrong is said.
    pub(super) fn look_for_plugin_updates(&mut self) {
        let Some(profiles) = self.profiles.clone() else {
            return self.tabs[self.active].say_again(
                tr("Plugins need a configuration folder, and none was found."),
                true,
            );
        };
        if self.updates.going.is_some() {
            self.tabs[self.active].say_again(tr("Already looking for plugin updates."), false);
            return;
        }
        self.tabs[self.active].say_again(tr("Looking for plugin updates…"), false);
        let ctx = self.ctx.clone();
        self.updates.start(&profiles, true, &ctx);
    }

    /// Takes what a look for updates came to, once it is done.
    pub(super) fn check_plugin_updates(&mut self) {
        let Some(going) = &self.updates.going else {
            return;
        };
        let result = match going.result.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            // Gone with nothing said: not looked for forever.
            Err(TryRecvError::Disconnected) => Err(tr("it stopped halfway").to_owned()),
        };
        let asked = going.asked;
        self.updates.going = None;
        match result {
            Ok(report) => self.apply_plugin_report(&report, asked),
            Err(error) if asked => self.tabs[self.active].say_again(
                &trf("Could not look for plugin updates: {0}", &[&error]),
                true,
            ),
            // Offline, say, which is no reason to bother anyone on each start.
            Err(_) => {}
        }
        // After the sessions took what came, those waiting go on.
        self.updates.download.set(false);
    }

    /// Tells the sessions the plugins changed, and the user what came.
    pub(crate) fn apply_plugin_report(&mut self, report: &Report, asked: bool) {
        if !report.changed.is_empty() {
            let sessions = self.tabs.iter_mut().chain(self.profile_session.as_mut());
            sessions.for_each(Tab::official_changed);
        }
        let mut said = Vec::new();
        if !report.changed.is_empty() {
            let names: Vec<&str> = report
                .changed
                .iter()
                .map(|(_, name)| name.as_str())
                .collect();
            said.push((trf("Plugins updated: {0}.", &[&names.join(", ")]), false));
        }
        for (name, version) in &report.newer {
            if self.updates.said_newer.insert(format!("{name} {version}")) {
                let text = trf(
                    "{0} needs Muddie {1} or newer, so it is not updated.",
                    &[name, version],
                );
                said.push((text, false));
            }
        }
        if asked {
            for why in &report.refused {
                said.push((trf("A plugin could not be installed: {0}", &[why]), true));
            }
            if report.changed.is_empty() && report.newer.is_empty() && report.refused.is_empty() {
                said.push((tr("Plugins are up to date.").to_owned(), false));
            }
        }
        for (text, problem) in said {
            self.tabs[self.active].say_again(&text, problem);
        }
    }
}
