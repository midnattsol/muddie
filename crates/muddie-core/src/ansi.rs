//! Turns decoded text with ANSI escape sequences into styled [`Line`]s.

use crate::text::{Color, Line, Style};

/// Longest CSI parameter string kept; a longer one is malformed and dropped.
const MAX_PARAMS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Esc,
    /// `ESC` plus an intermediate byte: skip up to the final byte.
    EscIntermediate,
    Csi,
    /// Operating system command: skipped up to BEL or `ESC \`.
    Osc,
    OscEsc,
}

/// Assembles lines from text arriving in arbitrary chunks. Style carries over
/// line breaks, as it does in a terminal.
#[derive(Debug)]
pub struct Assembler {
    state: State,
    params: String,
    style: Style,
    line: Line,
}

impl Default for Assembler {
    fn default() -> Self {
        Self {
            state: State::Ground,
            params: String::new(),
            style: Style::default(),
            line: Line::default(),
        }
    }
}

impl Assembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Consumes `text`, appending each line completed by a newline to `out`.
    pub fn feed(&mut self, text: &str, out: &mut Vec<Line>) {
        for ch in text.chars() {
            match self.state {
                State::Ground => match ch {
                    '\n' => out.push(std::mem::take(&mut self.line)),
                    '\x1b' => self.state = State::Esc,
                    '\t' => self.line.push(ch, self.style),
                    _ if ch.is_control() => {}
                    _ => self.line.push(ch, self.style),
                },
                State::Esc => {
                    self.state = match ch {
                        '[' => {
                            self.params.clear();
                            State::Csi
                        }
                        ']' => State::Osc,
                        ' '..='/' => State::EscIntermediate,
                        _ => State::Ground,
                    }
                }
                State::EscIntermediate => {
                    if !(' '..='/').contains(&ch) {
                        self.state = State::Ground;
                    }
                }
                State::Csi => match ch {
                    '\x1b' => self.state = State::Esc,
                    ' '..='?' => {
                        if self.params.len() < MAX_PARAMS {
                            self.params.push(ch);
                        }
                    }
                    _ => {
                        if ch == 'm' {
                            apply_sgr(&mut self.style, &self.params);
                        }
                        self.state = State::Ground;
                    }
                },
                State::Osc => match ch {
                    '\x07' => self.state = State::Ground,
                    '\x1b' => self.state = State::OscEsc,
                    _ => {}
                },
                State::OscEsc => {
                    self.state = if ch == '\\' {
                        State::Ground
                    } else {
                        State::Osc
                    };
                }
            }
        }
    }

    /// The text received since the last newline (typically a prompt).
    pub fn partial(&self) -> &Line {
        &self.line
    }

    /// Takes the unterminated text as a line of its own, if there is any.
    pub fn take_partial(&mut self) -> Option<Line> {
        if self.line.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.line))
        }
    }
}

fn apply_sgr(style: &mut Style, params: &str) {
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
fn extended_color(args: &[u16]) -> Option<(Color, usize)> {
    let byte = |v: u16| u8::try_from(v).ok();
    match *args {
        [5, n, ..] => Some((Color::Indexed(byte(n)?), 2)),
        [2, r, g, b, ..] => Some((Color::Rgb(byte(r)?, byte(g)?, byte(b)?), 4)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::Span;

    fn lines(chunks: &[&str]) -> (Vec<Line>, Assembler) {
        let mut assembler = Assembler::new();
        let mut out = Vec::new();
        for chunk in chunks {
            assembler.feed(chunk, &mut out);
        }
        (out, assembler)
    }

    fn span(text: &str, style: Style) -> Span {
        Span {
            text: text.into(),
            style,
        }
    }

    #[test]
    fn splits_lines_and_keeps_partial() {
        let (out, assembler) = lines(&["one\r\ntwo\r\nHP: 10> "]);
        let texts: Vec<String> = out.iter().map(Line::text).collect();
        assert_eq!(texts, ["one", "two"]);
        assert_eq!(assembler.partial().text(), "HP: 10> ");
    }

    #[test]
    fn colours_make_spans_and_carry_across_lines() {
        let red = Style {
            fg: Color::Indexed(1),
            ..Style::default()
        };
        let (out, _) = lines(&["a\x1b[31mb\nc\x1b[0md\n"]);
        assert_eq!(out[0].spans, [span("a", Style::default()), span("b", red)]);
        assert_eq!(out[1].spans, [span("c", red), span("d", Style::default())]);
    }

    #[test]
    fn extended_and_bright_colours() {
        let (out, _) = lines(&["\x1b[1;38;5;208;48;2;1;2;3mx\x1b[22;39;49;94my\n"]);
        assert_eq!(
            out[0].spans,
            [
                span(
                    "x",
                    Style {
                        bold: true,
                        fg: Color::Indexed(208),
                        bg: Color::Rgb(1, 2, 3),
                        ..Style::default()
                    }
                ),
                span(
                    "y",
                    Style {
                        fg: Color::Indexed(12),
                        ..Style::default()
                    }
                ),
            ]
        );
    }

    #[test]
    fn escape_split_at_every_position() {
        let whole = "a\x1b[1;32mb\x1b[mc\n";
        let (expected, _) = lines(&[whole]);
        assert_eq!(expected[0].text(), "abc");
        for cut in 1..whole.len() {
            let (a, b) = whole.split_at(cut);
            assert_eq!(lines(&[a, b]).0, expected, "cut at {cut}");
        }
    }

    #[test]
    fn non_sgr_sequences_are_dropped() {
        let (out, _) = lines(&["a\x1b[2J\x1b[?25l\x1b]0;title\x07b\x1b(Bc\x07\x1b]8;;\x1b\\d\n"]);
        assert_eq!(out[0].spans, [span("abcd", Style::default())]);
    }

    #[test]
    fn take_partial_empties_the_line() {
        let (_, mut assembler) = lines(&["prompt> "]);
        assert_eq!(assembler.take_partial().unwrap().text(), "prompt> ");
        assert_eq!(assembler.take_partial(), None);
    }
}
