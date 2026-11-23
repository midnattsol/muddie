//! The status strip of gauges and the helpers that feed the panels from
//! GMCP.

use std::collections::HashMap;

use eframe::egui::{Align2, Color32, Rect, Role, Sense, Ui, WidgetInfo, vec2};
use muddie_core::Line;
use muddie_core::ansi::Assembler;
use muddie_script::GaugeLook;
use serde_json::Value;

use crate::i18n::trf;
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
    /// The lowest of its warnings it is under, as a percentage.
    under: Option<f64>,
    /// The script that set it last, or the rules; none for those the
    /// client makes of the MUD's vitals.
    owner: Option<String>,
    /// Whether its owner is being run again and has not set it since:
    /// it goes if it does not (see [`Gauges::drop_unset`]).
    unset: bool,
}

impl Gauge {
    /// The lowest of its warnings it is under, as a percentage: the
    /// steps where it changes colour, and its low mark.
    fn under(&self) -> Option<f64> {
        if self.max <= 0.0 {
            return None;
        }
        let percent = (self.value / self.max).clamp(0.0, 1.0) * 100.0;
        let marks = self.look.steps.iter().map(|(under, _)| *under);
        let low = (self.look.low > 0.0).then_some(self.look.low);
        marks
            .chain(low)
            .filter(|under| percent < *under)
            .min_by(f64::total_cmp)
    }

    /// What it is, said aloud.
    fn said(&self) -> String {
        let value = self.value.to_string();
        match self.look.kind.as_str() {
            "light" if self.value != 0.0 => trf("{0} on", &[&self.name]),
            "light" => trf("{0} off", &[&self.name]),
            "text" => format!("{} {}", self.name, self.look.text),
            "effects" => effects_said(&self.name, &self.look.items),
            "number" if self.max <= 0.0 => format!("{} {value}", self.name),
            _ => trf(
                "{0} {1} of {2}",
                &[&self.name, &value, &self.max.to_string()],
            ),
        }
    }
}

/// Named bars such as health and mana, in the order they were first set.
#[derive(Default)]
pub struct Gauges(Vec<Gauge>);

impl Gauges {
    /// Sets the named gauge to `(value, max)`, or removes it. Returns what
    /// to say when it falls under one of its warnings, as when it turns
    /// to a warning colour: what is seen at a glance is heard.
    pub fn set(&mut self, name: &str, level: Option<(f64, f64)>) -> Option<String> {
        self.set_by(name, level, None, None)
    }

