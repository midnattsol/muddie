//! The kinds of rule made in the editor: what they all have, and what
//! each has of its own.

use super::*;

pub(super) fn yes() -> bool {
    true
}

/// A rule made in the editor: what all kinds have, its name, its switch,
/// the folder it is in, its tags and what it does, and `when`, what makes
/// it go off, which is what each kind has of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule<T> {
    /// What the user calls it; shown in lists instead of what it is made
    /// of.
    pub name: String,
    pub enabled: bool,
    /// The folder it is in, as `Folder/Inner`; empty for none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub folder: String,
    /// The tags it carries: each switches it with all else that carries it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(flatten)]
    pub when: T,
    /// Commands to send, one per line or separated by `;`. `%1`... are
    /// what the match caught, where there is a match.
    pub send: String,
    /// Luau to run instead.
    pub script: String,
    /// What it does is the script, not the commands.
    pub lua: bool,
}

impl<T: Default> Default for Rule<T> {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: true,
            folder: String::new(),
            tags: Vec::new(),
            when: T::default(),
            send: String::new(),
            script: String::new(),
            lua: false,
        }
    }
}

/// What makes a rule of a kind go off.
pub trait When: Default + Clone + PartialEq {
    /// What a new one is called before it has a name.
    const NEW: &'static str;
    /// What it is made of, to call it by when it has no name.
    fn made_of(&self) -> String;
    /// Whether there is enough of it to run.
    fn complete(&self) -> bool;
}

/// A trigger reacts to a line of output.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OnLine {
    /// Text the line must contain, or a regular expression if `regex`.
    pub pattern: String,
    pub regex: bool,
    /// Hide the line.
    pub gag: bool,
    /// Colour to give the matched text, as a name or `#rrggbb`; empty for
    /// none.
    pub highlight: String,
    /// Text to say; `%1`... are what the match caught.
    pub speak: String,
    /// Sound file to play.
    pub sound: String,
    /// A panel of text to show the line in, besides or instead of the
    /// output; empty for none.
    pub panel: String,
}

/// An alias replaces a typed command.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OnTyped {
    /// The word that starts the command, or a regular expression matched
    /// against the whole command if `regex`.
    pub pattern: String,
    pub regex: bool,
}

/// A macro does something at the press of a key.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OnKey {
    /// The key as `bind` names it, such as `f5` or `ctrl+k`.
    pub key: String,
}

/// A timer does something after a while, once or again and again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OnTime {
    /// How long it waits, in tenths of a second.
    pub tenths: u32,
    /// Goes off every so long, instead of only once after starting.
    pub repeat: bool,
}

impl Default for OnTime {
    fn default() -> Self {
        Self {
            tenths: 600,
            repeat: true,
        }
    }
}

pub type Trigger = Rule<OnLine>;
pub type Alias = Rule<OnTyped>;
pub type Macro = Rule<OnKey>;
pub type Timer = Rule<OnTime>;

impl When for OnLine {
    const NEW: &'static str = "New trigger";
    fn made_of(&self) -> String {
        self.pattern.trim().to_owned()
    }
    fn complete(&self) -> bool {
        !self.pattern.trim().is_empty()
    }
}

impl When for OnTyped {
    const NEW: &'static str = "New alias";
    fn made_of(&self) -> String {
        self.pattern.trim().to_owned()
    }
    fn complete(&self) -> bool {
        !self.pattern.trim().is_empty()
    }
}

impl When for OnKey {
    const NEW: &'static str = "New macro";
    fn made_of(&self) -> String {
        self.key.trim().to_owned()
    }
    fn complete(&self) -> bool {
        !self.key.trim().is_empty()
    }
}

impl When for OnTime {
    const NEW: &'static str = "New timer";
    fn made_of(&self) -> String {
        self.interval()
    }
    fn complete(&self) -> bool {
        self.tenths > 0
    }
}

impl<T: When> Rule<T> {
    /// What it is called: its name, or else what it is made of.
    pub fn title(&self) -> String {
        match self.name.trim() {
            "" => self.when.made_of(),
            name => name.to_owned(),
        }
    }

    /// What it does, in a line: its commands, or that it runs a script.
    pub fn does(&self) -> String {
        if self.lua {
            return "Luau".to_owned();
        }
        let sent: Vec<&str> = commands(&self.send).collect();
        match sent.is_empty() {
            true => String::new(),
            false => format!("→ {}", sent.join("; ")),
        }
    }
}

impl Rule<OnLine> {
    /// A line saying what the trigger does, for lists.
    pub fn summary(&self) -> String {
        let when = &self.when;
        let mut parts = vec![self.does()];
        if !when.speak.trim().is_empty() {
            parts.push(format!("“{}”", when.speak.trim()));
        }
        if !when.sound.trim().is_empty() {
            parts.push(format!("♪ {}", when.sound.trim()));
        }
        if !when.highlight.trim().is_empty() {
            parts.push(when.highlight.trim().to_owned());
        }
        if !when.panel.trim().is_empty() {
            parts.push(format!("▸ {}", when.panel.trim()));
        }
        if when.gag {
            parts.push("∅".to_owned());
        }
        parts.retain(|part| !part.is_empty());
        parts.join("  ·  ")
    }
}

/// A place inside a folder, or at the top for none.
pub fn joined(folder: &str, name: &str) -> String {
    match folder.trim() {
        "" => name.trim().to_owned(),
        folder => format!("{folder}/{}", name.trim()),
    }
}

/// A folder of one kind of thing, kept for its place in the list and its
/// switch, and so that it is there while it holds nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub kind: Kind,
    /// Its place, as `Folder/Inner`.
    pub path: String,
    #[serde(default = "yes")]
    pub enabled: bool,
}

/// A script file of the player's, as the editor keeps it: by its place
/// among the scripts, switched off or carrying tags.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptEntry {
    /// Its place, as `Folder/name` with no extension.
    pub path: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
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
    /// The panel of gauges it is shown in, by the panel's id; none shows
    /// it in the first there is.
    pub panel: u32,
}

impl OnTime {
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

impl OnLine {
    /// The regular expression the trigger matches lines with.
    pub fn regex_source(&self) -> String {
        if self.regex {
            self.pattern.clone()
        } else {
            regex_escape(&self.pattern)
        }
    }
}

impl OnTyped {
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
