//! The layout of the panels, in a view of its own beside the game and the
//! editor: the panels by the side they are on, the details of the one
//! chosen, a picture of the window, and arrangements to start from. All of
//! it is done with the keyboard too, as dragging on the game is not.

use super::*;
use crate::theme::Weight;
use crate::widgets;
use layout::{Drop, DropTarget, PanelSpec, Side};

use arranging::Arrangement;

mod arranging;
mod list;
mod miniature;

/// The sides, in the order they are listed: beside the game, then above
/// and below it.
const SIDES: [Side; 4] = [Side::Left, Side::Right, Side::Top, Side::Bottom];

/// How wide the list of panels is, and the details at most.
const LIST_WIDTH: f32 = 300.0;
const DETAIL_WIDTH: f32 = 760.0;

/// The view: whether it is open, the panel chosen, and whether the
/// keyboard is to be put on it and it brought into sight.
#[derive(Default)]
pub(super) struct LayoutView {
    pub open: bool,
    pub chosen: Option<u32>,
    pub focus: bool,
    pub reveal: bool,
    /// The panel being dragged in the picture of the window, where it was
    /// taken by from its corner, and where it would go, as settled on.
    pub dragged: Option<u32>,
    pub grab: egui::Vec2,
    pub aim: Option<(Option<layout::DropTarget>, Side)>,
    /// The name being typed for a panel, given it once typed.
    pub naming: Option<(u32, String)>,
    /// The name's field had the keyboard: Escape is its own.
    pub typing: bool,
    /// Escape was pressed in this frame.
    pub escape: bool,
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
fn kind_name(kind: layout::Kind) -> &'static str {
    match kind {
        layout::Kind::Text => tr("Text"),
        layout::Kind::Map => tr("Map"),
        layout::Kind::Gauges => tr("Gauges"),
    }
}

/// The line of the list a panel has, the same every frame.
fn row_id(panel: u32) -> egui::Id {
    egui::Id::unique("layout row").with(panel)
}

impl Tab {
    /// Opens the view, with a panel chosen if one is given.
    pub fn open_layout_view(&mut self, panel: Option<u32>) {
        if self.editor.open {
            self.close_automation();
        }
        self.layout_view.open = true;
        self.layout_view.focus = true;
        if panel.is_some() {
            self.layout_view.chosen = panel;
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
    pub(super) fn layout_page(&mut self, ui: &mut egui::Ui) {
        // Escape while a panel is dragged puts it back, and stays; in the
        // name's field, it is the field's, to leave what was typed. egui
        // has taken the keyboard from the field by now: whether it had it
        // is kept from the frame before.
        self.layout_view.escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
        let leaving = self.layout_view.dragged.is_none()
            && !std::mem::take(&mut self.layout_view.typing)
            && widgets::take_escape(ui);
        let listed = self.listed();
        if self
            .layout_view
            .chosen
            .is_none_or(|id| !listed.contains(&id))
        {
            self.layout_view.chosen = listed.first().copied();
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
                    .id_salt(("layout pane", name))
                    .max_rect(rect)
                    .layout(egui::Layout::top_down(egui::Align::Min))
            };
            ui.scope_builder(pane("list", list.shrink2(egui::vec2(10.0, 12.0))), |ui| {
                let surface = theme.surface;
                ui.visuals_mut().text_edit_bg_color = Some(surface);
                ui.visuals_mut().extreme_bg_color = surface;
                self.layout_list(ui, &listed);
            });
            ui.scope_builder(
                pane("detail", detail.shrink2(egui::vec2(14.0, 6.0))),
                |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            widgets::keep_clear_of_bar(ui);
                            ui.set_max_width(ui.available_width().min(DETAIL_WIDTH));
                            self.layout_detail(ui);
                        });
                },
            );
        });
        self.keep_layout(ui);
        if leaving {
            self.layout_view.open = false;
        }
    }

    /// The right of the view: the picture of the window, the details of
    /// the panel chosen, and the arrangements to start from.
    fn layout_detail(&mut self, ui: &mut egui::Ui) {
        self.unprofiled_banner(ui);
        self.miniature(ui);
        if let Some(id) = self.layout_view.chosen {
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
                    self.layout_as(how);
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
        let mut name = match &self.layout_view.naming {
            Some((naming, typed)) if *naming == id => typed.clone(),
            _ => panel.name.clone(),
        };
        let (mut visible, mut side) = (panel.visible, panel.side);
        let (mut named, mut typing) = (false, false);
        // Escape, as it was pressed before anything on the page took it.
        let escape = self.layout_view.escape;
        let was_away = self.layout.collapsed.contains(&panel.side);
        let mut away = was_away;
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
                let field = widgets::row_field(ui, &mut name, tr("Name"), 220.0);
                typing = field.has_focus() || field.lost_focus();
                // Given on leaving the field, as in the panel's header;
                // Escape keeps the name it had.
                if field.lost_focus() {
                    match escape {
                        true => name = panel.name.clone(),
                        false => named = true,
                    }
                }
                field
            });
            widgets::switch_setting(ui, &mut visible, tr("Shown"), None);
            widgets::switch_setting(
                ui,
                &mut away,
                tr("Its side collapsed"),
                Some(tr(
                    "The whole side is kept as a thin strip, to expand as it was.",
                )),
            );
            let sides: Vec<(Side, String)> = SIDES
                .iter()
                .map(|side| (*side, side_name(*side).to_owned()))
                .collect();
            widgets::setting_row(ui, tr("Side"), None, &mut |ui| {
                widgets::choose(ui, "layout side", &mut side, &sides, |side| {
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
                widgets::choose(ui, "layout place", &mut at, &places, |at| {
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
                    widgets::choose(ui, "layout tabs", &mut sharing, &others, |_| {
                        tr("None").to_owned()
                    })
                },
            );
            match panel.kind {
                layout::Kind::Text => drop(widgets::switch_setting(
                    ui,
                    &mut channels,
                    tr("Show the MUD's channels here"),
                    None,
                )),
                layout::Kind::Gauges => {
                    let edit = widgets::setting_row(ui, tr("Gauges"), None, &mut |ui| {
                        widgets::button(ui, tr("Edit the gauges"))
                    });
                    edit_gauges = edit.clicked();
                }
                layout::Kind::Map => {}
            }
            let gone = widgets::setting_row(
                ui,
                tr("Remove"),
                Some(tr("What it shows is lost with it.")),
                &mut |ui| widgets::quiet_button(ui, tr("Remove this panel")),
            );
            remove = gone.clicked();
        });
        self.layout_view.typing = typing;
        // What was changed, put in place now that nothing is borrowed.
        // Given once typed, as in the panel's header.
        if named {
            self.layout_view.naming = None;
            self.rename_to(id, &name);
        } else {
            self.layout_view.naming = (name != panel.name).then_some((id, name));
        }
        if let Some(panel) = self.layout.panel_mut(id) {
            panel.channels = channels;
        }
        if visible != panel.visible {
            self.layout.set_shown(id, visible);
        }
        if away != was_away {
            self.toggle_side(panel.side);
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
            // What had the keyboard is gone: it goes back to the list.
            self.layout_view.focus = true;
        }
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
