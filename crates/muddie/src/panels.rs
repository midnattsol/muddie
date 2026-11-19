//! The status strip of gauges and the helpers that feed the panels from
//! GMCP.

use std::collections::HashMap;

use eframe::egui::{Align2, Color32, FontId, Rect, Role, Sense, Ui, WidgetInfo, vec2};
use muddie_core::Line;
use muddie_core::ansi::Assembler;
use serde_json::Value;

use crate::theme;

/// Value and maximum field names of the vitals most MUDs report in
/// `Char.Vitals`, with the gauge each pair is shown as.
const VITALS: [(&str, &str, &str); 8] = [
    ("HP", "hp", "maxhp"),
    ("MP", "mp", "maxmp"),
    ("MP", "mana", "maxmana"),
    ("SP", "sp", "maxsp"),
    ("EP", "ep", "maxep"),
    ("WP", "wp", "maxwp"),
    ("MV", "mv", "maxmv"),
    ("MV", "moves", "maxmoves"),
];

/// The MSDP variables holding each vital's value and maximum, with the
/// gauge they are shown as.
const MSDP_VITALS: [(&str, &str, &str); 3] = [
    ("HP", "HEALTH", "HEALTH_MAX"),
    ("MP", "MANA", "MANA_MAX"),
    ("MV", "MOVEMENT", "MOVEMENT_MAX"),
];

struct Gauge {
    name: String,
    value: f64,
    max: f64,
}

/// Named bars such as health and mana, in the order they were first set.
#[derive(Default)]
pub struct Gauges(Vec<Gauge>);

impl Gauges {
    /// Sets the named gauge to `(value, max)`, or removes it.
    pub fn set(&mut self, name: &str, level: Option<(f64, f64)>) {
        let index = self.0.iter().position(|gauge| gauge.name == name);
        match (index, level) {
            (Some(index), Some((value, max))) => {
                self.0[index].value = value;
                self.0[index].max = max;
            }
            (None, Some((value, max))) => self.0.push(Gauge {
                name: name.to_owned(),
                value,
                max,
            }),
            (Some(index), None) => {
                self.0.remove(index);
            }
            (None, None) => {}
        }
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// All gauges as a sentence for speech, e.g. "HP 80 of 100, MP 5 of 50".
    pub fn summary(&self) -> String {
        let parts: Vec<String> = (self.0.iter())
            .map(|gauge| format!("{} {} of {}", gauge.name, gauge.value, gauge.max))
            .collect();
        parts.join(", ")
    }

    /// Draws each gauge as a slim bar with its name and numbers above.
    pub fn show(&self, ui: &mut Ui) {
        let theme = theme::current();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            for gauge in &self.0 {
                let fraction = if gauge.max > 0.0 {
                    (gauge.value / gauge.max).clamp(0.0, 1.0) as f32
                } else {
                    0.0
                };
                let numbers = format!("{} / {}", gauge.value, gauge.max);
                let (rect, response) = ui.allocate_exact_size(vec2(150.0, 26.0), Sense::hover());
                let described = format!("{} {numbers}", gauge.name);
                response.widget_info(|| {
                    WidgetInfo::labeled(Role::ProgressIndicator, true, described.as_str())
                });
                let painter = ui.painter();
                let small = FontId::proportional(11.5);
                painter.text(
                    rect.left_top(),
                    Align2::LEFT_TOP,
                    &gauge.name,
                    small.clone(),
                    theme.text_dim,
                );
                painter.text(
                    rect.right_top(),
                    Align2::RIGHT_TOP,
                    numbers,
                    small,
                    theme.text,
                );
                let track =
                    Rect::from_min_max(rect.left_bottom() - vec2(0.0, 6.0), rect.right_bottom());
                painter.rect_filled(track, 3.0, theme.raised);
                let filled =
                    Rect::from_min_size(track.min, vec2(track.width() * fraction, track.height()));
                painter.rect_filled(filled, 3.0, color(&gauge.name));
            }
        });
    }
}

/// The colour of a gauge's bar, from the theme's terminal colours.
fn color(name: &str) -> Color32 {
    let theme = theme::current();
    match name.to_lowercase().as_str() {
        "hp" | "health" => theme.terminal[1],
        "mp" | "mana" | "sp" => theme.terminal[4],
        "mv" | "moves" | "ep" => theme.terminal[2],
        _ => theme.accent,
    }
}

