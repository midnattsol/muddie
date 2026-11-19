//! Turns decoded text with ANSI escape sequences, and MXP markup when the
//! server uses it, into styled [`Line`]s.

use crate::text::{Color, Line, LineLink, LinkTarget, Style};

/// Longest CSI parameter string kept; a longer one is malformed and dropped.
const MAX_PARAMS: usize = 64;
/// Longest MXP tag kept; a longer `<...` is taken as ordinary text.
const MAX_TAG: usize = 1024;
/// Longest MXP entity name, as in `&name;`.
const MAX_ENTITY: usize = 10;

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
    /// Inside an MXP `<tag>`.
    Tag,
    /// Inside an MXP `&entity;`.
    Entity,
}

/// How much MXP markup is honoured on the current line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MxpMode {
    /// Formatting tags only: the text may come from other players.
    Open,
    /// Every supported tag, including links that send commands.
    Secure,
    /// No markup: `<` and `&` are ordinary characters.
    Locked,
}

/// Assembles lines from text arriving in arbitrary chunks. Style carries over
/// line breaks, as it does in a terminal.
#[derive(Debug)]
pub struct Assembler {
    state: State,
    params: String,
    style: Style,
    line: Line,
    /// Whether the server negotiated MXP.
    mxp: bool,
    mode: MxpMode,
    /// The mode lines start in.
    default_mode: MxpMode,
    /// The tag or entity being read.
    markup: String,
    /// Styles to go back to as formatting tags close.
    style_stack: Vec<Style>,
    /// The link being read: where its text starts in the line, and where
    /// it leads if the tag said (else its own text is the command).
    link: Option<(usize, Option<LinkTarget>)>,
    /// Answers to the server's MXP queries, for the caller to send.
    replies: Vec<String>,
    /// The MXP tags seen, kept only while someone is looking at them.
    keep_tags: bool,
    tags: Vec<String>,
}

impl Default for Assembler {
    fn default() -> Self {
        Self {
            state: State::Ground,
            params: String::new(),
            style: Style::default(),
            line: Line::default(),
            mxp: false,
            mode: MxpMode::Open,
            default_mode: MxpMode::Open,
            markup: String::new(),
            style_stack: Vec::new(),
            link: None,
            replies: Vec::new(),
            keep_tags: false,
            tags: Vec::new(),
        }
    }
}

