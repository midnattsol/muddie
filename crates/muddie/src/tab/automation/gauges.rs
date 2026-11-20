//! The details of a gauge, and putting gauges in order.

use super::*;

impl Tab {
    pub(in crate::tab) fn gauge_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        use crate::rules::{GaugeKind, Step};
        let theme = theme::current();
        // Alt and an arrow move it in the list, for those who do not drag.
        let (up, down) = ui.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp),
                input.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown),
            )
        });
        if up && index > 0 {
            self.move_gauge(index, index - 1);
            return false;
        }
        if down {
            self.move_gauge(index, index + 2);
            return false;
        }
        let panels: Vec<(u32, String)> = (self.layout.panels.iter())
            .filter(|panel| panel.kind == super::layout::Kind::Gauges)
            .map(|panel| (panel.id, panel.name.clone()))
            .collect();
        let mut add_panel = false;
        ui.set_max_width(ui.available_width().min(760.0));
        let rule = &mut self.editor.draft.gauges[index];
        let delete = rule_header(ui, &mut rule.name, &mut rule.enabled, tr("Name this gauge"));
        // What it is drawn as.
        let kinds = [
            GaugeKind::Bar,
            GaugeKind::Cells,
            GaugeKind::Number,
            GaugeKind::Light,
            GaugeKind::Text,
        ];
        ui.horizontal(|ui| {
            widgets::caption(ui, tr("Kind"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let names = [
                    tr("Bar"),
                    tr("Cells"),
                    tr("Number"),
                    tr("Light"),
                    tr("Text"),
                ];
                let current = kinds
                    .iter()
                    .position(|kind| *kind == rule.kind)
                    .unwrap_or(0);
                if let Some(chosen) = widgets::segmented(ui, &names, current) {
                    rule.kind = kinds[chosen];
                }
            });
        });
        let about = match rule.kind {
            GaugeKind::Bar | GaugeKind::Cells => tr("How full something is, of its maximum."),
            GaugeKind::Number => tr("A number as it is: gold, level, experience."),
            GaugeKind::Light => {
                tr("On or off, for what is so or is not: hidden, poisoned, in combat.")
            }
            GaugeKind::Text => tr("What the MUD says, as words: a stance, a target, a place."),
        };
        note(ui, about);
        ui.add_space(4.0);
        widgets::caption(ui, tr("Follows"));
        if rule.kind.has_level() {
            ui.columns(2, |columns| {
                widgets::field(
                    &mut columns[0],
                    tr("Value"),
                    &mut rule.value,
                    "Char.Vitals.hp",
                );
                let none = if rule.kind == GaugeKind::Number {
                    tr("none")
                } else {
                    "100"
                };
                widgets::field(&mut columns[1], tr("Maximum"), &mut rule.max, none);
            });
        } else {
            widgets::field(ui, tr("Value"), &mut rule.value, "Char.Status.poisoned");
        }
        let hint = match rule.kind {
            GaugeKind::Light => tr(
                "It is on when the value is true, a number other than 0, or words other than no, false or off.",
            ),
            _ => tr(
                "Where the MUD reports it: a GMCP package and the fields inside it, such as Char.Vitals.health.current, or the name of a MSDP variable. The maximum can also be a number. Debug shows what there is.",
            ),
        };
        note(ui, hint);
        ui.add_space(4.0);
        if rule.kind == GaugeKind::Light {
            widgets::caption(ui, tr("Colour when on"));
            swatches(ui, &mut rule.colour);
            ui.add_space(4.0);
            widgets::caption(ui, tr("Colour when off"));
            ui.horizontal(|ui| {
                swatches(ui, &mut rule.off_colour);
                let unlit = rule.off_colour.trim().is_empty();
                if widgets::swatch_none(ui, unlit, tr("Unlit")).clicked() {
                    rule.off_colour.clear();
                }
            });
            note(
                ui,
                tr(
                    "With a colour for each, it says which of the two it is: red for poisoned, green for not.",
                ),
            );
        } else {
            widgets::caption(ui, tr("Colour"));
            swatches(ui, &mut rule.colour);
        }
        if rule.kind.has_level() {
            ui.add_space(4.0);
            widgets::caption(ui, tr("Colour by level"));
            if rule.steps.is_empty() {
                note(
                    ui,
                    tr(
                        "It can turn another colour as it gets low: yellow under 60 %, red under 30 %.",
                    ),
                );
            }
            let mut removed = None;
            for (place, step) in rule.steps.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    let label = ui.label(RichText::new(tr("Under")).color(theme.text_dim));
                    ui.add(
                        crate::widgets::number(&mut step.under)
                            .range(1..=100)
                            .suffix(" %"),
                    )
                    .labelled_by(label.id);
                    ui.add_space(6.0);
                    ui.push_id(place, |ui| swatches(ui, &mut step.colour));
                    if widgets::close_button(ui, tr("Remove"), 22.0).clicked() {
                        removed = Some(place);
                    }
                });
            }
            if let Some(place) = removed {
                rule.steps.remove(place);
            }
            // Kept in order, highest first, once nothing is being changed.
            let busy = ui.input(|input| input.pointer.any_down())
                || ui.memory(|memory| memory.focused().is_some());
            if !busy {
                rule.steps.sort_by_key(|step| std::cmp::Reverse(step.under));
            }
            if crate::widgets::button(ui, tr("Add a level")).clicked() {
                // Under the lowest there is, or a first one at a third.
                let lowest = rule.steps.iter().map(|step| step.under).min();
                let (under, colour) = match lowest {
                    None => (60, "yellow"),
                    Some(lowest) => (lowest / 2, "red"),
                };
                rule.steps.push(Step {
                    under: under.max(1),
                    colour: colour.to_owned(),
                });
            }
        }
        // Which panel of gauges it is shown in.
        ui.add_space(4.0);
        widgets::caption(ui, tr("Shown in"));
        let order = tr(
            "In the order of the list: drag a gauge there, or press Alt+Up or Alt+Down, to move it.",
        );
        if panels.is_empty() {
            ui.horizontal(|ui| {
                note(ui, tr("There is no panel of gauges to show it in."));
                add_panel = crate::widgets::button(ui, tr("Add a panel of gauges")).clicked();
            });
        } else {
            let home = panels
                .iter()
                .find(|(id, _)| *id == rule.panel)
                .unwrap_or(&panels[0]);
            crate::widgets::dropdown("gauge panel")
                .selected_text(home.1.clone())
                .width(220.0)
                .show_ui(ui, |ui| {
                    for (id, name) in &panels {
                        crate::widgets::choice_value(ui, &mut rule.panel, *id, name);
                    }
                });
            note(ui, order);
        }
        ui.add_space(4.0);
        widgets::caption(ui, tr("Preview"));
        let look = |text: &str| muddie_script::GaugeLook {
            colour: rule.colour.clone(),
            kind: match rule.kind {
                GaugeKind::Bar => "bar",
                GaugeKind::Cells => "cells",
                GaugeKind::Number => "number",
                GaugeKind::Light => "light",
                GaugeKind::Text => "text",
            }
            .to_owned(),
            steps: rule
                .steps
                .iter()
                .map(|step| (f64::from(step.under), step.colour.clone()))
                .collect(),
            text: text.to_owned(),
            off_colour: rule.off_colour.clone(),
            ..muddie_script::GaugeLook::default()
        };
        let name = if rule.name.trim().is_empty() {
            tr("Gauge")
        } else {
            rule.name.trim()
        };
        // As it will sit under the output, at the levels that matter.
        let strip = egui::Frame::new()
            .fill(theme.background)
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::symmetric(16, 14));
        strip.show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(22.0, 12.0);
                match rule.kind {
                    GaugeKind::Light => {
                        crate::panels::draw_gauge(ui, name, 1.0, 1.0, &look(""));
                        crate::panels::draw_gauge(ui, name, 0.0, 1.0, &look(""));
                    }
                    GaugeKind::Text => {
                        crate::panels::draw_gauge(ui, name, 0.0, 0.0, &look(tr("standing")))
                    }
                    GaugeKind::Number if rule.max.trim().is_empty() => {
                        crate::panels::draw_gauge(ui, name, 1250.0, 0.0, &look(""));
                    }
                    _ => {
                        // Full, and just under each level it changes at.
                        let mut levels: Vec<f64> = vec![100.0];
                        let mut under: Vec<u8> = rule.steps.iter().map(|step| step.under).collect();
                        under.sort_unstable_by(|a, b| b.cmp(a));
                        under.dedup();
                        levels.extend(
                            under
                                .iter()
                                .take(3)
                                .map(|under| (f64::from(*under) - 8.0).max(1.0)),
                        );
                        if levels.len() == 1 {
                            levels.extend([64.0, 18.0]);
                        }
                        for value in levels {
                            crate::panels::draw_gauge(ui, name, value.round(), 100.0, &look(""));
                        }
                    }
                }
            });
        });
        if add_panel {
            self.add_panel(super::layout::Kind::Gauges);
        }
        delete
    }

    /// While a gauge is dragged in the list, marks where it would go, and
    /// puts it there once let go.
    pub(in crate::tab) fn drag_gauge(&mut self, ui: &egui::Ui, places: &[(usize, egui::Rect)]) {
        let Some(from) = self.editor.gauge_dragged else {
            return;
        };
        let pointer = ui.ctx().pointer_latest_pos();
        // Before the first row whose middle is under the pointer, or last.
        let before = pointer
            .and_then(|pointer| (places.iter()).find(|(_, rect)| pointer.y < rect.center().y));
        let to = before.map_or(self.editor.draft.gauges.len(), |(index, _)| *index);
        let y = match (before, places.last()) {
            (Some((_, rect)), _) => rect.top(),
            (None, Some((_, rect))) => rect.bottom(),
            (None, None) => return self.editor.gauge_dragged = None,
        };
        let across = places[0].1.x_range();
        ui.painter()
            .hline(across, y, egui::Stroke::new(2.0, theme::current().accent));
        if !ui.input(|input| input.pointer.any_down()) {
            self.editor.gauge_dragged = None;
            self.move_gauge(from, to);
        }
    }

    /// Moves a gauge to before the one now at `to`, keeping it chosen.
    pub(in crate::tab) fn move_gauge(&mut self, from: usize, to: usize) {
        let gauges = &mut self.editor.draft.gauges;
        if from >= gauges.len() || to == from || to == from + 1 {
            return;
        }
        let gauge = gauges.remove(from);
        let at = if to > from { to - 1 } else { to }.min(gauges.len());
        gauges.insert(at, gauge);
        self.editor.selected[GAUGES] = at;
    }
}
