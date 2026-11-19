//! Preferences that apply to every session, kept in `settings.toml`.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    #[default]
    Dark,
    Light,
    /// Pure black and white with saturated colours, for low vision.
    Contrast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Follow the system's locale.
    #[default]
    Auto,
    English,
    Spanish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Text,
    /// Keeps the colours.
    Html,
}

/// Missing fields take their defaults, so files written by older versions
/// keep loading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeName,
    pub language: Language,
    /// Size of the output's text, in points.
    pub font_size: f32,
    /// A TrueType or OpenType file to use for the output instead of the
    /// built-in monospace font; empty for the built-in one.
    pub font_file: String,
    /// Lines of output kept per session.
    pub scrollback_lines: usize,
    /// Speech rate from -100 (slowest) to 100 (fastest); 0 is the voice's
    /// own default.
    pub speech_rate: i32,
    /// Language code the voice speaks in, such as `es`; empty for the
    /// system's default.
    pub speech_language: String,
    /// Voice text through the screen reader instead of speaking directly,
    /// so that it also reaches a braille display.
    pub announce_with_screen_reader: bool,
    /// Write everything shown in a session to a log file.
    pub log_sessions: bool,
    pub log_format: LogFormat,
    /// Show the time each line arrived.
    pub timestamps: bool,
    /// Connect again when the connection drops unexpectedly.
    pub reconnect: bool,
    /// Seconds between keep-alive messages while idle; 0 sends none.
    pub keepalive_seconds: u32,
    /// Whether the chat and the map have their panels, as last chosen.
    pub show_chat: bool,
    pub show_map: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeName::default(),
            language: Language::default(),
            font_size: 15.0,
            font_file: String::new(),
            scrollback_lines: 20_000,
            speech_rate: 0,
            speech_language: String::new(),
            announce_with_screen_reader: false,
            log_sessions: false,
            log_format: LogFormat::default(),
            timestamps: false,
            reconnect: true,
            keepalive_seconds: 60,
            show_chat: true,
            show_map: true,
        }
    }
}

impl Settings {
    /// Reads the settings file. A missing file gives the defaults; a file
    /// that cannot be understood is an error, so it is not overwritten.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("cannot read {}: {error}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|error| error.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        }
        std::fs::write(path, text)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_default_when_missing() {
        let path = std::env::temp_dir().join(format!(
            "muddie-settings-{}/settings.toml",
            std::process::id()
        ));
        assert_eq!(Settings::load(&path).unwrap(), Settings::default());
        let settings = Settings {
            theme: ThemeName::Contrast,
            language: Language::Spanish,
            font_size: 18.5,
            log_format: LogFormat::Html,
            keepalive_seconds: 0,
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);
        std::fs::write(&path, "theme = \"light\"\n").unwrap();
        let partial = Settings::load(&path).unwrap();
        assert_eq!((partial.theme, partial.font_size), (ThemeName::Light, 15.0));
        std::fs::write(&path, "theme = \"plaid\"\n").unwrap();
        assert!(Settings::load(&path).is_err());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
