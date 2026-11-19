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
        }
    }
}

/// The panels of a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
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
        };
        Self {
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

    fn has(&self, kind: Kind) -> bool {
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
    }

    /// Adds what was said on a channel to the panels that show them.
    pub(super) fn channel_line(&mut self, line: &Line) {
        let ids: Vec<u32> = (self.layout.panels.iter())
            .filter(|panel| panel.channels)
            .map(|panel| panel.id)
            .collect();
        for id in ids {
            self.text_of(id).log.push(line);
        }
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
                        let add = ui.button(tr("Add a panel"));
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
                                // There is one map and one set of gauges.
                                let free = kind == Kind::Text || !self.layout.has(kind);
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

    fn add_panel(&mut self, kind: Kind) {
        let id = self.layout.next_id();
        let (name, side, size) = match kind {
            Kind::Text => {
                let taken =
                    |name: &String| self.layout.panels.iter().any(|panel| panel.name == *name);
                let names = (1..).map(|number| format!("{} {number}", tr("Panel")));
                let name = names
                    .take(200)
                    .find(|name| !taken(name))
                    .unwrap_or_default();
                (name, Side::Right, 280.0)
            }
            Kind::Map => (tr("Map").to_owned(), Side::Right, 330.0),
            Kind::Gauges => (tr("Gauges").to_owned(), Side::Bottom, 56.0),
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
        for side in [Side::Top, Side::Bottom, Side::Left, Side::Right] {
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
        self.gauges_dialog(ui.ctx());
        self.keep_layout(ui);
        asked
    }

    /// The panels of one side, sharing its length by their weights, with
    /// an edge to drag between each two.
    fn strip(&mut self, ui: &mut egui::Ui, side: Side, members: &[usize]) -> Option<MapAction> {
        const GAP: f32 = 8.0;
        let theme = theme::current();
        let full = ui.max_rect();
        let stacked = side.is_beside();
        let along = |rect: egui::Rect| if stacked { rect.height() } else { rect.width() };
        let length = (along(full) - GAP * (members.len() - 1) as f32).max(1.0);
        let total: f32 = members
            .iter()
            .map(|&index| self.layout.panels[index].weight.max(0.05))
            .sum();
        let mut asked = None;
        let mut at = if stacked { full.top() } else { full.left() };
        for (place, &index) in members.iter().enumerate() {
            let spec = self.layout.panels[index].clone();
            let share = length * spec.weight.max(0.05) / total;
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
            self.panel_rects.push((spec.id, rect));
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(12), theme.background);
            let inner = rect.shrink2(egui::vec2(14.0, 10.0));
            let builder = egui::UiBuilder::new()
                .id_salt(("panel", spec.id))
                .max_rect(inner)
                .layout(egui::Layout::top_down(egui::Align::Min));
            let got = ui.scope_builder(builder, |ui| {
                ui.set_clip_rect(inner.intersect(ui.clip_rect()));
                self.panel_body(ui, index, &spec)
            });
            asked = asked.or(got.inner);
            at += share;
            let Some(&next) = members.get(place + 1) else {
                continue;
            };
            // The edge between this panel and the next, to trade room.
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
            let handle = ui
                .interact(
                    edge,
                    ui.make_persistent_id(("panel edge", spec.id)),
                    egui::Sense::drag(),
                )
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
                    .max(least - self.layout.panels[index].weight);
                self.layout.panels[index].weight += give;
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
        // Over another panel: its side, before or after it.
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
            let before = if side.is_beside() {
                pointer.y < rect.center().y
            } else {
                pointer.x < rect.center().x
            };
            let half = match (side.is_beside(), before) {
                (true, true) => rect.with_max_y(rect.center().y),
                (true, false) => rect.with_min_y(rect.center().y),
                (false, true) => rect.with_max_x(rect.center().x),
                (false, false) => rect.with_min_x(rect.center().x),
            };
            Some((other, side, before, half))
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
        let to = match onto {
            Some((other, _, before, _)) => {
                let place = self
                    .layout
                    .panels
                    .iter()
                    .position(|panel| panel.id == other)
                    .unwrap_or(0);
                if before { place } else { place + 1 }
            }
            None => self.layout.panels.len(),
        };
        self.layout.panels.insert(to, moved);
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
    ) -> Option<MapAction> {
        if self.editing_layout {
            self.panel_handle(ui, index);
        }
        match spec.kind {
            Kind::Map => self.map_body(ui, spec.id),
            Kind::Gauges => {
                self.gauges_body(ui);
                None
            }
            Kind::Text => {
                if !self.editing_layout {
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
            let (mut earlier, mut later) = (false, false);
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
                if spec.kind == Kind::Text {
                    widgets::menu_separator(ui);
                    widgets::menu_check(
                        ui,
                        &mut panel.channels,
                        tr("Show the MUD's channels here"),
                    );
                }
            });
            if spec.kind == Kind::Gauges && ui.button(tr("Gauges…")).clicked() {
                self.open_gauges();
            }
            if earlier || later {
                self.shift_panel(index, if earlier { -1 } else { 1 });
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

    /// The gauges in a row, or how to get some.
    fn gauges_body(&mut self, ui: &mut egui::Ui) {
        if !self.gauges.is_empty() {
            return self.gauges.show(ui);
        }
        let theme = theme::current();
        ui.horizontal(|ui| {
            let none =
                tr("No gauges yet. They appear when the MUD reports your health and the like.");
            ui.label(
                RichText::new(none)
                    .font(theme::font(Weight::Regular, 12.5))
                    .color(theme.text_faint),
            );
            if widgets::quiet_button(ui, tr("Set them up…")).clicked() {
                self.open_gauges();
            }
        });
    }

    /// What a panel was given: text put there whole, centred as a block
    /// since it is often a drawing, or else its lines one after another.
    /// Returns whether there was anything.
    pub(super) fn text_body(&mut self, ui: &mut egui::Ui, id: u32, quiet: bool) -> bool {
        let fixed = self.text_of(id).fixed.clone();
        if !fixed.is_empty() {
            // What a drawing sent by the server has to fit in.
            self.panel_cells = Self::cells(ui, ui.available_size());
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
        if self.text_of(id).log.len() > 0 {
            let clicked = self.text_of(id).log.show(ui, &format!("panel {id}"));
            if let Some(command) = clicked {
                self.send(command);
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

    /// Opens the gauges to make and change them.
    pub(super) fn open_gauges(&mut self) {
        if !self.rules_open {
            self.rules_draft = self.rules.clone();
            self.rules_seen = self.rules.clone();
        }
        self.gauges_open = true;
    }

    /// The gauges, in a dialog: the list, and the one chosen.
    fn gauges_dialog(&mut self, ctx: &egui::Context) {
        if !self.gauges_open {
            return;
        }
        let mut open = true;
        widgets::dialog(ctx, "gauges", tr("Gauges"), &mut open, 720.0, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(220.0);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let add = widgets::primary_button(ui, "+");
                    ui.ctx().accesskit_node_builder(add.id, |node| {
                        node.set_label(tr("Add"));
                    });
                    if add.on_hover_text(tr("Add")).clicked() {
                        self.rules_draft.gauges.push(crate::rules::Gauge {
                            enabled: true,
                            segments: true,
                            low: 30,
                            ..crate::rules::Gauge::default()
                        });
                        self.gauge_selected = self.rules_draft.gauges.len() - 1;
                    }
                    ui.add_space(6.0);
                    for (index, gauge) in self.rules_draft.gauges.iter().enumerate() {
                        let title = if gauge.name.trim().is_empty() { tr("New gauge") } else { gauge.name.trim() };
                        if widgets::list_row(ui, self.gauge_selected == index, title, &gauge.value, gauge.enabled).clicked() {
                            self.gauge_selected = index;
                        }
                    }
                });
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    ui.set_min_height(380.0);
                    ui.spacing_mut().item_spacing.y = 10.0;
                    let index = self.gauge_selected;
                    if index >= self.rules_draft.gauges.len() {
                        let theme = theme::current();
                        ui.add_space(40.0);
                        let about = tr("A gauge is a bar that follows a number the MUD reports,\nsuch as your health.");
                        ui.label(RichText::new(about).color(theme.text_faint));
                    } else if self.gauge_detail(ui, index) {
                        // Take the bar off the screen too.
                        let gone = self.rules_draft.gauges.remove(index);
                        let name = if gone.name.trim().is_empty() { gone.value } else { gone.name };
                        self.gauges.set(name.trim(), None);
                        self.gauge_selected = index.saturating_sub(1);
                    }
                });
            });
        });
        self.rules_autosave(ctx, open);
        if !open {
            self.gauges_open = false;
        }
    }
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
