//! Turns decoded text with ANSI escape sequences, and MXP markup when the
//! server uses it, into styled [`Line`]s.

use std::collections::HashMap;

use crate::text::{Color, Line, LineLink, LinkTarget, Style};

/// Longest CSI parameter string kept; a longer one is malformed and dropped.
const MAX_PARAMS: usize = 64;
/// Longest MXP tag kept; a longer `<...` is taken as ordinary text.
const MAX_TAG: usize = 1024;
/// Longest MXP entity name, as in `&name;`.
const MAX_ENTITY: usize = 32;
/// How deep the elements a server defines may stand for one another.
const MAX_DEPTH: u8 = 8;
/// How many elements, and how many entities, a server may define.
const MAX_DEFINED: usize = 512;
/// The tags of MXP this client carries out, as it tells a server that
/// asks.
const SUPPORTED: [&str; 38] = [
    "b",
    "bold",
    "strong",
    "i",
    "italic",
    "em",
    "u",
    "underline",
    "s",
    "strikeout",
    "h",
    "high",
    "c",
    "color",
    "font",
    "nobr",
    "p",
    "br",
    "sbr",
    "hr",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "small",
    "tt",
    "a",
    "send",
    "expire",
    "version",
    "support",
    "var",
    "gauge",
    "stat",
    "frame",
    "dest",
];

/// Something an MXP tag asks of the client besides showing text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mxp {
    /// A value the server gave a name: with `<var>`, as an entity, or as
    /// what an element with a flag held, such as `RoomName`.
    Value {
        name: String,
        value: String,
    },
    /// A gauge, or with no bar a figure, that follows a value by name,
    /// out of another, with what to call it and its colour.
    Gauge {
        entity: String,
        max: String,
        caption: String,
        colour: String,
        bar: bool,
    },
    /// A frame to open beside the text, or to close.
    Frame {
        name: String,
        open: bool,
        title: String,
        align: String,
    },
    /// A frame to empty.
    Clear(String),
    /// A sound or music, asked for as the sound protocol would.
    Sound(String),
    /// The links of a name are no longer to be followed.
    Expire(String),
    /// The server asks to go on at another address.
    Relocate {
        host: String,
        port: String,
    },
    /// The server asks for the name of the character, or its password.
    User,
    Password,
}

/// An element the server defined: the tags it stands for, what it takes
/// with its defaults, and what it is flagged as.
#[derive(Debug, Clone, Default)]
struct Element {
    definition: String,
    attributes: Vec<(String, String)>,
    /// It has no closing tag.
    empty: bool,
    /// It may be used in text that is not the server's own.
    open: bool,
    flag: String,
}

/// An element that is open: what closes what it opened, and where its
/// text starts, for the one with a flag.
#[derive(Debug, Clone)]
struct Opened {
    name: String,
    closers: Vec<String>,
    flag: String,
    start: usize,
}

/// What a numbered kind of line is shown as.
#[derive(Debug, Clone, Default)]
struct LineTag {
    element: String,
    fore: Option<Color>,
    back: Option<Color>,
    gag: bool,
    frame: String,
}

