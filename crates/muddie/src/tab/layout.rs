//! The panels around the output: what there are, where each one is and
//! how big, kept per profile. They are placed and sized on the game itself,
//! from their headers, and in the view of the layout.

use serde::{Deserialize, Serialize};

use super::*;
use crate::theme::Weight;
use crate::widgets;

mod channels;
mod dragging;
mod folding;
mod gauges;
mod menu;
mod strips;
mod text;

pub(super) use dragging::{Drop, DropTarget, nearest_side, side_band};
pub(super) use text::well;

/// What a panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Lines of text, sent to it by triggers and scripts.
    #[default]
    Text,
    /// The map.
    Map,
    /// The gauges.
    Gauges,
}

/// How wide the panels beside the output are made, and how tall those
/// above and below it; the gauges, a row of them.
pub(super) const BESIDE: f32 = 330.0;
pub(super) const ACROSS: f32 = 120.0;
pub(super) const GAUGES_HEIGHT: f32 = 96.0;

impl Kind {
    /// Where a panel of this kind goes when it is made, and its size there.
    pub(super) fn home(self) -> (Side, f32) {
        match self {
            Self::Text | Self::Map => (Side::Right, BESIDE),
            Self::Gauges => (Side::Bottom, GAUGES_HEIGHT),
        }
    }
}

/// The side of the output a panel is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    #[default]
    Right,
    Top,
    Bottom,
}

impl Side {
    fn is_beside(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    /// What putting the side away, or bringing it back, is called.
    pub fn toggle_label(self, out: bool) -> &'static str {
        match (self, out) {
            (Self::Left, true) => tr("Collapse the left side"),
            (Self::Left, false) => tr("Expand the left side"),
            (Self::Right, true) => tr("Collapse the right side"),
            (Self::Right, false) => tr("Expand the right side"),
            (Self::Top, true) => tr("Collapse the top side"),
            (Self::Top, false) => tr("Expand the top side"),
            (Self::Bottom, true) => tr("Collapse the bottom side"),
            (Self::Bottom, false) => tr("Expand the bottom side"),
        }
    }

    /// What the edge dragged to make its panels wider or taller is called.
    pub fn size_label(self) -> &'static str {
        match self {
            Self::Left => tr("Size of the panels at the left"),
            Self::Right => tr("Size of the panels at the right"),
            Self::Top => tr("Size of the panels at the top"),
            Self::Bottom => tr("Size of the panels at the bottom"),
        }
    }

    /// The edge of the window it is on.
    pub fn edge(self) -> widgets::Edge {
        match self {
            Self::Left => widgets::Edge::Left,
            Self::Right => widgets::Edge::Right,
            Self::Top => widgets::Edge::Top,
            Self::Bottom => widgets::Edge::Bottom,
        }
    }
}

/// A panel as it is kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelSpec {
    /// What tells it apart from the others, whatever it is renamed to.
    pub id: u32,
    pub name: String,
    pub kind: Kind,
    pub side: Side,
    /// Its width if it is beside the output, its height if above or below.
    pub size: f32,
    pub visible: bool,
    /// A panel of text that shows what is said on the MUD's channels.
    pub channels: bool,
    /// Its share of the side it is on, among the panels there.
    pub weight: f32,
    /// The panels of a side with the same number here, other than none,
    /// share one place as tabs.
    pub stack: u32,
}

impl PanelSpec {
    /// Its name as shown: the ones the program gives are in the language
    /// of the interface; what it is kept as stays, as scripts name it so.
    pub fn shown_name(&self) -> String {
        // "Panel 2" as much as "Map": the name given, with its number.
        let (base, number) = match self.name.rsplit_once(' ') {
            Some((base, number)) if number.parse::<u32>().is_ok() => (base, Some(number)),
            _ => (self.name.as_str(), None),
        };
        let base = match base {
            "Map" => tr("Map"),
            "Gauges" => tr("Gauges"),
            "Panel" => tr("Panel"),
            "Channels" => tr("Channels"),
            _ => return self.name.clone(),
        };
        match number {
            Some(number) => format!("{base} {number}"),
            None => base.to_owned(),
        }
    }
}

impl Default for PanelSpec {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            kind: Kind::Text,
            side: Side::Right,
            size: 300.0,
            visible: true,
            channels: false,
            weight: 1.0,
            stack: 0,
        }
    }
}

