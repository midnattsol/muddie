//! The kinds of rule made in the editor: what each is made of.

use super::*;

pub(super) fn yes() -> bool {
    true
}

/// Reacts to a line of output.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Trigger {
    /// What the user calls it; shown in lists instead of the pattern.
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Text the line must contain, or a regular expression if `regex`.
    pub pattern: String,
    pub regex: bool,
    /// Commands to send, separated by `;`. `%1`... are the pattern's groups.
    pub send: String,
    /// Hide the line.
    pub gag: bool,
    /// Colour to give the matched text, as a name or `#rrggbb`; empty for
    /// none.
    pub highlight: String,
    /// Text to say; `%1`... are the pattern's groups.
    pub speak: String,
    /// Sound file to play.
    pub sound: String,
    /// A panel of text to show the line in, besides or instead of the
    /// output; empty for none.
    pub panel: String,
    /// Luau to run, with the match as `m` and the line as `line`.
    pub script: String,
    /// What it does is the script, not the commands.
    pub lua: bool,
    /// The group it is switched on and off with; empty for none.
    pub group: String,
}

/// Replaces a typed command.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Alias {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// The word that starts the command, or a regular expression matched
    /// against the whole command if `regex`.
    pub pattern: String,
    pub regex: bool,
    /// Commands to send, separated by `;`. For a word alias `%1` is the
    /// rest of what was typed; for a regular expression, its groups.
    pub send: String,
    /// Luau to run, with the match as `m` and what was typed as `input`.
    pub script: String,
    /// What it does is the script, not the commands.
    pub lua: bool,
    /// The group it is switched on and off with; empty for none.
    pub group: String,
}

/// Sends commands when a key is pressed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBinding {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// The key as `bind` names it, such as `f5` or `ctrl+k`.
    pub key: String,
    pub send: String,
    /// Luau to run when the key is pressed.
    pub script: String,
    /// What it does is the script, not the commands.
    pub lua: bool,
    /// The group it is switched on and off with; empty for none.
    pub group: String,
}

/// Does something after a while, once or again and again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Timer {
    pub name: String,
    pub enabled: bool,
    /// How long it waits, in tenths of a second.
    pub tenths: u32,
    /// Goes off every so long, instead of only once after starting.
    pub repeat: bool,
    pub send: String,
    pub script: String,
    /// What it does is the script, not the commands.
    pub lua: bool,
    /// The group it is switched on and off with; empty for none.
    pub group: String,
}

/// Whether a group of rules starts a session switched on. A group that
/// has no entry does.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Group {
    pub name: String,
    pub enabled: bool,
}

pub(super) fn is_zero(number: &u8) -> bool {
    *number == 0
}

/// What a gauge is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GaugeKind {
    /// A bar that fills.
    #[default]
    Bar,
    /// A row of ten cells.
    Cells,
    /// Its number, with no bar.
    Number,
    /// A light that is on or off: for what is so or is not.
    Light,
    /// What the MUD says, as text.
    Text,
}

/// A colour a gauge takes while it is under a percentage of its maximum.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Step {
    pub under: u8,
    pub colour: String,
}

/// A bar that follows a value the server reports.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Gauge {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Where the value is: a GMCP package and the fields inside it, such
    /// as `Char.Vitals.health.current`, or the name of a MSDP variable.
    pub value: String,
    /// Where the maximum is, in the same way, or a number.
    pub max: String,
    /// A colour name or `#rrggbb`; empty to let the client choose.
    pub colour: String,
    /// What it is drawn as.
    pub kind: GaugeKind,
    /// For a light, its colour while it is off; empty leaves it unlit.
    pub off_colour: String,
    /// The colour it takes under each percentage of its maximum.
    #[serde(rename = "step")]
    pub steps: Vec<Step>,
    /// As earlier versions kept the two above; read, and brought up to
    /// date by [`Rules::migrate`].
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub segments: bool,
    #[serde(skip_serializing_if = "is_zero")]
    pub low: u8,
    /// The panel of gauges it is shown in, by the panel's id; none shows
    /// it in the first there is.
    pub panel: u32,
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: true,
            tenths: 600,
            repeat: true,
            send: String::new(),
            script: String::new(),
            lua: false,
            group: String::new(),
        }
    }
}

impl Timer {
    /// How long it waits, as people say it: `1.5 s`, `2 min`.
    pub fn interval(&self) -> String {
        let seconds = f64::from(self.tenths) / 10.0;
        if self.tenths >= 600 && self.tenths.is_multiple_of(600) {
            format!("{} min", self.tenths / 600)
        } else if self.tenths >= 600 {
            format!(
                "{} min {} s",
                self.tenths / 600,
                f64::from(self.tenths % 600) / 10.0
            )
        } else {
            format!("{seconds} s")
        }
    }
}

impl GaugeKind {
    /// Whether it shows how full something is, and so has a maximum and
    /// can change colour by level.
    pub fn has_level(self) -> bool {
        matches!(self, Self::Bar | Self::Cells | Self::Number)
    }

    pub(super) fn style(self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Cells => "cells",
            Self::Number => "number",
            Self::Light => "light",
            Self::Text => "text",
        }
    }
}

impl Gauge {
    /// A gauge as one starts: cells, red when low.
    pub fn starter() -> Self {
        Self {
            enabled: true,
            kind: GaugeKind::Cells,
            steps: vec![Step {
                under: 30,
                colour: "red".to_owned(),
            }],
            ..Self::default()
        }
    }

    /// What the gauge is called on screen: its name, or failing that
    /// where its value is.
    pub fn shown_name(&self) -> &str {
        if self.name.trim().is_empty() {
            self.value.trim()
        } else {
            self.name.trim()
        }
    }
}

impl Trigger {
    /// The regular expression the trigger matches lines with.
    pub fn regex_source(&self) -> String {
        if self.regex {
            self.pattern.clone()
        } else {
            regex_escape(&self.pattern)
        }
    }

    /// A line saying what the trigger does, for lists.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.lua {
            parts.push("Luau".to_owned());
        } else if !self.send.trim().is_empty() {
            parts.push(format!(
                "→ {}",
                commands(&self.send).collect::<Vec<_>>().join("; ")
            ));
        }
        if !self.speak.trim().is_empty() {
            parts.push(format!("“{}”", self.speak.trim()));
        }
        if !self.sound.trim().is_empty() {
            parts.push(format!("♪ {}", self.sound.trim()));
        }
        if !self.highlight.trim().is_empty() {
            parts.push(self.highlight.trim().to_owned());
        }
        if !self.panel.trim().is_empty() {
            parts.push(format!("▸ {}", self.panel.trim()));
        }
        if self.gag {
            parts.push("∅".to_owned());
        }

        parts.join("  ·  ")
    }
}

impl Alias {
    /// The regular expression the alias matches typed commands with.
    pub fn regex_source(&self) -> String {
        if self.regex {
            self.pattern.clone()
        } else {
            // The word alone, or followed by arguments, which are %1.
            format!("^{}(?:\\s+(.*))?$", regex_escape(self.pattern.trim()))
        }
    }
}
