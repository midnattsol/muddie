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
        widgets::setting_row(ui, tr("Encryption"), None, &mut |ui| {
            encryption_choice(ui, &mut draft.tls, &mut draft.verify_certificate)
        });
        widgets::setting_row(
            ui,
            tr("Encoding"),
            Some(tr("Automatic works with nearly every MUD.")),
            &mut |ui| encoding_choice(ui, &mut draft.encoding),
        );
    });
}

/// Whether the connection is encrypted, and whether the server's
/// certificate is checked: one choice, as the second means nothing
/// without the first.
fn encryption_choice(ui: &mut Ui, tls: &mut bool, verify: &mut bool) -> egui::Response {
    let choices = [
        ((false, true), tr("Unencrypted").to_owned()),
        ((true, true), "TLS".to_owned()),
        (
            (true, false),
            tr("TLS without verifying the certificate").to_owned(),
        ),
    ];
    let mut value = (*tls, *verify || !*tls);
    let response = widgets::choose(ui, "encryption", &mut value, &choices, |_| String::new());
    (*tls, *verify) = value;
    response
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
    widgets::section(ui, tr("Login"), |ui| {
        widgets::setting_row(
            ui,
            tr("Character"),
            Some(tr("Sent on connecting.")),
            &mut |ui| {
                widgets::row_field(ui, &mut edit.draft.character, tr("Optional"), size::FIELD_L)
            },
        );
        widgets::setting_row(
            ui,
            tr("Password"),
            Some(tr("Kept in the system keyring.")),
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
    widgets::section(ui, tr("Connection"), |ui| {
        widgets::switch_setting(ui, &mut prefs.reconnect, tr("Auto-reconnect"), None);
        let about = tr("Stops the server or a router from closing an idle connection.");
        widgets::setting_row(ui, tr("Keep-alive"), Some(about), &mut |ui| {
            let every = |seconds: u32| match seconds {
                0 => tr("Off").to_owned(),
                seconds if seconds % 60 == 0 => format!("{} min", seconds / 60),
                seconds => format!("{seconds} s"),
            };
            let choices = [0, 30, 60, 300, 600].map(|seconds| (seconds, every(seconds)));
            widgets::choose(
                ui,
                "keep-alive",
                &mut prefs.keepalive_seconds,
                &choices,
                every,
            )
        });
    });
}

/// The session's log, and the sounds the MUD asks for.
pub(in crate::app) fn logs_group(ui: &mut Ui, prefs: &mut SessionPrefs) {
    use crate::settings::LogFormat;
    widgets::section(ui, tr("Logging and sound"), |ui| {
        let about = tr("Saves everything shown to a file.");
        widgets::setting_row(ui, tr("Session log"), Some(about), &mut |ui| {
            let choices = [
                (None, tr("Off").to_owned()),
                (Some(LogFormat::Text), tr("Text").to_owned()),
                (Some(LogFormat::Html), "HTML".to_owned()),
            ];
            let mut value = prefs.log_sessions.then_some(prefs.log_format);
            let response =
                widgets::choose(ui, "session log", &mut value, &choices, |_| String::new());
            prefs.log_sessions = value.is_some();
            if let Some(format) = value {
                prefs.log_format = format;
            }
            response
        });
        let about = tr("Those the MUD asks for that you do not have.");
        widgets::switch_setting(
            ui,
            &mut prefs.media_downloads,
            tr("Download sounds"),
            Some(about),
        );
    });
}