    /// The same, also saying how the gauge is drawn, and set by `owner`, a
    /// script or the rules. With no look given a gauge keeps the one it
    /// has, and with no owner the owner it has.
    pub fn set_by(
        &mut self,
        name: &str,
        level: Option<(f64, f64)>,
        look: Option<GaugeLook>,
        owner: Option<&str>,
    ) -> Option<String> {
        let index = self.0.iter().position(|gauge| gauge.name == name);
        let index = match (index, level) {
            (Some(index), Some((value, max))) => {
                let gauge = &mut self.0[index];
                (gauge.value, gauge.max) = (value, max);
                if let Some(look) = look {
                    gauge.look = look;
                }
                if let Some(owner) = owner {
                    gauge.owner = Some(owner.to_owned());
                }
                gauge.unset = false;
                index
            }
            (None, Some((value, max))) => {
                let mut gauge = Gauge {
                    name: name.to_owned(),
                    value,
                    max,
                    look: look.unwrap_or_default(),
                    under: None,
                    owner: owner.map(str::to_owned),
                    unset: false,
                };
                // As it starts is not news.
                gauge.under = gauge.under();
                self.0.push(gauge);
                return None;
            }
            (Some(index), None) => {
                self.0.remove(index);
                return None;
            }
            (None, None) => return None,
        };
        let gauge = &mut self.0[index];
        let (before, now) = (gauge.under, gauge.under());
        gauge.under = now;
        let lower = match (before, now) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(before), Some(now)) => now < before,
        };
        lower.then(|| gauge.said())
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Notes that `owner` is run again: what it set and does not set
    /// again by [`Gauges::drop_unset`] goes, as a gauge renamed, deleted or
    /// switched off.
    pub fn running_again(&mut self, owner: &str) {
        for gauge in &mut self.0 {
            gauge.unset |= gauge.owner.as_deref() == Some(owner);
        }
    }

    /// Takes away the gauges whose owner was run again and did not set
    /// them.
    pub fn drop_unset(&mut self) {
        self.0.retain(|gauge| !gauge.unset);
    }

    /// Takes away what `owner` set: a script stopped.
    pub fn drop_owned_by(&mut self, owner: &str) {
        self.0.retain(|gauge| gauge.owner.as_deref() != Some(owner));
    }

    /// Puts the gauges in the order `place` gives each by its name,
    /// those it gives none after, as they were.
    pub fn arrange(&mut self, place: impl Fn(&str) -> Option<usize>) {
        let place = |gauge: &Gauge| place(&gauge.name).unwrap_or(usize::MAX);
        if !self.0.is_sorted_by_key(place) {
            self.0.sort_by_key(place);
        }
    }

    /// All gauges as a sentence for speech, e.g. "HP 80 of 100, MP 5 of
    /// 50", each said as what it is.
    pub fn summary(&self) -> String {
        let parts: Vec<String> = self.0.iter().map(Gauge::said).collect();
        parts.join(", ")
    }

    /// Draws, in a row that wraps, the gauges `wanted` says yes to by
    /// their names. Returns whether there were any.
    /// `editable` says which can be pressed, to be changed: those made in
    /// the editor. Returns whether there were any, and the one pressed.
    pub fn show(
        &self,
        ui: &mut Ui,
        wanted: impl Fn(&str) -> bool,
        editable: impl Fn(&str) -> bool,
    ) -> (bool, Option<String>) {
        let chosen: Vec<&Gauge> = self.0.iter().filter(|gauge| wanted(&gauge.name)).collect();
        if chosen.is_empty() {
            return (false, None);
        }
        let mut pressed = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(22.0, 8.0);
            for gauge in chosen {
                let drawn =
                    ui.scope(|ui| draw_gauge(ui, &gauge.name, gauge.value, gauge.max, &gauge.look));
                if !editable(&gauge.name) {
                    continue;
                }
                let id = ui.make_persistent_id(("edit gauge", &gauge.name));
                let label = crate::i18n::trf("Edit {0}", &[&gauge.name]);
                let press = ui.interact(drawn.response.rect, id, Sense::click());
                press.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));
                if press.hovered() {
                    ui.painter().rect_stroke(
                        drawn.response.rect.expand(4.0),
                        6.0,
                        eframe::egui::Stroke::new(1.0, theme::current().border),
                        eframe::egui::StrokeKind::Outside,
                    );
                }
                if press
                    .on_hover_cursor(eframe::egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    pressed = Some(gauge.name.clone());
                }
            }
        });
        (true, pressed)
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
    let kind = match look.kind.as_str() {
        "" => "bar",
        kind => kind,
    };
    let shade = Shade::of(name, value, max, look);
    match kind {
        "light" => draw_light(ui, name, value != 0.0, &shade, look),
        "effects" => draw_effects(ui, name, &look.items, &shade),
        "text" => draw_words(ui, name, look.text.clone(), &shade, false),
        "number" => {
            let said = if max > 0.0 {
                format!("{} / {}", plain(value), plain(max))
            } else {
                plain(value)
            };
            draw_words(ui, name, said, &shade, true);
        }
        _ => draw_bar(ui, name, value, max, &shade, kind == "cells"),
    }
}

/// The colour a gauge is drawn in, and how full it is: the colour of the
/// lowest level it is under, else its own; and whether it is low enough to
/// stand out.
struct Shade {
    fraction: f32,
    level: Option<eframe::egui::Color32>,
    colour: eframe::egui::Color32,
    warned: bool,
}

impl Shade {
    fn of(name: &str, value: f64, max: f64, look: &GaugeLook) -> Self {
        let fraction = if max > 0.0 {
            (value / max).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let percent = f64::from(fraction) * 100.0;
        let level = (look.steps.iter())
            .filter(|(under, _)| max > 0.0 && percent < *under)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .and_then(|(_, colour)| colour_named(colour));
        let own = colour_named(&look.colour).unwrap_or_else(|| color(name));
        let warned = level.is_some() || (look.low > 0.0 && max > 0.0 && percent < look.low);
        Self {
            fraction,
            level,
            colour: level.unwrap_or(own),
            warned,
        }
    }
}

/// How wide a text is in a font.
fn measure(ui: &Ui, text: &str, font: &eframe::egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font.clone(), theme::current().text)
        .size()
        .x
}

