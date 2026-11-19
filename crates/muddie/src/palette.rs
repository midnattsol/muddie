//! Maps the core's terminal styles to egui text formats, in the colours of
//! the current theme.

use eframe::egui::{Color32, FontId, Stroke, TextFormat};
use muddie_core::{Color, Style};

use crate::theme;

pub fn font() -> FontId {
    theme::output_font()
}

/// Resolves an xterm 256-colour palette index.
fn indexed(index: u8) -> Color32 {
    match index {
        0..16 => theme::current().terminal[index as usize],
        16..232 => {
            let level = |n: u8| if n == 0 { 0 } else { 55 + 40 * n };
            let i = index - 16;
            Color32::from_rgb(level(i / 36), level(i / 6 % 6), level(i % 6))
        }
        232.. => Color32::from_gray(8 + 10 * (index - 232)),
    }
}

fn resolve(color: Color, default: Color32, brighten: bool) -> Color32 {
    match color {
        Color::Default => default,
        // Bold traditionally selects the bright variant of the base colours.
        Color::Indexed(i @ 0..8) if brighten => indexed(i + 8),
        Color::Indexed(i) => indexed(i),
        Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

pub fn format(style: Style) -> TextFormat {
    let theme = theme::current();
    let mut fg = resolve(style.fg, theme.text, style.bold);
    let mut bg = resolve(style.bg, Color32::TRANSPARENT, false);
    if style.inverse {
        let old_fg = fg;
        fg = if bg == Color32::TRANSPARENT {
            theme.background
        } else {
            bg
        };
        bg = old_fg;
    }
    // Servers choose colours for a black terminal, and some of them, a
    // deep blue above all, cannot be read on any dark background. Those
    // are lightened just enough, keeping their hue.
    let behind = if bg == Color32::TRANSPARENT {
        theme.background
    } else {
        bg
    };
    fg = legible(fg, behind, if style.faint { 3.0 } else { 4.5 });
    let line = Stroke::new(1.0, fg);
    TextFormat {
        font_id: font(),
        color: fg,
        background: bg,
        italics: style.italic,
        underline: if style.underline { line } else { Stroke::NONE },
        strikethrough: if style.strike { line } else { Stroke::NONE },
        ..TextFormat::default()
    }
}

/// How bright a colour looks, from 0 to 1, as contrast is measured by.
fn luminance(colour: Color32) -> f32 {
    let linear = |channel: u8| {
        let value = f32::from(channel) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(colour.r()) + 0.7152 * linear(colour.g()) + 0.0722 * linear(colour.b())
}

/// The contrast between two colours, from 1 (none) to 21.
fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// `colour`, moved away from `behind` towards white or black only as far
/// as it takes to stand out from it by `wanted`.
fn legible(colour: Color32, behind: Color32, wanted: f32) -> Color32 {
    if contrast(colour, behind) >= wanted {
        return colour;
    }
    let towards = if luminance(behind) < 0.4 {
        Color32::WHITE
    } else {
        Color32::BLACK
    };
    let steps = (1..=20).map(|step| theme::mix(colour, towards, step as f32 / 20.0));
    steps
        .into_iter()
        .find(|candidate| contrast(*candidate, behind) >= wanted)
        .unwrap_or(towards)
}

/// The format of lines the client adds itself (echoed input, notes).
pub fn local_format() -> TextFormat {
    TextFormat {
        font_id: font(),
        color: theme::current().text_dim,
        ..TextFormat::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_too_close_to_what_is_behind_them_are_made_legible() {
        let dark = Color32::from_rgb(0x11, 0x12, 0x1b);
        // The deep blue of the 256-colour palette: unreadable as sent.
        let blue = indexed(17);
        assert!(contrast(blue, dark) < 1.5);
        let shown = legible(blue, dark, 4.5);
        assert!(contrast(shown, dark) >= 4.5);
        // Still blue, and no lighter than it has to be.
        assert!(shown.b() > shown.r() && shown.b() > shown.g());
        assert!(contrast(shown, dark) < 6.0);
        // A colour that can be read is left exactly as the server sent it.
        let green = Color32::from_rgb(0x8f, 0xd6, 0x8a);
        assert_eq!(legible(green, dark, 4.5), green);
        // On a light background the move is towards black.
        let yellow = Color32::from_rgb(0xff, 0xff, 0x55);
        let shown = legible(yellow, Color32::WHITE, 4.5);
        assert!(contrast(shown, Color32::WHITE) >= 4.5 && shown.b() < 0x55);
        // Black on black, the classic: readable too.
        assert!(contrast(legible(Color32::BLACK, dark, 4.5), dark) >= 4.5);
    }
}
