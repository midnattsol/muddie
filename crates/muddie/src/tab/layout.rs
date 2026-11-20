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
    /// What a profile starts with: the channels above, the map at the
    /// right and the gauges below.
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
                panel(1, tr("Chat"), Kind::Text, Side::Top, 120.0, true),
                panel(2, tr("Map"), Kind::Map, Side::Right, 330.0, false),
                panel(3, tr("Gauges"), Kind::Gauges, Side::Bottom, 56.0, false),
            ],
        }
    }
}

impl Layout {
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

    fn has(&self, kind: Kind) -> bool {
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
        self.layout_saved = self.layout.clone();
    }

    /// Saves the panels once they have changed and nothing is being
    /// dragged.
    fn keep_layout(&mut self, ui: &egui::Ui) {
        if self.layout == self.layout_saved || ui.input(|input| input.pointer.any_down()) {
            return;
        }
        self.layout_saved = self.layout.clone();
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
        let index = match self.panel_text.iter().position(|(known, _)| *known == id) {
            Some(index) => index,
            None => {
                let mut content = TextContent::default();
                content
                    .log
                    .restyle(self.settings.scrollback_lines, self.settings.timestamps);
                self.panel_text.push((id, content));
                self.panel_text.len() - 1
            }
        };
        &mut self.panel_text[index].1
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
        let stack = self
            .layout
            .panels
            .iter()
            .find(|panel| panel.id == id)
            .map_or(0, |panel| panel.stack);
        let shown = self
            .shown_tabs
            .iter()
            .find(|(known, _)| *known == stack)
            .map(|(_, id)| *id);
        if stack != 0 && shown.is_some_and(|shown| shown != id) && !self.unread_tabs.contains(&id) {
            self.unread_tabs.push(id);
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
                let called = [tr("Chat"), tr("Channels")]
                    .into_iter()
                    .find(|name| !taken(name));
                self.layout.panels.push(PanelSpec {
                    id,
                    name: called.unwrap_or(tr("Channels")).to_owned(),
                    kind: Kind::Text,
                    side: Side::Top,
                    size: 140.0,
                    channels: true,
                    ..PanelSpec::default()
                });
            }
        } else {
            // The panel named: the one of that name, made if there is
            // none, and left where it is and as it is if there is.
            self.panel_called(panel);
        }
        let known = (self.channels.iter()).position(|channel| {
            channel.name.eq_ignore_ascii_case(name) && channel.panel.eq_ignore_ascii_case(panel)
        });
        let index = match known {
            Some(index) => Some(index),
            None if name.is_empty() || self.channels.len() >= MOST_CHANNELS => None,
            None => {
                let mut log = Output::default();
                log.restyle(self.settings.scrollback_lines, self.settings.timestamps);
                self.channels.push(Channel {
                    name: name.to_owned(),
                    panel: panel.to_owned(),
                    log,
                    unread: false,
                });
                Some(self.channels.len() - 1)
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
            let shown = self.channel_shown.iter().find(|(id, _)| *id == spec.id);
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
                self.channels[index].log.push(line);
            }
        }
        if let Some(index) = index {
            self.channels[index].unread |= !seen;
        }
    }

    /// The tabs of a panel of channels: all, and one for each channel,
    /// marked if something was said on it that has not been looked at.
    /// Returns the channel chosen; none for all.
    fn channel_tabs(&mut self, ui: &mut egui::Ui, id: u32) -> String {
        let theme = theme::current();
        let known = self.channel_shown.iter().find(|(panel, _)| *panel == id);
        let mut shown = known.map(|(_, shown)| shown.clone()).unwrap_or_default();
        let spec = self
            .layout
            .panels
            .iter()
            .find(|panel| panel.id == id)
            .cloned();
        let here = |channel: &&Channel| spec.as_ref().is_some_and(|spec| channel.shown_in(spec));
        let names: Vec<(String, bool)> = std::iter::once((String::new(), false))
            .chain(
                (self.channels.iter())
                    .filter(here)
                    .map(|channel| (channel.name.clone(), channel.unread)),
            )
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
            for (name, unread) in &names {
                let on = *name == shown;
                let label = if name.is_empty() {
                    tr("All")
                } else {
                    name.as_str()
                };
                let font = theme::font(
                    if on || *unread {
                        Weight::Medium
                    } else {
                        Weight::Regular
                    },
                    12.0,
                );
                // One that has something new stands out, and says so.
                let fresh = crate::panels::colour_named("yellow").unwrap_or(theme.accent);
                let colour = match (on, *unread) {
                    (true, _) => theme.text,
                    (false, true) => fresh,
                    (false, false) => theme.text_dim,
                };
                let galley = ui.painter().layout_no_wrap(label.to_owned(), font, colour);
                // Room for a dot beside the name of one with something new.
                let dot = if *unread { 10.0 } else { 0.0 };
                let size = galley.size() + egui::vec2(16.0 + dot, 8.0);
                let (rect, tab) = ui.allocate_exact_size(size, egui::Sense::click());
                tab.widget_info(|| {
                    let said = match *unread {
                        true => format!("{label}, {}", tr("new")),
                        false => label.to_owned(),
                    };
                    egui::WidgetInfo::selected(egui::Role::Tab, true, on, said)
                });
                if on {
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(6), theme.raised);
                } else if tab.hovered() || tab.has_focus() {
                    let fill = theme.raised.gamma_multiply(0.6);
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(6), fill);
                }
                let at = egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0);
                ui.painter().galley(at, galley, colour);
                if *unread {
                    let centre = rect.right_center() - egui::vec2(9.0, 0.0);
                    ui.painter().circle_filled(centre, 3.0, fresh);
                }
                if tab.clicked() {
                    shown.clone_from(name);
                }
            }
        });
        ui.add_space(2.0);
        self.channel_shown.retain(|(panel, _)| *panel != id);
        self.channel_shown.push((id, shown.clone()));
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
                    name: tr("Map").to_owned(),
                    kind: Kind::Map,
                    size: 330.0,
                    ..PanelSpec::default()
                });
            }
            None => {}
        }
    }

    /// The panels' entries of the View menu: one per panel, to show or
    /// hide it, and the way into the layout mode.
    pub(super) fn panels_menu(&mut self, ui: &mut egui::Ui) {
        for panel in &mut self.layout.panels {
            if panel.kind == Kind::Map {
                widgets::menu_row_with_shortcut(ui, &mut panel.visible, &panel.name, "Ctrl+M");
            } else {
                widgets::menu_check(ui, &mut panel.visible, &panel.name);
            }
        }
        if !self.layout.panels.is_empty() {
            widgets::menu_separator(ui);
        }
        if widgets::menu_item(ui, tr("Gauges…"), "").clicked() {
            // Those of the first panel of gauges, made if there is none.
            if !self.layout.has(Kind::Gauges) {
                self.add_panel(Kind::Gauges);
            }
            let panel = self
                .layout
                .panels
                .iter()
                .find(|panel| panel.kind == Kind::Gauges)
                .map_or(0, |panel| panel.id);
            self.open_gauges(panel);
            ui.close();
        }
        if widgets::menu_item(ui, tr("Edit the layout…"), "").clicked() {
            self.editing_layout = true;
            self.started = true;
            ui.close();
        }
    }

    /// The bar of the layout mode: what to do, a way to add a panel, and
    /// the way out.
    pub(super) fn layout_bar(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        let bar = egui::Frame::new()
            .fill(theme.raised)
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::symmetric(14, 8))
            .outer_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 2,
                bottom: 4,
            });
        Panel::top("layout bar")
            .frame(bar)
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let title = RichText::new(tr("Editing the layout"))
                        .font(theme::font(Weight::Medium, 13.5))
                        .color(theme.text);
                    ui.add(Label::new(title).selectable(false));
                    let how =
                        tr("Drag a panel by its handle to another side, and its edge to size it.");
                    ui.add(
                        Label::new(
                            RichText::new(how)
                                .font(theme::font(Weight::Regular, 12.5))
                                .color(theme.text_dim),
                        )
                        .selectable(false),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::primary_button(ui, tr("Done")).clicked() {
                            self.editing_layout = false;
                        }
                        // Who gets the corners of the window.
                        let corners = tr("Side panels reach the top and the bottom");
                        widgets::switch(ui, &mut self.layout.columns_full, corners)
                            .on_hover_text(corners);
                        let label = RichText::new(tr("Tall sides"))
                            .font(theme::font(Weight::Regular, 12.5))
                            .color(theme.text_dim);
                        ui.add(Label::new(label).selectable(false))
                            .on_hover_text(corners);
                        let add = crate::widgets::button(ui, tr("Add a panel"));
                        egui::Popup::menu(&add).show(|ui| {
                            widgets::menu_width(ui);
                            let kinds = [
                                (
                                    Kind::Text,
                                    tr("Text"),
                                    tr("Lines sent to it by triggers and scripts"),
                                ),
                                (Kind::Map, tr("Map"), tr("The map of where you are")),
                                (
                                    Kind::Gauges,
                                    tr("Gauges"),
                                    tr("Bars that follow your health and the like"),
                                ),
                            ];
                            for (kind, name, about) in kinds {
                                // There is one map.
                                let free = kind != Kind::Map || !self.layout.has(kind);
                                let item = ui
                                    .add_enabled_ui(free, |ui| widgets::menu_item(ui, name, ""))
                                    .inner;
                                if item.on_hover_text(about).clicked() {
                                    self.add_panel(kind);
                                    ui.close();
                                }
                            }
                        });
                    });
                });
            });
    }

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
            Kind::Text => (free(tr("Panel"), false), Side::Right, 280.0),
            Kind::Map => (tr("Map").to_owned(), Side::Right, 330.0),
            Kind::Gauges => (free(tr("Gauges"), true), Side::Bottom, 56.0),
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
        self.panel_rects.clear();
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
                Side::Top | Side::Bottom if self.editing_layout => (8, 8, 108.0),
                Side::Top | Side::Bottom => (8, 8, 44.0),
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
        if let Some(index) = self.removed_panel.take() {
            let gone = self.layout.panels.remove(index);
            self.panel_text.retain(|(id, _)| *id != gone.id);
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
                self.panel_rects.push((self.layout.panels[index].id, rect));
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
                let tabbed = place.len() > 1;
                let index = if tabbed {
                    self.tab_bar(ui, place)
                } else {
                    place[0]
                };
                let spec = self.layout.panels[index].clone();
                self.panel_body(ui, index, &spec, tabbed)
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
        let theme = theme::current();
        let stack = self.layout.panels[place[0]].stack;
        let ids: Vec<u32> = place
            .iter()
            .map(|&index| self.layout.panels[index].id)
            .collect();
        let kept = self
            .shown_tabs
            .iter()
            .find(|(known, _)| *known == stack)
            .map(|(_, id)| *id);
        let mut shown = kept.filter(|id| ids.contains(id)).unwrap_or(ids[0]);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for &index in place {
                let panel = &self.layout.panels[index];
                let on = panel.id == shown;
                let unread = self.unread_tabs.contains(&panel.id);
                let font = theme::font(if on { Weight::Medium } else { Weight::Regular }, 12.5);
                let wide = ui
                    .painter()
                    .layout_no_wrap(panel.name.clone(), font.clone(), theme.text)
                    .size()
                    .x;
                let size = egui::vec2(wide + if unread { 30.0 } else { 20.0 }, 24.0);
                let (rect, tab) = ui.allocate_exact_size(size, egui::Sense::click());
                tab.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::Tab, true, on, panel.name.as_str())
                });
                let hovered = tab.hovered() || tab.has_focus();
                if on {
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(7), theme.raised);
                } else if hovered {
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::same(7),
                        theme.raised.gamma_multiply(0.6),
                    );
                }
                let colour = if on || hovered {
                    theme.text
                } else {
                    theme.text_dim
                };
                ui.painter().text(
                    rect.left_center() + egui::vec2(10.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &panel.name,
                    font,
                    colour,
                );
                if unread {
                    ui.painter().circle_filled(
                        rect.right_center() - egui::vec2(10.0, 0.0),
                        3.0,
                        theme.accent,
                    );
                }
                if tab.clicked() {
                    shown = panel.id;
                }
            }
        });
        ui.add_space(2.0);
        self.shown_tabs.retain(|(known, _)| *known != stack);
        self.shown_tabs.push((stack, shown));
        self.unread_tabs.retain(|id| *id != shown);
        place[ids.iter().position(|id| *id == shown).unwrap_or(0)]
    }

    /// While a panel is dragged by its handle, shows where it would go;
    /// once it is let go, puts it there: before or after the panel it is
    /// over, or at the end of the side it is nearest to.
    fn finish_drag(&mut self, ui: &egui::Ui) {
        let Some(id) = self.dragged_panel else {
            return;
        };
        let area = ui.ctx().content_rect();
        let Some(pointer) = ui.ctx().pointer_latest_pos() else {
            self.dragged_panel = None;
            return;
        };
        // Over another panel: its side, and by where on it, before it,
        // after it, or in its place as a tab.
        let over = (self.panel_rects.iter())
            .find(|(other, rect)| *other != id && rect.contains(pointer))
            .copied();
        let onto = over.and_then(|(other, rect)| {
            let side = self
                .layout
                .panels
                .iter()
                .find(|panel| panel.id == other)?
                .side;
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
        self.dragged_panel = None;
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
                self.shown_tabs.retain(|(known, _)| *known != joined);
                self.shown_tabs.push((joined, moved.id));
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
        self.shown_tabs.retain(|(known, _)| *known != stack);
        self.shown_tabs.push((stack, id));
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

    /// What is inside a panel: in the layout mode, its handle and what
    /// can be changed about it; then what it shows.
    fn panel_body(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        spec: &PanelSpec,
        tabbed: bool,
    ) -> Option<MapAction> {
        if self.editing_layout {
            self.panel_handle(ui, index);
        }
        match spec.kind {
            Kind::Map => self.map_body(ui, spec.id),
            Kind::Gauges => {
                self.gauges_body(ui, spec.id);
                None
            }
            Kind::Text => {
                // Its name is already there in the layout mode and on a tab.
                if !self.editing_layout && !tabbed {
                    widgets::caption(ui, &spec.name);
                }
                self.text_body(ui, spec.id, false);
                None
            }
        }
    }

    /// The row a panel gets in the layout mode: a handle to drag it by,
    /// its name, the side it is on, what goes with its kind, and the cross
    /// that removes it.
    fn panel_handle(&mut self, ui: &mut egui::Ui, index: usize) {
        let theme = theme::current();
        let Some(spec) = self.layout.panels.get(index).cloned() else {
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // Six dots: this can be taken and moved.
            let (grip, handle) =
                ui.allocate_exact_size(egui::vec2(16.0, 24.0), egui::Sense::drag());
            let handle = handle.on_hover_cursor(egui::CursorIcon::Grab);
            handle.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, true, trf("Move {0}", &[&spec.name]))
            });
            let dots = if handle.hovered() || handle.dragged() {
                theme.text
            } else {
                theme.text_faint
            };
            for (column, row) in [(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2)] {
                let dot =
                    grip.center() + egui::vec2(-3.0 + 6.0 * column as f32, -6.0 + 6.0 * row as f32);
                ui.painter().circle_filled(dot, 1.4, dots);
            }
            if handle.drag_started() {
                self.dragged_panel = Some(spec.id);
            }
            let panel = &mut self.layout.panels[index];
            // As wide as leaves the rest of the row beside it.
            // Four sides, the dots and the cross, with the gaps between;
            // and the gauges' button where there is one.
            let beside = 4.0 * 24.0 + 28.0 + 22.0 + 7.0 * 6.0 + 44.0;
            let button = if spec.kind == Kind::Gauges {
                104.0
            } else {
                0.0
            };
            let spare = ui.available_width() - beside - button;
            let name = ui.add(
                TextEdit::singleline(&mut panel.name)
                    .margin(egui::Margin::symmetric(8, 4))
                    .desired_width(spare.clamp(56.0, 170.0)),
            );
            ui.ctx().accesskit_node_builder(name.id, |node| {
                node.set_label(tr("Name"));
            });
            // The four sides, for those who would rather not drag.
            for (side, label) in [
                (Side::Left, tr("At the left")),
                (Side::Right, tr("At the right")),
                (Side::Top, tr("Above")),
                (Side::Bottom, tr("Below")),
            ] {
                if side_button(ui, side, panel.side == side, label).clicked() && panel.side != side
                {
                    if panel.side.is_beside() != side.is_beside() {
                        panel.size = if side.is_beside() { 300.0 } else { 120.0 };
                    }
                    panel.side = side;
                }
            }
            let (mut earlier, mut later, mut join) = (false, false, false);
            let more = widgets::icon_button(ui, widgets::Icon::More, tr("More"), false);
            egui::Popup::menu(&more).show(|ui| {
                widgets::menu_width(ui);
                // For those who would rather not drag.
                let (before, after) = if panel.side.is_beside() {
                    (tr("Move up"), tr("Move down"))
                } else {
                    (tr("Move left"), tr("Move right"))
                };
                if widgets::menu_item(ui, before, "").clicked() {
                    earlier = true;
                    ui.close();
                }
                if widgets::menu_item(ui, after, "").clicked() {
                    later = true;
                    ui.close();
                }
                // Tabs: with the panel before it, or out of them.
                widgets::menu_separator(ui);
                if spec.stack == 0 {
                    if widgets::menu_item(ui, tr("Share the place of the one before, as tabs"), "")
                        .clicked()
                    {
                        join = true;
                        ui.close();
                    }
                } else if widgets::menu_item(ui, tr("Take out of the tabs"), "").clicked() {
                    panel.stack = 0;
                    ui.close();
                }
                if spec.kind == Kind::Text {
                    widgets::menu_separator(ui);
                    widgets::menu_check(
                        ui,
                        &mut panel.channels,
                        tr("Show the MUD's channels here"),
                    );
                }
            });
            if spec.kind == Kind::Gauges && crate::widgets::button(ui, tr("Gauges…")).clicked() {
                self.open_gauges(spec.id);
            }
            if earlier || later {
                self.shift_panel(index, if earlier { -1 } else { 1 });
            }
            if join {
                self.join_before(index);
            }
            // Beside the rest, not at the far end: a narrow panel has
            // no far end to spare.
            let remove = trf("Remove {0}", &[&spec.name]);
            if widgets::close_button(ui, &remove, 22.0).clicked() {
                self.removed_panel = Some(index);
            }
        });
        ui.add_space(2.0);
    }

    /// Whether the gauge called `name` belongs in the panel `id`: the
    /// one it was put in, or the first of the gauges' panels if it was put
    /// in none, as those the MUD's vitals and scripts make are.
    pub(super) fn gauge_is_in(&self, name: &str, id: u32) -> bool {
        let panels: Vec<u32> = (self.layout.panels.iter())
            .filter(|panel| panel.kind == Kind::Gauges)
            .map(|panel| panel.id)
            .collect();
        let made = self
            .rules
            .gauges
            .iter()
            .find(|gauge| gauge.shown_name() == name);
        let home = made
            .map(|gauge| gauge.panel)
            .filter(|panel| panels.contains(panel));
        home.or(panels.first().copied()) == Some(id)
    }

    /// The gauges of a panel in a row, or how to get some.
    fn gauges_body(&mut self, ui: &mut egui::Ui, id: u32) {
        let full = ui.max_rect();
        // In the order they have in the editor, whichever the MUD
        // reported first.
        let order: Vec<String> = (self.rules.gauges.iter())
            .map(|gauge| gauge.shown_name().to_owned())
            .collect();
        self.gauges.arrange(&order);
        if self.gauges.show(ui, |name| self.gauge_is_in(name, id)) {
            // The way to change them, at the corner, out of their way.
            if !self.editing_layout {
                let corner = egui::Rect::from_min_size(
                    egui::pos2(full.right() - 28.0, full.top()),
                    egui::vec2(28.0, 28.0),
                );
                let builder = egui::UiBuilder::new()
                    .id_salt(("gauges of", id))
                    .max_rect(corner);
                let edit = ui.scope_builder(builder, |ui| {
                    widgets::icon_button(ui, widgets::Icon::More, tr("Edit the gauges"), false)
                });
                if edit.inner.clicked() {
                    self.open_gauges(id);
                }
            }
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
            if widgets::quiet_button(ui, tr("Set them up…")).clicked() {
                self.open_gauges(id);
            }
        });
    }

    /// What a panel was given: text put there whole, centred as a block
    /// since it is often a drawing, or else its lines one after another.
    /// Returns whether there was anything.
    pub(super) fn text_body(&mut self, ui: &mut egui::Ui, id: u32, quiet: bool) -> bool {
        let fixed = self.text_of(id).fixed.clone();
        // What a drawing put in it has to fit in, there or yet to come.
        // The map's panel is measured where the map is drawn.
        let map =
            (self.layout.panels.iter()).any(|panel| panel.id == id && panel.kind == Kind::Map);
        if !map {
            let cells = Self::cells(ui, ui.available_size());
            self.panel_measured(id, cells, !fixed.is_empty());
        }
        if !fixed.is_empty() {
            let galleys: Vec<_> = fixed
                .iter()
                .map(|line| ui.painter().layout_job(server_job(line)))
                .collect();
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
        let spec = self
            .layout
            .panels
            .iter()
            .find(|panel| panel.id == id)
            .cloned();
        let here = |channel: &Channel| spec.as_ref().is_some_and(|spec| channel.shown_in(spec));
        if self.channels.iter().any(here) {
            let shown = self.channel_tabs(ui, id);
            let chosen = (self.channels.iter_mut())
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
                None => (self.channels.iter_mut())
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

    /// Goes to the gauges in the editor, on the first of this panel's.
    pub(super) fn open_gauges(&mut self, panel: u32) {
        self.debug_open = false;
        self.editing_layout = false;
        self.open_automation(automation::GAUGES);
        let first = panels_first(&self.layout, panel);
        let here =
            |gauge: &crate::rules::Gauge| gauge.panel == panel || (first && gauge.panel == 0);
        if let Some(index) = self.rules_draft.gauges.iter().position(here) {
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

/// A small picture of a panel on one side of a window, to put a panel
/// there. `label` says which, for screen readers.
fn side_button(ui: &mut egui::Ui, side: Side, on: bool, label: &str) -> egui::Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, label));
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() || response.has_focus();
        if on {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(6),
                theme.accent.gamma_multiply(0.18),
            );
        } else if hovered {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(6), theme.hover);
        }
        let colour = if on {
            theme.accent
        } else if hovered {
            theme.text
        } else {
            theme.text_faint
        };
        let window = egui::Rect::from_center_size(rect.center(), egui::vec2(14.0, 12.0));
        ui.painter().rect_stroke(
            window,
            egui::CornerRadius::same(2),
            egui::Stroke::new(1.2, colour),
            egui::StrokeKind::Middle,
        );
        let part = match side {
            Side::Left => window.with_max_x(window.left() + 5.0),
            Side::Right => window.with_min_x(window.right() - 5.0),
            Side::Top => window.with_max_y(window.top() + 4.5),
            Side::Bottom => window.with_min_y(window.bottom() - 4.5),
        };
        ui.painter()
            .rect_filled(part, egui::CornerRadius::same(1), colour);
    }
    response.on_hover_text(label)
}
