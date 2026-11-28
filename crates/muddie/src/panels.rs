//! The status strip of gauges and the helpers that feed the panels from
//! GMCP.

use eframe::egui::{Align2, Color32, Rect, Response, Role, Sense, Ui, WidgetInfo, vec2};
use muddie_core::Line;
use muddie_core::ansi::Assembler;
use muddie_script::{Aloud, Belongs, GaugeKind, GaugeLook, How, Saying};

use crate::i18n::{tr, trf};
use crate::theme;

mod arrange;
mod drawing;

use arrange::{Want, arrange};
pub use drawing::{BAR_WIDTH, draw_gauge};
use drawing::{GAP, effects_named, effects_said, want};

/// A gauge shown: what a script or a rule set it to, and how it looks.
pub struct Gauge {
    /// What it is told apart by: its place, if it has one, else its name.
    pub key: String,
    pub name: String,
    /// What it is switched by.
    pub belongs: Belongs,
    /// The panel of gauges it is shown in, by its name; empty for the
    /// first of them.
    pub panel: String,
    value: f64,
    max: f64,
    look: GaugeLook,
    /// The lowest of its warnings it was said to be under, as a
    /// percentage: said again only once it has risen clear of it.
    under: Option<f64>,
    /// Where it was when last said, while under a warning: each tenth
    /// more it falls is said too.
    said_at: Option<f64>,
    /// The script that set it last, or the rules; none for those the
    /// client makes of the MUD's vitals.
    owner: Option<String>,
    /// Whether its owner is being run again and has not set it since:
    /// it goes if it does not (see [`Gauges::drop_unset`]).
    unset: bool,
}

impl Gauge {
    /// The lowest of its warnings it is under, as a percentage: the
    /// steps where it changes colour.
    fn under(&self) -> Option<f64> {
        if self.max <= 0.0 {
            return None;
        }
        let percent = (self.value / self.max).clamp(0.0, 1.0) * 100.0;
        (self.look.steps.iter())
            .map(|(under, _)| *under)
            .filter(|under| percent < *under)
            .min_by(f64::total_cmp)
    }

    /// Takes it as it starts, which is not news.
    fn starts(&mut self) {
        self.under = self.under();
        self.said_at = (self.under.is_some() && self.max > 0.0)
            .then(|| (self.value / self.max).clamp(0.0, 1.0) * 100.0);
    }

    /// What to say of it now that it changed from `was`, as what is seen
    /// at a glance is heard. A light, words or effects: what they are
    /// now, if that changed. A level: falling under one of its warnings,
    /// as it turns colour, said again only once it has risen clear of it,
    /// so that one that wavers there is not said each time; and under
    /// one, each tenth more it falls. It cuts short what is being said as
    /// the gauge is told to be heard.
    fn warning(&mut self, was: &str) -> Option<Saying> {
        let news = self.news(was)?;
        let lowest = || {
            let steps = self.look.steps.iter().map(|(under, _)| *under);
            self.under() == steps.min_by(f64::total_cmp)
        };
        let cut = match self.look.aloud.how {
            How::Usual => self.look.kind.has_level(),
            How::Never => return None,
            How::Wait => false,
            How::Lowest => self.look.kind.has_level() && lowest(),
            How::Cut => true,
        };
        let aloud = Aloud {
            how: if cut { How::Cut } else { How::Wait },
            ..self.look.aloud
        };
        Some(Saying { text: news, aloud })
    }

    /// What there is to say of it now that it changed from `was`.
    fn news(&mut self, was: &str) -> Option<String> {
        if matches!(
            self.look.kind,
            GaugeKind::Light | GaugeKind::Text | GaugeKind::Effects
        ) {
            let now = self.said();
            return (now != was).then_some(now);
        }
        if self.max <= 0.0 {
            return None;
        }
        /// How far over a warning it must rise to be clear of it.
        const CLEAR: f64 = 5.0;
        let percent = (self.value / self.max).clamp(0.0, 1.0) * 100.0;
        let now = self.under();
        // Risen clear: of the warning said, and of every other.
        if let Some(under) = self.under
            && percent >= under + CLEAR
        {
            self.under = now;
            self.said_at = None;
        }
        let lower = match (self.under, now) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(before), Some(now)) => now < before,
        };
        let further = (self.said_at).is_some_and(|at| now.is_some() && percent <= at - 10.0);
        if !(lower || further) {
            return None;
        }
        if lower {
            self.under = now;
        }
        self.said_at = Some(percent);
        Some(self.said())
    }

    /// What it is, said aloud.
    fn said(&self) -> String {
        said_of(&self.name, self.value, self.max, &self.look)
    }

    /// The same, among the others: of effects, their names alone.
    fn said_shortly(&self) -> String {
        match self.look.kind {
            GaugeKind::Effects => effects_named(&self.name, &self.look.items),
            _ => self.said(),
        }
    }
}