/// Extracts the vitals this client knows how to show from a `Char.Vitals`
/// payload. Numbers may arrive as JSON numbers or as strings.
pub fn vitals(json: &str) -> Vec<(&'static str, f64, f64)> {
    let Ok(Value::Object(fields)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    let number = |key: &str| match fields.get(key)? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    };
    (VITALS.iter())
        .filter_map(|&(name, value, max)| Some((name, number(value)?, number(max)?)))
        .collect()
}

/// Collects vitals from MSDP, which reports a value and its maximum as
/// separate variables that may arrive at different times.
#[derive(Default)]
pub struct MsdpVitals(HashMap<String, f64>);

impl MsdpVitals {
    /// Records an MSDP variable. If it belongs to a vital whose value and
    /// maximum are now both known, returns that gauge's new level.
    pub fn update(&mut self, variable: &str, json: &str) -> Option<(&'static str, f64, f64)> {
        let (name, value, max) = (MSDP_VITALS.into_iter())
            .find(|(_, value, max)| variable == *value || variable == *max)?;
        let Ok(Value::String(text)) = serde_json::from_str(json) else {
            return None;
        };
        self.0
            .insert(variable.to_owned(), text.trim().parse().ok()?);
        Some((name, *self.0.get(value)?, *self.0.get(max)?))
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// The lines of a `Comm.Channel.Text` payload, with its colours.
pub fn channel_text(json: &str) -> Vec<Line> {
    let Ok(Value::Object(fields)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    match fields.get("text") {
        Some(Value::String(text)) => styled_lines(text),
        _ => Vec::new(),
    }
}

/// Splits text that may contain ANSI colour codes into styled lines.
pub fn styled_lines(text: &str) -> Vec<Line> {
    let mut assembler = Assembler::new();
    let mut lines = Vec::new();
    assembler.feed(text, &mut lines);
    lines.extend(assembler.take_partial());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vitals_accept_numbers_and_numeric_strings() {
        let json = r#"{"hp": 80, "maxhp": "100", "mana": "5", "maxmana": 50, "mv": 3, "nl": "9"}"#;
        assert_eq!(vitals(json), [("HP", 80.0, 100.0), ("MP", 5.0, 50.0)]);
        assert_eq!(vitals("[1, 2]"), []);
    }

    #[test]
    fn msdp_vitals_wait_for_both_halves() {
        let mut vitals = MsdpVitals::default();
        assert_eq!(vitals.update("HEALTH", r#""80""#), None);
        assert_eq!(vitals.update("MANA_MAX", r#""50""#), None);
        assert_eq!(
            vitals.update("HEALTH_MAX", r#""100""#),
            Some(("HP", 80.0, 100.0))
        );
        assert_eq!(
            vitals.update("HEALTH", r#""75""#),
            Some(("HP", 75.0, 100.0))
        );
        assert_eq!(vitals.update("ROOM", r#"{"VNUM":"7"}"#), None);
        assert_eq!(vitals.update("MANA", r#""lots""#), None);
    }

    #[test]
    fn gauges_update_in_place_and_summarise() {
        let mut gauges = Gauges::default();
        gauges.set("HP", Some((80.0, 100.0)));
        gauges.set("MP", Some((5.0, 50.0)));
        gauges.set("HP", Some((70.5, 100.0)));
        assert_eq!(gauges.summary(), "HP 70.5 of 100, MP 5 of 50");
        gauges.set("HP", None);
        gauges.set("XP", None);
        assert_eq!(gauges.summary(), "MP 5 of 50");
    }

    #[test]
    fn channel_text_keeps_colours_and_splits_lines() {
        let lines =
            channel_text(r#"{"channel": "ooc", "text": "\u001b[32mAna: hola\u001b[0m\nsegunda"}"#);
        let texts: Vec<String> = lines.iter().map(Line::text).collect();
        assert_eq!(texts, ["Ana: hola", "segunda"]);
        assert_ne!(lines[0].spans[0].style, lines[1].spans[0].style);
    }
}
