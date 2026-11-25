//! The details of a gauge.

use super::*;
use crate::rules::{Announce, AnnounceName, Gauge, GaugeKind, KindName, Step};

impl Tab {
    pub(in crate::tab) fn gauge_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let panels: Vec<(String, String)> = (self.layout.panels.iter())
            .filter(|panel| panel.kind == super::layout::Kind::Gauges)
            .map(|panel| (panel.name.clone(), panel.name.clone()))
            .collect();
        let delete = self.rule_head(ui, Kind::Gauge, index, tr("Gauge name"));
        let rule = &mut self.editor.draft.gauges[index];
        kind_row(ui, rule);
        follows(ui, rule);
        colours(ui, rule);
        read_aloud(ui, rule);
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
    let kinds = GaugeKind::ALL;
    widgets::card(
        ui,
        Some(widgets::Glyph::Type),
        tr("Kind"),
        |_| {},
        |ui| {
            let names = kinds.map(KindName::shown);
            let current = kinds
                .iter()
                .position(|kind| *kind == rule.kind)
                .unwrap_or(0);
            if let Some(chosen) = widgets::segmented(ui, &names, current) {
                rule.kind = kinds[chosen];
            }
            let about = match rule.kind {
                GaugeKind::Bar | GaugeKind::Cells => tr("How full something is, of its maximum."),
                GaugeKind::Number => tr("A number as it is: gold, level, experience."),
                GaugeKind::Light => {
                    tr("On or off, for what is so or is not: hidden, poisoned, in combat.")
                }
                GaugeKind::Text => tr("What the MUD says, as words: a stance, a target, a place."),
                GaugeKind::Effects => tr(
                    "What the character is in, each as a label, only while it lasts: poisoned, blessed, in combat.",
                ),
            };
            note(ui, about);
        },
    );
}

/// What it follows: the value, and its maximum for those with a level.
fn follows(ui: &mut egui::Ui, rule: &mut Gauge) {
    widgets::card(
        ui,
        Some(widgets::Glyph::Data),
        tr("Follows"),
        |_| {},
        |ui| {
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
                GaugeKind::Effects => tr(
                    "A list, or a table by name, of what the character is in: a GMCP package, or a variable a script or a trigger keeps. Each one a name, or a table of name, about and colour.",
                ),
                GaugeKind::Light => tr(
                    "It is on when the value is true, a number other than 0, or words other than no, false or off.",
                ),
                _ => tr(
                    "Where it is: a GMCP package and the fields inside it, such as Char.Vitals.health.current, the name of a MSDP variable, or a variable of the editor's, which a trigger can set. The maximum can also be a number. Incoming data and Variables, in the editor, show what there is.",
                ),
            };
            note(ui, hint);
        },
    );
}

/// Its colour, or for a light, its colours when on and when off; and for
/// one with a level, the colours it turns as it gets low.
fn colours(ui: &mut egui::Ui, rule: &mut Gauge) {
    widgets::card(
        ui,
        Some(widgets::Glyph::Colour),
        tr("Colour"),
        |_| {},
        |ui| {
            if rule.kind != GaugeKind::Light {
                swatches(ui, &mut rule.colour);
            } else {
                light_colours(ui, rule);
            }
            if rule.kind.has_level() {
                steps(ui, rule);
            }
        },
    );
}

