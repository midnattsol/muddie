//! The status strip of gauges and the helpers that feed the panels from
//! GMCP.

use eframe::egui::{Align2, Color32, Id, Rect, Role, Sense, Ui, WidgetInfo, vec2};
use muddie_core::Line;
use muddie_core::ansi::Assembler;
use muddie_script::{Aloud, Belongs, GaugeKind, GaugeLook, How, Saying};

use crate::i18n::{tr, trf};
use crate::theme;

mod drawing;

pub use drawing::{BAR_WIDTH, draw_gauge};
use drawing::{GAP, bar_width, effects_named, effects_said};

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
    /// Whether it is drawn as a bar, of one piece or of cells.
    fn is_bar(&self) -> bool {
        matches!(self.look.kind, GaugeKind::Bar | GaugeKind::Cells)
    }

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
                // As it starts is not news.
                gauge.under = gauge.under();
                gauge.said_at = (gauge.under.is_some() && gauge.max > 0.0)
                    .then(|| (gauge.value / gauge.max).clamp(0.0, 1.0) * 100.0);
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
        // The bars share the width of the panel, in as many columns as fit.
        let bars = chosen.iter().filter(|gauge| gauge.is_bar()).count();
        let width = bar_width(ui.available_width(), bars);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(GAP, 8.0);
            for gauge in chosen {
                let id = ui.make_persistent_id(("edit gauge", &gauge.key));
                // What presses it lies under what the gauge has to press of
                // its own, as its "+3": made before it, where it was drawn
                // the frame before.
                let editable = editable(gauge);
                let last = ui.data(|data| data.get_temp::<eframe::egui::Rect>(id));
                let press = last
                    .filter(|_| editable)
                    .map(|rect| ui.interact(rect, id, Sense::click()));
                let drawn = ui.scope(|ui| {
                    draw_gauge(ui, &gauge.name, gauge.value, gauge.max, &gauge.look, width)
                });
                if editable && last != Some(drawn.response.rect) {
                    ui.data_mut(|data| data.insert_temp(id, drawn.response.rect));
                    ui.ctx().request_repaint();
                }
                let Some(press) = press else {
                    continue;
                };
                // Named by what it is, as it is reached with Tab; what
                // pressing it does, after.
                let label = gauge.said();
                press.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));
                let about = crate::i18n::trf("Press to edit {0}", &[&gauge.name]);
                (ui.ctx()).accesskit_node_builder(press.id, |node| node.set_description(about));
                // What presses it tells it: not its drawing too.
                if let Some(told) = drawn.inner {
                    (ui.ctx()).accesskit_node_builder(told, |node| node.set_hidden());
                }
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
                    pressed = Some(gauge.key.clone());
                }
            }
        });
        (true, pressed)
    }
}

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
