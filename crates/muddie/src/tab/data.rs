//! What the MUD sends besides text, GMCP and MSDP, kept to be looked at:
//! to know what there is before writing a gauge or a script for it.

use std::collections::VecDeque;

use serde_json::Value;

use super::*;
use crate::theme::Weight;
use crate::widgets;

/// How many earlier messages of each name are kept.
const KEPT: usize = 12;

/// What was received under one name: a GMCP package or a MSDP variable.
#[derive(Clone)]
pub struct Seen {
    pub name: String,
    pub msdp: bool,
    /// The latest, as JSON; empty if it came with nothing.
    pub json: String,
    /// When the latest came, and how many have.
    pub at: String,
    pub count: u32,
    /// Those before the latest, newest first: when, and what.
    pub earlier: VecDeque<(String, String)>,
}

impl Seen {
    /// How many times and when, for lists.
    pub fn summary(&self) -> String {
        format!("{} × · {}", self.count, self.at)
    }
}

/// The values in a message, each with the path that leads to it.
/// `gauge` says whether a gauge could follow it: a number, reached
/// through names only.
pub struct Leaf {
    pub path: String,
    pub shown: String,
    pub gauge: bool,
}

/// Lays a message out as its values. Fields of tables are reached with a
/// dot, as gauges and scripts reach them; items of lists by their place,
/// counted from one as Luau counts.
pub fn leaves(path: &str, value: &Value, named: bool, into: &mut Vec<Leaf>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                leaves(&format!("{path}.{key}"), value, named, into);
            }
        }
        Value::Array(items) => {
            for (place, value) in items.iter().enumerate() {
                leaves(&format!("{path}[{}]", place + 1), value, false, into);
            }
        }
        leaf => {
            let number = match leaf {
                Value::Number(_) => true,
                Value::String(text) => text.trim().parse::<f64>().is_ok(),
                _ => false,
            };
            let shown = match leaf {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            into.push(Leaf {
                path: path.to_owned(),
                shown,
                gauge: number && named,
            });
        }
    }
}

/// Where the maximum of a value is likely to be, among the values beside
/// it: `max` for a `current`, `maxhp` for an `hp`, and their like.
pub fn likely_max(path: &str, all: &[Leaf]) -> Option<String> {
    let (parent, key) = path.rsplit_once('.')?;
    let key = key.to_lowercase();
    let wanted: Vec<String> = if matches!(key.as_str(), "current" | "cur" | "value" | "now") {
        vec!["max".into(), "maximum".into()]
    } else {
        vec![
            format!("max{key}"),
            format!("{key}max"),
            format!("max_{key}"),
            format!("{key}_max"),
        ]
    };
    let beside = all.iter().filter(|leaf| {
        leaf.gauge
            && leaf
                .path
                .rsplit_once('.')
                .is_some_and(|(of, _)| of == parent)
    });
    let mut beside = beside.filter(|leaf| leaf.path != path);
    beside
        .find(|leaf| {
            wanted.contains(
                &leaf
                    .path
                    .rsplit('.')
                    .next()
                    .unwrap_or_default()
                    .to_lowercase(),
            )
        })
        .map(|leaf| leaf.path.clone())
}

impl Tab {
    /// Notes something the MUD sent, for the editor to show.
    pub(super) fn saw_data(&mut self, name: &str, msdp: bool, json: &str) {
        let at = chrono::Local::now().format("%H:%M:%S").to_string();
        match self
            .data_seen
            .iter_mut()
            .find(|seen| seen.name == name && seen.msdp == msdp)
        {
            Some(seen) => {
                let before = (
                    std::mem::replace(&mut seen.at, at),
                    std::mem::replace(&mut seen.json, json.to_owned()),
                );
                seen.earlier.push_front(before);
                seen.earlier.truncate(KEPT);
                seen.count += 1;
            }
            None => {
                self.data_seen.push(Seen {
                    name: name.to_owned(),
                    msdp,
                    json: json.to_owned(),
                    at,
                    count: 1,
                    earlier: VecDeque::new(),
                });
                self.data_seen.sort_by_key(|seen| seen.name.to_lowercase());
            }
        }
    }

