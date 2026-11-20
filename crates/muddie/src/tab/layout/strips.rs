//! Drawing the panels around the output: each side a strip they share, each
//! panel with its header.

use super::*;

/// The gap between two places of a side, which is the edge dragged.
const EDGE: f32 = 8.0;

/// The way a side is shared: down it for those beside the output, across
/// it for those above and below.
struct Along {
    stacked: bool,
    full: egui::Rect,
}

impl Along {
    fn length(&self) -> f32 {
        if self.stacked {
            self.full.height()
        } else {
            self.full.width()
        }
    }

    fn start(&self) -> f32 {
        if self.stacked {
            self.full.top()
        } else {
            self.full.left()
        }
    }

    /// The part of the side from `at`, `share` long.
    fn span(&self, at: f32, share: f32) -> egui::Rect {
        let full = self.full;
        if self.stacked {
            egui::Rect::from_min_max(
                egui::pos2(full.left(), at),
                egui::pos2(full.right(), at + share),
            )
        } else {
            egui::Rect::from_min_max(
                egui::pos2(at, full.top()),
                egui::pos2(at + share, full.bottom()),
            )
        }
    }
}

impl Tab {
    /// Draws the panels around the output, which takes what is left.
    /// Each side is one strip that the panels on it share: those beside
    /// the output are stacked, those above or below it stand in a row.
    /// Returns what was asked of the map, if anything.
    pub(in crate::tab) fn show_panels(&mut self, ui: &mut egui::Ui) -> Option<MapAction> {
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
                    let panel = &self.layout.panels[index];
                    self.layout.on_screen(panel) && panel.side == side
                })
                .collect();
            let Some(&first) = members.first() else {
                self.side_strip(ui, side);
                continue;
            };
            let id = ui.make_persistent_id(("dock", side as u8));
            let (left, right, least) = match side {
                Side::Left => (8, 4, 160.0),
                Side::Right => (4, 8, 160.0),
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
            // The edge to drag is named for what it sizes, in the
            // language of the interface.
            let handle = ui.scope_id().with(id).with("__resize");
            ui.ctx().accesskit_node_builder(handle, |node| {
                node.set_label(side.size_label());
                // Its share of the window, read without a string of digits.
                let round = |value: f64| (value * 100.0).round() / 100.0;
                if let Some(value) = node.numeric_value() {
                    node.set_numeric_value(round(value));
                }
                if let Some(value) = node.min_numeric_value() {
                    node.set_min_numeric_value(round(value));
                }
                if let Some(value) = node.max_numeric_value() {
                    node.set_max_numeric_value(round(value));
                }
            });
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
    pub(in crate::tab) fn strip(
        &mut self,
        ui: &mut egui::Ui,
        side: Side,
        members: &[usize],
    ) -> Option<MapAction> {
        let full = ui.max_rect();
        let along = Along {
            stacked: side.is_beside(),
            full,
        };
        let places = self.places(members);
        let length = (along.length() - EDGE * (places.len() - 1) as f32).max(1.0);
        let weight = |tab: &Self, place: &[usize]| tab.layout.panels[place[0]].weight.max(0.05);
        let total: f32 = places.iter().map(|place| weight(self, place)).sum();
        let (mut asked, mut at) = (None, along.start());
        for (number, place) in places.iter().enumerate() {
            let share = length * weight(self, place) / total;
            asked = asked.or(self.place(ui, place, along.span(at, share)));
            at += share;
            if let Some(next) = places.get(number + 1) {
                self.edge(ui, &along, at, (place[0], next[0]), total / length);
                at += EDGE;
            }
        }
        ui.allocate_rect(full, egui::Sense::hover());
        asked
    }

    /// The places of a side: each a panel alone, or those of one set of tabs.
    fn places(&self, members: &[usize]) -> Vec<Vec<usize>> {
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
        places
    }

    /// One place on its card: its header, and the panel on show in it.
    fn place(&mut self, ui: &mut egui::Ui, place: &[usize], rect: egui::Rect) -> Option<MapAction> {
        for &index in place {
            self.panels.rects.push((self.layout.panels[index].id, rect));
        }
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(12),
            theme::current().background,
        );
        let inner = rect.shrink2(egui::vec2(14.0, 10.0));
        let builder = egui::UiBuilder::new()
            .id_salt(("place", self.layout.panels[place[0]].id))
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min));
        ui.scope_builder(builder, |ui| {
            ui.set_clip_rect(inner.intersect(ui.clip_rect()));
            let index = self.panel_header(ui, place);
            let spec = self.layout.panels[index].clone();
            self.panel_body(ui, &spec)
        })
        .inner
    }

    /// The edge between two places, dragged to trade room between them;
    /// neither gives up its last stretch. `scale` turns length into weight.
    fn edge(
        &mut self,
        ui: &mut egui::Ui,
        along: &Along,
        at: f32,
        (lead, next): (usize, usize),
        scale: f32,
    ) {
        let edge = along.span(at, EDGE);
        let cursor = if along.stacked {
            egui::CursorIcon::ResizeVertical
        } else {
            egui::CursorIcon::ResizeHorizontal
        };
        let id = ui.make_persistent_id(("panel edge", self.layout.panels[lead].id));
        let handle = ui
            .interact(edge, id, egui::Sense::drag())
            .on_hover_cursor(cursor);
        handle.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Splitter, true, tr("Resize")));
        if handle.dragged() {
            let moved = if along.stacked {
                handle.drag_delta().y
            } else {
                handle.drag_delta().x
            };
            let least = 48.0 * scale;
            let give = (moved * scale)
                .min(self.layout.panels[next].weight - least)
                .max(least - self.layout.panels[lead].weight);
            self.layout.panels[lead].weight += give;
            self.layout.panels[next].weight -= give;
            ui.ctx().set_cursor_icon(cursor);
        }
        if handle.hovered() || handle.dragged() {
            let reach = if along.stacked {
                egui::vec2(18.0, 0.0)
            } else {
                egui::vec2(0.0, 18.0)
            };
            let stroke = egui::Stroke::new(3.0, theme::current().accent);
            ui.painter()
                .line_segment([edge.center() - reach, edge.center() + reach], stroke);
        }
    }

    /// The names of the panels that share a place, to choose which is on
    /// show. One that got something while out of sight has a dot. Returns
    /// the one on show.
    pub(in crate::tab) fn tab_bar(&mut self, ui: &mut egui::Ui, place: &[usize]) -> usize {
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
            if widgets::tab(ui, &panel.shown_name(), panel.id == shown, unread).clicked() {
                shown = panel.id;
            }
        }
        self.panels.shown_tabs.retain(|(known, _)| *known != stack);
        self.panels.shown_tabs.push((stack, shown));
        self.panels.unread_tabs.retain(|id| *id != shown);
        place[ids.iter().position(|id| *id == shown).unwrap_or(0)]
    }

    /// What a panel shows, by its kind.
    pub(in crate::tab) fn panel_body(
        &mut self,
        ui: &mut egui::Ui,
        spec: &PanelSpec,
    ) -> Option<MapAction> {
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
    pub(in crate::tab) fn panel_header(&mut self, ui: &mut egui::Ui, place: &[usize]) -> usize {
        let theme = theme::current();
        let mut shown = place[0];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.set_min_height(24.0);
            // Six dots: this can be taken and moved.
            let name = self.layout.panels[shown].shown_name();
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
                self.fold_button(ui, place);
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
}