/// A light's colours, when on and when off.
fn light_colours(ui: &mut egui::Ui, rule: &mut Gauge) {
    widgets::sub_label(ui, tr("Colour when on"));
    swatches(ui, &mut rule.colour);
    ui.add_space(4.0);
    widgets::sub_label(ui, tr("Colour when off"));
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

/// The colours it turns as it gets low, each under a step, kept in order
/// highest first; and the way to add one.
fn steps(ui: &mut egui::Ui, rule: &mut Gauge) {
    ui.add_space(6.0);
    widgets::sub_label(ui, tr("Colour steps"));
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
            let under = ui.add(
                crate::widgets::number(&mut step.under)
                    .range(1..=100)
                    .suffix(" %"),
            );
            crate::widgets::named_by(ui, &under, &label, tr("Under"));
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
    if widgets::add_button(ui, tr("Add a step")).clicked() {
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

/// How its changes are said aloud; only one with a level has a lowest step.
fn read_aloud(ui: &mut egui::Ui, rule: &mut Gauge) {
    widgets::card(
        ui,
        Some(widgets::Glyph::Say),
        tr("Read aloud"),
        |_| {},
        |ui| {
            let ways: Vec<(Announce, String)> = (Announce::ALL.into_iter())
                .filter(|way| rule.kind.has_level() || *way != Announce::Lowest)
                .map(|way| (way, way.shown().to_owned()))
                .collect();
            let chosen =
                crate::widgets::choose(ui, "gauge announced", &mut rule.announce, &ways, |way| {
                    way.shown().to_owned()
                });
            (ui.ctx()).accesskit_node_builder(chosen.id, |node| node.set_label(tr("Read aloud")));
            let about = match rule.kind.has_level() {
                true => tr("It is said as it falls under a step: as usual, cutting in."),
                false => tr("It is said when it changes: as usual, after what is being read."),
            };
            note(ui, about);
        },
    );
}

/// Which panel of gauges it is shown in. Returns whether a panel was asked
/// for, there being none.
fn shown_in(ui: &mut egui::Ui, rule: &mut Gauge, panels: &[(String, String)]) -> bool {
    widgets::card(
        ui,
        Some(widgets::Glyph::Panel),
        tr("Shown in"),
        |_| {},
        |ui| shown_in_card(ui, rule, panels),
    )
}

fn shown_in_card(ui: &mut egui::Ui, rule: &mut Gauge, panels: &[(String, String)]) -> bool {
    if panels.is_empty() {
        let mut add = false;
        ui.horizontal(|ui| {
            note(ui, tr("There is no panel of gauges to show it in."));
            add = crate::widgets::button(ui, tr("Add a panel of gauges")).clicked();
        });
        return add;
    }
    // Put in none, it is in the first.
    let first = panels[0].1.clone();
    let chosen = crate::widgets::choose(ui, "gauge panel", &mut rule.panel, panels, |_| {
        first.clone()
    });
    (ui.ctx()).accesskit_node_builder(chosen.id, |node| node.set_label(tr("Shown in")));
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
    widgets::card(
        ui,
        Some(widgets::Glyph::Gauge),
        tr("Preview"),
        |_| {},
        |ui| preview_card(ui, rule),
    );
}

fn preview_card(ui: &mut egui::Ui, rule: &Gauge) {
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
                let look = look_of(rule, text);
                crate::panels::draw_gauge(ui, name, value, max, &look, crate::panels::BAR_WIDTH);
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
        GaugeKind::Effects => vec![(0.0, 0.0, "")],
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

/// How a gauge made in the editor looks, as the panel draws it; one of
/// effects, with two to show.
fn look_of(rule: &Gauge, text: &str) -> muddie_script::GaugeLook {
    let sample = |name: &str, colour: &str| muddie_script::Effect {
        name: name.to_owned(),
        colour: colour.to_owned(),
        ..muddie_script::Effect::default()
    };
    let items = match rule.kind {
        GaugeKind::Effects => vec![sample(tr("Poisoned"), "red"), sample(tr("Blessed"), "")],
        _ => Vec::new(),
    };
    muddie_script::GaugeLook {
        colour: rule.colour.clone(),
        kind: rule.kind,
        items,
        steps: rule
            .steps
            .iter()
            .map(|step| (f64::from(step.under), step.colour.clone()))
            .collect(),
        text: text.to_owned(),
        off_colour: rule.off_colour.clone(),
        announce: rule.announce,
    }
}
