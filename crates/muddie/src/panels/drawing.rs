//! Drawing a gauge as its look says: bars, cells, lights, words and effects.

use super::*;

/// The room between gauges side by side.
pub(super) const GAP: f32 = 22.0;
/// How wide a bar is drawn where nothing says otherwise, as in a preview.
pub const BAR_WIDTH: f32 = 158.0;

/// How wide each of `count` bars is in a panel `room` wide: in as many
/// columns as fit, none narrower than it reads well, and in one row no
/// wider than it looks well; in a narrow panel, one under another, its
/// whole width.
pub(super) fn bar_width(room: f32, count: usize) -> f32 {
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
pub(super) fn effects_said(name: &str, items: &[muddie_script::Effect]) -> String {
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
