//! The groups of settings of a profile: where it connects, who logs in, how
//! the connection is kept, and its logs and sounds.

use super::*;

/// The text encodings offered, by the label the decoder knows them by.
pub(in crate::app) const ENCODINGS: [(&str, &str); 9] = [
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

/// Where the profile connects to.
pub(in crate::app) fn server_group(ui: &mut Ui, draft: &mut Profile) {
    widgets::section(ui, tr("Server"), |ui| {
        widgets::setting_row(ui, tr("Host"), None, &mut |ui| {
            widgets::row_field(ui, &mut draft.host, "mud.example.org", size::FIELD_L)
        });
        widgets::setting_row(ui, tr("Port"), None, &mut |ui| {
            ui.add_sized(
                [size::FIELD_S, size::CONTROL],
                widgets::number(&mut draft.port).range(1..=65535),
            )
        });
        widgets::switch_setting(ui, &mut draft.tls, tr("Secure connection (TLS)"), None);
        if draft.tls {
            widgets::switch_setting(
                ui,
                &mut draft.verify_certificate,
                tr("Verify certificate"),
                Some(tr(
                    "Off only for a server with a certificate of its own making.",
                )),
            );
        }
        widgets::setting_row(
            ui,
            tr("Text encoding"),
            Some(tr(
                "Automatic suits nearly every MUD: UTF-8, or Latin-1 for what is not.",
            )),
            &mut |ui| encoding_choice(ui, &mut draft.encoding),
        );
    });
}

/// The text encoding, from those offered; one set by hand is shown as it is.
pub(in crate::app) fn encoding_choice(ui: &mut Ui, encoding: &mut String) -> egui::Response {
    let current = encoding.trim().to_lowercase();
    let current = if current.is_empty() {
        "auto".to_owned()
    } else {
        current
    };
    let name = |label: &str, shown: &str| match label {
        "auto" => tr("Automatic").to_owned(),
        _ => shown.to_owned(),
    };
    let chosen = (ENCODINGS.iter())
        .find(|(label, _)| *label == current)
        .map_or(encoding.clone(), |(label, shown)| name(label, shown));
    widgets::dropdown("encoding")
        .selected_text(chosen)
        .width(size::FIELD_L)
        .show_ui(ui, |ui| {
            for (label, shown) in ENCODINGS {
                if widgets::choice(ui, label == current, &name(label, shown)).clicked() {
                    *encoding = label.to_owned();
                }
            }
        })
        .response
}

/// Who to log in as.
pub(in crate::app) fn login_group(ui: &mut Ui, edit: &mut ProfileEdit) {
    widgets::section(ui, tr("Log in"), |ui| {
        widgets::setting_row(
            ui,
            tr("Character"),
            Some(tr("Muddie answers the first prompt with it.")),
            &mut |ui| widgets::row_field(ui, &mut edit.draft.character, "", size::FIELD_L),
        );
        widgets::setting_row(
            ui,
            tr("Password"),
            Some(tr(
                "Kept in the system's keyring, never in a file, and sent when the server hides typing.",
            )),
            &mut |ui| {
                widgets::secret_field(
                    ui,
                    &mut edit.password,
                    tr("Leave empty to keep it"),
                    size::FIELD_L,
                )
            },
        );
    });
}

/// Connecting again when the connection drops, and keeping it alive.
pub(in crate::app) fn keeping_group(ui: &mut Ui, prefs: &mut SessionPrefs) {
    widgets::section(ui, tr("Keeping the connection"), |ui| {
        widgets::switch_setting(
            ui,
            &mut prefs.reconnect,
            tr("Reconnect when the connection is lost"),
            Some(tr("When it drops without you closing it.")),
        );
        let mut alive = prefs.keepalive_seconds > 0;
        if widgets::switch_setting(
            ui,
            &mut alive,
            tr("Keep the connection alive"),
            Some(tr(
                "Some servers and routers close a connection that has been quiet for a while.",
            )),
        )
        .changed()
        {
            prefs.keepalive_seconds = if alive { 60 } else { 0 };
        }
        if alive {
            widgets::setting_row(ui, tr("Every"), None, &mut |ui| {
                ui.add_sized(
                    [size::FIELD_S, size::CONTROL],
                    widgets::number(&mut prefs.keepalive_seconds)
                        .range(5..=3600)
                        .suffix(" s"),
                )
            });
        }
    });
}

/// The session's log, and the sounds the MUD asks for.
pub(in crate::app) fn logs_group(ui: &mut Ui, prefs: &mut SessionPrefs) {
    use crate::settings::LogFormat;
    widgets::section(ui, tr("Logs and sounds"), |ui| {
        widgets::switch_setting(
            ui,
            &mut prefs.log_sessions,
            tr("Log every session"),
            Some(tr(
                "Everything shown in the session, written to a file as it comes.",
            )),
        );
        if prefs.log_sessions {
            widgets::setting_row(
                ui,
                tr("Log format"),
                Some(tr("HTML keeps the colours.")),
                &mut |ui| {
                    let name = |format: LogFormat| match format {
                        LogFormat::Text => tr("Text"),
                        LogFormat::Html => "HTML",
                    };
                    widgets::dropdown("log format")
                        .selected_text(name(prefs.log_format))
                        .width(size::FIELD_M)
                        .show_ui(ui, |ui| {
                            for format in [LogFormat::Text, LogFormat::Html] {
                                widgets::choice_value(
                                    ui,
                                    &mut prefs.log_format,
                                    format,
                                    name(format),
                                );
                            }
                        })
                        .response
                },
            );
        }
        widgets::switch_setting(
            ui,
            &mut prefs.media_downloads,
            tr("Download the sounds a MUD asks to play"),
            Some(tr(
                "A sound the MUD asks for that you do not have is fetched from the web and kept.",
            )),
        );
    });
}
