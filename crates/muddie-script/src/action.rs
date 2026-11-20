//! What scripts ask the client to do, and how they want a gauge to look.

use super::*;

/// Something a script asked the client to do.
/// How a gauge is drawn, as far as the script cares to say.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GaugeLook {
    /// A colour name or `#rrggbb`; empty lets the client choose.
    pub colour: String,
    /// Drawn as a row of cells instead of a continuous bar.
    pub segments: bool,
    /// The percentage under which the numbers turn to a warning colour.
    pub low: f64,
    /// What it is drawn as: `bar`, `cells`, `number`, `light` or `text`;
    /// empty for a bar, or cells if `segments`.
    pub kind: String,
    /// The colour it takes under each percentage, whatever its order.
    pub steps: Vec<(f64, String)>,
    /// What a gauge of the `text` kind says.
    pub text: String,
    /// The colour of a light while it is off; empty leaves it unlit.
    pub off_colour: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Send(String),
    Echo(String),
    Speak {
        text: String,
        interrupt: bool,
    },
    /// Send a GMCP message; `data` is JSON, or empty for none.
    Gmcp {
        package: String,
        data: String,
    },
    /// Set the named gauge to `(value, max)`, or remove it.
    Gauge {
        name: String,
        level: Option<(f64, f64)>,
        look: GaugeLook,
    },
    /// Add a line to the chat panel.
    Chat(String),
    /// A line said on a channel of the MUD's, by the channel's name; or
    /// with no text, a channel there is, to have its tab before anyone
    /// speaks on it.
    /// `panel` names the panel it is shown in, made if there is none of
    /// that name; empty for the panel of channels.
    Channel {
        name: String,
        text: Option<String>,
        panel: String,
    },
    /// Add lines to the end of a panel of text.
    PanelAdd {
        name: String,
        text: String,
    },
    /// Add the line a trigger is looking at to a panel, colours and all.
    ToPanel(String),
    /// Words to offer when completing, under a name; none removes them.
    Completions {
        name: String,
        words: Vec<String>,
    },
    /// Set the text of the named panel, or remove the panel.
    Panel {
        name: String,
        text: Option<String>,
    },
    /// Walk to the room with this number.
    Walk(RoomId),
    /// A script moved the player to this room on the map.
    Entered(RoomId),
    /// Send an MSDP message such as `REPORT` with variable names.
    Msdp {
        variable: String,
        values: Vec<String>,
    },
    /// Play a sound file at `volume` (0 to 1), repeating if `looped`.
    PlaySound {
        file: String,
        volume: f32,
        looped: bool,
    },
    StopSounds,
    /// A script failed; the message is for the user.
    Error(String),
}
