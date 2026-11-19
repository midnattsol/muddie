//! The status strip of gauges and the helpers that feed the panels from
//! GMCP.

use std::collections::HashMap;

use eframe::egui::{Align2, Color32, Rect, Role, Sense, Ui, WidgetInfo, vec2};
use muddie_core::Line;
use muddie_core::ansi::Assembler;
use muddie_script::GaugeLook;
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
    look: GaugeLook,
}

/// Named bars such as health and mana, in the order they were first set.
#[derive(Default)]
pub struct Gauges(Vec<Gauge>);

impl Gauges {
    /// Sets the named gauge to `(value, max)`, or removes it.
    pub fn set(&mut self, name: &str, level: Option<(f64, f64)>) {
        self.set_with(name, level, None);
    }

    /// The same, also saying how the gauge is drawn. With no look given a
    /// gauge keeps the one it has.
    pub fn set_with(&mut self, name: &str, level: Option<(f64, f64)>, look: Option<GaugeLook>) {
        let index = self.0.iter().position(|gauge| gauge.name == name);
        match (index, level) {
            (Some(index), Some((value, max))) => {
                let gauge = &mut self.0[index];
                (gauge.value, gauge.max) = (value, max);
                if let Some(look) = look {
                    gauge.look = look;
                }
            }
            (None, Some((value, max))) => self.0.push(Gauge {
                name: name.to_owned(),
                value,
                max,
                look: look.unwrap_or_default(),
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

    /// All gauges as a sentence for speech, e.g. "HP 80 of 100, MP 5 of
    /// 50", each said as what it is.
    pub fn summary(&self) -> String {
        let parts: Vec<String> = (self.0.iter())
            .map(|gauge| match gauge.look.kind.as_str() {
                "light" if gauge.value != 0.0 => format!("{} on", gauge.name),
                "light" => format!("{} off", gauge.name),
                "text" => format!("{} {}", gauge.name, gauge.look.text),
                "number" if gauge.max <= 0.0 => format!("{} {}", gauge.name, gauge.value),
                _ => format!("{} {} of {}", gauge.name, gauge.value, gauge.max),
            })
            .collect();
        parts.join(", ")
    }

    /// Draws, in a row that wraps, the gauges `wanted` says yes to by
    /// their names. Returns whether there were any.
    pub fn show(&self, ui: &mut Ui, wanted: impl Fn(&str) -> bool) -> bool {
        let chosen: Vec<&Gauge> = self.0.iter().filter(|gauge| wanted(&gauge.name)).collect();
        if chosen.is_empty() {
            return false;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(22.0, 8.0);
            for gauge in chosen {
                draw_gauge(ui, &gauge.name, gauge.value, gauge.max, &gauge.look);
            }
        });
        true
    }
}

/// A number without the `.0` a whole one would otherwise have.
fn plain(number: f64) -> String {
    if number.fract() == 0.0 && number.abs() < 1e15 {
        format!("{number:.0}")
    } else {
        format!("{number}")
    }
}

/// Draws one gauge as its look says: a bar or a row of cells under its
/// name and numbers, a number alone, a light that is on or off, or a few
/// words.
pub fn draw_gauge(ui: &mut Ui, name: &str, value: f64, max: f64, look: &GaugeLook) {
    const CELLS: usize = 10;
    let theme = theme::current();
    let small = theme::font(theme::Weight::Medium, 11.5);
    let kind = match look.kind.as_str() {
        "" if look.segments => "cells",
        "" => "bar",
        kind => kind,
    };
    let fraction = if max > 0.0 {
        (value / max).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    // Its colour: the one of the lowest level it is under, else its own.
    let percent = f64::from(fraction) * 100.0;
    let level = (look.steps.iter())
        .filter(|(under, _)| max > 0.0 && percent < *under)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .and_then(|(_, colour)| colour_named(colour));
    let own = colour_named(&look.colour).unwrap_or_else(|| color(name));
    let colour = level.unwrap_or(own);
    let warned = level.is_some() || (look.low > 0.0 && max > 0.0 && percent < look.low);
    let measure = |text: &str, font: &eframe::egui::FontId| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), theme.text)
            .size()
            .x
    };
    match kind {
        "light" => {
            let on = value != 0.0;
            let width = 18.0 + measure(name, &small);
            let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
            let state = if on { "on" } else { "off" };
            response.widget_info(|| {
                WidgetInfo::selected(Role::CheckBox, true, on, format!("{name} {state}"))
            });
            let centre = rect.left_center() + vec2(5.0, 0.0);
            let painter = ui.painter();
            // Off, it is unlit, unless being off has a colour of its own:
            // green for not poisoned, say.
            let off = colour_named(&look.off_colour);
            match (on, off) {
                (true, _) => {
                    painter.circle_filled(centre, 7.0, colour.gamma_multiply(0.25));
                    painter.circle_filled(centre, 4.5, colour);
                }
                (false, Some(off)) => {
                    painter.circle_filled(centre, 4.5, off);
                }
                (false, None) => {
                    painter.circle_stroke(
                        centre,
                        4.0,
                        eframe::egui::Stroke::new(1.2, theme.text_faint),
                    );
                }
            }
            let text = if on || off.is_some() {
                theme.text
            } else {
                theme.text_faint
            };
            painter.text(
                rect.left_center() + vec2(18.0, 0.0),
                Align2::LEFT_CENTER,
                name,
                small,
                text,
            );
        }
        "text" | "number" => {
            let said = if kind == "text" {
                look.text.clone()
            } else if max > 0.0 {
                format!("{} / {}", plain(value), plain(max))
            } else {
                plain(value)
            };
            let large = theme::font(theme::Weight::Medium, 13.5);
            let width = measure(name, &small)
                .max(measure(&said, &large))
                .clamp(40.0, 260.0);
            let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
            response
                .widget_info(|| WidgetInfo::labeled(Role::Label, true, format!("{name} {said}")));
            let painter = ui.painter().with_clip_rect(rect);
            painter.text(
                rect.left_top(),
                Align2::LEFT_TOP,
                name,
                small,
                theme.text_dim,
            );
            let figures = if kind == "number" && warned {
                colour
            } else {
                theme.text
            };
            painter.text(
                rect.left_bottom() + vec2(0.0, 2.0),
                Align2::LEFT_BOTTOM,
                said,
                large,
                figures,
            );
        }
        _ => {
            let numbers = format!("{} / {}", plain(value), plain(max));
            let (rect, response) = ui.allocate_exact_size(vec2(158.0, 28.0), Sense::hover());
            let described = format!("{name} {numbers}");
            response.widget_info(|| {
                WidgetInfo::labeled(Role::ProgressIndicator, true, described.as_str())
            });
            let painter = ui.painter();
            painter.text(
                rect.left_top(),
                Align2::LEFT_TOP,
                name,
                small.clone(),
                theme.text_dim,
            );
            let figures = if warned {
                level.unwrap_or(theme.danger)
            } else {
                theme.text
            };
            painter.text(rect.right_top(), Align2::RIGHT_TOP, numbers, small, figures);
            let empty = theme::mix(theme.surface, theme.text, 0.1);
            if kind == "cells" {
                let gap = 3.0;
                let width = (rect.width() - gap * (CELLS as f32 - 1.0)) / CELLS as f32;
                let filled = (fraction * CELLS as f32).round() as usize;
                // Never all empty while something is left, nor full while short.
                let filled = match filled {
                    0 if fraction > 0.0 => 1,
                    CELLS if fraction < 1.0 => CELLS - 1,
                    filled => filled,
                };
                for cell in 0..CELLS {
                    let left = rect.left() + cell as f32 * (width + gap);
                    let (top, bottom) = (rect.bottom() - 8.0, rect.bottom());
                    // Leaning cells, like the ▰ of a terminal's status line.
                    let lean = 2.5;
                    let points = vec![
                        eframe::egui::pos2(left + lean, top),
                        eframe::egui::pos2(left + width, top),
                        eframe::egui::pos2(left + width - lean, bottom),
                        eframe::egui::pos2(left, bottom),
                    ];
                    let fill = if cell < filled { colour } else { empty };
                    painter.add(eframe::egui::Shape::convex_polygon(
                        points,
                        fill,
                        eframe::egui::Stroke::NONE,
                    ));
                }
            } else {
                let track =
                    Rect::from_min_max(rect.left_bottom() - vec2(0.0, 6.0), rect.right_bottom());
                painter.rect_filled(track, 3.0, empty);
                let filled =
                    Rect::from_min_size(track.min, vec2(track.width() * fraction, track.height()));
                painter.rect_filled(filled, 3.0, colour);
            }
        }
    }
}

/// The colour of a gauge's bar when it was given none, from its name.
fn color(name: &str) -> Color32 {
    let theme = theme::current();
    match name.to_lowercase().as_str() {
        "hp" | "health" | "salud" | "vida" | "pv" => theme.terminal[1],
        "mp" | "mana" | "sp" | "maná" | "pm" => theme.terminal[4],
        "mv" | "moves" | "ep" | "stamina" | "resistencia" => theme.terminal[2],
        _ => theme.accent,
    }
}

/// The names a gauge's colour can be chosen by.
pub const GAUGE_COLOURS: [&str; 7] = ["red", "green", "yellow", "blue", "magenta", "cyan", "white"];

/// A colour given by name, from the theme's palette, or as `#rrggbb`.
pub fn colour_named(name: &str) -> Option<Color32> {
    let name = name.trim().to_lowercase();
    if let Some(hex) = name.strip_prefix('#') {
        let value = u32::from_str_radix(hex, 16)
            .ok()
            .filter(|_| hex.len() == 6)?;
        return Some(Color32::from_rgb(
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        ));
    }
    let index = match name.as_str() {
        "red" => 1,
        "green" => 2,
        "yellow" => 3,
        "blue" => 4,
        "magenta" | "purple" => 5,
        "cyan" => 6,
        "white" => 7,
        "accent" => return Some(theme::current().accent),
        _ => return None,
    };
    Some(theme::current().terminal[index])
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