/// A light, on or off, with its name. Off, it is unlit, unless being off
/// has a colour of its own: green for not poisoned, say.
fn draw_light(ui: &mut Ui, name: &str, on: bool, shade: &Shade, look: &GaugeLook) {
    let theme = theme::current();
    let small = theme::font(theme::Weight::Medium, 11.5);
    let width = 18.0 + measure(ui, name, &small);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
    let state = if on { "on" } else { "off" };
    response
        .widget_info(|| WidgetInfo::selected(Role::CheckBox, true, on, format!("{name} {state}")));
    let centre = rect.left_center() + vec2(5.0, 0.0);
    let painter = ui.painter();
    let off = colour_named(&look.off_colour);
    match (on, off) {
        (true, _) => {
            painter.circle_filled(centre, 7.0, shade.colour.gamma_multiply(0.25));
            painter.circle_filled(centre, 4.5, shade.colour);
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

/// What a gauge of effects says: its name and what is on, or that nothing
/// is.
fn effects_said(name: &str, items: &[muddie_script::Effect]) -> String {
    if items.is_empty() {
        return trf("{0}: none", &[name]);
    }
    let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
    format!("{name}: {}", names.join(", "))
}

/// The effects that are on, each a label in its colour: a dot and its
/// name on a tint of it, what it is shown on hovering it. As many as fit
/// in the width of the panel; the rest under one more, "+3", that lists
/// them. Nothing, while none is on.
fn draw_effects(ui: &mut Ui, name: &str, items: &[muddie_script::Effect], shade: &Shade) {
    const HEIGHT: f32 = 24.0;
    const GAP: f32 = 6.0;
    if items.is_empty() {
        return;
    }
    let theme = theme::current();
    let font = theme::font(theme::Weight::Medium, 12.0);
    let width_of = |text: &str| 26.0 + measure(ui, text, &font);
    let more_of = |left: usize| format!("+{left}");
    // How many fit, leaving room for the one more if not all do.
    let room = ui.max_rect().width();
    let widths: Vec<f32> = items.iter().map(|item| width_of(&item.name)).collect();
    let mut shown = items.len();
    let total = |count: usize| -> f32 {
        let chips: f32 = widths[..count].iter().sum::<f32>() + GAP * count as f32;
        match count < items.len() {
            true => chips + width_of(&more_of(items.len() - count)) - 12.0,
            false => chips - GAP,
        }
    };
    while shown > 1 && total(shown) > room {
        shown -= 1;
    }
    let (rect, response) =
        ui.allocate_exact_size(vec2(total(shown).min(room), HEIGHT), Sense::hover());
    let said = effects_said(name, items);
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &said));
    let painter = ui.painter_at(rect.expand(2.0));
    let mut x = rect.left();
    for (item, width) in items.iter().zip(&widths).take(shown) {
        let colour = colour_named(&item.colour).unwrap_or(shade.colour);
        let chip = Rect::from_min_size(eframe::egui::pos2(x, rect.top()), vec2(*width, HEIGHT));
        painter.rect_filled(chip, HEIGHT / 2.0, colour.gamma_multiply(0.18));
        painter.circle_filled(chip.left_center() + vec2(11.0, 0.0), 3.5, colour);
        painter.text(
            chip.left_center() + vec2(19.0, 0.0),
            Align2::LEFT_CENTER,
            &item.name,
            font.clone(),
            theme.text,
        );
        if !item.about.is_empty() {
            let id = ui.make_persistent_id(("effect", name, &item.name));
            ui.interact(chip, id, Sense::hover())
                .on_hover_text(&item.about);
        }
        x += width + GAP;
    }
    let left = &items[shown..];
    if left.is_empty() {
        return;
    }
    // The rest, under one more: listed with what each is.
    let label = more_of(left.len());
    let chip = Rect::from_min_max(eframe::egui::pos2(x, rect.top()), rect.max);
    let names: Vec<&str> = left.iter().map(|item| item.name.as_str()).collect();
    let said = trf(
        "{0} more: {1}",
        &[&left.len().to_string(), &names.join(", ")],
    );
    let more = ui.interact(
        chip,
        ui.make_persistent_id(("more effects", name)),
        Sense::click(),
    );
    more.widget_info(|| WidgetInfo::labeled(Role::Button, true, &said));
    let fill = if more.hovered() {
        theme.raised
    } else {
        theme.surface
    };
    painter.rect_filled(chip, HEIGHT / 2.0, fill);
    painter.text(
        chip.center(),
        Align2::CENTER_CENTER,
        &label,
        font,
        theme.text_dim,
    );
    crate::widgets::menu_of(&more).show(|ui| {
        crate::widgets::menu_width(ui);
        for item in left {
            crate::widgets::menu_heading(ui, &item.name);
            if !item.about.is_empty() {
                ui.label(eframe::egui::RichText::new(&item.about).color(theme.text_dim));
            }
        }
    });
}

/// Words or a number under the gauge's name; a number low enough to stand
/// out in its colour.
fn draw_words(ui: &mut Ui, name: &str, said: String, shade: &Shade, number: bool) {
    let theme = theme::current();
    let (small, large) = (
        theme::font(theme::Weight::Medium, 11.5),
        theme::font(theme::Weight::Medium, 13.5),
    );
    let width = measure(ui, name, &small)
        .max(measure(ui, &said, &large))
        .clamp(40.0, 260.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, format!("{name} {said}")));
    let painter = ui.painter().with_clip_rect(rect);
    painter.text(
        rect.left_top(),
        Align2::LEFT_TOP,
        name,
        small,
        theme.text_dim,
    );
    let figures = if number && shade.warned {
        shade.colour
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

/// A bar, or a row of cells, under the gauge's name and its numbers.
fn draw_bar(ui: &mut Ui, name: &str, value: f64, max: f64, shade: &Shade, cells: bool) {
    let theme = theme::current();
    let small = theme::font(theme::Weight::Medium, 11.5);
    let numbers = format!("{} / {}", plain(value), plain(max));
    let (rect, response) = ui.allocate_exact_size(vec2(158.0, 31.0), Sense::hover());
    let described = format!("{name} {numbers}");
    response.widget_info(|| WidgetInfo::labeled(Role::ProgressIndicator, true, described.as_str()));
    let painter = ui.painter();
    painter.text(
        rect.left_top(),
        Align2::LEFT_TOP,
        name,
        small.clone(),
        theme.text_dim,
    );
    let figures = if shade.warned {
        shade.level.unwrap_or(theme.danger)
    } else {
        theme.text
    };
    painter.text(rect.right_top(), Align2::RIGHT_TOP, numbers, small, figures);
    let empty = theme::mix(theme.surface, theme.text, 0.1);
    let track = Rect::from_min_max(rect.left_bottom() - vec2(0.0, 9.0), rect.right_bottom());
    // Under a warning it glows faintly in its colour, as it is heard.
    if shade.warned {
        painter.rect_filled(track.expand(3.0), 7.0, shade.colour.gamma_multiply(0.16));
    }
    if cells {
        draw_cells(painter, rect, shade, empty);
    } else {
        painter.rect_filled(track, 4.5, empty);
        let filled = Rect::from_min_size(
            track.min,
            vec2(track.width() * shade.fraction, track.height()),
        );
        painter.rect_filled(filled, 4.5, shade.colour);
    }
}

/// Leaning cells along the bottom, like the ▰ of a terminal's status line:
/// never all empty while something is left, nor full while short.
fn draw_cells(
    painter: &eframe::egui::Painter,
    rect: Rect,
    shade: &Shade,
    empty: eframe::egui::Color32,
) {
    const CELLS: usize = 10;
    let gap = 3.0;
    let width = (rect.width() - gap * (CELLS as f32 - 1.0)) / CELLS as f32;
    let filled = match (shade.fraction * CELLS as f32).round() as usize {
        0 if shade.fraction > 0.0 => 1,
        CELLS if shade.fraction < 1.0 => CELLS - 1,
        filled => filled,
    };
    let (top, bottom, lean) = (rect.bottom() - 9.0, rect.bottom(), 2.5);
    for cell in 0..CELLS {
        let left = rect.left() + cell as f32 * (width + gap);
        let points = vec![
            eframe::egui::pos2(left + lean, top),
            eframe::egui::pos2(left + width, top),
            eframe::egui::pos2(left + width - lean, bottom),
            eframe::egui::pos2(left, bottom),
        ];
        let fill = if cell < filled { shade.colour } else { empty };
        painter.add(eframe::egui::Shape::convex_polygon(
            points,
            fill,
            eframe::egui::Stroke::NONE,
        ));
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

/// The channel a `Comm.Channel.Text` payload was said on, if it says.
pub fn channel_name(json: &str) -> String {
    let named = serde_json::from_str::<Value>(json).ok();
    let name = named
        .as_ref()
        .and_then(|value| value.get("channel")?.as_str());
    name.unwrap_or_default().trim().to_owned()
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
    fn a_gauge_is_said_when_it_falls_under_a_warning_and_not_when_it_rises() {
        let mut gauges = Gauges::default();
        let look = GaugeLook {
            steps: vec![(60.0, "yellow".into()), (30.0, "red".into())],
            ..GaugeLook::default()
        };
        let mut set =
            |value: f64| gauges.set_by("Health", Some((value, 100.0)), Some(look.clone()), None);
        // As it starts is not news; above every warning, nothing either.
        assert_eq!(set(50.0), None);
        assert_eq!(set(80.0), None);
        assert_eq!(set(55.0).as_deref(), Some("Health 55 of 100"));
        assert_eq!(set(50.0), None);
        assert_eq!(set(25.0).as_deref(), Some("Health 25 of 100"));
        assert_eq!(set(45.0), None);
        assert_eq!(set(20.0).as_deref(), Some("Health 20 of 100"));
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
