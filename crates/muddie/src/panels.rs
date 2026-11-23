//! The status strip of gauges and the helpers that feed the panels from
//! GMCP.

use eframe::egui::{Align2, Color32, Rect, Role, Sense, Ui, WidgetInfo, vec2};
use muddie_core::Line;
use muddie_core::ansi::Assembler;
use muddie_script::{Belongs, GaugeKind, GaugeLook};

use crate::i18n::{tr, trf};
use crate::theme;

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
    /// one, each tenth more it falls.
    fn warning(&mut self, was: &str) -> Option<Warning> {
        if matches!(
            self.look.kind,
            GaugeKind::Light | GaugeKind::Text | GaugeKind::Effects
        ) {
            let now = self.said();
            return (now != was).then_some(Warning {
                said: now,
                urgent: false,
            });
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
        Some(Warning {
            said: self.said(),
            urgent: true,
        })
    }

    /// What it is, said aloud.
    fn said(&self) -> String {
        said_of(&self.name, self.value, self.max, &self.look)
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

/// What to say of a gauge that changed: `urgent` cuts short what is said.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    pub said: String,
    pub urgent: bool,
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
            .map(|warning| warning.said)
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
    ) -> Option<Warning> {
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
            .map(Gauge::said)
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
                let drawn = ui.scope(|ui| {
                    draw_gauge(ui, &gauge.name, gauge.value, gauge.max, &gauge.look, width);
                });
                if !editable(gauge) {
                    continue;
                }
                let id = ui.make_persistent_id(("edit gauge", &gauge.key));
                // Named by what it is, as it is reached with Tab; what
                // pressing it does, after.
                let label = gauge.said();
                let press = ui.interact(drawn.response.rect, id, Sense::click());
                press.widget_info(|| WidgetInfo::labeled(Role::Button, true, &label));
                let about = crate::i18n::trf("Press to edit {0}", &[&gauge.name]);
                (ui.ctx()).accesskit_node_builder(press.id, |node| node.set_description(about));
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

/// The room between gauges side by side.
const GAP: f32 = 22.0;
/// How wide a bar is drawn where nothing says otherwise, as in a preview.
pub const BAR_WIDTH: f32 = 158.0;

/// How wide each of `count` bars is in a panel `room` wide: in as many
/// columns as fit, none narrower than it reads well, and in one row no
/// wider than it looks well; in a narrow panel, one under another, its
/// whole width.
fn bar_width(room: f32, count: usize) -> f32 {
    const NARROWEST: f32 = 140.0;
    const WIDEST: f32 = 320.0;
    if count == 0 {
        return BAR_WIDTH;
    }
    let fit = ((room + GAP) / (NARROWEST + GAP)).floor().max(1.0) as usize;
    let columns = fit.min(count);
    let width = (room - GAP * (columns - 1) as f32) / columns as f32;
    match columns == count && columns > 1 {
        true => width.min(WIDEST),
        false => width.max(NARROWEST.min(room)),
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

/// Draws one gauge as its look says: a bar or a row of cells under its
/// name and numbers, a number alone, a light that is on or off, or a few
/// words.
pub fn draw_gauge(ui: &mut Ui, name: &str, value: f64, max: f64, look: &GaugeLook, bar: f32) {
    let shade = Shade::of(name, value, max, look);
    let heard = said_of(name, value, max, look);
    match look.kind {
        GaugeKind::Light => draw_light(ui, name, value != 0.0, &shade, look, &heard),
        GaugeKind::Effects => draw_effects(ui, name, &look.items, &shade),
        GaugeKind::Text => draw_words(ui, name, look.text.clone(), &shade, false, &heard),
        GaugeKind::Number => {
            let said = if max > 0.0 {
                format!("{} / {}", plain(value), plain(max))
            } else {
                plain(value)
            };
            draw_words(ui, name, said, &shade, true, &heard);
        }
        GaugeKind::Bar | GaugeKind::Cells => {
            let cells = look.kind == GaugeKind::Cells;
            draw_bar(ui, name, value, max, &shade, cells, bar, &heard);
        }
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
        let own = colour_named(&look.colour).unwrap_or_else(|| colour_for(name));
        let warned = level.is_some();
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
fn draw_light(ui: &mut Ui, name: &str, on: bool, shade: &Shade, look: &GaugeLook, heard: &str) {
    let theme = theme::current();
    let small = theme::font(theme::Weight::Medium, 11.5);
    let width = 18.0 + measure(ui, name, &small);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
    // What it says, not a box to tick: it is only shown.
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, heard));
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
    // Each with what more it says of itself, as it is seen hovering it.
    let names: Vec<String> = (items.iter())
        .map(|item| match item.about.trim() {
            "" => item.name.clone(),
            about => trf("{0} ({1})", &[&item.name, about]),
        })
        .collect();
    trf("{0}: {1}", &[name, &names.join(", ")])
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
    // How many fit in what is left of the row, or in the next if little is
    // left, leaving room for the one more if not all do.
    let room = match ui.available_width() {
        left if left >= 200.0 => left,
        _ => ui.max_rect().width(),
    };
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
fn draw_words(ui: &mut Ui, name: &str, said: String, shade: &Shade, number: bool, heard: &str) {
    let theme = theme::current();
    let (small, large) = (
        theme::font(theme::Weight::Medium, 11.5),
        theme::font(theme::Weight::Medium, 13.5),
    );
    let width = measure(ui, name, &small)
        .max(measure(ui, &said, &large))
        .clamp(40.0, 260.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, heard));
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
#[allow(clippy::too_many_arguments)]
fn draw_bar(
    ui: &mut Ui,
    name: &str,
    value: f64,
    max: f64,
    shade: &Shade,
    cells: bool,
    width: f32,
    heard: &str,
) {
    let theme = theme::current();
    let small = theme::font(theme::Weight::Medium, 11.5);
    let numbers = format!("{} / {}", plain(value), plain(max));
    let (rect, response) = ui.allocate_exact_size(vec2(width, 31.0), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::ProgressIndicator, true, heard));
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
fn colour_for(name: &str) -> Color32 {
    let theme = theme::current();
    match name.to_lowercase().as_str() {
        "hp" | "health" | "salud" | "vida" | "pv" => theme.terminal[1],
        "mp" | "mana" | "sp" | "maná" | "pm" => theme.terminal[4],
        "mv" | "moves" | "ep" | "stamina" | "resistencia" => theme.terminal[2],
        _ => theme.accent,
    }
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
mod tests {
    use super::*;

    /// Bars share a wide panel, no wider than looks well; in a narrow
    /// one, each takes its whole width, one under another.
    #[test]
    fn bars_share_the_width_of_their_panel() {
        // Three in a row at the bottom of a wide window: their widest.
        assert_eq!(bar_width(1460.0, 3), 320.0);
        // Three in a row that fits them just: the width shared.
        assert_eq!(bar_width(600.0, 3), (600.0 - 2.0 * GAP) / 3.0);
        // Beside the output, narrow: one under another, the whole width.
        assert_eq!(bar_width(300.0, 3), 300.0);
        // In between: two columns.
        assert_eq!(bar_width(400.0, 3), (400.0 - GAP) / 2.0);
        assert_eq!(bar_width(500.0, 0), BAR_WIDTH);
    }

    #[test]
    fn colours_are_named_as_scripts_name_them() {
        let theme = theme::current();
        assert_eq!(colour_named("Bright Red"), Some(theme.terminal[9]));
        assert_eq!(colour_named("green"), Some(theme.terminal[2]));
        assert_eq!(colour_named("#102030"), Some(Color32::from_rgb(16, 32, 48)));
        assert!(colour_named("purple").is_none() && colour_named("").is_none());
    }

    #[test]
    fn a_gauge_is_said_when_it_falls_under_a_warning_and_not_when_it_rises() {
        let mut gauges = Gauges::default();
        let look = GaugeLook {
            steps: vec![(60.0, "yellow".into()), (30.0, "red".into())],
            ..GaugeLook::default()
        };
        let mut set = |value: f64| {
            let look = Some(look.clone());
            (gauges.set_by(
                "Health",
                Some((value, 100.0)),
                look,
                None,
                Belongs::default(),
                String::new(),
            ))
            .map(|warning| warning.said)
        };
        // As it starts is not news; above every warning, nothing either.
        assert_eq!(set(50.0), None);
        assert_eq!(set(80.0), None);
        assert_eq!(set(55.0).as_deref(), Some("Health 55 of 100"));
        assert_eq!(set(50.0), None);
        assert_eq!(set(25.0).as_deref(), Some("Health 25 of 100"));
        assert_eq!(set(45.0), None);
        assert_eq!(set(20.0).as_deref(), Some("Health 20 of 100"));
        // Under the lowest, each tenth more it falls is said.
        assert_eq!(set(15.0), None);
        assert_eq!(set(9.0).as_deref(), Some("Health 9 of 100"));
        // Wavering about a warning, it is said once, until clear of it.
        assert_eq!(set(31.0), None);
        assert_eq!(set(29.0), None);
        assert_eq!(set(31.0), None);
        assert_eq!(set(40.0), None);
        assert_eq!(set(29.0).as_deref(), Some("Health 29 of 100"));
    }

    /// A light, words or effects are said when what they say changes.
    #[test]
    fn a_light_words_and_effects_are_said_when_they_change() {
        let mut gauges = Gauges::default();
        let light = GaugeLook {
            kind: GaugeKind::Light,
            ..GaugeLook::default()
        };
        let mut set = |value: f64| {
            (gauges.set_by(
                "Poisoned",
                Some((value, 1.0)),
                Some(light.clone()),
                None,
                Belongs::default(),
                String::new(),
            ))
            .map(|warning| (warning.said, warning.urgent))
        };
        assert_eq!(set(0.0), None);
        assert_eq!(set(1.0), Some(("Poisoned on".to_owned(), false)));
        assert_eq!(set(1.0), None);
        assert_eq!(set(0.0), Some(("Poisoned off".to_owned(), false)));
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
}
