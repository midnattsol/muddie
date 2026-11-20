//! The panels around the output: what there are, where each one is and
//! how big, kept per profile. They are made, placed and sized on the game
//! itself, in its layout mode, not in a form.

use serde::{Deserialize, Serialize};

use super::*;
use crate::theme::Weight;
use crate::widgets;

mod channels;
mod dragging;
mod gauges;
mod menu;
mod strips;
mod text;

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
            panels: vec![
                panel(1, "Map", Kind::Map, Side::Right, 330.0, false),
                panel(2, "Gauges", Kind::Gauges, Side::Bottom, 96.0, false),
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

    /// Whether it has a panel of this kind.
    pub(super) fn has(&self, kind: Kind) -> bool {
        self.panels.iter().any(|panel| panel.kind == kind)
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
                self.note(&trf("Could not load the panels: {0}", &[&error]));
            }
        }
        self.panels.saved = self.layout.clone();
    }

    /// Saves the panels once they have changed and nothing is being
    /// dragged.
    fn keep_layout(&mut self, ui: &egui::Ui) {
        if self.layout == self.panels.saved || ui.input(|input| input.pointer.any_down()) {
            return;
        }
        self.panels.saved = self.layout.clone();
        if let Some(profiles) = &self.profiles
            && let Err(error) = self
                .layout
                .save(&profiles.layout_path(self.profile.as_deref()))
        {
            self.note(&trf("Could not save the panels: {0}", &[&error]));
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
    /// The layout mode: panels are made, moved and removed.
    /// The panel being dragged to another side, and one to be removed.
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
    /// Stands in for the channels when no panel shows them.
    pub(super) no_chat: Output,
}