impl Assembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts interpreting MXP markup, once the server has negotiated it.
    pub fn enable_mxp(&mut self) {
        self.mxp = true;
    }

    /// Takes the answers to MXP queries (`<VERSION>`, `<SUPPORT>`) seen so
    /// far, each a line to send to the server.
    pub fn take_replies(&mut self) -> Vec<String> {
        std::mem::take(&mut self.replies)
    }

    /// Starts or stops keeping the MXP tags that arrive, to be shown to
    /// whoever is debugging; they are not kept otherwise.
    pub fn keep_tags(&mut self, keep: bool) {
        self.keep_tags = keep;
        if !keep {
            self.tags = Vec::new();
        }
    }

    /// Takes the MXP tags kept since they were last taken.
    pub fn take_tags(&mut self) -> Vec<String> {
        std::mem::take(&mut self.tags)
    }

    /// Consumes `text`, appending each line completed by a newline to `out`.
    pub fn feed(&mut self, text: &str, out: &mut Vec<Line>) {
        for ch in text.chars() {
            match self.state {
                State::Ground => match ch {
                    '\n' => self.end_line(out),
                    '\x1b' => self.state = State::Esc,
                    '<' if self.mxp && self.mode != MxpMode::Locked => {
                        self.markup.clear();
                        self.state = State::Tag;
                    }
                    '&' if self.mxp && self.mode != MxpMode::Locked => {
                        self.markup.clear();
                        self.state = State::Entity;
                    }
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
                        match ch {
                            'm' => apply_sgr(&mut self.style, &self.params),
                            'z' if self.mxp => self.set_mxp_mode(),
                            _ => {}
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
                State::Tag => match ch {
                    // A `>` inside a quoted attribute is part of the tag.
                    '>' if !ends_in_quotes(&self.markup) => {
                        self.state = State::Ground;
                        let tag = std::mem::take(&mut self.markup);
                        self.tag(&tag, out);
                    }
                    // Not a tag after all: a lone `<` in ordinary text.
                    '\n' | '\x1b' => {
                        self.literal('<');
                        self.state = State::Ground;
                        self.feed(ch.encode_utf8(&mut [0; 4]), out);
                    }
                    _ if self.markup.len() >= MAX_TAG => {
                        self.literal('<');
                        self.state = State::Ground;
                    }
                    _ => self.markup.push(ch),
                },
                State::Entity => match ch {
                    ';' => {
                        self.state = State::Ground;
                        match entity(&self.markup) {
                            Some(decoded) => self.line.push(decoded, self.style),
                            None => {
                                self.literal('&');
                                self.line.push(';', self.style);
                            }
                        }
                        self.markup.clear();
                    }
                    _ if (ch.is_ascii_alphanumeric() || ch == '#')
                        && self.markup.len() < MAX_ENTITY =>
                    {
                        self.markup.push(ch);
                    }
                    // Not an entity after all: a lone `&` in ordinary text.
                    _ => {
                        self.literal('&');
                        self.state = State::Ground;
                        self.feed(ch.encode_utf8(&mut [0; 4]), out);
                    }
                },
            }
        }
    }

    /// Puts back what looked like the start of markup, as ordinary text.
    fn literal(&mut self, opener: char) {
        self.line.push(opener, self.style);
        for ch in std::mem::take(&mut self.markup).chars() {
            self.line.push(ch, self.style);
        }
    }

    fn end_line(&mut self, out: &mut Vec<Line>) {
        self.close_link();
        out.push(std::mem::take(&mut self.line));
        if self.mxp {
            // Tags do not outlive their line, nor does a one-line mode.
            if let Some(base) = self.style_stack.first() {
                self.style = *base;
            }
            self.style_stack.clear();
            self.mode = self.default_mode;
        }
    }

    /// Applies an MXP line mode, `ESC [ n z`.
    fn set_mxp_mode(&mut self) {
        let mode = match self.params.parse::<u8>().unwrap_or(0) {
            0 => MxpMode::Open,
            1 | 4 => MxpMode::Secure,
            2 => MxpMode::Locked,
            5 => MxpMode::Open,
            6 => MxpMode::Secure,
            7 => MxpMode::Locked,
            // 3 resets: close everything and go back to the default.
            _ => {
                self.close_link();
                if let Some(base) = self.style_stack.first() {
                    self.style = *base;
                }
                self.style_stack.clear();
                self.default_mode = MxpMode::Open;
                MxpMode::Open
            }
        };
        if matches!(self.params.as_str(), "5" | "6" | "7") {
            self.default_mode = mode;
        }
        self.mode = mode;
    }

    /// Carries out an MXP tag, given without its angle brackets.
    fn tag(&mut self, tag: &str, out: &mut Vec<Line>) {
        // No more than a burst of them, if nobody comes to take them.
        if self.keep_tags && self.tags.len() < 256 {
            self.tags.push(tag.to_owned());
        }
        // Definitions and comments (`<!ELEMENT ...>`) change nothing here.
        if tag.starts_with('!') {
            return;
        }
        let (name, attributes) = parse_tag(tag);
        let secure = self.mode == MxpMode::Secure;
        if let Some(closing) = name.strip_prefix('/') {
            match closing {
                "send" | "a" => self.close_link(),
                _ if is_formatting(closing) => {
                    if let Some(style) = self.style_stack.pop() {
                        self.style = style;
                    }
                }
                _ => {}
            }
            return;
        }
        let attribute = |key: &str, position: usize| {
            let named = attributes
                .iter()
                .find(|(name, _)| name.as_deref() == Some(key));
            let positional = attributes
                .iter()
                .filter(|(name, _)| name.is_none())
                .nth(position);
            named.or(positional).map(|(_, value)| value.as_str())
        };
        match name.as_str() {
            "b" | "bold" | "strong" | "h" | "high" => self.format(|style| style.bold = true),
            "i" | "italic" | "em" => self.format(|style| style.italic = true),
            "u" | "underline" => self.format(|style| style.underline = true),
            "s" | "strikeout" => self.format(|style| style.strike = true),
            "c" | "color" | "font" => {
                let fore = attribute(if name == "font" { "color" } else { "fore" }, 0)
                    .and_then(named_color);
                let back = attribute("back", 1).and_then(named_color);
                self.format(|style| {
                    style.fg = fore.unwrap_or(style.fg);
                    style.bg = back.unwrap_or(style.bg);
                });
            }
            "br" => self.end_line(out),
            "send" if secure => {
                // Several commands may be offered as a menu; the first is
                // the one a click sends.
                let command =
                    attribute("href", 0).map(|href| href.split('|').next().unwrap_or(href));
                self.open_link(command.map(|command| LinkTarget::Send(command.to_owned())));
            }
            "a" if secure => {
                let target = attribute("href", 0).map(|href| LinkTarget::Url(href.to_owned()));
                if target.is_some() {
                    self.open_link(target);
                }
            }
            "version" if secure => {
                let version = env!("CARGO_PKG_VERSION");
                self.replies.push(format!(
                    "\x1b[1z<VERSION MXP=\"1.0\" CLIENT=muddie VERSION=\"{version}\">"
                ));
            }
            "support" if secure => {
                let supported = "+b +i +u +s +h +c +color +font +br +send +a +version +support";
                self.replies.push(format!("\x1b[1z<SUPPORTS {supported}>"));
            }
            _ => {}
        }
    }

    /// Changes the style for the text up to the matching closing tag.
    fn format(&mut self, change: impl FnOnce(&mut Style)) {
        self.style_stack.push(self.style);
        change(&mut self.style);
    }

    fn open_link(&mut self, target: Option<LinkTarget>) {
        self.close_link();
        self.link = Some((self.line.text().len(), target));
    }

    fn close_link(&mut self) {
        let Some((start, target)) = self.link.take() else {
            return;
        };
        let text = self.line.text();
        if start >= text.len() {
            return;
        }
        // A link with no target sends its own text.
        let target = target.unwrap_or_else(|| LinkTarget::Send(text[start..].to_owned()));
        self.line.links.push(LineLink {
            range: start..text.len(),
            target,
        });
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
            self.close_link();
            Some(std::mem::take(&mut self.line))
        }
    }
}