/// The panels of a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// The panels beside the output reach the top and the bottom of the
    /// window, and those above and below it fit between them, instead of
    /// the other way round.
    #[serde(default)]
    pub columns_full: bool,
    /// The sides collapsed for now, with their panels as they were, to be
    /// expanded as they are.
    #[serde(default, alias = "hidden", skip_serializing_if = "Vec::is_empty")]
    pub collapsed: Vec<Side>,
    #[serde(rename = "panel", default)]
    pub panels: Vec<PanelSpec>,
}

impl Default for Layout {
    /// What a profile starts with: the map at the right and the gauges
    /// below. The MUD's channels get a panel of their own, under the map,
    /// when the first of them comes.
    fn default() -> Self {
        let panel = |id, name: &str, kind, side, size, channels| PanelSpec {
            id,
            name: name.to_owned(),
            kind,
            side,
            size,
            visible: true,
            channels,
            weight: 1.0,
            stack: 0,
        };
        Self {
            columns_full: false,
            collapsed: Vec::new(),
            panels: vec![
                panel(1, "Map", Kind::Map, Side::Right, BESIDE, false),
                panel(
                    2,
                    "Gauges",
                    Kind::Gauges,
                    Side::Bottom,
                    GAUGES_HEIGHT,
                    false,
                ),
            ],
        }
    }
}

impl Layout {
    /// The panel with this number.
    pub fn panel(&self, id: u32) -> Option<&PanelSpec> {
        self.panels.iter().find(|panel| panel.id == id)
    }

    pub fn panel_mut(&mut self, id: u32) -> Option<&mut PanelSpec> {
        self.panels.iter_mut().find(|panel| panel.id == id)
    }

    /// Reads a layout; a missing file gives the one to start with.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("cannot read {}: {error}", path.display())),
        }
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|error| error.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        }
        std::fs::write(path, text)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    }

    fn next_id(&self) -> u32 {
        self.panels.iter().map(|panel| panel.id).max().unwrap_or(0) + 1
    }

    fn next_stack(&self) -> u32 {
        self.panels
            .iter()
            .map(|panel| panel.stack)
            .max()
            .unwrap_or(0)
            + 1
    }

    /// A name for a new panel no other has: `base`, if `plain` and it is
    /// free, else `base` with the first number free.
    pub(super) fn free_name(&self, base: &str, plain: bool) -> String {
        let taken = |name: &String| self.panels.iter().any(|panel| panel.name == *name);
        let first = plain.then(|| base.to_owned()).into_iter();
        let numbered = (if plain { 2 } else { 1 }..).map(|number| format!("{base} {number}"));
        (first.chain(numbered).take(300))
            .find(|name| !taken(name))
            .unwrap_or_else(|| base.to_owned())
    }

    /// Adds a panel of `kind` called `name`, where one of its kind goes,
    /// that side brought out. Returns its id.
    pub(super) fn add(&mut self, name: &str, kind: Kind) -> u32 {
        let id = self.next_id();
        let (side, size) = kind.home();
        self.panels.push(PanelSpec {
            id,
            name: name.to_owned(),
            kind,
            side,
            size,
            ..PanelSpec::default()
        });
        self.reveal(side);
        id
    }

    /// Whether it has a panel of this kind.
    pub(super) fn has(&self, kind: Kind) -> bool {
        self.panels.iter().any(|panel| panel.kind == kind)
    }

    /// Whether a panel is on screen: shown, on a side not put away.
    pub(super) fn on_screen(&self, panel: &PanelSpec) -> bool {
        panel.visible && !self.collapsed.contains(&panel.side)
    }

    /// The sides with panels to show, in the order of their buttons, and
    /// whether each is out or put away.
    pub(super) fn sides(&self) -> Vec<(Side, bool)> {
        [Side::Left, Side::Top, Side::Bottom, Side::Right]
            .into_iter()
            .filter(|&side| (self.panels.iter()).any(|panel| panel.visible && panel.side == side))
            .map(|side| (side, !self.collapsed.contains(&side)))
            .collect()
    }

    /// Brings a side back, if it was put away.
    pub(super) fn reveal(&mut self, side: Side) {
        self.collapsed.retain(|collapsed| *collapsed != side);
    }

    /// Shows or hides a panel: one shown on a collapsed side expands it.
    pub(super) fn set_shown(&mut self, id: u32, shown: bool) {
        let Some(panel) = self.panel_mut(id) else {
            return;
        };
        panel.visible = shown;
        let side = panel.side;
        if shown {
            self.reveal(side);
        }
    }
}

