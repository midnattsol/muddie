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
    if style.faint {
        fg = fg.gamma_multiply(0.6);
    }
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

/// The format of lines the client adds itself (echoed input, notes).
pub fn local_format() -> TextFormat {
    TextFormat {
        font_id: font(),
        color: theme::current().text_dim,
        ..TextFormat::default()
    }
}
