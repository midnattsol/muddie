//! The layout of the panels, in a view of its own beside the game and the
//! editor: the panels by the side they are on, the details of the one
//! chosen, a picture of the window, and arrangements to start from. All of
//! it is done with the keyboard too, as dragging on the game is not.

use super::*;
use crate::theme::Weight;
use crate::widgets;
use layout::{Drop, Kind as PanelKind, PanelSpec, Side};

/// The sides, in the order they are listed: beside the game, then above
/// and below it.
const SIDES: [Side; 4] = [Side::Left, Side::Right, Side::Top, Side::Bottom];

/// How wide the list of panels is, and the details at most.
const LIST_WIDTH: f32 = 300.0;
const DETAIL_WIDTH: f32 = 760.0;

/// The view: whether it is open, the panel chosen, and whether the
/// keyboard is to be put on it and it brought into sight.
#[derive(Default)]
pub(super) struct Arrange {
    pub open: bool,
    pub chosen: Option<u32>,
    pub focus: bool,
    pub reveal: bool,
}

/// An arrangement to start from.
#[derive(Clone, Copy)]
enum Arrangement {
    /// The map and the panels of text at the right, the gauges below.
    Classic,
    /// The map at the left, the panels of text at the right.
    Wide,
    /// No panel: the game alone.
    GameOnly,
}

impl Arrangement {
    fn name(self) -> &'static str {
        match self {
            Self::Classic => tr("Classic"),
            Self::Wide => tr("Wide screen"),
            Self::GameOnly => tr("The game alone"),
        }
    }

    fn about(self) -> &'static str {
        match self {
            Self::Classic => tr("The map and the panels of text at the right, the gauges below."),
            Self::Wide => {
                tr("The map at the left, the panels of text at the right, the gauges below.")
            }
            Self::GameOnly => tr("Every panel hidden; they are kept, to show again."),
        }
    }
}

/// Where a side is, in words.
fn side_name(side: Side) -> &'static str {
    match side {
        Side::Left => tr("At the left"),
        Side::Top => tr("At the top"),
        Side::Bottom => tr("At the bottom"),
        Side::Right => tr("At the right"),
    }
}

/// What kind of panel it is, in words.
fn kind_name(kind: PanelKind) -> &'static str {
    match kind {
        PanelKind::Text => tr("Text"),
        PanelKind::Map => tr("Map"),
        PanelKind::Gauges => tr("Gauges"),
    }
}

/// The line of the list a panel has, the same every frame.
fn row_id(panel: u32) -> egui::Id {
    egui::Id::unique("arrange row").with(panel)
}

impl Tab {
    /// Opens the view, with a panel chosen if one is given.
    pub fn open_arrange(&mut self, panel: Option<u32>) {
        if self.editor.open {
            self.close_automation();
        }
        self.arrange.open = true;
        self.arrange.focus = true;
        if panel.is_some() {
            self.arrange.chosen = panel;
        }
    }

    /// The panels as they are listed: by side, each side in its order.
    fn listed(&self) -> Vec<u32> {
        (SIDES.iter())
            .flat_map(|side| {
                (self.layout.panels.iter())
                    .filter(move |panel| panel.side == *side)
                    .map(|panel| panel.id)
            })
            .collect()
    }