/// What a panel has been given to show.
#[derive(Default)]
pub struct TextContent {
    /// Lines added one after another, as in a log.
    pub log: Output,
    /// Text that was put there whole, such as a drawing; it replaces
    /// whatever was shown before.
    pub fixed: Vec<Line>,
}

/// A channel of the MUD's: what was said on it, and whether something
/// was that has not been looked at.
pub struct Channel {
    pub name: String,
    /// The panel it is shown in, by its name; empty for the panel of
    /// channels, whichever it is.
    pub panel: String,
    pub log: Output,
    pub unread: bool,
}

impl Channel {
    /// Whether this channel has its tab in `panel`.
    fn shown_in(&self, panel: &PanelSpec) -> bool {
        match self.panel.is_empty() {
            true => panel.channels,
            false => panel.name.eq_ignore_ascii_case(&self.panel),
        }
    }
}

impl Tab {
    /// Puts a side away with its panels, or brings it back as it was.
    pub(super) fn toggle_side(&mut self, side: Side) {
        match self.layout.collapsed.contains(&side) {
            true => self.layout.reveal(side),
            false => self.layout.collapsed.push(side),
        }
    }

    /// Reads the panels of the profile in use.
    pub(super) fn load_layout(&mut self) {
        let loaded = match &self.profiles {
            Some(profiles) => Layout::load(&profiles.layout_path(self.profile.as_deref())),
            None => Ok(Layout::default()),
        };
        match loaded {
            Ok(layout) => self.layout = layout,
            Err(error) => {
                self.layout = Layout::default();
                self.refuse(&trf("Could not load the panels: {0}", &[&error]));
            }
        }
        self.panels.saved = self.layout.clone();
    }

    /// Removes a panel, and what it was given to show.
    /// Removes a panel, and says so.
    pub(super) fn remove_panel(&mut self, index: usize) {
        let gone = self.layout.panels.remove(index);
        self.panels.text.retain(|(id, _)| *id != gone.id);
        self.voice
            .say_now(&trf("{0} removed.", &[&gone.shown_name()]));
    }

    /// Saves the panels once they have changed and nothing is being
    /// dragged.
    pub(super) fn keep_layout(&mut self, ui: &egui::Ui) {
        if self.layout == self.panels.saved || ui.input(|input| input.pointer.any_down()) {
            return;
        }
        self.panels.saved = self.layout.clone();
        if let Some(profiles) = &self.profiles
            && let Err(error) = self
                .layout
                .save(&profiles.layout_path(self.profile.as_deref()))
        {
            self.refuse(&trf("Could not save the panels: {0}", &[&error]));
        }
    }
}

/// What the panels are doing: what each shows, the one being dragged, where
/// each is, which tab of a place is shown, the channels, and the layout as
/// last saved.
#[derive(Default)]
pub(super) struct Panels {
    pub(super) saved: layout::Layout,
    /// What each panel was given to show, by the panel's id.
    pub(super) text: Vec<(u32, layout::TextContent)>,
    /// The panel being dragged to another place.
    pub(super) dragged: Option<u32>,
    /// Where each panel was drawn, to drop another before or after it.
    pub(super) rects: Vec<(u32, egui::Rect)>,
    /// Of the panels that share a place as tabs, by the place: which is
    /// on show; and those that got something while they were not.
    pub(super) shown_tabs: Vec<(u32, u32)>,
    pub(super) unread_tabs: Vec<u32>,
    /// The channels of the MUD's that have been heard of: each with what
    /// was said on it, and whether something was that has not been seen.
    pub(super) channels: Vec<layout::Channel>,
    /// The channel each panel of channels is showing; none is all.
    pub(super) channel_shown: Vec<(u32, String)>,
    pub(super) removed: Option<usize>,
    /// The panel being named again in its header, with the name so far,
    /// and whether the field is yet to take the keyboard.
    pub(super) renaming: Option<(u32, String, bool)>,
    /// Stands in for the channels when no panel shows them.
    pub(super) no_chat: Output,
}