/// A link being read.
#[derive(Debug, Clone, Default)]
struct Link {
    /// Where its text starts in the line.
    start: usize,
    /// Where it leads, if the tag said.
    href: Option<String>,
    hint: String,
    name: String,
    /// It puts its command in the command line instead of sending it.
    prompt: bool,
    /// It leads to the web.
    web: bool,
}

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
    /// The link being read.
    link: Option<Link>,
    /// What the server defined: elements, entities, and kinds of line.
    elements: HashMap<String, Element>,
    entities: HashMap<String, String>,
    line_tags: HashMap<u8, LineTag>,
    /// The elements of the server's that are open.
    opened: Vec<Opened>,
    /// The values being read: each name, and where its text starts.
    values: Vec<(String, usize)>,
    /// What this line is flagged as by its number, and from where.
    line_flag: Option<(String, usize)>,
    /// The mode to go back to after one tag.
    one_tag: Option<MxpMode>,
    /// The frame text is going to, and whether only for this line.
    dest: Option<String>,
    dest_for_line: bool,
    /// This line is not to be shown.
    gag: bool,
    /// The next end of line is not one, and none is inside a paragraph.
    no_break: bool,
    paragraph: bool,
    /// What tags asked of the client besides text, for the caller.
    asked: Vec<Mxp>,
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
            elements: HashMap::new(),
            entities: HashMap::new(),
            line_tags: HashMap::new(),
            opened: Vec::new(),
            values: Vec::new(),
            line_flag: None,
            one_tag: None,
            dest: None,
            dest_for_line: false,
            gag: false,
            no_break: false,
            paragraph: false,
            asked: Vec::new(),
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
                    // Not an end of line if the server said the next is
                    // not, or inside a paragraph, where it is a space.
                    '\n' if std::mem::take(&mut self.no_break) => {}
                    '\n' if self.paragraph => self.line.push(' ', self.style),
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
                            'z' if self.mxp => self.set_mxp_mode(out),
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
                        match self.entity_value(&self.markup) {
                            Some(value) => {
                                value.chars().for_each(|ch| self.line.push(ch, self.style));
                            }
                            None => {
                                self.literal('&');
                                self.line.push(';', self.style);
                            }
                        }
                        self.markup.clear();
                    }
                    _ if (ch.is_ascii_alphanumeric() || ch == '#' || ch == '_')
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

    /// Ends the line where a line of the server's ends: what was open
    /// on it is closed, and it goes back to how lines start.
    fn end_line(&mut self, out: &mut Vec<Line>) {
        // What an element with a flag, or the line itself, held.
        for opened in std::mem::take(&mut self.opened) {
            self.flagged(&opened);
        }
        if let Some((name, start)) = self.line_flag.take() {
            let value = self.line.text().get(start..).unwrap_or_default().to_owned();
            self.asked.push(Mxp::Value { name, value });
        }
        self.values.clear();
        self.push_line(out);
        if std::mem::take(&mut self.dest_for_line) {
            self.dest = None;
        }
        self.gag = false;
        if self.mxp {
            // Tags do not outlive their line, nor does a one-line mode.
            if let Some(base) = self.style_stack.first() {
                self.style = *base;
            }
            self.style_stack.clear();
            self.mode = self.default_mode;
            self.one_tag = None;
        }
    }

    /// Ends the line where a tag breaks it, with what is open still open.
    fn push_line(&mut self, out: &mut Vec<Line>) {
        self.close_link();
        let mut line = std::mem::take(&mut self.line);
        line.dest.clone_from(&self.dest);
        if !self.gag {
            out.push(line);
        }
        // What was being read started on the line that is gone.
        for opened in &mut self.opened {
            opened.start = 0;
        }
        for (_, start) in &mut self.values {
            *start = 0;
        }
    }

    /// Applies an MXP line mode, `ESC [ n z`.
    fn set_mxp_mode(&mut self, out: &mut Vec<Line>) {
        let number = self.params.parse::<u8>().unwrap_or(0);
        let mode = match number {
            // 0 to 2 are for this line; 5 to 7 are for every line after.
            0 | 5 => MxpMode::Open,
            1 | 6 => MxpMode::Secure,
            2 | 7 => MxpMode::Locked,
            // Secure for the next tag only.
            4 => {
                self.one_tag = Some(self.mode);
                MxpMode::Secure
            }
            // A numbered kind of line: the rest of it is of that kind.
            10..=99 => return self.line_tag(number, out),
            // 3 resets: close everything and go back to the default.
            _ => {
                self.close_link();
                if let Some(base) = self.style_stack.first() {
                    self.style = *base;
                }
                self.style_stack.clear();
                self.opened.clear();
                self.values.clear();
                self.default_mode = MxpMode::Open;
                MxpMode::Open
            }
        };
        if matches!(number, 5..=7) {
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
        // A mode for one tag only gives way to the one before it.
        let back = self.one_tag.take();
        self.carry_out(tag, out, 0);
        if let Some(mode) = back {
            self.mode = mode;
        }
    }

    /// Takes what the tags seen so far ask of the client besides text.
    pub fn take_asked(&mut self) -> Vec<Mxp> {
        std::mem::take(&mut self.asked)
    }

    fn carry_out(&mut self, tag: &str, out: &mut Vec<Line>, depth: u8) {
        let secure = self.mode == MxpMode::Secure;
        // Definitions are the server's to make, not another player's.
        if let Some(definition) = tag.strip_prefix('!') {
            if secure {
                self.define(definition);
            }
            return;
        }
        let (name, mut attributes) = parse_tag(tag);
        for (_, value) in &mut attributes {
            *value = self.expand(value);
        }
        if let Some(closing) = name.strip_prefix('/') {
            return self.close(closing, out);
        }
        // An element the server defined stands for the tags it was
        // defined as, with what it was given put in them.
        if let Some(element) = self.elements.get(&name).cloned()
            && (secure || element.open)
            && depth < MAX_DEPTH
        {
            return self.open_element(&name, &element, &attributes, out, depth);
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
        let said = |word: &str| {
            (attributes.iter()).any(|(name, value)| {
                name.as_deref() == Some(word)
                    || (name.is_none() && value.eq_ignore_ascii_case(word))
            })
        };
        let text =
            |key: &str, position: usize| attribute(key, position).unwrap_or_default().to_owned();
        match name.as_str() {
            "b" | "bold" | "strong" | "h" | "high" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.format(|style| style.bold = true)
            }
            "i" | "italic" | "em" => self.format(|style| style.italic = true),
            "u" | "underline" => self.format(|style| style.underline = true),
            "s" | "strikeout" => self.format(|style| style.strike = true),
            // Kept as they are: text here is of one size and one face.
            "small" | "tt" => self.format(|_| {}),
            "c" | "color" | "font" => {
                let fore = attribute(if name == "font" { "color" } else { "fore" }, 0)
                    .and_then(named_color);
                let back = attribute("back", 1).and_then(named_color);
                self.format(|style| {
                    style.fg = fore.unwrap_or(style.fg);
                    style.bg = back.unwrap_or(style.bg);
                });
            }
            "br" => self.push_line(out),
            // A break only if the line needs one: a space, here.
            "sbr" => self.line.push(' ', self.style),
            "nobr" => self.no_break = true,
            "p" => self.paragraph = true,
            "hr" => {
                if !self.line.is_empty() {
                    self.push_line(out);
                }
                (0..40).for_each(|_| self.line.push('─', self.style));
                self.push_line(out);
            }
            "send" if secure => {
                self.open_link(Link {
                    href: attribute("href", 0).map(str::to_owned),
                    hint: text("hint", 1),
                    name: attribute("expire", usize::MAX)
                        .unwrap_or_default()
                        .to_owned(),
                    prompt: said("prompt"),
                    ..Link::default()
                });
            }
            "a" if secure => {
                if let Some(href) = attribute("href", 0) {
                    self.open_link(Link {
                        href: Some(href.to_owned()),
                        hint: text("hint", 1),
                        name: attribute("expire", usize::MAX)
                            .unwrap_or_default()
                            .to_owned(),
                        web: true,
                        ..Link::default()
                    });
                }
            }
            "expire" if secure => self.asked.push(Mxp::Expire(text("name", 0))),
            // What follows, up to where it closes, is a value by a name.
            "v" | "var" if secure => {
                let name = text("name", 0);
                if !name.is_empty() {
                    self.values.push((name, self.line.text().len()));
                }
            }
            "gauge" | "stat" if secure => self.asked.push(Mxp::Gauge {
                entity: text("entity", 0),
                max: text("max", 1),
                caption: text("caption", 2),
                colour: text("color", 3),
                bar: name == "gauge",
            }),
            "frame" if secure => self.asked.push(Mxp::Frame {
                name: text("name", 0),
                open: !text("action", 1).eq_ignore_ascii_case("close"),
                title: text("title", 2),
                align: text("align", 4).to_lowercase(),
            }),
            // What follows goes to a frame; what came before on this
            // line stays where it was going.
            "dest" if secure => {
                if !self.line.is_empty() {
                    self.push_line(out);
                }
                let frame = text("name", 0);
                if said("eof") {
                    self.asked.push(Mxp::Clear(frame.clone()));
                }
                self.dest = (!frame.is_empty()).then_some(frame);
            }
            // A picture cannot be shown among text: its name, to open.
            "image" if secure => {
                let file = text("fname", 0);
                let from = attribute("url", 1).unwrap_or_default();
                if !file.is_empty() {
                    let start = self.line.text().len();
                    format!("[{file}]")
                        .chars()
                        .for_each(|ch| self.line.push(ch, self.style));
                    if !from.is_empty() {
                        self.line.links.push(LineLink {
                            range: start..self.line.text().len(),
                            target: LinkTarget::Url(format!(
                                "{}/{file}",
                                from.trim_end_matches('/')
                            )),
                            hint: String::new(),
                            name: String::new(),
                        });
                    }
                }
            }
            // Asked for as the sound protocol would, to go the same way.
            "sound" | "music" if secure => {
                let file = text("fname", 0);
                if !file.is_empty() {
                    let mut request = format!("!!{}({file}", name.to_uppercase());
                    let fourth = if name == "music" {
                        ("c", "C")
                    } else {
                        ("p", "P")
                    };
                    let fields = [
                        ("v", "V", 1),
                        ("l", "L", 2),
                        (fourth.0, fourth.1, 3),
                        ("t", "T", 4),
                        ("u", "U", 5),
                    ];
                    for (key, letter, position) in fields {
                        if let Some(value) =
                            attribute(key, position).filter(|value| !value.is_empty())
                        {
                            request.push_str(&format!(" {letter}={value}"));
                        }
                    }
                    request.push(')');
                    self.asked.push(Mxp::Sound(request));
                }
            }
            "relocate" if secure => self.asked.push(Mxp::Relocate {
                host: text("hostname", 0),
                port: text("port", 1),
            }),
            "user" if secure => self.asked.push(Mxp::User),
            "password" if secure => self.asked.push(Mxp::Password),
            "version" if secure => {
                let version = env!("CARGO_PKG_VERSION");
                self.replies.push(format!(
                    "\x1b[1z<VERSION MXP=\"1.0\" CLIENT=muddie VERSION=\"{version}\">"
                ));
            }
            // All there is, or of those asked about, which there are.
            "support" if secure => {
                let asked: Vec<String> = (attributes.iter())
                    .map(|(name, value)| name.clone().unwrap_or_else(|| value.to_lowercase()))
                    .collect();
                let listed: Vec<String> = if asked.is_empty() {
                    SUPPORTED.iter().map(|tag| format!("+{tag}")).collect()
                } else {
                    (asked.iter())
                        .map(|tag| {
                            let known = SUPPORTED.contains(&tag.split('.').next().unwrap_or(tag));
                            format!("{}{tag}", if known { '+' } else { '-' })
                        })
                        .collect()
                };
                self.replies
                    .push(format!("\x1b[1z<SUPPORTS {}>", listed.join(" ")));
            }
            _ => {}
        }
    }

    /// Closes a tag: an element the server defined, by closing what it
    /// opened, or one of the client's own.
    fn close(&mut self, name: &str, out: &mut Vec<Line>) {
        if let Some(place) = self.opened.iter().rposition(|opened| opened.name == name) {
            let opened = self.opened.remove(place);
            self.flagged(&opened);
            for closer in opened.closers.iter().rev() {
                self.close(closer, out);
            }
            return;
        }
        match name {
            "send" | "a" => self.close_link(),
            "v" | "var" => {
                if let Some((name, start)) = self.values.pop() {
                    let value = self.line.text().get(start..).unwrap_or_default().to_owned();
                    self.entities.insert(name.to_lowercase(), value.clone());
                    self.asked.push(Mxp::Value { name, value });
                }
            }
            "dest" => {
                if !self.line.is_empty() {
                    self.push_line(out);
                }
                self.dest = None;
            }
            "p" => self.paragraph = false,
            _ if is_formatting(name) => {
                if let Some(style) = self.style_stack.pop() {
                    self.style = style;
                }
            }
            _ => {}
        }
    }

    /// Opens an element the server defined: its tags, with `&name;` in
    /// them standing for what it was given under that name, or in that
    /// place, or by default.
    fn open_element(
        &mut self,
        name: &str,
        element: &Element,
        given: &[(Option<String>, String)],
        out: &mut Vec<Line>,
        depth: u8,
    ) {
        let mut in_place = given.iter().filter(|(name, _)| name.is_none());
        let mut definition = element.definition.clone();
        for (attribute, default) in &element.attributes {
            let named =
                (given.iter()).find(|(name, _)| name.as_deref() == Some(attribute.as_str()));
            let value = match named {
                Some((_, value)) => value.clone(),
                None => in_place
                    .next()
                    .map_or(default.clone(), |(_, value)| value.clone()),
            };
            definition = replace_entity(&definition, attribute, &value);
        }
        let start = self.line.text().len();
        let mut closers = Vec::new();
        for inner in tags_in(&definition) {
            let opens = !inner.starts_with(['/', '!']);
            if opens {
                closers.push(parse_tag(&inner).0);
            }
            self.carry_out(&inner, out, depth + 1);
        }
        if !element.empty {
            self.opened.push(Opened {
                name: name.to_owned(),
                closers,
                flag: element.flag.clone(),
                start,
            });
        }
    }

    /// Tells what an element with a flag held, such as the name of the
    /// room in one flagged `RoomName`.
    fn flagged(&mut self, opened: &Opened) {
        if !opened.flag.is_empty() {
            let value = self
                .line
                .text()
                .get(opened.start..)
                .unwrap_or_default()
                .to_owned();
            self.asked.push(Mxp::Value {
                name: opened.flag.clone(),
                value,
            });
        }
    }

    /// Takes a definition: `!ELEMENT`, `!ATTLIST`, `!ENTITY` or `!TAG`,
    /// given without the `!`.
    fn define(&mut self, definition: &str) {
        let (kind, words) = parse_tag(definition);
        let mut in_place = words
            .iter()
            .filter(|(name, _)| name.is_none())
            .map(|(_, value)| value);
        let keyed = |key: &str| {
            (words.iter())
                .find(|(name, _)| name.as_deref() == Some(key))
                .map(|(_, value)| value.as_str())
        };
        let said = |word: &str| {
            (words.iter()).any(|(name, value)| name.is_none() && value.eq_ignore_ascii_case(word))
        };
        let Some(name) = in_place.next().map(|name| name.to_lowercase()) else {
            return;
        };
        match kind.as_str() {
            "element" | "el" => {
                if said("delete") {
                    self.elements.remove(&name);
                    return;
                }
                // Its tags are the next thing given that is not a flag.
                let flags = ["open", "empty", "delete"];
                let definition = in_place
                    .find(|word| !flags.iter().any(|flag| word.eq_ignore_ascii_case(flag)))
                    .cloned()
                    .unwrap_or_default();
                let element = Element {
                    definition,
                    attributes: keyed("att").map(attribute_list).unwrap_or_default(),
                    empty: said("empty"),
                    open: said("open"),
                    flag: keyed("flag").unwrap_or_default().to_owned(),
                };
                // A line marked with its number is inside it.
                if let Some(number) = keyed("tag").and_then(|number| number.parse::<u8>().ok()) {
                    self.line_tags.entry(number).or_default().element = name.clone();
                }
                if self.elements.len() < MAX_DEFINED || self.elements.contains_key(&name) {
                    self.elements.insert(name, element);
                }
            }
            "attlist" | "at" => {
                if let (Some(element), Some(list)) = (self.elements.get_mut(&name), in_place.next())
                {
                    element.attributes = attribute_list(list);
                }
            }
            "entity" | "en" => {
                if said("delete") {
                    self.entities.remove(&name);
                    return;
                }
                let value = in_place.next().cloned().unwrap_or_default();
                if self.entities.len() < MAX_DEFINED || self.entities.contains_key(&name) {
                    self.entities.insert(name.clone(), value.clone());
                }
                if !said("private") {
                    self.asked.push(Mxp::Value { name, value });
                }
            }
            "tag" => {
                if let Ok(number) = name.parse::<u8>() {
                    let tag = self.line_tags.entry(number).or_default();
                    tag.fore = keyed("fore").and_then(named_color).or(tag.fore);
                    tag.back = keyed("back").and_then(named_color).or(tag.back);
                    tag.gag |= said("gag");
                    if let Some(frame) = keyed("windowname") {
                        tag.frame = frame.to_owned();
                    }
                }
            }
            _ => {}
        }
    }

    /// `text` with each `&name;` in it replaced by what it stands for.
    fn expand(&self, text: &str) -> String {
        if !text.contains('&') {
            return text.to_owned();
        }
        let mut expanded = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find('&') {
            expanded.push_str(&rest[..at]);
            let after = &rest[at + 1..];
            let name = after.split(';').next().unwrap_or_default();
            let whole = after.len() > name.len();
            // `&text;` is the text of the link, known once it closes.
            match self.entity_value(name).filter(|_| whole && name != "text") {
                Some(value) => {
                    expanded.push_str(&value);
                    rest = &after[name.len() + 1..];
                }
                None => {
                    expanded.push('&');
                    rest = after;
                }
            }
        }
        expanded.push_str(rest);
        expanded
    }

    /// What an entity stands for: one the server defined, or a usual one.
    fn entity_value(&self, name: &str) -> Option<String> {
        if let Some(value) = self.entities.get(&name.to_lowercase()) {
            return Some(value.clone());
        }
        entity(name).map(String::from)
    }

    /// Marks a line as one of a numbered kind, `ESC [ n z` with a number
    /// from 10: the name of a room, its description, its exits, a
    /// prompt, or what the server said the number means.
    fn line_tag(&mut self, number: u8, out: &mut Vec<Line>) {
        let usual = match number {
            10 => "RoomName",
            11 => "RoomDesc",
            12 => "RoomExit",
            13 => "WelcomeText",
            _ => "",
        };
        let tag = self.line_tags.get(&number).cloned().unwrap_or_default();
        if !usual.is_empty() {
            self.line_flag = Some((usual.to_owned(), self.line.text().len()));
        }
        if tag.fore.is_some() || tag.back.is_some() {
            self.format(|style| {
                style.fg = tag.fore.unwrap_or(style.fg);
                style.bg = tag.back.unwrap_or(style.bg);
            });
        }
        self.gag |= tag.gag;
        if !tag.frame.is_empty() {
            self.dest = Some(tag.frame.clone());
            self.dest_for_line = true;
        }
        if !tag.element.is_empty() {
            let back = std::mem::replace(&mut self.mode, MxpMode::Secure);
            self.carry_out(&tag.element.clone(), out, 0);
            self.mode = back;
        }
    }

    /// Changes the style for the text up to the matching closing tag.
    fn format(&mut self, change: impl FnOnce(&mut Style)) {
        self.style_stack.push(self.style);
        change(&mut self.style);
    }

    fn open_link(&mut self, link: Link) {
        self.close_link();
        self.link = Some(Link {
            start: self.line.text().len(),
            ..link
        });
    }

    fn close_link(&mut self) {
        let Some(link) = self.link.take() else {
            return;
        };
        let text = self.line.text();
        if link.start >= text.len() {
            return;
        }
        let shown = &text[link.start..];
        // A link that does not say where it leads sends its own text;
        // `&text;` in where it leads stands for that text too.
        let href = link.href.unwrap_or_else(|| shown.to_owned());
        let href = replace_entity(&href, "text", shown);
        // The hint may be several: what the link is for, then a name for
        // each of the commands it offers.
        let mut hints = link.hint.split('|').map(str::trim);
        let hint = hints.next().unwrap_or_default().to_owned();
        let commands: Vec<&str> = href.split('|').map(str::trim).collect();
        let target = match commands.as_slice() {
            _ if link.web => LinkTarget::Url(href.clone()),
            [command] if link.prompt => LinkTarget::Prompt((*command).to_owned()),
            [command] => LinkTarget::Send((*command).to_owned()),
            several => LinkTarget::Menu(
                (several.iter())
                    .map(|command| {
                        let called = hints.next().filter(|name| !name.is_empty());
                        (called.unwrap_or(command).to_owned(), (*command).to_owned())
                    })
                    .collect(),
            ),
        };
        self.line.links.push(LineLink {
            range: link.start..text.len(),
            target,
            hint,
            name: link.name,
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
            | "small"
            | "tt"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
    )
}

/// Splits a tag into its lower-cased name and its attributes, each with
/// its lower-cased name if it has one (`href="look"`) or without (`red`).
/// A `=` inside quotes is part of a value, not what follows a name.
fn parse_tag(tag: &str) -> (String, Vec<(Option<String>, String)>) {
    let mut words: Vec<(Option<usize>, String)> = Vec::new();
    let mut word = String::new();
    let mut named_at = None;
    let mut quote = None;
    for ch in tag.chars() {
        match quote {
            Some(open) if ch == open => quote = None,
            Some(_) => word.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !word.is_empty() {
                    words.push((named_at.take(), std::mem::take(&mut word)));
                }
            }
            None => {
                if ch == '=' && named_at.is_none() {
                    named_at = Some(word.len());
                }
                word.push(ch);
            }
        }
    }
    if !word.is_empty() {
        words.push((named_at, word));
    }
    let mut words = words.into_iter();
    let name = words
        .next()
        .map(|(_, name)| name)
        .unwrap_or_default()
        .to_lowercase();
    let attributes = words
        .map(|(named_at, word)| match named_at {
            Some(at) if at > 0 => (Some(word[..at].to_lowercase()), word[at + 1..].to_owned()),
            _ => (None, word),
        })
        .collect();
    (name, attributes)
}

/// What an element takes, from its list: each name, lower-cased, with
/// its default if the list gives one (`colour=red size`).
fn attribute_list(list: &str) -> Vec<(String, String)> {
    (list.split_whitespace())
        .map(|attribute| match attribute.split_once('=') {
            Some((name, default)) => (name.to_lowercase(), default.to_owned()),
            None => (attribute.to_lowercase(), String::new()),
        })
        .collect()
}

/// `text` with each `&name;` in it, whatever its case, replaced by
/// `value`.
fn replace_entity(text: &str, name: &str, value: &str) -> String {
    let wanted = format!("&{};", name.to_lowercase());
    let lower = text.to_lowercase();
    let mut replaced = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(found) = lower[at..].find(&wanted) {
        replaced.push_str(&text[at..at + found]);
        replaced.push_str(value);
        at += found + wanted.len();
    }
    replaced.push_str(&text[at..]);
    replaced
}

/// The tags in a definition, each without its angle brackets.
fn tags_in(definition: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut rest = definition;
    while let Some(start) = rest.find('<') {
        let Some(length) = rest[start..].find('>') else {
            break;
        };
        tags.push(rest[start + 1..start + length].to_owned());
        rest = &rest[start + length + 1..];
    }
    tags
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
        let link = |range, target| LineLink {
            range,
            target,
            hint: String::new(),
            name: String::new(),
        };
        assert_eq!(
            out[0].links,
            [
                // Several commands are a menu; the first is what a click sends.
                link(
                    3..8,
                    LinkTarget::Menu(vec![
                        ("north".into(), "north".into()),
                        ("look north".into(), "look north".into()),
                    ]),
                ),
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
            mxp(&["\x1b[1z<VERSION><support><!ELEMENT hp '<b>'><filter x><image x.png>ok\n"]);
        // What is not carried out is dropped; a picture is its name.
        assert_eq!(out[0].text(), "[x.png]ok");
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
            hint: String::new(),
            name: String::new(),
        };
        assert_eq!(line.links, [expected]);
    }

    #[test]
    fn links_have_hints_names_prompts_and_menus_with_names() {
        let line = "\x1b[1z<send href=\"buy &text;|look &text;\" hint=\"A sword|Buy it|Look at it\" expire=shop>sword</send> \
                    <send \"tell guard \" prompt>talk</send> <a href=\"http://x.io\" hint=\"The site\">site</a>\n";
        let (out, _) = mxp(&[line]);
        assert_eq!(out[0].text(), "sword talk site");
        let links = &out[0].links;
        // The text of the link stands for `&text;`, and the hint names
        // the link and then each of its commands.
        let menu = vec![
            ("Buy it".to_owned(), "buy sword".to_owned()),
            ("Look at it".to_owned(), "look sword".to_owned()),
        ];
        assert_eq!(links[0].target, LinkTarget::Menu(menu));
        assert_eq!(
            (links[0].hint.as_str(), links[0].name.as_str()),
            ("A sword", "shop")
        );
        assert_eq!(links[1].target, LinkTarget::Prompt("tell guard".into()));
        assert_eq!(
            (&links[2].target, links[2].hint.as_str()),
            (&LinkTarget::Url("http://x.io".into()), "The site")
        );
        // The links of a name can be taken away later.
        let (_, mut assembler) = mxp(&["\x1b[1z<expire shop>\n"]);
        assert_eq!(assembler.take_asked(), [Mxp::Expire("shop".into())]);
    }

    #[test]
    fn elements_and_entities_the_server_defines_are_carried_out() {
        let define = "\x1b[1z<!ELEMENT Ex '<color &col;><send href=\"go &dir;\">' ATT='dir col=lime' FLAG=RoomExit>\
                      <!EN hero \"Ana\" PUBLISH>\x1b[1z<!ELEMENT boss '<b><color red>' OPEN>\n";
        let (out, mut assembler) = mxp(&[
            define,
            "\x1b[1zExits: <Ex north>north</Ex>, <ex DIR=\"east wing\" col=red>east</ex>. Hi &hero;!\n",
        ]);
        assert_eq!(out[1].text(), "Exits: north, east. Hi Ana!");
        // What it was given goes in its tags: by place, by name, by default.
        let links: Vec<&LinkTarget> = out[1].links.iter().map(|link| &link.target).collect();
        assert_eq!(
            links,
            [
                &LinkTarget::Send("go north".into()),
                &LinkTarget::Send("go east wing".into())
            ]
        );
        let colour = |text: &str| {
            out[1]
                .spans
                .iter()
                .find(|span| span.text == text)
                .map(|span| span.style.fg)
        };
        assert_eq!(colour("north"), Some(Color::Indexed(10)));
        assert_eq!(colour("east"), Some(Color::Indexed(9)));
        // Closing it closes what it opened, and what it held is told
        // under its flag; an entity is told as a value too.
        assert_eq!(colour(". Hi Ana!"), Some(Color::Default));
        let value = |name: &str, value: &str| Mxp::Value {
            name: name.into(),
            value: value.into(),
        };
        assert_eq!(
            assembler.take_asked(),
            [
                value("hero", "Ana"),
                value("RoomExit", "north"),
                value("RoomExit", "east")
            ]
        );
        // One marked open may be used where text is not the server's
        // own; the others, and definitions, only where it is.
        let mut out = Vec::new();
        assembler.feed(
            "<boss>Orc</boss> <Ex north>n</Ex> <!EN hero \"Eve\">&hero;\n",
            &mut out,
        );
        assert_eq!(out[0].text(), "Orc n Ana");
        assert!(out[0].spans[0].style.bold && out[0].links.is_empty());
        // And one can be taken away.
        assembler.feed("\x1b[1z<!EL boss DELETE><boss>Orc</boss>\n", &mut out);
        assert!(!out[1].spans[0].style.bold);
    }

    #[test]
    fn values_gauges_frames_and_sounds_are_asked_of_the_client() {
        let (out, mut assembler) = mxp(&[
            "\x1b[1zHP: <v hp>62</v>/<var maxhp>100</var> <gauge hp max=maxhp caption=\"Health\" color=red><stat gold>\n",
            "\x1b[1z<frame name=Map action=open title=\"The map\" align=right><dest Map eof> #-@\n</dest>after\n",
            "\x1b[1z<sound hit.wav 80 2 60 combat http://mud.example/s/><music town.mid c=0>\x1b[1z<relocate mud.example 4001>\x1b[1z<user>\n",
        ]);
        assert_eq!(out[0].text(), "HP: 62/100 ");
        // What goes to a frame says so, and what follows does not.
        assert_eq!(
            (out[1].text().as_str(), out[1].dest.as_deref()),
            (" #-@", Some("Map"))
        );
        assert_eq!(
            (out[2].text().as_str(), out[2].dest.as_deref()),
            ("after", None)
        );
        let value = |name: &str, value: &str| Mxp::Value {
            name: name.into(),
            value: value.into(),
        };
        assert_eq!(
            assembler.take_asked(),
            [
                value("hp", "62"),
                value("maxhp", "100"),
                Mxp::Gauge {
                    entity: "hp".into(),
                    max: "maxhp".into(),
                    caption: "Health".into(),
                    colour: "red".into(),
                    bar: true
                },
                Mxp::Gauge {
                    entity: "gold".into(),
                    max: String::new(),
                    caption: String::new(),
                    colour: String::new(),
                    bar: false
                },
                Mxp::Frame {
                    name: "Map".into(),
                    open: true,
                    title: "The map".into(),
                    align: "right".into()
                },
                Mxp::Clear("Map".into()),
                Mxp::Sound(
                    "!!SOUND(hit.wav V=80 L=2 P=60 T=combat U=http://mud.example/s/)".into()
                ),
                Mxp::Sound("!!MUSIC(town.mid C=0)".into()),
                Mxp::Relocate {
                    host: "mud.example".into(),
                    port: "4001".into()
                },
                Mxp::User,
            ]
        );
        // A value read is an entity from then on.
        let mut out = Vec::new();
        assembler.feed("You have &hp; left.\n", &mut out);
        assert_eq!(out[0].text(), "You have 62 left.");
    }

    #[test]
    fn numbered_lines_are_of_a_kind() {
        let (out, mut assembler) = mxp(&[
            "\x1b[1z<!TAG 20 FORE=red><!TAG 21 GAG><!ELEMENT shout '<b>' TAG=22>\n",
            "\x1b[10zThe Plaza\n\x1b[20zA red line\n\x1b[21zNot shown\n\x1b[22zLoud\nplain\n",
        ]);
        let texts: Vec<String> = out.iter().map(Line::text).collect();
        assert_eq!(texts, ["", "The Plaza", "A red line", "Loud", "plain"]);
        assert_eq!(out[2].spans[0].style.fg, Color::Indexed(9));
        assert!(out[3].spans[0].style.bold && !out[4].spans[0].style.bold);
        assert_eq!(out[4].spans[0].style.fg, Color::Default);
        // The name of a room is told as one.
        let named = Mxp::Value {
            name: "RoomName".into(),
            value: "The Plaza".into(),
        };
        assert_eq!(assembler.take_asked(), [named]);
    }

    #[test]
    fn breaks_paragraphs_rules_and_one_tag_modes() {
        let (out, _) = mxp(&["one<nobr>\ntwo<br>three<sbr>four\n<p>a\nb</p>\nc\n<hr>d\n"]);
        let texts: Vec<String> = out.iter().map(Line::text).collect();
        assert_eq!(texts[..4], ["onetwo", "three four", "a b", "c"]);
        assert!(texts[4].starts_with("────") && texts[5] == "d");
        // Secure for the tag that follows, and no further.
        let (out, _) = mxp(&["\x1b[4z<send>look</send> <send>quit</send>\n"]);
        assert_eq!(out[0].text(), "look quit");
        let targets: Vec<&LinkTarget> = out[0].links.iter().map(|link| &link.target).collect();
        assert_eq!(targets, [&LinkTarget::Send("look".into())]);
        // Asked what it carries out, it says; and of some, which.
        let (_, mut assembler) = mxp(&["\x1b[1z<support>\x1b[1z<support send image gauge>\n"]);
        let replies = assembler.take_replies();
        assert!(replies[0].contains("+send") && replies[0].contains("+frame"));
        assert_eq!(replies[1], "\x1b[1z<SUPPORTS +send -image +gauge>");
    }
}