    /// One thing the MUD sent, laid out: its values one by one, each with
    /// its path to copy and, if it is a number, a gauge to make of it;
    /// then the message as it came, and those before it.
    pub(super) fn data_detail(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let theme = theme::current();
        let Some(seen) = self.data_seen.get(index).cloned() else {
            return false;
        };
        ui.horizontal(|ui| {
            let title = RichText::new(&seen.name)
                .font(theme::font(Weight::Semibold, 20.0))
                .color(theme.text);
            ui.add(Label::new(title).selectable(true));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let kind = if seen.msdp { "MSDP" } else { "GMCP" };
                let times = crate::i18n::trn(
                    "received {0} time",
                    "received {0} times",
                    u64::from(seen.count),
                );
                let said = format!("{kind} · {times} · {}", seen.at);
                ui.label(
                    RichText::new(said)
                        .font(theme::font(Weight::Regular, 12.0))
                        .color(theme.text_faint),
                );
            });
        });
        ui.add(egui::Separator::default().spacing(2.0));
        let value: Value = serde_json::from_str(&seen.json).unwrap_or(Value::Null);
        let mut all = Vec::new();
        leaves(
            &seen.name,
            &value,
            !seen.msdp || !value.is_object(),
            &mut all,
        );
        widgets::caption(ui, tr("Values"));
        if seen.json.is_empty() {
            ui.label(RichText::new(tr("It came with nothing in it.")).color(theme.text_faint));
        }
        let mut gauge = None;
        widgets::group(ui, |ui| {
            for leaf in &all {
                widgets::setting_row(ui, &leaf.path, &mut |ui| {
                    if leaf.gauge && ui.button(tr("Make a gauge")).clicked() {
                        gauge = Some(leaf.path.clone());
                    }
                    let copy = ui.button(tr("Copy the path"));
                    if copy.clicked() {
                        ui.ctx().copy_text(leaf.path.clone());
                    }
                    let shown = RichText::new(&leaf.shown)
                        .font(egui::FontId::monospace(13.0))
                        .color(theme.text);
                    ui.add(Label::new(shown).truncate());
                    copy
                });
            }
        });
        if !all.is_empty() {
            let how =
                tr("A path is what a gauge follows, and what a script reads after gmcp(\"…\").");
            ui.label(
                RichText::new(how)
                    .font(theme::font(Weight::Regular, 11.5))
                    .color(theme.text_faint),
            );
        }
        ui.add_space(6.0);
        widgets::caption(ui, tr("As it came"));
        let pretty = serde_json::to_string_pretty(&value).unwrap_or_default();
        let mut raw = if seen.json.is_empty() {
            ""
        } else {
            pretty.as_str()
        };
        let rows = pretty.lines().count().clamp(1, 16);
        ui.add(
            TextEdit::multiline(&mut raw)
                .font(egui::FontId::monospace(13.0))
                .margin(egui::Margin::symmetric(12, 10))
                .desired_rows(rows)
                .desired_width(f32::INFINITY),
        );
        if !seen.earlier.is_empty() {
            ui.add_space(6.0);
            widgets::caption(ui, tr("Before that"));
            for (at, json) in &seen.earlier {
                let line = format!("{at}  {json}");
                ui.add(
                    Label::new(
                        RichText::new(line)
                            .font(egui::FontId::monospace(12.0))
                            .color(theme.text_dim),
                    )
                    .truncate(),
                );
            }
        }
        if let Some(path) = gauge {
            self.gauge_from(&path, &all);
        }
        false
    }

    /// Makes a gauge that follows the value at `path`, with the maximum
    /// that seems to go with it, and opens it to be finished.
    fn gauge_from(&mut self, path: &str, all: &[Leaf]) {
        if !self
            .layout
            .panels
            .iter()
            .any(|panel| panel.kind == layout::Kind::Gauges)
        {
            self.add_panel(layout::Kind::Gauges);
        }
        let panel = (self.layout.panels.iter())
            .find(|panel| panel.kind == layout::Kind::Gauges)
            .map_or(0, |panel| panel.id);
        // Named after what it is: `health`, not the `current` inside it.
        let mut parts = path.rsplit('.');
        let last = parts.next().unwrap_or(path);
        let plain = matches!(
            last.to_lowercase().as_str(),
            "current" | "cur" | "value" | "now"
        );
        let name = if plain {
            parts.next().unwrap_or(last)
        } else {
            last
        };
        self.rules_draft.gauges.push(crate::rules::Gauge {
            name: name.to_owned(),
            enabled: true,
            value: path.to_owned(),
            max: likely_max(path, all).unwrap_or_default(),
            segments: true,
            low: 30,
            panel,
            ..crate::rules::Gauge::default()
        });
        self.gauges_open = true;
        self.gauges_panel = panel;
        self.gauge_selected = self.rules_draft.gauges.len() - 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_laid_out_as_values_with_their_paths() {
        let value: Value = serde_json::from_str(
            r#"{"health": {"current": 25, "max": "100"}, "hp": 3, "maxhp": 9, "name": "Ana", "bag": [5, "sword"]}"#,
        )
        .unwrap();
        let mut all = Vec::new();
        leaves("Char.Vitals", &value, true, &mut all);
        let shown: Vec<(&str, &str, bool)> = all
            .iter()
            .map(|leaf| (leaf.path.as_str(), leaf.shown.as_str(), leaf.gauge))
            .collect();
        assert_eq!(
            shown,
            [
                ("Char.Vitals.bag[1]", "5", false),
                ("Char.Vitals.bag[2]", "sword", false),
                ("Char.Vitals.health.current", "25", true),
                ("Char.Vitals.health.max", "100", true),
                ("Char.Vitals.hp", "3", true),
                ("Char.Vitals.maxhp", "9", true),
                ("Char.Vitals.name", "Ana", false),
            ]
        );
        // The maximum that goes with a value is found beside it.
        assert_eq!(
            likely_max("Char.Vitals.health.current", &all).unwrap(),
            "Char.Vitals.health.max"
        );
        assert_eq!(
            likely_max("Char.Vitals.hp", &all).unwrap(),
            "Char.Vitals.maxhp"
        );
        assert_eq!(likely_max("Char.Vitals.maxhp", &all), None);
    }
}
