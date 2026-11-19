//! Maps the core's terminal styles to egui text formats.

use eframe::egui::{Color32, FontId, Stroke, TextFormat};
use muddie_core::{Color, Style};

pub const BACKGROUND: Color32 = Color32::from_rgb(0x16, 0x18, 0x1d);
pub const FOREGROUND: Color32 = Color32::from_rgb(0xd4, 0xd7, 0xdd);
/// Colour of lines the client adds itself (echoed input, status notes).
pub const LOCAL: Color32 = Color32::from_rgb(0x8a, 0x93, 0xa6);

const FONT_SIZE: f32 = 15.0;

/// The 16 base colours, tuned to stay readable on [`BACKGROUND`].
const BASE: [[u8; 3]; 16] = [
    [0x2b, 0x2f, 0x37],
    [0xe0, 0x6c, 0x75],
    [0x98, 0xc3, 0x79],
    [0xe5, 0xc0, 0x7b],
    [0x61, 0xaf, 0xef],
    [0xc6, 0x78, 0xdd],
    [0x56, 0xb6, 0xc2],
    [0xc8, 0xcc, 0xd4],
    [0x6b, 0x72, 0x80],
    [0xff, 0x7b, 0x86],
    [0xb5, 0xe8, 0x90],
    [0xff, 0xd8, 0x86],
    [0x7c, 0xc4, 0xff],
    [0xe0, 0x9c, 0xf5],
    [0x7f, 0xdb, 0xe6],
    [0xff, 0xff, 0xff],
];

pub fn font() -> FontId {
    FontId::monospace(FONT_SIZE)
}

/// Resolves an xterm 256-colour palette index.
fn indexed(index: u8) -> Color32 {
    match index {
        0..16 => {
            let [r, g, b] = BASE[index as usize];
            Color32::from_rgb(r, g, b)
        }
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
    let mut fg = resolve(style.fg, FOREGROUND, style.bold);
    let mut bg = resolve(style.bg, Color32::TRANSPARENT, false);
    if style.inverse {
        let old_fg = fg;
        fg = if bg == Color32::TRANSPARENT {
            BACKGROUND
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

pub fn local_format() -> TextFormat {
    TextFormat {
        font_id: font(),
        color: LOCAL,
        ..TextFormat::default()
    }
}
