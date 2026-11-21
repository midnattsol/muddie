//! Preferences that apply to every session, kept in `settings.toml`.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// What speaks, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpeechOutput {
    /// Nothing.
    Off,
    /// The screen reader in use, while one is running: what it says also
    /// reaches a braille display. With none, nothing is said.
    #[default]
    Reader,
    /// The system's own voice, with or without a screen reader.
    Voice,
}

/// The sixteen colours the MUD's text is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MudPalette {
    /// Saturated, as a game's colours are meant to look.
    #[default]
    Vivid,
    /// The theme's own, toned down to go with it.
    Soft,
    /// The plain colours of a terminal.
    Classic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    /// The default: neutral charcoal, with no colour of its own.
    #[default]
    Dark,
    /// Soft, warm darks with a mauve accent.
    Mocha,
    /// Blue-violet with an amber accent.
    Dusk,
    /// Deep blues with a blue accent.
    TokyoNight,
    /// Cool blue-greys with a frost accent.
    Nord,
    /// Neutral greys with a blue accent.
    Graphite,
    Light,
    /// Pure black and white with saturated colours, for low vision.
    Contrast,
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
    /// The code of the interface's language, such as `es`; `en` for
    /// English, empty to follow the system.
    pub language: String,
    /// The colours of the MUD's text, whatever the theme.
    pub mud_palette: MudPalette,
    /// Size of the output's text, in points.
    pub font_size: f32,
    /// A TrueType or OpenType file to use for the output instead of the
    /// built-in monospace font; empty for the built-in one.
    pub font_file: String,
    /// Which face of `font_file` is meant, for files that hold several.
    pub font_index: u32,
    /// The family of an installed font to use for the output instead of
    /// the built-in one; empty for the built-in one.
    pub font_family: String,
    /// How far apart the lines of the output are: 1 is the font's own.
    pub line_spacing: f32,
    /// How large the interface is drawn: 1 is its natural size.
    pub ui_scale: f32,
    /// Lines of output kept per session.
    pub scrollback_lines: usize,
    /// Speech rate from -100 (slowest) to 100 (fastest); 0 is the voice's
    /// own default.
    pub speech_rate: i32,
    /// Language code the voice speaks in, such as `es`; empty for the
    /// system's default.
    pub speech_language: String,
    /// What speaks, if anything.
    pub speech: SpeechOutput,
    /// A new line of the game cuts short what is being said, rather than
    /// waiting for it: for fast games, where what came before is old.
    pub speech_interrupts: bool,
    /// What is said on the MUD's channels is said aloud too, with the
    /// channel's name before it.
    pub speech_channels: bool,
    /// Fetch from the web the sounds a MUD asks to be played and the
    /// player does not have.
    pub media_downloads: bool,
    /// Write everything shown in a session to a log file.
    pub log_sessions: bool,
    pub log_format: LogFormat,
    /// Show the time each line arrived.
    pub timestamps: bool,
    /// Connect again when the connection drops unexpectedly.
    pub reconnect: bool,
    /// Seconds between keep-alive messages while idle; 0 sends none.
    pub keepalive_seconds: u32,
    /// Leave a command in the command line after sending it, selected:
    /// Enter sends it again, and typing replaces it.
    pub keep_command: bool,
    /// The widths of the automation view's first two panes.
    pub automation_panes: [f32; 2],
    /// How tall the editors of commands and of scripts were made, in
    /// points; 0 fits them to what they hold.
    pub code_heights: [f32; 2],
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeName::default(),
            language: String::new(),
            mud_palette: MudPalette::default(),
            font_size: 15.0,
            font_file: String::new(),
            font_index: 0,
            font_family: String::new(),
            line_spacing: 1.0,
            ui_scale: 1.0,
            scrollback_lines: 20_000,
            speech_rate: 0,
            speech_language: String::new(),
            speech: SpeechOutput::default(),
            speech_interrupts: false,
            speech_channels: true,
            media_downloads: true,
            log_sessions: false,
            log_format: LogFormat::default(),
            timestamps: false,
            reconnect: true,
            keepalive_seconds: 60,
            keep_command: false,
            automation_panes: [190.0, 320.0],
            code_heights: [0.0; 2],
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
            language: "es".to_owned(),
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
