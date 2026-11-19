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
    /// Parts of the line that can be clicked, when the server marked any.
    pub links: Vec<LineLink>,
    /// The frame the server sent the line to, if not among the rest.
    pub dest: Option<String>,
}

/// Where a link leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// A command to send to the server.
    Send(String),
    /// A web address to open.
    Url(String),
    /// Several commands to choose from, each with what it is called:
    /// the first is the one a click sends.
    Menu(Vec<(String, String)>),
    /// A command to put in the command line, to be finished there.
    Prompt(String),
}

/// A clickable part of a line: a byte range of [].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineLink {
    pub range: std::ops::Range<usize>,
    pub target: LinkTarget,
    /// What it is for, to say when pointed at; empty if not said.
    pub hint: String,
    /// The name the server can later take it away by; empty if none.
    pub name: String,
}

impl Line {
    /// The line without styling: what triggers match and speech reads.
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Changes the style of the text between two byte offsets of
    /// [`Self::text`], splitting spans where the range cuts through them.
    /// Offsets that fall inside a character are moved back to its start.
    pub fn restyle(&mut self, range: std::ops::Range<usize>, change: impl Fn(&mut Style)) {
        let mut spans = Vec::with_capacity(self.spans.len() + 2);
        let mut offset = 0;
        for span in self.spans.drain(..) {
            let end = offset + span.text.len();
            let floor = |at: usize| {
                let mut at = at.clamp(offset, end) - offset;
                while !span.text.is_char_boundary(at) {
                    at -= 1;
                }
                at
            };
            let (from, to) = (floor(range.start), floor(range.end));
            let mut styled = span.style;
            change(&mut styled);
            for (text, style) in [
                (&span.text[..from], span.style),
                (&span.text[from..to], styled),
                (&span.text[to..], span.style),
            ] {
                if !text.is_empty() {
                    spans.push(Span {
                        text: text.to_owned(),
                        style,
                    });
                }
            }
            offset = end;
        }
        self.spans = spans;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restyle_splits_spans_at_the_range() {
        let red = Style {
            fg: Color::Indexed(1),
            ..Style::default()
        };
        let mut line = Line::default();
        "An ".chars().for_each(|ch| line.push(ch, Style::default()));
        "orco ñu".chars().for_each(|ch| line.push(ch, red));
        // Byte 9 is in the middle of "ñ", so the range ends before it.
        line.restyle(3..9, |style| style.bold = true);
        let parts: Vec<(&str, bool, Color)> = (line.spans.iter())
            .map(|span| (span.text.as_str(), span.style.bold, span.style.fg))
            .collect();
        assert_eq!(
            parts,
            [
                ("An ", false, Color::Default),
                ("orco ", true, Color::Indexed(1)),
                ("ñu", false, Color::Indexed(1)),
            ]
        );
        assert_eq!(line.text(), "An orco ñu");
        line.restyle(100..200, |style| style.italic = true);
        assert_eq!(line.spans.len(), 3);
    }
}
