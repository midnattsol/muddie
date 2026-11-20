//! What each MXP tag does, and the modes MXP is in.

use super::*;

/// The tags of MXP this client carries out, as it tells a server that
/// asks.
pub(crate) const SUPPORTED: [&str; 38] = [
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

/// An element that is open: what closes what it opened, and where its
/// text starts, for the one with a flag.
#[derive(Debug, Clone)]
pub(crate) struct Opened {
    pub(crate) name: String,
    pub(crate) closers: Vec<String>,
    pub(crate) flag: String,
    pub(crate) start: usize,
}

/// How much MXP markup is honoured on the current line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MxpMode {
    /// Formatting tags only: the text may come from other players.
    Open,
    /// Every supported tag, including links that send commands.
    Secure,
    /// No markup: `<` and `&` are ordinary characters.
    Locked,
}

impl Assembler {
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

    /// Applies an MXP line mode, `ESC [ n z`.
    pub(crate) fn set_mxp_mode(&mut self, out: &mut Vec<Line>) {
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
    pub(crate) fn tag(&mut self, tag: &str, out: &mut Vec<Line>) {
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

    /// Does what a tag says: defines something, closes what it opened,
    /// stands for the tags an element was defined as, formats text, or,
    /// from the server alone, the rest.
    pub(crate) fn carry_out(&mut self, tag: &str, out: &mut Vec<Line>, depth: u8) {
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
        let given = Attributes(attributes);
        if !self.formatting_tag(&name, &given, out) && secure {
            self.secure_tag(&name, &given, out);
        }
    }

    /// The tags anyone may send: of the look of text and of its lines.
    /// Returns whether it was one.
    fn formatting_tag(&mut self, name: &str, given: &Attributes, out: &mut Vec<Line>) -> bool {
        match name {
            "b" | "bold" | "strong" | "h" | "high" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.format(|style| style.bold = true)
            }
            "i" | "italic" | "em" => self.format(|style| style.italic = true),
            "u" | "underline" => self.format(|style| style.underline = true),
            "s" | "strikeout" => self.format(|style| style.strike = true),
            // Kept as they are: text here is of one size and one face.
            "small" | "tt" => self.format(|_| {}),
            "c" | "color" | "font" => {
                let fore = given
                    .get(if name == "font" { "color" } else { "fore" }, 0)
                    .and_then(named_color);
                let back = given.get("back", 1).and_then(named_color);
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
            "hr" => self.rule(out),
            _ => return false,
        }
        true
    }

    /// The tags only the server may send: links, values, gauges, frames,
    /// pictures, sounds, and what the client is asked about itself.
    fn secure_tag(&mut self, name: &str, given: &Attributes, out: &mut Vec<Line>) {
        match name {
            "send" => self.open_link(Link {
                href: given.get("href", 0).map(str::to_owned),
                hint: given.text("hint", 1),
                name: given.text("expire", usize::MAX),
                prompt: given.has("prompt"),
                ..Link::default()
            }),
            "a" => {
                if let Some(href) = given.get("href", 0) {
                    self.open_link(Link {
                        href: Some(href.to_owned()),
                        hint: given.text("hint", 1),
                        name: given.text("expire", usize::MAX),
                        web: true,
                        ..Link::default()
                    });
                }
            }
            "expire" => self.asked.push(Mxp::Expire(given.text("name", 0))),
            // What follows, up to where it closes, is a value by a name.
            "v" | "var" => {
                let name = given.text("name", 0);
                if !name.is_empty() {
                    self.values.push((name, self.line.text().len()));
                }
            }
            "gauge" | "stat" => self.asked.push(Mxp::Gauge {
                entity: given.text("entity", 0),
                max: given.text("max", 1),
                caption: given.text("caption", 2),
                colour: given.text("color", 3),
                bar: name == "gauge",
            }),
            "frame" => self.asked.push(Mxp::Frame {
                name: given.text("name", 0),
                open: !given.text("action", 1).eq_ignore_ascii_case("close"),
                title: given.text("title", 2),
                align: given.text("align", 4).to_lowercase(),
            }),
            "dest" => self.destination(given, out),
            "image" => self.image(given),
            "sound" | "music" => self.sound(name, given),
            "relocate" => self.asked.push(Mxp::Relocate {
                host: given.text("hostname", 0),
                port: given.text("port", 1),
            }),
            "user" => self.asked.push(Mxp::User),
            "password" => self.asked.push(Mxp::Password),
            "version" => {
                let version = env!("CARGO_PKG_VERSION");
                self.replies.push(format!(
                    "\x1b[1z<VERSION MXP=\"1.0\" CLIENT=muddie VERSION=\"{version}\">"
                ));
            }
            "support" => self.support(given),
            _ => {}
        }
    }

    /// A rule across the text, on a line of its own.
    fn rule(&mut self, out: &mut Vec<Line>) {
        if !self.line.is_empty() {
            self.push_line(out);
        }
        (0..40).for_each(|_| self.line.push('─', self.style));
        self.push_line(out);
    }

    /// What follows goes to a frame; what came before on this line stays
    /// where it was going.
    fn destination(&mut self, given: &Attributes, out: &mut Vec<Line>) {
        if !self.line.is_empty() {
            self.push_line(out);
        }
        let frame = given.text("name", 0);
        if given.has("eof") {
            self.asked.push(Mxp::Clear(frame.clone()));
        }
        self.dest = (!frame.is_empty()).then_some(frame);
    }

    /// A picture cannot be shown among text: its name, to open.
    fn image(&mut self, given: &Attributes) {
        let file = given.text("fname", 0);
        if file.is_empty() {
            return;
        }
        let start = self.line.text().len();
        format!("[{file}]")
            .chars()
            .for_each(|ch| self.line.push(ch, self.style));
        let from = given.text("url", 1);
        if !from.is_empty() {
            self.line.links.push(LineLink {
                range: start..self.line.text().len(),
                target: LinkTarget::Url(format!("{}/{file}", from.trim_end_matches('/'))),
                hint: String::new(),
                name: String::new(),
            });
        }
    }

    /// A sound or music, asked for as the sound protocol would, to go the
    /// same way.
    fn sound(&mut self, name: &str, given: &Attributes) {
        let file = given.text("fname", 0);
        if file.is_empty() {
            return;
        }
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
            if let Some(value) = given.get(key, position).filter(|value| !value.is_empty()) {
                request.push_str(&format!(" {letter}={value}"));
            }
        }
        request.push(')');
        self.asked.push(Mxp::Sound(request));
    }

    /// Says which tags there are: all, or of those asked about, which.
    fn support(&mut self, given: &Attributes) {
        let asked: Vec<String> = (given.0.iter())
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

    /// Closes a tag: an element the server defined, by closing what it
    /// opened, or one of the client's own.
    pub(crate) fn close(&mut self, name: &str, out: &mut Vec<Line>) {
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
    pub(crate) fn open_element(
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
    pub(crate) fn flagged(&mut self, opened: &Opened) {
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

    /// Changes the style for the text up to the matching closing tag.
    pub(crate) fn format(&mut self, change: impl FnOnce(&mut Style)) {
        self.style_stack.push(self.style);
        change(&mut self.style);
    }
}
