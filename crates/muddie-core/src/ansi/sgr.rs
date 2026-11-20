//! The colours and styles of ANSI SGR sequences.

use super::*;

/// A colour given by name or as `#rrggbb` in an MXP tag.
pub(crate) fn named_color(name: &str) -> Option<Color> {
    if let Some(hex) = name.strip_prefix('#') {
        let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
        return match hex.len() {
            6 => Some(Color::Rgb(channel(0)?, channel(2)?, channel(4)?)),
            _ => None,
        };
    }
    Some(match name.to_lowercase().as_str() {
        "black" => Color::Indexed(0),
        "red" => Color::Indexed(9),
        "green" => Color::Indexed(2),
        "lime" => Color::Indexed(10),
        "yellow" => Color::Indexed(11),
        "blue" => Color::Indexed(12),
        "navy" => Color::Indexed(4),
        "magenta" | "fuchsia" => Color::Indexed(13),
        "purple" => Color::Indexed(5),
        "cyan" | "aqua" => Color::Indexed(14),
        "teal" => Color::Indexed(6),
        "white" => Color::Indexed(15),
        "silver" => Color::Indexed(7),
        "gray" | "grey" => Color::Indexed(8),
        "maroon" => Color::Indexed(1),
        "olive" => Color::Indexed(3),
        "orange" => Color::Rgb(0xff, 0xa5, 0x00),
        "brown" => Color::Rgb(0xa5, 0x2a, 0x2a),
        "pink" => Color::Rgb(0xff, 0xc0, 0xcb),
        "gold" => Color::Rgb(0xff, 0xd7, 0x00),
        _ => return None,
    })
}

pub(crate) fn apply_sgr(style: &mut Style, params: &str) {
    // Private or intermediate bytes mean this is not a plain SGR sequence.
    if params
        .bytes()
        .any(|b| !matches!(b, b'0'..=b'9' | b';' | b':'))
    {
        return;
    }
    let codes: Vec<u16> = params
        .split([';', ':'])
        .map(|p| p.parse().unwrap_or(0))
        .collect();
    let mut i = 0;
    while i < codes.len() {
        let code = codes[i];
        i += 1;
        match code {
            0 => *style = Style::default(),
            1 => style.bold = true,
            2 => style.faint = true,
            3 => style.italic = true,
            4 => style.underline = true,
            5 | 6 => style.blink = true,
            7 => style.inverse = true,
            9 => style.strike = true,
            22 => {
                style.bold = false;
                style.faint = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            25 => style.blink = false,
            27 => style.inverse = false,
            29 => style.strike = false,
            30..=37 => style.fg = Color::Indexed((code - 30) as u8),
            39 => style.fg = Color::Default,
            40..=47 => style.bg = Color::Indexed((code - 40) as u8),
            49 => style.bg = Color::Default,
            90..=97 => style.fg = Color::Indexed((code - 90 + 8) as u8),
            100..=107 => style.bg = Color::Indexed((code - 100 + 8) as u8),
            38 | 48 => {
                let Some((color, used)) = extended_color(&codes[i..]) else {
                    return;
                };
                i += used;
                if code == 38 {
                    style.fg = color;
                } else {
                    style.bg = color;
                }
            }
            _ => {}
        }
    }
}

/// Parses the arguments of SGR 38/48, returning the colour and how many
/// codes it used.
pub(crate) fn extended_color(args: &[u16]) -> Option<(Color, usize)> {
    let byte = |v: u16| u8::try_from(v).ok();
    match *args {
        [5, n, ..] => Some((Color::Indexed(byte(n)?), 2)),
        [2, r, g, b, ..] => Some((Color::Rgb(byte(r)?, byte(g)?, byte(b)?), 4)),
        _ => None,
    }
}
