//! What scripts ask the client to do, and how they want a gauge to look.

use super::*;

/// How a gauge is drawn, as far as the script cares to say.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GaugeLook {
    /// A colour name or `#rrggbb`; empty lets the client choose.
    pub colour: String,
    /// What it is drawn as.
    pub kind: GaugeKind,
    /// The colour it takes under each percentage, whatever its order.
    pub steps: Vec<(f64, String)>,
    /// What a gauge of the `text` kind says.
    pub text: String,
    /// The colour of a light while it is off; empty leaves it unlit.
    pub off_colour: String,
    /// What a gauge of the `effects` kind shows, in order.
    pub items: Vec<Effect>,
    /// How its changes are heard.
    pub aloud: Aloud,
}

/// When what a script says is heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum How {
    /// As what it is: a level that falls under a step cuts short what is
    /// being said, and the rest wait their turn.
    #[default]
    Usual,
    /// Not at all.
    Never,
    /// After what is being said.
    Wait,
    /// Of a gauge: after what is being said, but cutting it short under the
    /// lowest of its steps.
    Lowest,
    /// Cutting short what is being said.
    Cut,
}

impl How {
    /// Every way, in the order the editor offers them.
    pub const ALL: [Self; 5] = [
        Self::Usual,
        Self::Never,
        Self::Wait,
        Self::Lowest,
        Self::Cut,
    ];

    /// The way of this name, as rules and scripts write it.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|way| way.key() == key.trim())
    }

    /// Its name, as rules and scripts write it.
    pub fn key(self) -> &'static str {
        match self {
            Self::Usual => "usual",
            Self::Never => "never",
            Self::Wait => "wait",
            Self::Lowest => "lowest",
            Self::Cut => "cut",
        }
    }

    /// Whether it is the one there is if none is told.
    pub fn is_usual(&self) -> bool {
        *self == Self::Usual
    }
}

/// How what a script says is heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Aloud {
    pub how: How,
    /// While the game is not read aloud too.
    pub always: bool,
}

/// How a script plays a sound.
#[derive(Debug, Clone, PartialEq)]
pub struct Playing {
    /// From 0 to 1.
    pub volume: f32,
    /// Again and again, until stopped.
    pub looped: bool,
    /// Where in it to start, and to end if before its own end, and how long
    /// it takes to come up and to die away, in seconds.
    pub start: f32,
    pub finish: Option<f32>,
    pub fade_in: f32,
    pub fade_out: f32,
    /// Where it is heard, from -1, all left, to 1, all right.
    pub pan: f32,
    /// The group whose volume it follows: none for the rest.
    pub group: String,
}

impl Default for Playing {
    fn default() -> Self {
        Self {
            volume: 1.0,
            looped: false,
            start: 0.0,
            finish: None,
            fade_in: 0.0,
            fade_out: 0.0,
            pan: 0.0,
            group: String::new(),
        }
    }
}

/// Something said, and how it is heard.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Saying {
    pub text: String,
    pub aloud: Aloud,
}

impl<T: Into<String>> From<T> for Saying {
    fn from(text: T) -> Self {
        Self {
            text: text.into(),
            aloud: Aloud::default(),
        }
    }
}

/// What a gauge is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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
    /// What the character is in, each as a label: poisoned, blessed.
    Effects,
}

impl GaugeKind {
    /// Every kind, in the order the editor offers them.
    pub const ALL: [Self; 6] = [
        Self::Bar,
        Self::Cells,
        Self::Number,
        Self::Light,
        Self::Text,
        Self::Effects,
    ];

    /// Whether it shows how full something is, and so has a maximum and
    /// can change colour by level.
    pub fn has_level(self) -> bool {
        matches!(self, Self::Bar | Self::Cells | Self::Number)
    }

    /// The kind of this name, as rules and scripts write it.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key.trim())
    }

    /// Its name, as rules and scripts write it.
    pub fn key(self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Cells => "cells",
            Self::Number => "number",
            Self::Light => "light",
            Self::Text => "text",
            Self::Effects => "effects",
        }
    }
}

/// One of what a gauge of effects shows: a state the character is in, as
/// poisoned or blessed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Effect {
    pub name: String,
    /// What it is, said and shown with it; empty for nothing.
    pub about: String,
    /// A colour name or `#rrggbb`; empty takes the gauge's.
    pub colour: String,
}