/// What a gauge is, said aloud: when asked for, and as a screen reader
/// reads it where it is shown.
fn said_of(name: &str, value: f64, max: f64, look: &GaugeLook) -> String {
    match look.kind {
        GaugeKind::Light if value != 0.0 => trf("{0} on", &[name]),
        GaugeKind::Light => trf("{0} off", &[name]),
        GaugeKind::Text => trf("{0}: {1}", &[name, &look.text]),
        GaugeKind::Effects => effects_said(name, &look.items),
        GaugeKind::Number if max <= 0.0 => trf("{0}: {1}", &[name, &plain(value)]),
        _ => trf("{0} {1} of {2}", &[name, &plain(value), &plain(max)]),
    }
}

/// Named bars such as health and mana, in the order they were first set.
#[derive(Default)]
pub struct Gauges(Vec<Gauge>);

impl Gauges {
    #[cfg(test)]
    /// Sets the named gauge to `(value, max)`, or removes it. Returns what
    /// to say when it falls under one of its warnings, as when it turns
    /// to a warning colour: what is seen at a glance is heard.
    pub fn set(&mut self, name: &str, level: Option<(f64, f64)>) -> Option<String> {
        (self.set_by(name, level, None, None, Belongs::default(), String::new()))
            .map(|said| said.text)
    }

    /// The same, also saying how the gauge is drawn, and set by `owner`, a
    /// script or the rules, with what it is switched by and where it is
    /// shown. With no look given a gauge keeps the one it has, and with no
    /// owner the owner it has.
    pub fn set_by(
        &mut self,
        name: &str,
        level: Option<(f64, f64)>,
        look: Option<GaugeLook>,
        owner: Option<&str>,
        belongs: Belongs,
        panel: String,
    ) -> Option<Saying> {
        let key = belongs.place.clone().unwrap_or_else(|| name.to_owned());
        let index = self.0.iter().position(|gauge| gauge.key == key);
        let was;
        let index = match (index, level) {
            (Some(index), Some((value, max))) => {
                let gauge = &mut self.0[index];
                was = gauge.said();
                (gauge.value, gauge.max) = (value, max);
                name.clone_into(&mut gauge.name);
                (gauge.belongs, gauge.panel) = (belongs, panel);
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
                    key,
                    name: name.to_owned(),
                    belongs,
                    panel,
                    value,
                    max,
                    look: look.unwrap_or_default(),
                    under: None,
                    said_at: None,
                    owner: owner.map(str::to_owned),
                    unset: false,
                };
                gauge.starts();
                self.0.push(gauge);
                return None;
            }
            (Some(index), None) => {
                self.0.remove(index);
                return None;
            }
            (None, None) => return None,
        };
        self.0[index].warning(&was)
    }

    #[cfg(test)]
    /// The gauge of this key: its place, or else its name.
    pub fn get(&self, key: &str) -> Option<&Gauge> {
        self.0.iter().find(|gauge| gauge.key == key)
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

    /// Puts the gauges in the order `place` gives each by its key, those
    /// it gives none after, as they were.
    pub fn arrange(&mut self, place: impl Fn(&str) -> Option<usize>) {
        let place = |gauge: &Gauge| place(&gauge.key).unwrap_or(usize::MAX);
        if !self.0.is_sorted_by_key(place) {
            self.0.sort_by_key(place);
        }
    }

    #[cfg(test)]
    /// All gauges as a sentence for speech, e.g. "HP 80 of 100, MP 5 of
    /// 50", each said as what it is.
    pub fn summary(&self) -> String {
        self.summary_of(|_| true)
    }

    /// The same, of those `shown` says yes to.
    pub fn summary_of(&self, shown: impl Fn(&Gauge) -> bool) -> String {
        let parts: Vec<String> = (self.0.iter())
            .filter(|gauge| shown(gauge))
            .map(Gauge::said_shortly)
            .collect();
        parts.join(", ")
    }

    /// Draws, in a row that wraps, the gauges `wanted` says yes to.
    /// `editable` says which can be pressed, to be changed: those made in
    /// the editor. Returns whether there were any, and the key of the one
    /// pressed.
    pub fn show(
        &self,
        ui: &mut Ui,
        wanted: impl Fn(&Gauge) -> bool,
        editable: impl Fn(&Gauge) -> bool,
    ) -> (bool, Option<String>) {
        let chosen: Vec<&Gauge> = self.0.iter().filter(|gauge| wanted(gauge)).collect();
        if chosen.is_empty() {
            return (false, None);
        }
        let mut pressed = None;
        // Where each goes, worked out first: in the order they have, in rows
        // as full as fit, the same at the same width.
        let shown: Vec<(&Gauge, Want)> = (chosen.into_iter())
            .filter_map(|gauge| {
                let wanted = want(ui, &gauge.name, gauge.value, gauge.max, &gauge.look)?;
                Some((gauge, wanted))
            })
            .collect();
        let wants: Vec<Want> = shown.iter().map(|(_, wanted)| *wanted).collect();
        let (places, height) = arrange(ui.available_width(), GAP, ROW_GAP, &wants);
        let origin = ui.cursor().min;
        for ((gauge, wanted), place) in shown.iter().zip(&places) {
            let rect = Rect::from_min_size(
                origin + vec2(place.x, place.y),
                vec2(place.width, wanted.height),
            );
            // Known by the same id whatever the row it is in.
            let id = ui.make_persistent_id(("edit gauge", &gauge.key));
            let builder = eframe::egui::UiBuilder::new().max_rect(rect);
            let editable = editable(gauge);
            let chosen = (ui
                .scope_builder(builder, |ui| show_one(ui, gauge, id, place.width, editable)))
            .inner;
            pressed = pressed.or(chosen);
        }
        let used = Rect::from_min_size(origin, vec2(ui.available_width(), height));
        ui.advance_cursor_after_rect(used);
        (true, pressed)
    }
}