/// Whether `text` ends inside a quoted string.
fn ends_in_quotes(text: &str) -> bool {
    let mut quote = None;
    for ch in text.chars() {
        match quote {
            Some(open) if ch == open => quote = None,
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            _ => {}
        }
    }
    quote.is_some()
}

fn is_formatting(name: &str) -> bool {
    matches!(
        name,
        "b" | "bold"
            | "strong"
            | "h"
            | "high"
            | "i"
            | "italic"
            | "em"
            | "u"
            | "underline"
            | "s"
            | "strikeout"
            | "c"
            | "color"
            | "font"
    )
}

/// Splits a tag into its lower-cased name and its attributes, each with
/// its lower-cased name if it has one (`href="look"`) or without (`red`).
fn parse_tag(tag: &str) -> (String, Vec<(Option<String>, String)>) {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    for ch in tag.chars() {
        match quote {
            Some(open) if ch == open => quote = None,
            Some(_) => word.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            None => word.push(ch),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    let mut words = words.into_iter();
    let name = words.next().unwrap_or_default().to_lowercase();
    let attributes = words
        .map(|word| match word.split_once('=') {
            Some((key, value)) => (Some(key.to_lowercase()), value.to_owned()),
            None => (None, word),
        })
        .collect();
    (name, attributes)
}

/// The character an MXP entity name stands for, as in `&lt;` or `&#233;`.
fn entity(name: &str) -> Option<char> {
    if let Some(code) = name.strip_prefix('#') {
        return char::from_u32(code.parse().ok()?).filter(|ch| !ch.is_control());
    }
    Some(match name {
        "lt" => '<',
        "gt" => '>',
        "amp" => '&',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        _ => return None,
    })
}

/// A colour given by name or as `#rrggbb` in an MXP tag.
fn named_color(name: &str) -> Option<Color> {
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

    fn mxp(chunks: &[&str]) -> (Vec<Line>, Assembler) {
        let mut assembler = Assembler::new();
        assembler.enable_mxp();
        let mut out = Vec::new();
        for chunk in chunks {
            assembler.feed(chunk, &mut out);
        }
        (out, assembler)
    }

    #[test]
    fn markup_is_ordinary_text_unless_mxp_is_on() {
        let (out, _) = lines(&["<b>5 &lt; 6</b>\n"]);
        assert_eq!(out[0].text(), "<b>5 &lt; 6</b>");
    }

    #[test]
    fn mxp_formatting_tags_and_entities() {
        let (out, _) = mxp(&[
            "a <b>bold <c fore=red back=\"#102030\">red</c> still</B> plain &lt;&amp;&#233;&gt;\n",
        ]);
        let bold = Style {
            bold: true,
            ..Style::default()
        };
        let red = Style {
            fg: Color::Indexed(9),
            bg: Color::Rgb(0x10, 0x20, 0x30),
            ..bold
        };
        assert_eq!(
            out[0].spans,
            [
                span("a ", Style::default()),
                span("bold ", bold),
                span("red", red),
                span(" still", bold),
                span(" plain <&é>", Style::default()),
            ]
        );
    }

    #[test]
    fn mxp_tags_do_not_outlive_their_line() {
        let (out, _) = mxp(&["<b><i>loud\nquiet\n"]);
        assert!(out[0].spans[0].style.bold && out[0].spans[0].style.italic);
        assert_eq!(out[1].spans, [span("quiet", Style::default())]);
    }

    #[test]
    fn mxp_links_need_secure_mode() {
        let secure = "\x1b[1zGo <send href=\"north|look north\">north</send> or <send>look</send>, see <a \"http://x.io\">site</a>\n";
        let (out, _) = mxp(&[secure]);
        assert_eq!(out[0].text(), "Go north or look, see site");
        let link = |range, target| LineLink { range, target };
        assert_eq!(
            out[0].links,
            [
                link(3..8, LinkTarget::Send("north".into())),
                link(12..16, LinkTarget::Send("look".into())),
                link(22..26, LinkTarget::Url("http://x.io".into())),
            ]
        );
        // In open mode, where text may come from players, links are not made.
        let (out, _) = mxp(&["Go <send href=\"quit\">here</send>\n"]);
        assert_eq!(out[0].text(), "Go here");
        assert!(out[0].links.is_empty());
    }

    #[test]
    fn mxp_modes_lock_and_reset() {
        // Locked for one line, then back to the default.
        let (out, _) = mxp(&["\x1b[2z<b>&lt;\n<b>x</b>\n"]);
        assert_eq!(out[0].text(), "<b>&lt;");
        assert_eq!(out[1].text(), "x");
        // Locked until told otherwise.
        let (out, _) = mxp(&["\x1b[7z<b>\n<i>\n\x1b[6z<send>go</send>\n"]);
        let texts: Vec<String> = out.iter().map(Line::text).collect();
        assert_eq!(texts, ["<b>", "<i>", "go"]);
        assert_eq!(out[2].links.len(), 1);
    }

    #[test]
    fn mxp_queries_are_answered_and_unknown_tags_dropped() {
        let (out, mut assembler) =
            mxp(&["\x1b[1z<VERSION><support><!ELEMENT hp '<b>'><image x.png>ok\n"]);
        assert_eq!(out[0].text(), "ok");
        let replies = assembler.take_replies();
        assert!(replies[0].starts_with("\x1b[1z<VERSION MXP=\"1.0\" CLIENT=muddie"));
        assert!(replies[1].contains("+send"));
        assert!(assembler.take_replies().is_empty());
    }

    #[test]
    fn mxp_markup_split_at_every_position_and_lone_markers() {
        let whole = "\x1b[1zA <send href=\"n\">door</send> &amp; 3 < 4 & so\n";
        let (expected, _) = mxp(&[whole]);
        assert_eq!(expected[0].text(), "A door & 3 < 4 & so");
        assert_eq!(expected[0].links.len(), 1);
        for cut in 1..whole.len() {
            let (a, b) = whole.split_at(cut);
            assert_eq!(mxp(&[a, b]).0, expected, "cut at {cut}");
        }
    }

    #[test]
    fn partial_line_closes_its_link() {
        let (_, mut assembler) = mxp(&["\x1b[1z<send>look"]);
        let line = assembler.take_partial().unwrap();
        let expected = LineLink {
            range: 0..4,
            target: LinkTarget::Send("look".into()),
        };
        assert_eq!(line.links, [expected]);
    }
}
