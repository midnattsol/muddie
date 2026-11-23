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
    pub(in crate::tab) fn show_panels(
        &mut self,
        ui: &mut egui::Ui,
        central: impl FnOnce(&mut Self, &mut egui::Ui),
    ) -> Option<MapAsk> {
        let mut asked = None;
        self.panels.rects.clear();
        // Whichever goes first spans the window; the others fit between:
        // those above and below, or those at the sides. Those below are
        // drawn last, in the room they had, so that the keyboard reaches
        // them after those above and beside them, and the output.
        let (before, after): (&[Side], &[Side]) = if self.layout.columns_full {
            (&[Side::Left, Side::Right, Side::Top], &[])
        } else {
            (&[Side::Top], &[Side::Left, Side::Right])
        };
        for &side in before {
            asked = asked.or(self.show_side(ui, side).0);
        }
        let id = ui.make_persistent_id("panels below");
        let mut state = (&mut *self, &mut asked);
        widgets::below_last(
            ui,
            id,
            &mut state,
            |(this, asked), ui| {
                for &side in after {
                    let said = this.show_side(ui, side).0;
                    if asked.is_none() {
                        **asked = said;
                    }
                }
                central(this, ui);
            },
            |(this, asked), ui| {
                let (said, rect) = this.show_side(ui, Side::Bottom);
                if asked.is_none() {
                    **asked = said;
                }
                rect.map_or(0.0, |rect| rect.height())
            },
        );
        self.finish_drag(ui);
        if let Some(index) = self.panels.removed.take() {
            self.remove_panel(index);
        }
        self.keep_layout(ui);
        asked
    }

    /// Draws one side: its panels in one strip, or the strip of their names
    /// while it is put away. Returns what was asked of the map, and where
    /// it was drawn, if it was.
    fn show_side(&mut self, ui: &mut egui::Ui, side: Side) -> (Option<MapAsk>, Option<egui::Rect>) {
        let members: Vec<usize> = (0..self.layout.panels.len())
            .filter(|&index| {
                let panel = &self.layout.panels[index];
                self.layout.on_screen(panel) && panel.side == side
            })
            .collect();
        let Some(&first) = members.first() else {
            let strip = self.side_strip(ui, side);
            return (None, strip);
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
        // The arrows move the edge while it has the keyboard.
        let handle = ui.scope_id().with(id).with("__resize");
        let resized = size_by_keys(ui, side, handle);
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
        let asked = shown.inner;
        // The edge to drag is named for what it sizes, and described by
        // its share of the window, in words: as a number a screen reader
        // reads it as a fraction, and then as a share of how far it can
        // move, which is two numbers that say little.
        let mut share = None;
        ui.ctx().accesskit_node_builder(handle, |node| {
            node.set_label(side.size_label());
            if let Some(value) = node.numeric_value() {
                let text = trf(
                    "{0} % of the window",
                    &[&((value * 100.0).round() as u32).to_string()],
                );
                node.set_description(text.clone());
                share = Some(text);
            }
            node.clear_numeric_value();
            node.clear_min_numeric_value();
            node.clear_max_numeric_value();
            node.clear_numeric_value_step();
            node.remove_action(egui::accesskit::Action::SetValue);
        });
        if let (true, Some(share)) = (resized, share) {
            self.voice.say_now(&share);
        }
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
        (asked, Some(rect))
    }

    /// The panels of one side, sharing its length by their weights, with
    /// an edge to drag between each two. Panels joined as tabs share one
    /// place, with their names along its top to choose between them.
    pub(in crate::tab) fn strip(
        &mut self,
        ui: &mut egui::Ui,
        side: Side,
        members: &[usize],
    ) -> Option<MapAsk> {
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
    fn place(&mut self, ui: &mut egui::Ui, place: &[usize], rect: egui::Rect) -> Option<MapAsk> {
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
        let id = ui.make_persistent_id(("panel edge", self.layout.panels[lead].id));
        let names = [lead, next].map(|index| self.layout.panels[index].shown_name());
        let label = trf("Between {0} and {1}", &[&names[0], &names[1]]);
        let moved = widgets::edge(ui, edge, id, &label, !along.stacked);
        if moved.moved != 0.0 {
            let least = 48.0 * scale;
            let give = (moved.moved * scale)
                .min(self.layout.panels[next].weight - least)
                .max(least - self.layout.panels[lead].weight);
            self.layout.panels[lead].weight += give;
            self.layout.panels[next].weight -= give;
        }
        if moved.by_keys {
            let (lead, next) = (&self.layout.panels[lead], &self.layout.panels[next]);
            let share = (100.0 * lead.weight / (lead.weight + next.weight)).round() as u32;
            let text = trf("{0}: {1} %", &[&names[0], &share.to_string()]);
            self.voice.say_now(&text);
        }
        let handle = moved.response;
        if handle.hovered() || handle.dragged() || handle.has_focus() {
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
        // A list of tabs, to a screen reader, saying how many and which.
        let list = egui::UiBuilder::new().accessibility_role(egui::accesskit::Role::TabList);
        ui.scope_builder(list, |ui| {
            for &index in place {
                let panel = &self.layout.panels[index];
                let unread = self.panels.unread_tabs.contains(&panel.id);
                if widgets::tab(ui, &panel.shown_name(), panel.id == shown, unread).clicked() {
                    shown = panel.id;
                }
            }
        });
        self.panels.shown_tabs.retain(|(known, _)| *known != stack);
        self.panels.shown_tabs.push((stack, shown));
        self.panels.unread_tabs.retain(|id| *id != shown);
        place[ids.iter().position(|id| *id == shown).unwrap_or(0)]
    }

    /// The field a panel is named again in, in its header, where its name
    /// was: Enter or leaving it keeps the name, Escape keeps the old one.
    fn rename_field(&mut self, ui: &mut egui::Ui) {
        let Some((id, mut name, focus)) = self.panels.renaming.take() else {
            return;
        };
        let theme = theme::current();
        // In the room the name had, as tall: wider, it would go on to
        // another line and the header grow.
        // What is left of the row: in a row that wraps, the width
        // available is a whole line's.
        let left = ui.max_rect().right() - ui.cursor().left();
        let size = egui::vec2(left.max(40.0), 22.0);
        let (place, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.spacing_mut().interact_size.y = 0.0;
        let field = ui.put(
            place,
            egui::TextEdit::singleline(&mut name)
                .font(theme::font(Weight::Semibold, 12.5))
                .text_color(theme.text)
                .margin(egui::Margin::symmetric(6, 2)),
        );
        let label = tr("Name of the panel");
        ui.ctx()
            .accesskit_node_builder(field.id, |node| node.set_label(label));
        if focus {
            field.request_focus();
        }
        if field.lost_focus() {
            let escaped = ui.input(|input| input.key_pressed(egui::Key::Escape));
            let before = self.layout.panel(id).map(|panel| panel.name.clone());
            if !escaped {
                self.rename_to(id, &name);
            }
            // Said, as the field goes with the keyboard in it.
            let now = self.layout.panel(id).map(|panel| panel.name.clone());
            match now {
                Some(now) if now != before.unwrap_or_default() => {
                    self.voice.say_now(&trf("Renamed {0}.", &[&now]));
                }
                _ if escaped => self.voice.say_now(tr("The name is kept.")),
                _ => {}
            }
            return;
        }
        self.panels.renaming = Some((id, name, focus && !field.has_focus()));
    }

    /// What a panel shows, by its kind.
    pub(in crate::tab) fn panel_body(
        &mut self,
        ui: &mut egui::Ui,
        spec: &PanelSpec,
    ) -> Option<MapAsk> {
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
        let id = ui.make_persistent_id(("panel header", self.layout.panels[place[0]].id));
        let mut dragged = false;
        ui.spacing_mut().item_spacing.x = 4.0;
        let mut state = (&mut *self, &mut shown, &mut dragged);
        widgets::right_last(
            ui,
            id,
            &mut state,
            |(this, shown, dragged), ui| {
                ui.set_min_height(24.0);
                // Six dots: this can be taken and moved.
                let name = this.layout.panels[**shown].shown_name();
                // Dragged, and not a stop of the keyboard, which moves a
                // panel from its menu, in Layout.
                let (grip, handle) =
                    ui.allocate_exact_size(egui::vec2(12.0, 22.0), egui::Sense::DRAG);
                let handle = handle.on_hover_cursor(egui::CursorIcon::Grab);
                // Not a button: it is only dragged. The keyboard moves a
                // panel from its menu, or in Layout.
                handle.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Image, true, trf("Move {0}", &[&name]))
                });
                let dots = if handle.hovered() || handle.dragged() {
                    theme.text
                } else {
                    theme.text_faint
                };
                for (column, row) in [(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2)] {
                    let dot = grip.center()
                        + egui::vec2(-2.5 + 5.0 * column as f32, -5.0 + 5.0 * row as f32);
                    ui.painter().circle_filled(dot, 1.2, dots);
                }
                **dragged = handle.drag_started();
                let renaming = this.panels.renaming.as_ref().map(|(id, ..)| *id);
                let renamed = (place.iter().copied())
                    .find(|&index| Some(this.layout.panels[index].id) == renaming);
                if let Some(index) = renamed {
                    **shown = index;
                    this.rename_field(ui);
                } else if place.len() > 1 {
                    **shown = this.tab_bar(ui, place);
                } else {
                    let spec = &this.layout.panels[**shown];
                    let glyph = match spec.kind {
                        Kind::Map => widgets::Glyph::Map,
                        Kind::Gauges => widgets::Glyph::Gauge,
                        Kind::Text if spec.channels => widgets::Glyph::Channels,
                        Kind::Text => widgets::Glyph::Text,
                    };
                    ui.add_space(2.0);
                    widgets::glyph(ui, glyph, 11.5, theme.text_faint);
                    let caption = RichText::new(name)
                        .font(theme::font(Weight::Semibold, 12.5))
                        .color(theme.text_dim);
                    let id = spec.id;
                    let caption = ui.add(
                        Label::new(caption)
                            .selectable(false)
                            .sense(egui::Sense::click()),
                    );
                    // Named again with a double click, or Enter on it, as
                    // it says to a screen reader.
                    let enter = caption.has_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                    (ui.ctx()).accesskit_node_builder(caption.id, |node| {
                        node.set_description(tr("Enter names it again."));
                    });
                    if caption.double_clicked() || enter {
                        this.rename_panel(id);
                    }
                }
            },
            |(this, shown, _), ui| {
                let kind = this.layout.panels[**shown].kind;
                if kind == Kind::Map && this.map.borrow().current().is_some() {
                    let walking = this.walk.is_some();
                    this.map_view.tools(ui, walking);
                }
                this.panel_menu(ui, **shown);
                this.fold_button(ui, place);
            },
        );
        if dragged {
            self.panels.dragged = Some(self.layout.panels[place[0]].id);
        }
        ui.add_space(2.0);
        shown
    }
}

