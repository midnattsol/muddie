//! Styled text model shared by the core and every frontend.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    /// The frontend's default foreground or background.
    #[default]
    Default,
    /// xterm 256-colour palette index; 0-15 are the classic ANSI colours.
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub faint: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub inverse: bool,
    pub strike: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// One line of output as a run of uniformly styled spans.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    /// The line without styling: what triggers match and speech reads.
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    pub fn push(&mut self, ch: char, style: Style) {
        match self.spans.last_mut() {
            Some(span) if span.style == style => span.text.push(ch),
            _ => self.spans.push(Span {
                text: ch.to_string(),
                style,
            }),
        }
    }
}
