//! What the MUD sends besides text, GMCP and MSDP, kept to be looked at:
//! to know what there is before writing a gauge or a script for it.

use std::collections::VecDeque;

use serde_json::Value;

use super::*;
use crate::theme::Weight;
use crate::widgets;

/// How many earlier messages of each name are kept.
const KEPT: usize = 12;

/// How many different things are kept at most.
const MOST: usize = 400;

/// What was received under one name: a GMCP package or a MSDP variable.
#[derive(Clone)]
pub struct Seen {
    pub name: String,
    /// The protocol it came by: GMCP, MSDP, MXP...
    pub protocol: String,
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
        format!("{} · {} × · {}", self.protocol, self.count, self.at)
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
        self.saw(if msdp { "MSDP" } else { "GMCP" }, name, json);
    }

    /// Whether what arrives is being kept to be looked at: while it is
    /// shown, in the editor, or if asked to go on outside it. It is not
    /// otherwise, so that it costs nothing.
    pub(super) fn recording(&self) -> bool {
        self.data_open() || self.data_keep
    }

    /// Whether what the MUD sends is on the screen.
    pub(super) fn data_open(&self) -> bool {
        self.rules_open && self.rules_section == automation::DATA
    }

    /// Tells the connection whether to say what it otherwise passes over,
    /// when that has changed.
    pub(super) fn sync_trace(&mut self) {
        let wanted = self.recording();
        if self.trace_sent != Some(wanted)
            && let Some(connection) = &self.connection
        {
            let _ = connection.commands.send(Command::Trace(wanted));
            self.trace_sent = Some(wanted);
        }
    }

    /// Keeps something that arrived by `protocol` under `name`: the
    /// latest, and a few of those before it.
    pub(super) fn saw(&mut self, protocol: &str, name: &str, json: &str) {
        let msdp = protocol == "MSDP";
        let at = crate::output::clock();
        match self
            .data_seen
            .iter_mut()
            .find(|seen| seen.name == name && seen.protocol == protocol)
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
                // No more names than can be looked through.
                if self.data_seen.len() >= MOST {
                    return;
                }
                self.data_seen.push(Seen {
                    name: name.to_owned(),
                    protocol: protocol.to_owned(),
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

    /// Which of its views the session is on: the game, the editor, or
    /// the editor on what the MUD sends.
    pub fn view(&self) -> usize {
        match (self.rules_open, self.data_open()) {
            (_, true) => 2,
            (true, false) => 1,
            (false, false) => 0,
        }
    }

    /// Goes to one of the session's views: the game, the editor where it
    /// was left, or the editor on what the MUD sends.
    pub fn set_view(&mut self, view: usize) {
        match view {
            0 if self.rules_open => self.close_automation(),
            1 => self.reopen_automation(),
            2 => self.open_automation(automation::DATA),
            _ => {}
        }
    }

    /// Which protocols the MUD offered in this connection, and which of
    /// them are in use: without GMCP or MSDP there is nothing below.
    fn protocols_shown(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        let small = |text: &str, colour| {
            RichText::new(text)
                .font(theme::font(Weight::Medium, 11.5))
                .color(colour)
        };
        let protocols = self.protocols.clone();
        let state = |name: &str| protocols.iter().find(|(known, _)| known == name);
        let mut filter = self.data_protocol.clone();
        // The ones that carry something to see, then whatever else the
        // MUD offered.
        let main = ["GMCP", "MSDP", "MSSP", "MXP", "MSP", "MCCP2", "CHARSET"];
        let others = (protocols.iter())
            .map(|(name, _)| name.as_str())
            .filter(|name| !main.contains(name) && !["ECHO", "SGA", "EOR"].contains(name));
        let names: Vec<&str> = main.iter().copied().chain(others).collect();
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            widgets::caption(ui, tr("Protocols"));
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for name in names {
                let chosen = filter.as_deref() == Some(name);
                // In use, it is filled; offered and not used, an outline;
                // not offered, only its name, faint.
                let (colour, fill, edge, about) = match state(name) {
                    _ if chosen => (
                        theme.on_accent,
                        theme.accent,
                        theme.accent,
                        tr("Filtering by this protocol"),
                    ),
                    Some((_, true)) => (theme.text, theme.hover, theme.hover, tr("In use")),
                    Some((_, false)) => (
                        theme.text_dim,
                        egui::Color32::TRANSPARENT,
                        theme.text_faint,
                        tr("Offered by the MUD, not used by Muddie"),
                    ),
                    None => (
                        theme.text_faint.gamma_multiply(0.55),
                        egui::Color32::TRANSPARENT,
                        egui::Color32::TRANSPARENT,
                        tr("Not offered by this MUD"),
                    ),
                };
                // Drawn to its own size, so that a row of them wraps.
                let galley = ui.painter().layout_no_wrap(
                    name.to_owned(),
                    theme::font(Weight::Medium, 11.5),
                    colour,
                );
                let size = galley.size() + egui::vec2(14.0, 8.0);
                let (rect, shown) = ui.allocate_exact_size(size, egui::Sense::click());
                let corner = egui::CornerRadius::same(6);
                ui.painter().rect_filled(rect, corner, fill);
                ui.painter().rect_stroke(
                    rect,
                    corner,
                    egui::Stroke::new(1.0, edge),
                    egui::StrokeKind::Inside,
                );
                ui.painter()
                    .galley(rect.center() - galley.size() / 2.0, galley, colour);
                shown.widget_info(|| {
                    let said = format!("{name}: {about}");
                    egui::WidgetInfo::selected(egui::Role::Button, true, chosen, said)
                });
                // A click shows only what came by it; another, all again.
                if shown.clicked() {
                    filter = (!chosen).then(|| name.to_owned());
                }
                shown.on_hover_text(about);
            }
        });
        self.data_protocol = filter;
        ui.horizontal_wrapped(|ui| {
            let key = tr("Filled: in use. Outlined: offered by the MUD, not used by Muddie. Faint: not offered.");
            ui.add(Label::new(small(key, theme.text_faint)).wrap());
        });
        // Why there may be nothing to see.
        let data = |name: &str| state(name).is_some_and(|(_, on)| *on);
        if self.connection.is_some() && !data("GMCP") && !data("MSDP") {
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_space(4.0);
                let none = tr("This MUD offers neither GMCP nor MSDP: all it sends is text, and what it says on its channels comes as lines of the game.");
                ui.add(Label::new(small(none, theme.text_dim)).wrap());
            });
        }
        // It is kept only while it is looked at, unless asked otherwise.
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let keep = tr("Record in the background");
            widgets::switch(ui, &mut self.data_keep, keep);
            ui.add(Label::new(small(keep, theme.text_dim)).selectable(false));
        });
        if !self.data_seen.is_empty() && widgets::quiet_button(ui, tr("Clear")).clicked() {
            self.data_seen.clear();
            self.data_selected = 0;
        }
        ui.add_space(6.0);
    }

    /// The list of what the MUD has sent, in the editor: which protocols
    /// are in use, as a filter, then each thing by its name.
    pub(super) fn data_list(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        self.protocols_shown(ui);
        widgets::text_box(ui, &mut self.data_filter, tr("Search"));
        ui.add_space(4.0);
        let filter = self.data_filter.trim().to_lowercase();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if self.data_seen.is_empty() {
                    ui.add_space(24.0);
                    let none = RichText::new(tr("No data received since this view was opened."))
                        .color(theme.text_faint);
                    ui.vertical_centered(|ui| ui.label(none));
                }
                // Those that pass what is searched for and the protocol
                // chosen; the one laid out is among them.
                let listed: Vec<usize> = (0..self.data_seen.len())
                    .filter(|&index| {
                        let seen = &self.data_seen[index];
                        let found = filter.is_empty()
                            || seen.name.to_lowercase().contains(&filter)
                            || seen.json.to_lowercase().contains(&filter);
                        let by =
                            (self.data_protocol.as_ref()).is_none_or(|only| *only == seen.protocol);
                        found && by
                    })
                    .collect();
                if !listed.contains(&self.data_selected) {
                    self.data_selected = listed.first().copied().unwrap_or(usize::MAX);
                }
                for index in listed {
                    let seen = &self.data_seen[index];
                    let (name, summary) = (seen.name.clone(), seen.summary());
                    if widgets::list_row(ui, self.data_selected == index, &name, &summary, true)
                        .clicked()
                    {
                        self.data_selected = index;
                    }
                }
            });
    }

    /// The thing chosen in the list, laid out; or what the list is for.
    pub(super) fn data_view(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        ui.spacing_mut().item_spacing.y = 10.0;
        let index = self.data_selected;
        if index < self.data_seen.len() {
            return self.data_detail(ui, index);
        }
        ui.add_space(60.0);
        let about = tr(
            "What the MUD sends besides text, with GMCP or MSDP, appears here as it comes:
look at it to know what a gauge can follow or a script can use.",
        );
        ui.vertical_centered(|ui| ui.label(RichText::new(about).color(theme.text_faint)));
    }

    /// One thing the MUD sent, laid out: its values one by one, each with
    /// its path to copy and, if it is a number, a gauge to make of it;
    /// then the message as it came, and those before it.
    fn data_detail(&mut self, ui: &mut egui::Ui, index: usize) {
        let theme = theme::current();
        let Some(seen) = self.data_seen.get(index).cloned() else {
            return;
        };
        ui.horizontal(|ui| {
            let title = RichText::new(&seen.name)
                .font(theme::font(Weight::Semibold, 20.0))
                .color(theme.text);
            ui.add(Label::new(title).selectable(true));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let kind = seen.protocol.as_str();
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
            ui.label(RichText::new(tr("Empty message.")).color(theme.text_faint));
        }
        let mut gauge = None;
        widgets::group(ui, |ui| {
            for leaf in &all {
                widgets::setting_row(ui, &leaf.path, &mut |ui| {
                    if leaf.gauge && crate::widgets::button(ui, tr("Make a gauge")).clicked() {
                        gauge = Some(leaf.path.clone());
                    }
                    let copy = crate::widgets::button(ui, tr("Copy the path"));
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
        widgets::caption(ui, tr("Raw message"));
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
            widgets::caption(ui, tr("History"));
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
        // Made in the editor, and shown there to be finished.
        self.open_automation(automation::GAUGES);
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
            value: path.to_owned(),
            max: likely_max(path, all).unwrap_or_default(),
            panel,
            ..crate::rules::Gauge::starter()
        });
        self.select_rule(automation::GAUGES, self.rules_draft.gauges.len() - 1);
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