    /// The view, which takes the session's whole space: the list of panels
    /// on a card at the left, the picture of the window and the details of
    /// the one chosen at the right. Escape goes back to the game.
    pub(super) fn arrange_page(&mut self, ui: &mut egui::Ui) {
        let leaving = widgets::take_escape(ui);
        let listed = self.listed();
        if self.arrange.chosen.is_none_or(|id| !listed.contains(&id)) {
            self.arrange.chosen = listed.first().copied();
        }
        let theme = theme::current();
        let page = egui::Frame::new()
            .fill(theme.surface)
            .inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 2,
                bottom: 8,
            });
        CentralPanel::default().frame(page).show(ui, |ui| {
            let full = ui.available_rect_before_wrap();
            let list = egui::Rect::from_min_size(full.min, egui::vec2(LIST_WIDTH, full.height()));
            let detail =
                egui::Rect::from_min_max(egui::pos2(list.right() + 22.0, full.top()), full.max);
            ui.painter()
                .rect_filled(list, egui::CornerRadius::same(14), theme.field);
            let pane = |name: &str, rect: egui::Rect| {
                egui::UiBuilder::new()
                    .id_salt(("arrange pane", name))
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min))
            };
            ui.scope_builder(pane("list", list.shrink2(egui::vec2(10.0, 12.0))), |ui| {
                let surface = theme.surface;
                ui.visuals_mut().text_edit_bg_color = Some(surface);
                ui.visuals_mut().extreme_bg_color = surface;
                self.arrange_list(ui, &listed);
            });
            ui.scope_builder(
                pane("detail", detail.shrink2(egui::vec2(14.0, 6.0))),
                |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            widgets::keep_clear_of_bar(ui);
                            ui.set_max_width(ui.available_width().min(DETAIL_WIDTH));
                            self.arrange_detail(ui);
                        });
                },
            );
        });
        self.keep_layout(ui);
        if leaving {
            self.arrange.open = false;
        }
    }

    /// The list: its heading with the button that adds a panel, then the
    /// panels under the side each is on.
    fn arrange_list(&mut self, ui: &mut egui::Ui, listed: &[u32]) {
        ui.horizontal(|ui| {
            let heading = RichText::new(tr("Layout"))
                .font(theme::font(Weight::Semibold, 17.0))
                .color(theme::current().text);
            ui.add_space(6.0);
            ui.add(Label::new(heading).selectable(false));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let add = widgets::primary_button(ui, "+");
                ui.ctx()
                    .accesskit_node_builder(add.id, |node| node.set_label(tr("New panel")));
                let add = add.on_hover_text(tr("New panel"));
                crate::widgets::menu_of(&add).show(|ui| {
                    widgets::menu_width(ui);
                    let mut kinds = vec![(PanelKind::Text, tr("Panel of text"))];
                    if !self.layout.has(PanelKind::Map) {
                        kinds.push((PanelKind::Map, tr("Map")));
                    }
                    kinds.push((PanelKind::Gauges, tr("Gauges")));
                    for (kind, name) in kinds {
                        if widgets::menu_item(ui, name, "").clicked() {
                            self.add_panel(kind);
                            self.arrange.chosen = self.layout.panels.last().map(|panel| panel.id);
                            self.arrange.focus = true;
                            ui.close();
                        }
                    }
                });
            });
        });
        ui.add_space(10.0);
        self.arrange_keys(ui, listed);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                widgets::keep_clear_of_bar(ui);
                if listed.is_empty() {
                    widgets::page_note(ui, tr("There are no panels: + adds one."));
                    return;
                }
                let whole = egui::UiBuilder::new()
                    .accessibility_role(egui::accesskit::Role::List)
                    .accessibility_label(tr("Panels"));
                ui.scope_builder(whole, |ui| {
                    ui.spacing_mut().item_spacing.y = widgets::LINE_GAP;
                    for side in SIDES {
                        let members: Vec<PanelSpec> = (self.layout.panels.iter())
                            .filter(|panel| panel.side == side)
                            .cloned()
                            .collect();
                        if members.is_empty() {
                            continue;
                        }
                        let mut caption = side_name(side).to_owned();
                        if self.layout.hidden.contains(&side) {
                            caption = trf("{0}, put away", &[&caption]);
                        }
                        widgets::section_caption(ui, &caption);
                        for panel in &members {
                            self.arrange_row(ui, panel);
                        }
                    }
                });
            });
    }

    /// One panel's line: its name, and what it is and how it is shown
    /// under it. Tab stops at the one chosen; the arrows go through the
    /// rest.
    fn arrange_row(&mut self, ui: &mut egui::Ui, panel: &PanelSpec) {
        let chosen = self.arrange.chosen == Some(panel.id);
        let name = panel.shown_name();
        // What it is, unless its name says so already.
        let mut about = Vec::new();
        if name != kind_name(panel.kind) {
            about.push(kind_name(panel.kind).to_owned());
        }
        if panel.kind == PanelKind::Text && panel.channels {
            about.push(tr("the MUD's channels").to_owned());
        }
        let sharing: Vec<String> = (self.layout.panels.iter())
            .filter(|other| {
                other.id != panel.id
                    && panel.stack != 0
                    && other.stack == panel.stack
                    && other.side == panel.side
            })
            .map(PanelSpec::shown_name)
            .collect();
        if !sharing.is_empty() {
            about.push(trf("a tab with {0}", &[&sharing.join(", ")]));
        }
        if !panel.visible {
            about.push(tr("hidden").to_owned());
        }
        let detail = about.join(", ");
        let row = widgets::ListRow {
            title: &name,
            detail: &detail,
            selected: chosen,
            active: true,
            key: Some(row_id(panel.id)),
            current: chosen,
            ..widgets::ListRow::default()
        };
        let line = row.show(ui);
        widgets::hold_arrows(ui, &line);
        // Gone to by the keys, it is brought into sight.
        if chosen && std::mem::take(&mut self.arrange.reveal) {
            line.scroll_to_me(None);
        }
        if chosen && std::mem::take(&mut self.arrange.focus) && widgets::keyboard_in_use(ui.ctx()) {
            line.request_focus();
        }
        if line.clicked() {
            self.arrange.chosen = Some(panel.id);
        }
    }

    /// With the keyboard on the panel chosen: the arrows go to another,
    /// Home and End to the first and last; Alt and Up or Down move it among
    /// those of its side, and Alt and Left or Right to another side.
    fn arrange_keys(&mut self, ui: &mut egui::Ui, listed: &[u32]) {
        let Some(chosen) = self.arrange.chosen else {
            return;
        };
        if ui.memory(|memory| memory.focused()) != Some(row_id(chosen)) {
            return;
        }
        let with = crate::shortcuts::WITH_ARROWS;
        let (move_up, move_down, move_back, move_on) = ui.input_mut(|input| {
            (
                input.consume_key(with, Key::ArrowUp),
                input.consume_key(with, Key::ArrowDown),
                input.consume_key(with, Key::ArrowLeft),
                input.consume_key(with, Key::ArrowRight),
            )
        });
        if move_up || move_down {
            return self.move_panel(chosen, move_up);
        }
        if move_back || move_on {
            let side = self
                .layout
                .panel(chosen)
                .map_or(Side::Right, |panel| panel.side);
            let at = SIDES.iter().position(|known| *known == side).unwrap_or(0);
            let to = match move_on {
                true => SIDES[(at + 1) % SIDES.len()],
                false => SIDES[(at + SIDES.len() - 1) % SIDES.len()],
            };
            self.put_on_side(chosen, to);
            self.voice.say_now(&trf("Moved: {0}.", &[side_name(to)]));
            return;
        }
        let pressed = |key: Key| ui.input_mut(|input| input.consume_key(Modifiers::NONE, key));
        let (up, down, home, end) = (
            pressed(Key::ArrowUp),
            pressed(Key::ArrowDown),
            pressed(Key::Home),
            pressed(Key::End),
        );
        let at = listed.iter().position(|id| *id == chosen).unwrap_or(0);
        let to = match (up, down, home, end) {
            (true, ..) => Some(at.saturating_sub(1)),
            (_, true, ..) => Some((at + 1).min(listed.len().saturating_sub(1))),
            (_, _, true, _) => Some(0),
            (.., true) => listed.len().checked_sub(1),
            _ => None,
        };
        if let Some(&id) = to.and_then(|to| listed.get(to)) {
            self.arrange.chosen = Some(id);
            self.arrange.reveal = true;
            ui.memory_mut(|memory| memory.request_focus(row_id(id)));
        }
    }

    /// Moves a panel before the one above it on its side, or after the one
    /// below, and says where it is now. Among those it shares a place with,
    /// it changes its place among the tabs.
    fn move_panel(&mut self, id: u32, up: bool) {
        let Some(from) = self.layout.panels.iter().position(|panel| panel.id == id) else {
            return;
        };
        let side = self.layout.panels[from].side;
        let beside: Vec<usize> = (0..self.layout.panels.len())
            .filter(|&index| self.layout.panels[index].side == side)
            .collect();
        let at = beside.iter().position(|&index| index == from).unwrap_or(0);
        let next = match up {
            true => at.checked_sub(1).map(|at| beside[at]),
            false => beside.get(at + 1).copied(),
        };
        let Some(next) = next else {
            return;
        };
        let (stack, other) = (
            self.layout.panels[from].stack,
            self.layout.panels[next].stack,
        );
        if stack != 0 && stack == other {
            self.layout.panels.swap(from, next);
        } else {
            let mut moved = self.layout.panels.remove(from);
            moved.stack = 0;
            let next = if next > from { next - 1 } else { next };
            let drop = if up { Drop::Before } else { Drop::After };
            self.drop_beside(moved, next, drop);
        }
        let now: Vec<u32> = (self.layout.panels.iter())
            .filter(|panel| panel.side == side)
            .map(|panel| panel.id)
            .collect();
        let place = now.iter().position(|known| *known == id).unwrap_or(0);
        self.say_place(up, place, now.len());
    }

    /// Moves a panel to the end of another side, which takes it at its size.
    fn put_on_side(&mut self, id: u32, side: Side) {
        if self
            .layout
            .panel(id)
            .is_some_and(|panel| panel.side != side)
        {
            self.drop_panel(id, None, side);
        }
    }

    /// The right of the view: the picture of the window, the details of
    /// the panel chosen, and the arrangements to start from.
    fn arrange_detail(&mut self, ui: &mut egui::Ui) {
        self.miniature(ui);
        if let Some(id) = self.arrange.chosen {
            self.panel_detail(ui, id);
        }
        widgets::section(ui, tr("Arrangements"), |ui| {
            for how in [
                Arrangement::Classic,
                Arrangement::Wide,
                Arrangement::GameOnly,
            ] {
                let name = how.name();
                // The button is named by its row: the arrangement.
                let used = widgets::setting_row(ui, name, Some(how.about()), &mut |ui| {
                    widgets::button(ui, tr("Use"))
                });
                if used.clicked() {
                    self.arrange_as(how);
                    self.voice.say_now(&trf("{0}: in place.", &[name]));
                }
            }
            widgets::switch_setting(
                ui,
                &mut self.layout.columns_full,
                tr("Side panels at full height"),
                Some(tr(
                    "The panels at the left and right reach the top and bottom of the window.",
                )),
            );
        });
    }

    /// The details of a panel: its name, whether it is shown, its side,
    /// its place there, its size, the panel it shares a place with, what
    /// goes with its kind, and removing it.
    fn panel_detail(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(panel) = self.layout.panel(id).cloned() else {
            return;
        };
        let beside: Vec<PanelSpec> = (self.layout.panels.iter())
            .filter(|other| other.side == panel.side)
            .cloned()
            .collect();
        let place = beside.iter().position(|other| other.id == id).unwrap_or(0);
        let (mut name, mut visible, mut side) = (panel.name.clone(), panel.visible, panel.side);
        let mut size = panel.size;
        let mut sharing = (beside.iter())
            .find(|other| other.id != id && panel.stack != 0 && other.stack == panel.stack)
            .map_or(0, |other| other.id);
        let (mut channels, mut moved, mut edit_gauges, mut remove) =
            (panel.channels, None, false, false);
        let shown = panel.shown_name();
        let title = match shown == kind_name(panel.kind) {
            true => shown,
            false => format!("{shown} · {}", kind_name(panel.kind)),
        };
        widgets::section(ui, &title, |ui| {
            widgets::setting_row(ui, tr("Name"), None, &mut |ui| {
                widgets::row_field(ui, &mut name, tr("Name"), 220.0)
            });
            widgets::switch_setting(ui, &mut visible, tr("Shown"), None);
            let sides: Vec<(Side, String)> = SIDES
                .iter()
                .map(|side| (*side, side_name(*side).to_owned()))
                .collect();
            widgets::setting_row(ui, tr("Side"), None, &mut |ui| {
                widgets::choose(ui, "arrange side", &mut side, &sides, |side| {
                    side_name(side).to_owned()
                })
            });
            let mut at = place;
            let places: Vec<(usize, String)> = (0..beside.len())
                .map(|at| {
                    (
                        at,
                        trf(
                            "{0} of {1}",
                            &[&(at + 1).to_string(), &beside.len().to_string()],
                        ),
                    )
                })
                .collect();
            widgets::setting_row(ui, tr("Place on its side"), None, &mut |ui| {
                widgets::choose(ui, "arrange place", &mut at, &places, |at| {
                    (at + 1).to_string()
                })
            });
            if at != place {
                moved = Some(at);
            }
            let measure = match side {
                Side::Left | Side::Right => tr("Width"),
                Side::Top | Side::Bottom => tr("Height"),
            };
            widgets::setting_row(
                ui,
                measure,
                Some(tr("The same for every panel of its side.")),
                &mut |ui| {
                    ui.add(
                        widgets::number(&mut size)
                            .range(80.0..=1600.0)
                            .speed(2.0)
                            .suffix(" pt"),
                    )
                },
            );
            let mut others: Vec<(u32, String)> = vec![(0, tr("None").to_owned())];
            others.extend(
                (beside.iter())
                    .filter(|other| other.id != id)
                    .map(|other| (other.id, other.shown_name())),
            );
            widgets::setting_row(
                ui,
                tr("A tab with"),
                Some(tr(
                    "Panels that share a place show their names as tabs, to choose between them.",
                )),
                &mut |ui| {
                    widgets::choose(ui, "arrange tabs", &mut sharing, &others, |_| {
                        tr("None").to_owned()
                    })
                },
            );
            match panel.kind {
                PanelKind::Text => drop(widgets::switch_setting(
                    ui,
                    &mut channels,
                    tr("Show the MUD's channels here"),
                    None,
                )),
                PanelKind::Gauges => {
                    let edit = widgets::setting_row(ui, tr("Gauges"), None, &mut |ui| {
                        widgets::button(ui, tr("Edit the gauges"))
                    });
                    edit_gauges = edit.clicked();
                }
                PanelKind::Map => {}
            }
            let gone = widgets::setting_row(
                ui,
                tr("Remove"),
                Some(tr("What it shows is lost with it.")),
                &mut |ui| widgets::quiet_button(ui, tr("Remove this panel")),
            );
            remove = gone.clicked();
        });
        // What was changed, put in place now that nothing is borrowed.
        if let Some(panel) = self.layout.panel_mut(id) {
            panel.name = name;
            panel.visible = visible;
            panel.channels = channels;
        }
        if size != panel.size {
            let size = size.round();
            (self.layout.panels.iter_mut())
                .filter(|other| other.side == panel.side)
                .for_each(|other| other.size = size);
        }
        if side != panel.side {
            self.put_on_side(id, side);
        }
        let was_sharing = (beside.iter())
            .find(|other| other.id != id && panel.stack != 0 && other.stack == panel.stack)
            .map_or(0, |other| other.id);
        if sharing != was_sharing {
            self.share_place(id, sharing);
        }
        // To the place chosen, a step at a time, as the keys move it.
        if let Some(to) = moved {
            for _ in 0..to.abs_diff(place) {
                self.move_panel(id, to < place);
            }
        }
        if edit_gauges {
            self.open_gauges(id);
        }
        if remove && let Some(index) = self.layout.panels.iter().position(|panel| panel.id == id) {
            self.remove_panel(index);
            self.voice
                .say_now(&trf("{0} removed.", &[&panel.shown_name()]));
        }
    }

    /// Puts a panel in the place of another, as a tab beside it; or, with
    /// none, in a place of its own after the one it shared.
    fn share_place(&mut self, id: u32, with: u32) {
        let Some(from) = self.layout.panels.iter().position(|panel| panel.id == id) else {
            return;
        };
        let mut moved = self.layout.panels.remove(from);
        moved.stack = 0;
        let place = match with {
            0 => None,
            other => self
                .layout
                .panels
                .iter()
                .position(|panel| panel.id == other),
        };
        match place {
            Some(place) => self.drop_beside(moved, place, Drop::Into),
            None => self.layout.panels.insert(from, moved),
        }
    }

    /// Puts the panels as an arrangement has them. Panels are kept: those
    /// of text go together, the map and the gauges where it says.
    fn arrange_as(&mut self, how: Arrangement) {
        self.layout.hidden.clear();
        if let Arrangement::GameOnly = how {
            self.layout
                .panels
                .iter_mut()
                .for_each(|panel| panel.visible = false);
            return;
        }
        if !self.layout.has(PanelKind::Map) {
            self.add_panel(PanelKind::Map);
        }
        let map_side = match how {
            Arrangement::Wide => Side::Left,
            _ => Side::Right,
        };
        for panel in &mut self.layout.panels {
            panel.visible = true;
            panel.stack = 0;
            panel.weight = 1.0;
            (panel.side, panel.size) = match panel.kind {
                PanelKind::Map => (map_side, 330.0),
                PanelKind::Text => (Side::Right, 330.0),
                PanelKind::Gauges => (Side::Bottom, 96.0),
            };
        }
        let rank = |panel: &PanelSpec| match panel.kind {
            PanelKind::Map => 0,
            PanelKind::Text => 1,
            PanelKind::Gauges => 2,
        };
        self.layout.panels.sort_by_key(rank);
    }

    /// A picture of the window as the panels are, the one chosen marked.
    /// Choosing a panel in it chooses it in the list. A screen reader is
    /// told what it is, and has the list for what it shows.
    fn miniature(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        let screen = ui.ctx().content_rect();
        let width = ui.available_width();
        let height = (width * screen.height() / screen.width().max(1.0)).clamp(120.0, 240.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::CLICK);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Image,
                true,
                tr("Picture of the window's panels"),
            )
        });
        let blocks = self.blocks(rect, rect.width() / screen.width().max(1.0));
        let painter = ui.painter_at(rect);
        let round = egui::CornerRadius::same(8);
        painter.rect_filled(rect, round, theme.field);
        for (block, ids, name) in &blocks {
            let chosen = self.arrange.chosen.is_some_and(|id| ids.contains(&id));
            let game = ids.is_empty();
            let fill = match (game, chosen) {
                (true, _) => theme.surface,
                (false, true) => theme.accent.gamma_multiply(0.35),
                (false, false) => theme.raised,
            };
            let inner = block.shrink(2.0);
            painter.rect_filled(inner, egui::CornerRadius::same(5), fill);
            if chosen {
                painter.rect_stroke(
                    inner,
                    egui::CornerRadius::same(5),
                    egui::Stroke::new(1.5, theme.accent),
                    egui::StrokeKind::Inside,
                );
            }
            let colour = if game {
                theme.text_faint
            } else {
                theme.text_dim
            };
            painter.with_clip_rect(inner.shrink(4.0)).text(
                inner.center(),
                egui::Align2::CENTER_CENTER,
                name,
                theme::font(Weight::Regular, 11.5),
                colour,
            );
        }
        if response.clicked()
            && let Some(point) = response.interact_pointer_pos()
            && let Some((_, ids, _)) = blocks
                .iter()
                .find(|(block, ids, _)| !ids.is_empty() && block.contains(point))
        {
            self.arrange.chosen = ids.first().copied();
        }
    }

    /// Where the panels on screen, and the game, are in a picture of the
    /// window at `rect`, `scale` times its size: each place with its panels
    /// and its words. The game's place has no panels.
    fn blocks(&self, rect: egui::Rect, scale: f32) -> Vec<(egui::Rect, Vec<u32>, String)> {
        let on: Vec<&PanelSpec> = (self.layout.panels.iter())
            .filter(|panel| self.layout.on_screen(panel))
            .collect();
        let depth = |side: Side| {
            let most = (on.iter())
                .filter(|panel| panel.side == side)
                .map(|panel| panel.size)
                .fold(0.0_f32, f32::max);
            let limit = match side {
                Side::Left | Side::Right => rect.width() * 0.4,
                Side::Top | Side::Bottom => rect.height() * 0.4,
            };
            (most * scale).min(limit)
        };
        let (left, right, top, bottom) = (
            depth(Side::Left),
            depth(Side::Right),
            depth(Side::Top),
            depth(Side::Bottom),
        );
        let full = self.layout.columns_full;
        let region = |side: Side| match side {
            Side::Left => egui::Rect::from_min_max(
                egui::pos2(rect.left(), rect.top() + if full { 0.0 } else { top }),
                egui::pos2(
                    rect.left() + left,
                    rect.bottom() - if full { 0.0 } else { bottom },
                ),
            ),
            Side::Right => egui::Rect::from_min_max(
                egui::pos2(
                    rect.right() - right,
                    rect.top() + if full { 0.0 } else { top },
                ),
                egui::pos2(
                    rect.right(),
                    rect.bottom() - if full { 0.0 } else { bottom },
                ),
            ),
            Side::Top => egui::Rect::from_min_max(
                egui::pos2(rect.left() + if full { left } else { 0.0 }, rect.top()),
                egui::pos2(
                    rect.right() - if full { right } else { 0.0 },
                    rect.top() + top,
                ),
            ),
            Side::Bottom => egui::Rect::from_min_max(
                egui::pos2(
                    rect.left() + if full { left } else { 0.0 },
                    rect.bottom() - bottom,
                ),
                egui::pos2(rect.right() - if full { right } else { 0.0 }, rect.bottom()),
            ),
        };
        let mut blocks = Vec::new();
        for side in SIDES {
            // The places of the side: a panel, or those sharing one as tabs.
            let mut places: Vec<(Vec<u32>, Vec<String>, f32)> = Vec::new();
            for panel in on.iter().filter(|panel| panel.side == side) {
                match places.last_mut() {
                    Some((ids, names, _))
                        if panel.stack != 0
                            && self
                                .layout
                                .panel(ids[0])
                                .is_some_and(|first| first.stack == panel.stack) =>
                    {
                        ids.push(panel.id);
                        names.push(panel.shown_name());
                    }
                    _ => places.push((
                        vec![panel.id],
                        vec![panel.shown_name()],
                        panel.weight.max(0.1),
                    )),
                }
            }
            let total: f32 = places.iter().map(|(_, _, weight)| weight).sum();
            let area = region(side);
            let mut start = 0.0;
            for (ids, names, weight) in places {
                let (from, to) = (start / total, (start + weight) / total);
                start += weight;
                let block = match side {
                    Side::Left | Side::Right => egui::Rect::from_x_y_ranges(
                        area.x_range(),
                        area.top() + area.height() * from..=area.top() + area.height() * to,
                    ),
                    Side::Top | Side::Bottom => egui::Rect::from_x_y_ranges(
                        area.left() + area.width() * from..=area.left() + area.width() * to,
                        area.y_range(),
                    ),
                };
                blocks.push((block, ids, names.join(" | ")));
            }
        }
        let game = egui::Rect::from_min_max(
            egui::pos2(rect.left() + left, rect.top() + top),
            egui::pos2(rect.right() - right, rect.bottom() - bottom),
        );
        blocks.push((game, Vec::new(), tr("Game").to_owned()));
        blocks
    }
}

#[cfg(test)]
impl Tab {
    pub(super) fn listed_for_tests(&self) -> Vec<u32> {
        self.listed()
    }

    pub(super) fn share_place_for_tests(&mut self, id: u32, with: u32) {
        self.share_place(id, with);
    }
}