/// Something a script asked the client to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Send(String),
    /// The same, not told to who listens to what is sent: it is of theirs.
    SendAsIs(String),
    Echo(Saying),
    Speak(Saying),
    /// Send a GMCP message; `data` is JSON, or empty for none.
    Gmcp {
        package: String,
        data: String,
    },
    /// Set the named gauge to `(value, max)`, or remove it. `owner` is
    /// the script that set it, or `rules` for the rules.
    Gauge {
        name: String,
        level: Option<(f64, f64)>,
        look: GaugeLook,
        owner: String,
        /// What it is switched by: its place and its tags.
        belongs: Belongs,
        /// The panel of gauges it is shown in, by its name; empty for the
        /// first of them.
        panel: String,
    },
    /// A script was stopped: what it showed, as its gauges, goes.
    Unloaded(String),
    /// The MUD is played in this language, by its code or name: its words
    /// for directions are those of the map, under any a script teaches.
    Language(String),
    /// A channel of the MUD's is no longer heard: its tab goes.
    ChannelClosed(String),
    /// Something a script tells the user, as the client tells its own:
    /// over the corner, aloud, and under the bell.
    Notify(Saying),
    /// A line said on a channel of the MUD's, by the channel's name; or
    /// with no text, a channel there is, to have its tab before anyone
    /// speaks on it.
    /// `panel` names the panel it is shown in, made if there is none of
    /// that name; empty for the panel of channels.
    Channel {
        name: String,
        text: Option<String>,
        panel: String,
        aloud: Aloud,
    },
    /// Add lines to the end of a panel of text.
    PanelAdd {
        name: String,
        text: Saying,
    },
    /// Add the line a trigger is looking at to a panel, colours and all.
    ToPanel(String),
    /// Set the text of the named panel, or remove the panel.
    Panel {
        name: String,
        text: Option<String>,
    },
    /// Walk to the room with this number.
    Walk(RoomId),
    /// A script moved the player to this room on the map.
    Entered(RoomId),
    /// The map lost track of where the player is: it cannot tell which of
    /// several rooms it is, or knows none like it.
    LostTrack,
    /// The server said a move failed: the moves sent did not happen, and
    /// a walk stops.
    MoveFailed,
    /// Stop walking, as the user would.
    StopWalk,
    /// Show this area on the map, to look at it, as the user would.
    ShowArea(String),
    /// Take the map made with Mudlet at this address in place of the one
    /// there is; if it was taken from there, only if it has changed.
    ImportMap(String),
    /// Find out whether the map at this address is newer than the one the
    /// map was taken from it, and answer `answer` (see
    /// [`crate::Engine::answer`]) if it is.
    MapNewer {
        address: String,
        answer: u64,
    },
    /// What the scripts offer to do with the map has changed.
    MapActions,
    /// Send an MSDP message such as `REPORT` with variable names.
    Msdp {
        variable: String,
        values: Vec<String>,
    },
    /// Play a sound file; `owner` is the script that asked, whose sounds stop with it.
    PlaySound {
        /// What the script stops it by.
        id: u64,
        file: String,
        how: Playing,
        owner: String,
    },
    StopSounds,
    /// Stop the sound played with this id.
    StopSound(u64),
    /// Silence what is being read.
    Hush,
    /// Put this in the command line.
    Input(String),
    /// Take this as if the MUD had sent it.
    Feed(String),
    /// Offer these, by what each says and what more it says of itself,
    /// to choose one under `title`: what is chosen, or that none was, is
    /// told back with [`crate::Engine::chosen`] and `id`.
    Choose {
        id: u64,
        title: String,
        items: Vec<(String, String)>,
    },
    /// Put this on the clipboard.
    Copy(String),
    /// Make the sound played with this id louder or quieter as it plays.
    SoundVolume {
        id: u64,
        volume: f32,
    },
    /// Fetch these sounds of the plugin whose script `owner` is, if it has
    /// them on the web, in the background.
    FetchSounds {
        owner: String,
        names: Vec<String>,
    },
    /// Make a group of sounds louder or quieter, those playing too.
    GroupVolume {
        group: String,
        volume: f32,
    },
    /// Read the game aloud, or not.
    Reading(bool),
    /// A script failed; the message is for the user.
    Error(String),
    /// Something a script shows on the standard output of a run started
    /// with `--debug`; nothing otherwise.
    Log(String),
}
