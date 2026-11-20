//! The panels around the output: what there are, where each one is and
//! how big, kept per profile. They are made, placed and sized on the game
//! itself, in its layout mode, not in a form.

use serde::{Deserialize, Serialize};

use super::*;
use crate::theme::Weight;
use crate::widgets;

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

/// Where a panel dragged over another is put.
#[derive(Clone, Copy, PartialEq)]
enum Drop {
    Before,
    After,
    /// In its place, as a tab beside it.
    Into,
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

/// How many channels are kept apart; what is said on more goes with the
/// rest only.
const MOST_CHANNELS: usize = 64;

/// The frame of a panel: a rounded well on the window's chrome.
pub(super) fn well(left: i8, right: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(theme::current().background)
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .outer_margin(egui::Margin {
            left,
            right,
            top: 4,
            bottom: 4,
        })
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

    /// The content of the panel with this id, made if it had none.
    pub(super) fn text_of(&mut self, id: u32) -> &mut TextContent {
        let index = match self.panels.text.iter().position(|(known, _)| *known == id) {
            Some(index) => index,
            None => {
                let mut content = TextContent::default();
                content
                    .log
                    .restyle(self.settings.scrollback_lines, self.settings.timestamps);
                self.panels.text.push((id, content));
                self.panels.text.len() - 1
            }
        };
        &mut self.panels.text[index].1
    }

    /// The panel a script or a trigger means by `name`, made as a panel
    /// of text if there is none. `map` and `chat` mean the panel that is
    /// the map and the one of the channels, whatever they are called.
    pub(super) fn panel_called(&mut self, name: &str) -> u32 {
        let name = name.trim();
        let panels = &self.layout.panels;
        let by_name = panels
            .iter()
            .find(|panel| panel.name.eq_ignore_ascii_case(name));
        let by_role = || match name.to_lowercase().as_str() {
            "map" => panels.iter().find(|panel| panel.kind == Kind::Map),
            "chat" => panels.iter().find(|panel| panel.channels),
            _ => None,
        };
        if let Some(panel) = by_name.or_else(by_role) {
            return panel.id;
        }
        let id = self.layout.next_id();
        self.layout.panels.push(PanelSpec {
            id,
            name: name.to_owned(),
            ..PanelSpec::default()
        });
        id
    }

    /// Puts `text` in a panel, in place of what it showed; none clears it.
    pub(super) fn show_in_panel(&mut self, name: &str, text: Option<&str>) {
        match text {
            Some(text) => {
                let id = self.panel_called(name);
                self.text_of(id).fixed = panels::styled_lines(text);
            }
            None => {
                let known = self
                    .layout
                    .panels
                    .iter()
                    .find(|panel| panel.name.eq_ignore_ascii_case(name.trim()));
                if let Some(id) = known.map(|panel| panel.id) {
                    self.text_of(id).fixed.clear();
                }
            }
        }
    }

    /// Adds a line at the end of a panel.
    pub(super) fn add_to_panel(&mut self, name: &str, line: &Line) {
        let id = self.panel_called(name);
        self.text_of(id).log.push(line);
        self.note_unread(id);
    }

    /// Marks a panel that got something while another tab of its place
    /// was the one on show.
    fn note_unread(&mut self, id: u32) {
        let stack = self.layout.panel(id).map_or(0, |panel| panel.stack);
        let shown = self
            .panels
            .shown_tabs
            .iter()
            .find(|(known, _)| *known == stack)
            .map(|(_, id)| *id);
        if stack != 0
            && shown.is_some_and(|shown| shown != id)
            && !self.panels.unread_tabs.contains(&id)
        {
            self.panels.unread_tabs.push(id);
        }
    }

    /// Adds what was said on a channel to the panels that show them.
    pub(super) fn channel_line(&mut self, line: &Line) {
        let ids: Vec<u32> = (self.layout.panels.iter())
            .filter(|panel| panel.channels)
            .map(|panel| panel.id)
            .collect();
        for id in ids {
            self.text_of(id).log.push(line);
            self.note_unread(id);
        }
    }

    /// Takes lines said on a channel of the MUD's: they go with the rest
    /// of what is said, and under the channel's own tab. With no lines,
    /// the channel is made known, to have its tab.
    pub(super) fn channel_said(&mut self, name: &str, panel: &str, lines: Vec<Line>) {
        let (name, panel) = (name.trim(), panel.trim());
        if panel.is_empty() {
            // With nowhere to show the channels, they are given a panel:
            // one that is hidden is somewhere, and is left as it is.
            if !self.layout.panels.iter().any(|panel| panel.channels) {
                let id = self.layout.next_id();
                let taken = |name: &str| self.layout.panels.iter().any(|panel| panel.name == name);
                // Under the map, as the last of those at its side.
                let name = ["Channels", "Channels 2", "Channels 3"]
                    .into_iter()
                    .find(|name| !taken(name))
                    .unwrap_or("Channels");
                let side = (self.layout.panels.iter())
                    .find(|panel| panel.kind == Kind::Map)
                    .map_or(Side::Right, |map| map.side);
                let size = (self.layout.panels.iter())
                    .find(|panel| panel.side == side)
                    .map_or(330.0, |panel| panel.size);
                self.layout.panels.push(PanelSpec {
                    id,
                    name: name.to_owned(),
                    kind: Kind::Text,
                    side,
                    size,
                    channels: true,
                    ..PanelSpec::default()
                });
            }
        } else {
            // The panel named: the one of that name, made if there is
            // none, and left where it is and as it is if there is.
            self.panel_called(panel);
        }
        let known = (self.panels.channels.iter()).position(|channel| {
            channel.name.eq_ignore_ascii_case(name) && channel.panel.eq_ignore_ascii_case(panel)
        });
        let index = match known {
            Some(index) => Some(index),
            None if name.is_empty() || self.panels.channels.len() >= MOST_CHANNELS => None,
            None => {
                let mut log = Output::default();
                log.restyle(self.settings.scrollback_lines, self.settings.timestamps);
                self.panels.channels.push(Channel {
                    name: name.to_owned(),
                    panel: panel.to_owned(),
                    log,
                    unread: false,
                });
                Some(self.panels.channels.len() - 1)
            }
        };
        if lines.is_empty() {
            return;
        }
        // Seen as it comes where a panel it is shown in is on this
        // channel, or on all of them.
        let seen = (self.layout.panels.iter()).any(|spec| {
            let hosts = match panel.is_empty() {
                true => spec.channels,
                false => spec.name.eq_ignore_ascii_case(panel),
            };
            let shown = self
                .panels
                .channel_shown
                .iter()
                .find(|(id, _)| *id == spec.id);
            let on_it =
                shown.is_none_or(|(_, shown)| shown.is_empty() || shown.eq_ignore_ascii_case(name));
            hosts && spec.visible && on_it
        });
        for line in &lines {
            // With the rest of what is said there, and under its own tab.
            match panel.is_empty() {
                true => self.channel_line(line),
                false => self.add_to_panel(panel, line),
            }
            if let Some(index) = index {
                self.panels.channels[index].log.push(line);
            }
        }
        if let Some(index) = index {
            self.panels.channels[index].unread |= !seen;
        }
    }

    /// The tabs of a panel of channels: all, and one for each channel,
    /// marked if something was said on it that has not been looked at.
    /// Returns the channel chosen; none for all.
    fn channel_tabs(&mut self, ui: &mut egui::Ui, id: u32) -> String {
        let known = self
            .panels
            .channel_shown
            .iter()
            .find(|(panel, _)| *panel == id);
        let mut shown = known.map(|(_, shown)| shown.clone()).unwrap_or_default();
        let spec = self.layout.panel(id).cloned();
        let here = |channel: &&Channel| spec.as_ref().is_some_and(|spec| channel.shown_in(spec));
        let names: Vec<(String, bool)> = std::iter::once((String::new(), false))
            .chain(
                (self.panels.channels.iter())
                    .filter(here)
                    .map(|channel| (channel.name.clone(), channel.unread)),
            )
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
            for (name, unread) in &names {
                let label = if name.is_empty() {
                    tr("All")
                } else {
                    name.as_str()
                };
                if widgets::tab(ui, label, *name == shown, *unread).clicked() {
                    shown.clone_from(name);
                }
            }
        });
        ui.add_space(2.0);
        self.panels.channel_shown.retain(|(panel, _)| *panel != id);
        self.panels.channel_shown.push((id, shown.clone()));
        shown
    }

    /// Whether the map has its panel on screen.
    pub fn map_shown(&self) -> bool {
        (self.layout.panels.iter()).any(|panel| panel.kind == Kind::Map && panel.visible)
    }

    /// Shows or hides the map's panel, making one if there is none.
    pub(super) fn set_map_shown(&mut self, shown: bool) {
        match self
            .layout
            .panels
            .iter_mut()
            .find(|panel| panel.kind == Kind::Map)
        {
            Some(panel) => panel.visible = shown,
            None if shown => {
                let id = self.layout.next_id();
                self.layout.panels.push(PanelSpec {
                    id,
                    name: "Map".to_owned(),
                    kind: Kind::Map,
                    size: 330.0,
                    ..PanelSpec::default()
                });
            }
            None => {}
        }
    }

    /// The bar of the layout mode: what to do, a way to add a panel, and
    /// the way out.
    pub(super) fn add_panel(&mut self, kind: Kind) {
        let id = self.layout.next_id();
        // A name not taken: the plain one, then numbered.
        let free = |base: &str, first: bool| -> String {
            let taken = |name: &String| self.layout.panels.iter().any(|panel| panel.name == *name);
            let plain = first.then(|| base.to_owned()).into_iter();
            let numbered = (if first { 2 } else { 1 }..).map(|number| format!("{base} {number}"));
            plain
                .chain(numbered)
                .take(300)
                .find(|name| !taken(name))
                .unwrap_or_default()
        };
        let (name, side, size) = match kind {
            Kind::Text => (free("Panel", false), Side::Right, 280.0),
            Kind::Map => ("Map".to_owned(), Side::Right, 330.0),
            Kind::Gauges => (free("Gauges", true), Side::Bottom, 96.0),
        };
        self.layout.panels.push(PanelSpec {
            id,
            name,
            kind,
            side,
            size,
            ..PanelSpec::default()
        });
    }

    /// Draws the panels around the output, which takes what is left.
    /// Each side is one strip that the panels on it share: those beside
    /// the output are stacked, those above or below it stand in a row.
    /// Returns what was asked of the map, if anything.
    pub(super) fn show_panels(&mut self, ui: &mut egui::Ui) -> Option<MapAction> {
        let mut asked = None;
        self.panels.rects.clear();
        // Those above and below first, so that they span the window.
        // Whichever goes first spans the window; the others fit between.
        let order = if self.layout.columns_full {
            [Side::Left, Side::Right, Side::Top, Side::Bottom]
        } else {
            [Side::Top, Side::Bottom, Side::Left, Side::Right]
        };
        for side in order {
            let members: Vec<usize> = (0..self.layout.panels.len())
                .filter(|&index| {
                    self.layout.panels[index].visible && self.layout.panels[index].side == side
                })
                .collect();
            let Some(&first) = members.first() else {
                continue;
            };
            let id = ui.make_persistent_id(("dock", side as u8));
            let (left, right, least) = match side {
                Side::Left => (8, 4, 160.0),
                Side::Right => (4, 8, 160.0),
                // In the layout mode each panel also has its handle's row.
                // Room for the header and a row of gauges under it.
                Side::Top | Side::Bottom => (8, 8, 92.0),
            };
            let around = egui::Frame::new().inner_margin(egui::Margin {
                left,
                right,
                top: 4,
                bottom: 4,
            });
            let strip = match side {
                Side::Left => Panel::left(id),
                Side::Right => Panel::right(id),
                Side::Top => Panel::top(id),
                Side::Bottom => Panel::bottom(id),
            };
            let shown = strip
                .frame(around)
                .show_separator_line(false)
                .resizable(true)
                .default_size(self.layout.panels[first].size)
                .min_size(least)
                .show(ui, |ui| self.strip(ui, side, &members));
            asked = asked.or(shown.inner);
            // As it was left, to be the same the next time.
            let rect = shown.response.rect;
            let size = if side.is_beside() {
                rect.width()
            } else {
                rect.height()
            };
            for index in members {
                self.layout.panels[index].size = size.round();
            }
        }
        self.finish_drag(ui);
        if let Some(index) = self.panels.removed.take() {
            let gone = self.layout.panels.remove(index);
            self.panels.text.retain(|(id, _)| *id != gone.id);
        }
        self.keep_layout(ui);
        asked
    }

    /// The panels of one side, sharing its length by their weights, with
    /// an edge to drag between each two. Panels joined as tabs share one
    /// place, with their names along its top to choose between them.
    fn strip(&mut self, ui: &mut egui::Ui, side: Side, members: &[usize]) -> Option<MapAction> {
        const GAP: f32 = 8.0;
        let theme = theme::current();
        let full = ui.max_rect();
        let stacked = side.is_beside();
        // The places: each a panel alone, or those of one set of tabs.
        let mut places: Vec<Vec<usize>> = Vec::new();
        for &index in members {
            let stack = self.layout.panels[index].stack;
            let joined = places
                .iter_mut()
                .find(|place| stack != 0 && self.layout.panels[place[0]].stack == stack);
            match joined {
                Some(place) => place.push(index),
                None => places.push(vec![index]),
            }
        }
        let along = |rect: egui::Rect| if stacked { rect.height() } else { rect.width() };
        let length = (along(full) - GAP * (places.len() - 1) as f32).max(1.0);
        let weight = |tab: &Self, place: &[usize]| tab.layout.panels[place[0]].weight.max(0.05);
        let total: f32 = places.iter().map(|place| weight(self, place)).sum();
        let mut asked = None;
        let mut at = if stacked { full.top() } else { full.left() };
        for (number, place) in places.iter().enumerate() {
            let share = length * weight(self, place) / total;
            let rect = if stacked {
                egui::Rect::from_min_max(
                    egui::pos2(full.left(), at),
                    egui::pos2(full.right(), at + share),
                )
            } else {
                egui::Rect::from_min_max(
                    egui::pos2(at, full.top()),
                    egui::pos2(at + share, full.bottom()),
                )
            };
            for &index in place {
                self.panels.rects.push((self.layout.panels[index].id, rect));
            }
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(12), theme.background);
            let inner = rect.shrink2(egui::vec2(14.0, 10.0));
            let builder = egui::UiBuilder::new()
                .id_salt(("place", self.layout.panels[place[0]].id))
                .max_rect(inner)
                .layout(egui::Layout::top_down(egui::Align::Min));
            let got = ui.scope_builder(builder, |ui| {
                ui.set_clip_rect(inner.intersect(ui.clip_rect()));
                let index = self.panel_header(ui, place);
                let spec = self.layout.panels[index].clone();
                self.panel_body(ui, &spec)
            });
            asked = asked.or(got.inner);
            at += share;
            let Some(next) = places.get(number + 1).map(|next| next[0]) else {
                continue;
            };
            let lead = place[0];
            // The edge between this place and the next, to trade room.
            let edge = if stacked {
                egui::Rect::from_min_max(
                    egui::pos2(full.left(), at),
                    egui::pos2(full.right(), at + GAP),
                )
            } else {
                egui::Rect::from_min_max(
                    egui::pos2(at, full.top()),
                    egui::pos2(at + GAP, full.bottom()),
                )
            };
            let cursor = if stacked {
                egui::CursorIcon::ResizeVertical
            } else {
                egui::CursorIcon::ResizeHorizontal
            };
            let id = ui.make_persistent_id(("panel edge", self.layout.panels[lead].id));
            let handle = ui
                .interact(edge, id, egui::Sense::drag())
                .on_hover_cursor(cursor);
            handle.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Splitter, true, tr("Resize"))
            });
            if handle.dragged() {
                let moved = if stacked {
                    handle.drag_delta().y
                } else {
                    handle.drag_delta().x
                };
                // Neither gives up its last stretch.
                let least = total * 48.0 / length;
                let give = (moved * total / length)
                    .min(self.layout.panels[next].weight - least)
                    .max(least - self.layout.panels[lead].weight);
                self.layout.panels[lead].weight += give;
                self.layout.panels[next].weight -= give;
                ui.ctx().set_cursor_icon(cursor);
            }
            if handle.hovered() || handle.dragged() {
                let reach = if stacked {
                    egui::vec2(18.0, 0.0)
                } else {
                    egui::vec2(0.0, 18.0)
                };
                let stroke = egui::Stroke::new(3.0, theme.accent);
                ui.painter()
                    .line_segment([edge.center() - reach, edge.center() + reach], stroke);
            }
            at += GAP;
        }
        ui.allocate_rect(full, egui::Sense::hover());
        asked
    }

    /// The names of the panels that share a place, to choose which is on
    /// show. One that got something while out of sight has a dot. Returns
    /// the one on show.
    fn tab_bar(&mut self, ui: &mut egui::Ui, place: &[usize]) -> usize {
        let stack = self.layout.panels[place[0]].stack;
        let ids: Vec<u32> = place
            .iter()
            .map(|&index| self.layout.panels[index].id)
            .collect();
        let kept = self
            .panels
            .shown_tabs
            .iter()
            .find(|(known, _)| *known == stack)
            .map(|(_, id)| *id);
        let mut shown = kept.filter(|id| ids.contains(id)).unwrap_or(ids[0]);
        for &index in place {
            let panel = &self.layout.panels[index];
            let unread = self.panels.unread_tabs.contains(&panel.id);
            if widgets::tab(ui, &panel.name.clone(), panel.id == shown, unread).clicked() {
                shown = panel.id;
            }
        }
        self.panels.shown_tabs.retain(|(known, _)| *known != stack);
        self.panels.shown_tabs.push((stack, shown));
        self.panels.unread_tabs.retain(|id| *id != shown);
        place[ids.iter().position(|id| *id == shown).unwrap_or(0)]
    }

    /// While a panel is dragged by its handle, shows where it would go;
    /// once it is let go, puts it there: before or after the panel it is
    /// over, or at the end of the side it is nearest to.
    fn finish_drag(&mut self, ui: &egui::Ui) {
        let Some(id) = self.panels.dragged else {
            return;
        };
        let area = ui.ctx().content_rect();
        let Some(pointer) = ui.ctx().pointer_latest_pos() else {
            self.panels.dragged = None;
            return;
        };
        // Over another panel: its side, and by where on it, before it,
        // after it, or in its place as a tab.
        let over = (self.panels.rects.iter())
            .find(|(other, rect)| *other != id && rect.contains(pointer))
            .copied();
        let onto = over.and_then(|(other, rect)| {
            let side = self.layout.panel(other)?.side;
            let (start, length, at) = if side.is_beside() {
                (rect.top(), rect.height(), pointer.y)
            } else {
                (rect.left(), rect.width(), pointer.x)
            };
            let part = (at - start) / length.max(1.0);
            let (drop, from, to) = match part {
                part if part < 0.3 => (Drop::Before, 0.0, 0.3),
                part if part > 0.7 => (Drop::After, 0.7, 1.0),
                _ => (Drop::Into, 0.3, 0.7),
            };
            let band = if side.is_beside() {
                egui::Rect::from_min_max(
                    egui::pos2(rect.left(), start + length * from),
                    egui::pos2(rect.right(), start + length * to),
                )
            } else {
                egui::Rect::from_min_max(
                    egui::pos2(start + length * from, rect.top()),
                    egui::pos2(start + length * to, rect.bottom()),
                )
            };
            Some((other, side, drop, band))
        });
        // Over nothing: the side the pointer is nearest to, each way in
        // its own measure.
        let reach = [
            (Side::Left, (pointer.x - area.left()) / area.width()),
            (Side::Right, (area.right() - pointer.x) / area.width()),
            (Side::Top, (pointer.y - area.top()) / area.height()),
            (Side::Bottom, (area.bottom() - pointer.y) / area.height()),
        ];
        let nearest = reach
            .into_iter()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(Side::Right, |(side, _)| side);
        if ui.input(|input| input.pointer.any_down()) {
            let band = match (onto, nearest) {
                (Some((.., half)), _) => half,
                (None, Side::Left) => area.with_max_x(area.left() + area.width() * 0.28),
                (None, Side::Right) => area.with_min_x(area.right() - area.width() * 0.28),
                (None, Side::Top) => area.with_max_y(area.top() + area.height() * 0.28),
                (None, Side::Bottom) => area.with_min_y(area.bottom() - area.height() * 0.28),
            };
            let theme = theme::current();
            let layer =
                egui::LayerId::new(egui::Order::Foreground, ui.make_persistent_id("panel drop"));
            let painter = ui.ctx().layer_painter(layer);
            let band = band.shrink(6.0);
            painter.rect_filled(
                band,
                egui::CornerRadius::same(12),
                theme.accent.gamma_multiply(0.14),
            );
            let edge = egui::Stroke::new(1.5, theme.accent.gamma_multiply(0.7));
            painter.rect_stroke(
                band,
                egui::CornerRadius::same(12),
                edge,
                egui::StrokeKind::Inside,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            return;
        }
        self.panels.dragged = None;
        let Some(from) = self.layout.panels.iter().position(|panel| panel.id == id) else {
            return;
        };
        let mut moved = self.layout.panels.remove(from);
        let side = onto.map_or(nearest, |(_, side, ..)| side);
        // It takes the size of the side it joins, or one that suits it.
        let there = self
            .layout
            .panels
            .iter()
            .find(|panel| panel.visible && panel.side == side);
        moved.size = match there {
            Some(panel) => panel.size,
            None if moved.side.is_beside() == side.is_beside() => moved.size,
            None if side.is_beside() => 300.0,
            None => 120.0,
        };
        moved.side = side;
        moved.weight = 1.0;
        moved.stack = 0;
        let Some((other, _, drop, _)) = onto else {
            return self.layout.panels.push(moved);
        };
        let Some(place) = self
            .layout
            .panels
            .iter()
            .position(|panel| panel.id == other)
        else {
            return self.layout.panels.push(moved);
        };
        // The panels the one dropped on shares its place with, if any.
        let stack = self.layout.panels[place].stack;
        let shares = |panel: &PanelSpec| stack != 0 && panel.stack == stack && panel.side == side;
        let first = self.layout.panels.iter().position(&shares).unwrap_or(place);
        let last = self.layout.panels.iter().rposition(shares).unwrap_or(place);
        match drop {
            Drop::Before => self.layout.panels.insert(first, moved),
            Drop::After => self.layout.panels.insert(last + 1, moved),
            Drop::Into => {
                // Joined as a tab, and shown, so that it is seen to be there.
                let joined = if stack == 0 {
                    self.layout.next_stack()
                } else {
                    stack
                };
                self.layout.panels[place].stack = joined;
                moved.stack = joined;
                moved.weight = self.layout.panels[place].weight;
                self.panels.shown_tabs.retain(|(known, _)| *known != joined);
                self.panels.shown_tabs.push((joined, moved.id));
                self.layout.panels.insert(last + 1, moved);
            }
        }
    }

    /// Makes a panel share the place of the one before it on its side.
    fn join_before(&mut self, index: usize) {
        let side = self.layout.panels[index].side;
        let Some(before) = (0..index).rev().find(|&other| {
            self.layout.panels[other].visible && self.layout.panels[other].side == side
        }) else {
            return;
        };
        let stack = match self.layout.panels[before].stack {
            0 => self.layout.next_stack(),
            stack => stack,
        };
        self.layout.panels[before].stack = stack;
        self.layout.panels[index].stack = stack;
        let id = self.layout.panels[index].id;
        self.panels.shown_tabs.retain(|(known, _)| *known != stack);
        self.panels.shown_tabs.push((stack, id));
    }

    /// Moves a panel one place among those of its side: `-1` earlier,
    /// `1` later.
    fn shift_panel(&mut self, index: usize, by: isize) {
        let side = self.layout.panels[index].side;
        let same: Vec<usize> = (0..self.layout.panels.len())
            .filter(|&other| self.layout.panels[other].side == side)
            .collect();
        let place = same.iter().position(|&other| other == index).unwrap_or(0);
        if let Some(&other) = place
            .checked_add_signed(by)
            .and_then(|wanted| same.get(wanted))
        {
            self.layout.panels.swap(index, other);
        }
    }

    /// What a panel shows, by its kind.
    fn panel_body(&mut self, ui: &mut egui::Ui, spec: &PanelSpec) -> Option<MapAction> {
        match spec.kind {
            Kind::Map => self.map_body(ui, spec.id),
            Kind::Gauges => {
                self.gauges_body(ui, spec.id);
                None
            }
            Kind::Text => {
                self.text_body(ui, spec.id, false);
                None
            }
        }
    }

    /// The header every panel has: a handle to drag it to another place,
    /// its name, or the names of those that share its place as tabs; the
    /// tools of its kind, and its menu. Returns the panel on show.
    fn panel_header(&mut self, ui: &mut egui::Ui, place: &[usize]) -> usize {
        let theme = theme::current();
        let mut shown = place[0];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.set_min_height(24.0);
            // Six dots: this can be taken and moved.
            let name = self.layout.panels[shown].name.clone();
            let (grip, handle) =
                ui.allocate_exact_size(egui::vec2(12.0, 22.0), egui::Sense::drag());
            let handle = handle.on_hover_cursor(egui::CursorIcon::Grab);
            handle.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, true, trf("Move {0}", &[&name]))
            });
            let dots = if handle.hovered() || handle.dragged() {
                theme.text
            } else {
                theme.text_faint
            };
            for (column, row) in [(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2)] {
                let dot =
                    grip.center() + egui::vec2(-2.5 + 5.0 * column as f32, -5.0 + 5.0 * row as f32);
                ui.painter().circle_filled(dot, 1.2, dots);
            }
            if place.len() > 1 {
                shown = self.tab_bar(ui, place);
            } else {
                let caption = RichText::new(name.to_uppercase())
                    .font(theme::font(Weight::Medium, 10.5))
                    .color(theme.text_dim);
                ui.add(Label::new(caption).selectable(false));
            }
            if handle.drag_started() {
                self.panels.dragged = Some(self.layout.panels[place[0]].id);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.panel_menu(ui, shown);
                let kind = self.layout.panels[shown].kind;
                if kind == Kind::Map && self.map.borrow().current().is_some() {
                    let walking = self.walk.is_some();
                    self.map_view.tools(ui, walking);
                }
            });
        });
        ui.add_space(2.0);
        shown
    }

    /// What a panel's menu has: its name, where it goes, how it shares
    /// its place, what goes with its kind, and taking it away.
    fn panel_menu(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(spec) = self.layout.panels.get(index).cloned() else {
            return;
        };
        let more = widgets::icon_button(
            ui,
            widgets::Icon::More,
            &trf("{0} options", &[&spec.name.clone()]),
            false,
        );
        let (mut earlier, mut later, mut join, mut gauges) = (false, false, false, false);
        egui::Popup::menu(&more).show(|ui| {
            widgets::menu_width(ui);
            let panel = &mut self.layout.panels[index];
            widgets::menu_heading(ui, tr("Name"));
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let name = ui.add(TextEdit::singleline(&mut panel.name).desired_width(200.0));
                ui.ctx()
                    .accesskit_node_builder(name.id, |node| node.set_label(tr("Name")));
            });
            widgets::menu_separator(ui);
            widgets::menu_heading(ui, tr("Place"));
            for (side, label) in [
                (Side::Left, tr("At the left")),
                (Side::Right, tr("At the right")),
                (Side::Top, tr("Above")),
                (Side::Bottom, tr("Below")),
            ] {
                let mut here = panel.side == side;
                if widgets::menu_check(ui, &mut here, label).clicked() && panel.side != side {
                    if panel.side.is_beside() != side.is_beside() {
                        panel.size = if side.is_beside() { 300.0 } else { 120.0 };
                    }
                    panel.side = side;
                    ui.close();
                }
            }
            let (before, after) = match panel.side.is_beside() {
                true => (tr("Move up"), tr("Move down")),
                false => (tr("Move left"), tr("Move right")),
            };
            if widgets::menu_item(ui, before, "").clicked() {
                earlier = true;
                ui.close();
            }
            if widgets::menu_item(ui, after, "").clicked() {
                later = true;
                ui.close();
            }
            if spec.stack == 0 {
                if widgets::menu_item(ui, tr("Share the place of the one before, as tabs"), "")
                    .clicked()
                {
                    join = true;
                    ui.close();
                }
            } else if widgets::menu_item(ui, tr("Detach from tabs"), "").clicked() {
                panel.stack = 0;
                ui.close();
            }
            widgets::menu_separator(ui);
            match spec.kind {
                Kind::Text => {
                    widgets::menu_check(
                        ui,
                        &mut panel.channels,
                        tr("Show the MUD's channels here"),
                    );
                }
                Kind::Gauges => {
                    if widgets::menu_item(ui, tr("Edit the gauges"), "").clicked() {
                        gauges = true;
                        ui.close();
                    }
                }
                Kind::Map => self.map_view.menu_items(ui),
            }
            let corners = tr("Full-height sides");
            widgets::menu_check(ui, &mut self.layout.columns_full, corners)
                .on_hover_text(tr("Side panels reach the top and the bottom"));
            widgets::menu_separator(ui);
            let panel = &mut self.layout.panels[index];
            if widgets::menu_item(ui, &trf("Hide {0}", &[&spec.name.clone()]), "").clicked() {
                panel.visible = false;
                ui.close();
            }
            if widgets::menu_item(ui, &trf("Remove {0}", &[&spec.name.clone()]), "").clicked() {
                self.panels.removed = Some(index);
                ui.close();
            }
        });
        if earlier || later {
            self.shift_panel(index, if earlier { -1 } else { 1 });
        }
        if join {
            self.join_before(index);
        }
        if gauges {
            self.open_gauges(spec.id);
        }
    }

    /// Whether the gauge called `name` belongs in the panel `id`: the
    /// one it was put in, or the first of the gauges' panels if it was put
    /// in none, as those the MUD's vitals and scripts make are.
    pub(super) fn gauge_is_in(&self, name: &str, id: u32) -> bool {
        let of_gauges = |panel: &&PanelSpec| panel.kind == Kind::Gauges;
        // Its own panel if that is one of gauges, else the first of them.
        let made = (self.rules.gauges.iter()).find(|gauge| gauge.shown_name() == name);
        let own = made.and_then(|gauge| self.layout.panel(gauge.panel).filter(of_gauges));
        let home = own.or_else(|| self.layout.panels.iter().find(of_gauges));
        home.is_some_and(|panel| panel.id == id)
    }

    /// The gauges of a panel in a row, or how to get some.
    fn gauges_body(&mut self, ui: &mut egui::Ui, id: u32) {
        // In the order they have in the editor, whichever the MUD
        // reported first.
        let made = &self.rules.gauges;
        (self.gauges).arrange(|name| made.iter().position(|gauge| gauge.shown_name() == name));
        let made = |name: &str| {
            self.rules
                .gauges
                .iter()
                .any(|gauge| gauge.shown_name() == name)
        };
        let (any, pressed) = self
            .gauges
            .show(ui, |name| self.gauge_is_in(name, id), made);
        if let Some(name) = pressed {
            self.edit_gauge_here(&name);
        }
        if any {
            // The way to change them, at the corner, out of their way.
            return;
        }
        let theme = theme::current();
        ui.horizontal_wrapped(|ui| {
            let none = tr("No gauges here yet.");
            ui.label(
                RichText::new(none)
                    .font(theme::font(Weight::Regular, 12.5))
                    .color(theme.text_faint),
            );
            if widgets::quiet_button(ui, tr("Add gauges")).clicked() {
                self.open_gauges(id);
            }
        });
    }

    /// What a panel was given: text put there whole, centred as a block
    /// since it is often a drawing, or else its lines one after another.
    /// Returns whether there was anything.
    pub(super) fn text_body(&mut self, ui: &mut egui::Ui, id: u32, quiet: bool) -> bool {
        // The drawing put in it, if there is one, laid out where it is
        // kept: it is not copied to be drawn.
        let galleys: Vec<_> = (self.text_of(id).fixed.iter())
            .map(|line| ui.painter().layout_job(server_job(line)))
            .collect();
        // What a drawing put in it has to fit in, there or yet to come.
        // The map's panel is measured where the map is drawn.
        let map =
            (self.layout.panels.iter()).any(|panel| panel.id == id && panel.kind == Kind::Map);
        if !map {
            let cells = Self::cells(ui, ui.available_size());
            self.panel_measured(id, cells, !galleys.is_empty());
        }
        if !galleys.is_empty() {
            let widest = galleys
                .iter()
                .map(|galley| galley.size().x)
                .fold(0.0, f32::max);
            let height: f32 = galleys.iter().map(|galley| galley.size().y).sum();
            let left = ((ui.available_width() - widest) / 2.0).max(0.0);
            ui.add_space(((ui.available_height() - height) / 2.0).max(0.0));
            ui.horizontal(|ui| {
                ui.add_space(left);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for galley in galleys {
                        // Never wrapped: the columns must stay in line.
                        ui.add(Label::new(galley).extend());
                    }
                });
            });
            return true;
        }
        // A panel that channels are shown in has a tab for each of them:
        // one alone, or all.
        let spec = self.layout.panel(id).cloned();
        let here = |channel: &Channel| spec.as_ref().is_some_and(|spec| channel.shown_in(spec));
        if self.panels.channels.iter().any(here) {
            let shown = self.channel_tabs(ui, id);
            let chosen = (self.panels.channels.iter_mut())
                .find(|channel| !shown.is_empty() && channel.name == shown && here(channel));
            match chosen {
                Some(channel) => {
                    channel.unread = false;
                    let clicked = match channel.log.len() {
                        0 => None,
                        _ => channel.log.show(ui, &format!("panel {id} channel {shown}")),
                    };
                    if let Some(clicked) = clicked {
                        self.link_clicked(clicked);
                    }
                    return true;
                }
                // With all of them in view there is nothing unseen.
                None => (self.panels.channels.iter_mut())
                    .filter(|channel| here(channel))
                    .for_each(|channel| channel.unread = false),
            }
        }
        if self.text_of(id).log.len() > 0 {
            let clicked = self.text_of(id).log.show(ui, &format!("panel {id}"));
            if let Some(clicked) = clicked {
                self.link_clicked(clicked);
            }
            return true;
        }
        if !quiet {
            let theme = theme::current();
            let none = tr(
                "Nothing here yet. A trigger can show lines in this panel, and so can a script.",
            );
            ui.label(
                RichText::new(none)
                    .font(theme::font(Weight::Regular, 12.5))
                    .color(theme.text_faint),
            );
        }
        false
    }

    /// Opens the details of a gauge made in the editor over the game, to
    /// change it where it is shown.
    pub(super) fn edit_gauge_here(&mut self, name: &str) {
        let Some(index) = (self.rules.gauges.iter()).position(|gauge| gauge.shown_name() == name)
        else {
            return;
        };
        if !self.editor.open {
            self.editor.draft = self.rules.clone();
            self.editor.seen = self.rules.clone();
        }
        self.editor.gauge_here = Some(index);
    }

    /// The details of the gauge opened over the game: the same as in the
    /// editor, saved the same way, with the way to the editor.
    pub(super) fn gauge_window(&mut self, ctx: &egui::Context) {
        let Some(index) = self.editor.gauge_here else {
            return;
        };
        if index >= self.editor.draft.gauges.len() || self.editor.open {
            self.editor.gauge_here = None;
            return;
        }
        let (mut open, mut delete, mut editor) = (true, false, false);
        let name = self.editor.draft.gauges[index].shown_name().to_owned();
        widgets::dialog(
            ctx,
            "gauge here",
            &name,
            &mut open,
            crate::theme::size::DIALOG_M,
            |ui| {
                let room = (ctx.content_rect().height() - 180.0).max(240.0);
                egui::ScrollArea::vertical()
                    .max_height(room)
                    .show(ui, |ui| {
                        delete = self.gauge_detail(ui, index);
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    editor = widgets::quiet_button(ui, tr("Open in the editor")).clicked();
                });
            },
        );
        if delete {
            let gone = self.editor.draft.gauges.remove(index);
            self.gauges.set(gone.shown_name(), None);
            open = false;
        }
        if editor {
            open = false;
        }
        if !open {
            self.editor.gauge_here = None;
        }
        // Saved as the editor saves: when the changes stop, and on closing.
        self.rules_autosave(ctx, open);
        if editor {
            self.open_automation(automation::GAUGES);
            self.select_rule(automation::GAUGES, index);
        }
    }

    /// Goes to the gauges in the editor, on the first of this panel's.
    pub(super) fn open_gauges(&mut self, panel: u32) {
        self.open_automation(automation::GAUGES);
        let first = panels_first(&self.layout, panel);
        let here =
            |gauge: &crate::rules::Gauge| gauge.panel == panel || (first && gauge.panel == 0);
        if let Some(index) = self.editor.draft.gauges.iter().position(here) {
            self.select_rule(automation::GAUGES, index);
        }
    }
}

/// Whether `panel` is the first of the panels of gauges, the one that
/// shows the gauges put in none.
fn panels_first(layout: &Layout, panel: u32) -> bool {
    let first = layout.panels.iter().find(|spec| spec.kind == Kind::Gauges);
    first.is_some_and(|spec| spec.id == panel)
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
