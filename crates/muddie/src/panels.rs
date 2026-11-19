//! The status strip of gauges and the helpers that feed the panels from
//! GMCP.

use eframe::egui::{Color32, ProgressBar, Ui};
use muddie_core::Line;
use muddie_core::ansi::Assembler;
use serde_json::Value;

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

    pub fn show(&self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            for gauge in &self.0 {
                let fraction = if gauge.max > 0.0 {
                    (gauge.value / gauge.max).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                ui.add(
                    ProgressBar::new(fraction as f32)
                        .desired_width(170.0)
                        .fill(color(&gauge.name))
                        .text(format!("{} {}/{}", gauge.name, gauge.value, gauge.max)),
                );
            }
        });
    }
}

/// Dark enough for the white label on top to stay readable.
fn color(name: &str) -> Color32 {
    match name.to_lowercase().as_str() {
        "hp" | "health" => Color32::from_rgb(0xa8, 0x3a, 0x45),
        "mp" | "mana" | "sp" => Color32::from_rgb(0x2f, 0x62, 0xb0),
        "mv" | "moves" | "ep" => Color32::from_rgb(0x2e, 0x7d, 0x4f),
        _ => Color32::from_rgb(0x6a, 0x5a, 0x9c),
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
