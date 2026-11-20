//! The profile's part of the settings: where and how to connect, who to
//! log in as, its plugins, and its logs and sounds. What is changed is
//! kept with the profile as it is changed.

use super::*;
use crate::app::Section;
use crate::profile::SessionPrefs;
use crate::theme::size;
use crate::widgets;

/// How long the fields are left alone before what changed is saved.
const SAVE_AFTER: Duration = Duration::from_millis(900);

/// The text encodings offered, by the label the decoder knows them by.
const ENCODINGS: [(&str, &str); 9] = [
    ("auto", ""),
    ("utf-8", "UTF-8"),
    ("windows-1252", "Latin-1 (Windows-1252)"),
    ("iso-8859-15", "Latin-9 (ISO-8859-15)"),
    ("koi8-r", "KOI8-R"),
    ("big5", "Big5"),
    ("gbk", "GBK"),
    ("shift_jis", "Shift JIS"),
    ("euc-kr", "EUC-KR"),
];

impl Tab {
    /// The profile this session plays with, if it has one.
    pub fn profile_name(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    /// How this session goes: its profile's, or the settings' without one.
    pub fn prefs(&self) -> &SessionPrefs {
        &self.prefs
    }

    /// Asks for the settings, at a section of the profile's.
    pub fn open_profile(&mut self, section: Section) {
        self.app_request = Some(AppRequest::Settings(section));
    }

    /// One of the profile's sections of the settings.
    pub fn profile_section(&mut self, ui: &mut egui::Ui, section: Section) {
        match section {
            Section::Connection => self.connection_section(ui),
            Section::Login => self.login_section(ui),
            Section::Plugins => self.plugins_ui(ui),
            _ => self.logs_section(ui),
        }
    }

    /// The profile as the fields have it, or nothing while the port is
    /// not a number.
    pub(super) fn profile_now(&self) -> Option<Profile> {
        let port = self.port.trim().parse::<u16>().ok()?;
        Some(Profile {
            host: self.host.trim().to_owned(),
            port,
            tls: self.tls,
            verify_certificate: self.verify_certificate,
            speech: self.voice.enabled(),
            character: self.character.trim().to_owned(),
            encoding: self.encoding.trim().to_owned(),
            packages: self.packages.clone(),
            included_on: self.included_on.clone(),
            included_off: self.included_off.clone(),
            included_declined: self.included_declined.clone(),
            session: Some(self.prefs.clone()),
        })
    }

    /// What a connection is made with, to tell when the fields say
    /// otherwise.
    pub(super) fn connection_key(&self) -> String {
        let fields = [&self.host, &self.port, &self.encoding, &self.character];
        let fields: Vec<&str> = fields.iter().map(|field| field.trim()).collect();
        format!(
            "{}|{}|{}",
            fields.join("|"),
            self.tls,
            self.verify_certificate
        )
    }

    /// Saves the profile once its fields have been left alone a moment,
    /// and at once when the settings close, with the password typed.
    pub fn profile_autosave(&mut self, ctx: &egui::Context, open: bool) {
        let Some(name) = self.profile.clone() else {
            return;
        };
        if !open && !self.new_password.is_empty() {
            self.keep_password(&name);
        }
        let Some(now) = self.profile_now() else {
            return;
        };
        if self.profile_saved.as_ref() == Some(&now) {
            self.profile_changed_at = None;
            return;
        }
        let since = *self.profile_changed_at.get_or_insert_with(Instant::now);
        if open && since.elapsed() < SAVE_AFTER {
            ctx.request_repaint_after(SAVE_AFTER - since.elapsed());
            return;
        }
        self.profile_changed_at = None;
        let Some(profiles) = &self.profiles else {
            return;
        };
        match profiles.save(&name, &now) {
            Ok(()) => self.profile_saved = Some(now),
            Err(error) => self.note(&trf(
                "Could not save the profile: {0}",
                &[&error.to_string()],
            )),
        }
    }

    /// Puts the password typed in the system's keyring, for the profile.
    fn keep_password(&mut self, name: &str) {
        let password = std::mem::take(&mut self.new_password);
        match secrets::set_password(name, Some(&password)) {
            Ok(()) => self.note(tr("Password kept in the system's keyring.")),
            Err(error) => self.note(&trf(
                "The password could not be saved in the keyring: {0}",
                &[&error],
            )),
        }
    }

    /// Where and how to connect, and how the connection is kept.
    fn connection_section(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        // Without a profile, the way to keep one comes first.
        if self.profile.is_none() {
            widgets::group(ui, |ui| {
                widgets::setting_row(ui, tr("Profile name"), &mut |ui| {
                    let ready = !self.profile_name.trim().is_empty();
                    let save =
                        ui.add_enabled_ui(ready, |ui| widgets::button(ui, tr("Save as profile")));
                    if save.inner.clicked() {
                        self.save_profile();
                    }
                    widgets::row_field(
                        ui,
                        &mut self.profile_name,
                        tr("Aardwolf, Reinos…"),
                        size::FIELD_M,
                    )
                });
                widgets::row_note(
                    ui,
                    tr(
                        "A profile keeps the connection, the scripts, the map and the layout, to come back to them.",
                    ),
                );
            });
            ui.add_space(size::GAP_M);
        }
        // Connected, what is changed here waits for connecting again.
        let changed = self.connection.is_some()
            && self.connected_with.as_deref() != Some(&self.connection_key());
        if changed {
            let mut again = false;
            notice_bar(
                ui,
                tr("The changes take effect on reconnecting."),
                &mut |ui| {
                    again = widgets::primary_button(ui, tr("Reconnect")).clicked();
                },
            );
            if again {
                self.reconnect(&ctx);
                self.app_request = Some(AppRequest::CloseSettings);
            }
            ui.add_space(size::GAP_S);
        }
        widgets::caption(ui, tr("Server"));
        widgets::group(ui, |ui| {
            widgets::setting_row(ui, tr("Host"), &mut |ui| {
                widgets::row_field(ui, &mut self.host, "mud.example.org", size::FIELD_L)
            });
            widgets::setting_row(ui, tr("Port"), &mut |ui| {
                widgets::row_field(ui, &mut self.port, "23", size::FIELD_S)
            });
            widgets::switch_setting(ui, &mut self.tls, tr("Secure connection (TLS)"));
            if self.tls {
                widgets::switch_setting(ui, &mut self.verify_certificate, tr("Verify certificate"));
                widgets::row_note(
                    ui,
                    tr("Off only for a server with a certificate of its own making."),
                );
            }
            widgets::setting_row(ui, tr("Text encoding"), &mut |ui| {
                let current = self.encoding.trim().to_lowercase();
                let name = |label: &str, shown: &str| match label {
                    "auto" => tr("Automatic").to_owned(),
                    _ => shown.to_owned(),
                };
                let chosen = (ENCODINGS.iter())
                    .find(|(label, _)| {
                        *label == current || (current.is_empty() && *label == "auto")
                    })
                    .map_or(self.encoding.clone(), |(label, shown)| name(label, shown));
                widgets::dropdown("encoding")
                    .selected_text(chosen)
                    .width(size::FIELD_L)
                    .show_ui(ui, |ui| {
                        for (label, shown) in ENCODINGS {
                            if widgets::choice(ui, label == current, &name(label, shown)).clicked()
                            {
                                self.encoding = label.to_owned();
                            }
                        }
                    })
                    .response
            });
            widgets::row_note(
                ui,
                tr("Automatic suits nearly every MUD: UTF-8, or Latin-1 for what is not."),
            );
        });
        ui.add_space(size::GAP_M);
        widgets::caption(ui, tr("Keeping the connection"));
        widgets::group(ui, |ui| {
            widgets::switch_setting(
                ui,
                &mut self.prefs.reconnect,
                tr("Reconnect when the connection is lost"),
            );
            widgets::row_note(ui, tr("When it drops without you closing it."));
            let mut alive = self.prefs.keepalive_seconds > 0;
            if widgets::switch_setting(ui, &mut alive, tr("Keep the connection alive")).changed() {
                self.prefs.keepalive_seconds = if alive { 60 } else { 0 };
            }
            if alive {
                widgets::setting_row(ui, tr("Every"), &mut |ui| {
                    ui.add(
                        widgets::number(&mut self.prefs.keepalive_seconds)
                            .range(5..=3600)
                            .suffix(" s"),
                    )
                });
            }
            widgets::row_note(
                ui,
                tr("Some servers and routers close a connection that has been quiet for a while."),
            );
        });
        ui.add_space(size::GAP_M);
        if self.connection.is_none() {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, tr("Connect")).clicked() {
                        self.connect(&ctx);
                        self.app_request = Some(AppRequest::CloseSettings);
                    }
                });
            });
        }
    }

    /// Who to log in as.
    fn login_section(&mut self, ui: &mut egui::Ui) {
        let saved = self.profile.clone();
        widgets::group(ui, |ui| {
            widgets::setting_row(ui, tr("Character"), &mut |ui| {
                widgets::row_field(ui, &mut self.character, "", size::FIELD_L)
            });
            widgets::row_note(ui, tr("Muddie answers the first prompt with it."));
            // A saved profile may have one kept already.
            let hint = if saved.is_some() {
                tr("Leave empty to keep it")
            } else {
                ""
            };
            let mut field = None;
            widgets::setting_row(ui, tr("Password"), &mut |ui| {
                let typed = widgets::secret_field(ui, &mut self.new_password, hint, size::FIELD_L);
                field = Some(typed.clone());
                typed
            });
            widgets::row_note(
                ui,
                tr(
                    "Kept in the system's keyring, never in a file, and sent when the server hides typing.",
                ),
            );
            if let (Some(name), Some(field)) = (&saved, field)
                && field.lost_focus()
                && !self.new_password.is_empty()
            {
                self.keep_password(name);
            }
        });
        if saved.is_none() {
            ui.add_space(size::GAP_S);
            widgets::row_note(
                ui,
                tr("The password is kept once the session is saved as a profile."),
            );
        }
    }

    /// The session's log, and the sounds the MUD asks for.
    fn logs_section(&mut self, ui: &mut egui::Ui) {
        if self.profile.is_none() {
            widgets::row_note(
                ui,
                tr("Without a profile, these are those of every session that has none."),
            );
        }
        widgets::group(ui, |ui| {
            widgets::switch_setting(ui, &mut self.prefs.log_sessions, tr("Log every session"));
            widgets::row_note(
                ui,
                tr("Everything shown in the session, written to a file as it comes."),
            );
            if self.prefs.log_sessions {
                widgets::setting_row(ui, tr("Log format"), &mut |ui| {
                    let name = |format: LogFormat| match format {
                        LogFormat::Text => tr("Text"),
                        LogFormat::Html => "HTML",
                    };
                    widgets::dropdown("log format")
                        .selected_text(name(self.prefs.log_format))
                        .width(size::FIELD_M)
                        .show_ui(ui, |ui| {
                            for format in [LogFormat::Text, LogFormat::Html] {
                                widgets::choice_value(
                                    ui,
                                    &mut self.prefs.log_format,
                                    format,
                                    name(format),
                                );
                            }
                        })
                        .response
                });
                widgets::row_note(ui, tr("HTML keeps the colours."));
            }
            widgets::switch_setting(
                ui,
                &mut self.prefs.media_downloads,
                tr("Download the sounds a MUD asks to play"),
            );
            widgets::row_note(
                ui,
                tr(
                    "A sound the MUD asks for that you do not have is fetched from the web and kept.",
                ),
            );
        });
    }
}

/// A bar that says something about the section, with what to do about
/// it at the right.
fn notice_bar(ui: &mut egui::Ui, text: &str, add: &mut dyn FnMut(&mut egui::Ui)) {
    let theme = theme::current();
    egui::Frame::new()
        .fill(theme.accent.gamma_multiply(0.14))
        .stroke(egui::Stroke::new(1.0, theme.accent.gamma_multiply(0.5)))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(text).color(theme.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add);
            });
        });
}