/// Draws a gauge `width` wide; and, if it can be changed, what presses it,
/// which lies under what the gauge has to press of its own, as its "+3":
/// made before it, where it was drawn the frame before. Returns its key if
/// it was pressed.
fn show_one(
    ui: &mut Ui,
    gauge: &Gauge,
    id: eframe::egui::Id,
    width: f32,
    editable: bool,
) -> Option<String> {
    let last = ui.data(|data| data.get_temp::<Rect>(id));
    let press = last
        .filter(|_| editable)
        .map(|rect| ui.interact(rect, id, crate::widgets::press_sense(ui)));
    let drawn = draw_gauge(ui, &gauge.name, gauge.value, gauge.max, &gauge.look, width);
    let rect = drawn.as_ref().map(|drawn| drawn.rect);
    if editable && last != rect {
        ui.data_mut(|data| match rect {
            Some(rect) => drop(data.insert_temp(id, rect)),
            None => data.remove::<Rect>(id),
        });
        ui.ctx().request_repaint();
    }
    let (Some(press), Some(drawn)) = (press, drawn) else {
        return None;
    };
    // Named by what it is, as it is reached with Tab; what pressing it
    // does, after.
    let label = gauge.said();
    press.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));
    let about = crate::i18n::trf("Press to edit {0}", &[&gauge.name]);
    (ui.ctx()).accesskit_node_builder(press.id, |node| node.set_description(about));
    // What presses it tells it: not its drawing too.
    (ui.ctx()).accesskit_node_builder(drawn.id, |node| node.set_hidden());
    if press.hovered() {
        ui.painter().rect_stroke(
            drawn.rect.expand(4.0),
            6.0,
            eframe::egui::Stroke::new(1.0, theme::current().border),
            eframe::egui::StrokeKind::Outside,
        );
    }
    let clicked = press
        .on_hover_cursor(eframe::egui::CursorIcon::PointingHand)
        .clicked();
    clicked.then(|| gauge.key.clone())
}

/// The room between one row of gauges and the next.
const ROW_GAP: f32 = 8.0;

/// A number without the `.0` a whole one would otherwise have.
fn plain(number: f64) -> String {
    let written = match number.fract() == 0.0 && number.abs() < 1e15 {
        true => format!("{number:.0}"),
        false => format!("{number}"),
    };
    // With the decimal point of the language: a comma in Spanish.
    written.replace('.', tr("."))
}

/// The colours offered to choose from, as a gauge's or a match's, by name.
pub const COLOUR_CHOICES: [&str; 7] =
    ["red", "green", "yellow", "blue", "magenta", "cyan", "white"];

/// A colour given as text, read as scripts read one: the sixteen of the
/// palette in the theme's colours.
pub fn colour_named(name: &str) -> Option<Color32> {
    let theme = theme::current();
    Some(match muddie_script::colour_named(name)? {
        muddie_core::Color::Indexed(index) if index < 16 => theme.terminal[usize::from(index)],
        muddie_core::Color::Indexed(index) => {
            let (red, green, blue) = muddie_core::Color::xterm_rgb(index)?;
            Color32::from_rgb(red, green, blue)
        }
        muddie_core::Color::Rgb(red, green, blue) => Color32::from_rgb(red, green, blue),
        _ => return None,
    })
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
mod tests;
