//! The details of a gauge.

use super::*;
use crate::rules::{Gauge, GaugeKind, Step};

impl Tab {
    pub(in crate::tab) fn gauge_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let panels: Vec<(u32, String)> = (self.layout.panels.iter())
            .filter(|panel| panel.kind == super::layout::Kind::Gauges)
            .map(|panel| (panel.id, panel.name.clone()))
            .collect();
        ui.set_max_width(ui.available_width().min(760.0));
        let rule = &mut self.editor.draft.gauges[index];
        let hint = tr("Name this gauge");
        let delete = rule_header(
            ui,
            &mut rule.name,
            &mut rule.enabled,
            hint,
            &mut self.editor.rename,
        );
        kind_row(ui, rule);
        follows(ui, rule);
        colours(ui, rule);
        if rule.kind.has_level() {
            levels(ui, rule);
        }
        let add_panel = shown_in(ui, rule, &panels);
        preview(ui, rule);
        if add_panel {
            self.add_panel(super::layout::Kind::Gauges);
        }
        delete
    }
}

/// What a gauge is drawn as, and what each kind is for.
fn kind_row(ui: &mut egui::Ui, rule: &mut Gauge) {
    let kinds = [
        GaugeKind::Bar,
        GaugeKind::Cells,
        GaugeKind::Number,
        GaugeKind::Light,
        GaugeKind::Text,
    ];
    ui.horizontal(|ui| {
        widgets::caption(ui, tr("Type"));
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
        GaugeKind::Light => tr("On or off, for what is so or is not: hidden, poisoned, in combat."),
        GaugeKind::Text => tr("What the MUD says, as words: a stance, a target, a place."),
    };
    note(ui, about);
    ui.add_space(4.0);
}

/// What it follows: the value, and its maximum for those with a level.
fn follows(ui: &mut egui::Ui, rule: &mut Gauge) {
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
            "Where the MUD reports it: a GMCP package and the fields inside it, such as Char.Vitals.health.current, or the name of a MSDP variable. The maximum can also be a number. Incoming data, in the editor, shows what there is.",
        ),
    };
    note(ui, hint);
    ui.add_space(4.0);
}

/// Its colour, or for a light, its colours when on and when off.
fn colours(ui: &mut egui::Ui, rule: &mut Gauge) {
    if rule.kind != GaugeKind::Light {
        widgets::caption(ui, tr("Colour"));
        swatches(ui, &mut rule.colour);
        return;
    }
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
}

/// The colours it turns as it gets low, each under a level, kept in order
/// highest first; and the way to add one.
fn levels(ui: &mut egui::Ui, rule: &mut Gauge) {
    ui.add_space(4.0);
    widgets::caption(ui, tr("Colour by level"));
    if rule.steps.is_empty() {
        note(
            ui,
            tr("It can turn another colour as it gets low: yellow under 60 %, red under 30 %."),
        );
    }
    let mut removed = None;
    for (place, step) in rule.steps.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let label = ui.label(RichText::new(tr("Under")).color(theme::current().text_dim));
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
    // Kept in order once nothing is being changed.
    let busy = ui.input(|input| input.pointer.any_down())
        || ui.memory(|memory| memory.focused().is_some());
    if !busy {
        rule.steps.sort_by_key(|step| std::cmp::Reverse(step.under));
    }
    if crate::widgets::button(ui, tr("Add a level")).clicked() {
        // Under the lowest there is, or a first one at 60 %.
        let (under, colour) = match rule.steps.iter().map(|step| step.under).min() {
            None => (60, "yellow"),
            Some(lowest) => (lowest / 2, "red"),
        };
        rule.steps.push(Step {
            under: under.max(1),
            colour: colour.to_owned(),
        });
    }
}

/// Which panel of gauges it is shown in. Returns whether a panel was asked
/// for, there being none.
fn shown_in(ui: &mut egui::Ui, rule: &mut Gauge, panels: &[(u32, String)]) -> bool {
    ui.add_space(4.0);
    widgets::caption(ui, tr("Shown in"));
    if panels.is_empty() {
        let mut add = false;
        ui.horizontal(|ui| {
            note(ui, tr("There is no panel of gauges to show it in."));
            add = crate::widgets::button(ui, tr("Add a panel of gauges")).clicked();
        });
        return add;
    }
    let home = panels
        .iter()
        .find(|(id, _)| *id == rule.panel)
        .unwrap_or(&panels[0]);
    crate::widgets::dropdown("gauge panel")
        .selected_text(home.1.clone())
        .show_ui(ui, |ui| {
            for (id, name) in panels {
                crate::widgets::choice_value(ui, &mut rule.panel, *id, name);
            }
        });
    note(
        ui,
        tr(
            "In the order of the list: drag a gauge there, or press Alt+Up or Alt+Down, to move it.",
        ),
    );
    false
}

/// How it will look under the output, at the levels that matter.
fn preview(ui: &mut egui::Ui, rule: &Gauge) {
    ui.add_space(4.0);
    widgets::caption(ui, tr("Preview"));
    let name = if rule.name.trim().is_empty() {
        tr("Gauge")
    } else {
        rule.name.trim()
    };
    let strip = egui::Frame::new()
        .fill(theme::current().background)
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(16, 14));
    strip.show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(22.0, 12.0);
            for (value, max, text) in preview_values(rule) {
                crate::panels::draw_gauge(ui, name, value, max, &look_of(rule, text));
            }
        });
    });
}

/// The values a preview shows: a light on and off, words, a number, or a
/// bar full and just under each level it changes at.
fn preview_values(rule: &Gauge) -> Vec<(f64, f64, &'static str)> {
    match rule.kind {
        GaugeKind::Light => vec![(1.0, 1.0, ""), (0.0, 1.0, "")],
        GaugeKind::Text => vec![(0.0, 0.0, tr("standing"))],
        GaugeKind::Number if rule.max.trim().is_empty() => vec![(1250.0, 0.0, "")],
        _ => {
            let mut under: Vec<u8> = rule.steps.iter().map(|step| step.under).collect();
            under.sort_unstable_by(|a, b| b.cmp(a));
            under.dedup();
            let mut levels = vec![100.0];
            levels.extend(
                under
                    .iter()
                    .take(3)
                    .map(|under| (f64::from(*under) - 8.0).max(1.0)),
            );
            if levels.len() == 1 {
                levels.extend([64.0, 18.0]);
            }
            levels
                .into_iter()
                .map(|value| (value.round(), 100.0, ""))
                .collect()
        }
    }
}

/// How a gauge made in the editor looks, as the panel draws it.
fn look_of(rule: &Gauge, text: &str) -> muddie_script::GaugeLook {
    let kind = match rule.kind {
        GaugeKind::Bar => "bar",
        GaugeKind::Cells => "cells",
        GaugeKind::Number => "number",
        GaugeKind::Light => "light",
        GaugeKind::Text => "text",
    };
    muddie_script::GaugeLook {
        colour: rule.colour.clone(),
        kind: kind.to_owned(),
        steps: rule
            .steps
            .iter()
            .map(|step| (f64::from(step.under), step.colour.clone()))
            .collect(),
        text: text.to_owned(),
        off_colour: rule.off_colour.clone(),
        ..muddie_script::GaugeLook::default()
    }
}