/// Moves the edge of `side` with the arrows while `handle`, the edge, has
/// the keyboard: away from the output makes its panels smaller, toward it
/// larger. Returns whether it was moved.
fn size_by_keys(ui: &egui::Ui, side: Side, handle: egui::Id) -> bool {
    if !ui.memory(|memory| memory.has_focus(handle)) {
        return false;
    }
    let (larger, smaller) = match side {
        Side::Left => (egui::Key::ArrowRight, egui::Key::ArrowLeft),
        Side::Right => (egui::Key::ArrowLeft, egui::Key::ArrowRight),
        Side::Top => (egui::Key::ArrowDown, egui::Key::ArrowUp),
        Side::Bottom => (egui::Key::ArrowUp, egui::Key::ArrowDown),
    };
    ui.input_mut(|input| {
        let action = if input.consume_key(egui::Modifiers::NONE, larger) {
            egui::accesskit::Action::Increment
        } else if input.consume_key(egui::Modifiers::NONE, smaller) {
            egui::accesskit::Action::Decrement
        } else {
            return false;
        };
        // As a screen reader would ask: the panel does it as it does then.
        input.events.push(egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action,
                target_tree: egui::accesskit::TreeId::ROOT,
                target_node: handle.accesskit_id(),
                data: None,
            },
        ));
        true
    })
}
