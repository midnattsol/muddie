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
    #[serde(alias = "script")]
    pub code: String,
    /// What it does is its Luau, not the commands.
    #[serde(alias = "lua")]
    pub luau: bool,
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
            code: String::new(),
            luau: false,
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
    /// Show the line, but not read it aloud.
    pub silence: bool,
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

/// A timer does something after a while, once or again and again. It is
/// saved in seconds, as it is given everywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "TimeSaved", into = "TimeSaved")]
pub struct OnTime {
    /// How long it waits, in tenths of a second.
    pub tenths: u32,
    /// Goes off every so long, instead of only once after starting.
    pub repeat: bool,
}

/// A timer as it is saved: how long it waits in seconds, or, as it was
/// saved once, in tenths of a second.
#[derive(Serialize, Deserialize)]
struct TimeSaved {
    #[serde(default)]
    seconds: Option<f64>,
    #[serde(default, skip_serializing)]
    tenths: Option<u32>,
    #[serde(default = "yes")]
    repeat: bool,
}

impl From<TimeSaved> for OnTime {
    fn from(saved: TimeSaved) -> Self {
        let tenths = match (saved.seconds, saved.tenths) {
            (Some(seconds), _) => (seconds * 10.0).round().max(0.0) as u32,
            (None, Some(tenths)) => tenths,
            (None, None) => Self::default().tenths,
        };
        Self {
            tenths,
            repeat: saved.repeat,
        }
    }
}

impl From<OnTime> for TimeSaved {
    fn from(time: OnTime) -> Self {
        Self {
            seconds: Some(f64::from(time.tenths) / 10.0),
            tenths: None,
            repeat: time.repeat,
        }
    }
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

/// What a part of a rule's line in a list is: each is shown with its own
/// picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The commands it sends.
    Send,
    /// That it runs a Luau script.
    Script,
    /// What it says aloud.
    Speak,
    Sound,
    /// The colour the match is drawn in.
    Highlight,
    /// The panel it shows the line in.
    Panel,
    /// That it hides the line.
    Gag,
    /// That it keeps the line from being read aloud.
    Silence,
    /// The key that sets it off.
    Key,
    /// How often it goes off, or after how long.
    Time,
}

impl<T: When> Rule<T> {
    /// What it is called: its name, or else what it is made of.
    pub fn title(&self) -> String {
        match self.name.trim() {
            "" => self.when.made_of(),
            name => name.to_owned(),
        }
    }

    /// What it does: its commands, or that it runs a script.
    pub fn does(&self) -> Option<(Part, String)> {
        if self.luau {
            return Some((Part::Script, "Luau".to_owned()));
        }
        let sent = commands(&self.send);
        (!sent.is_empty()).then(|| (Part::Send, sent.join("; ")))
    }
}

impl Rule<OnLine> {
    /// What the trigger does, part by part, for lists. Hiding the line
    /// has no words of its own.
    pub fn summary(&self) -> Vec<(Part, String)> {
        let when = &self.when;
        let mut parts: Vec<(Part, String)> = self.does().into_iter().collect();
        for (part, text) in [
            (Part::Speak, &when.speak),
            (Part::Sound, &when.sound),
            (Part::Highlight, &when.highlight),
            (Part::Panel, &when.panel),
        ] {
            if !text.trim().is_empty() {
                parts.push((part, text.trim().to_owned()));
            }
        }
        if when.gag {
            parts.push((Part::Gag, String::new()));
        }
        if when.silence {
            parts.push((Part::Silence, String::new()));
        }
        parts
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

pub use muddie_script::GaugeKind;

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
    /// The folder it is in, as `Folder/Inner`; empty for none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub folder: String,
    /// The tags it carries: each switches it with all else that carries it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
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
    /// The panel of gauges it is shown in, by the panel's name; none shows
    /// it in the first there is.
    #[serde(
        alias = "shown_in",
        deserialize_with = "panel_named",
        skip_serializing_if = "String::is_empty"
    )]
    pub panel: String,
}

/// A gauge's panel as it is written: its name, or, as rules were once
/// written, its id, kept as `#` and the id until it is named (see
/// `Tab::name_gauge_panels`).
fn panel_named<'de, D: serde::Deserializer<'de>>(given: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Written {
        Name(String),
        Id(u32),
    }
    Ok(match Written::deserialize(given)? {
        Written::Name(name) => name,
        Written::Id(0) => String::new(),
        Written::Id(id) => format!("#{id}"),
    })
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

/// A gauge's kind, as the interface names it.
pub trait KindName {
    /// Its name, as shown.
    fn shown(self) -> &'static str;
}

impl KindName for GaugeKind {
    fn shown(self) -> &'static str {
        use crate::i18n::tr;
        match self {
            Self::Bar => tr("Bar"),
            Self::Cells => tr("Cells"),
            Self::Number => tr("Number"),
            Self::Light => tr("Light"),
            Self::Text => tr("Text"),
            Self::Effects => tr("Effects"),
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

    /// Whether there is enough of it to run, as for any rule: what it
    /// follows.
    pub fn complete(&self) -> bool {
        !self.value.trim().is_empty()
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

/// The regular expression a pattern is: itself if it is one, else what
/// `literal` makes of the words it is.
fn pattern_source(regex: bool, pattern: &str, literal: impl Fn(&str) -> String) -> String {
    match regex {
        true => pattern.to_owned(),
        false => literal(pattern),
    }
}

impl OnLine {
    /// The regular expression the trigger matches lines with: the words
    /// anywhere in the line.
    pub fn regex_source(&self) -> String {
        pattern_source(self.regex, &self.pattern, regex_escape)
    }
}

impl OnTyped {
    /// The regular expression the alias matches typed commands with: the
    /// word alone, or followed by arguments, which are %1.
    pub fn regex_source(&self) -> String {
        pattern_source(self.regex, &self.pattern, |word| {
            format!("^{}(?:\\s+(.*))?$", regex_escape(word.trim()))
        })
    }
}
