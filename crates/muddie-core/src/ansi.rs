//! Turns decoded text with ANSI escape sequences, and MXP markup when the
//! server uses it, into styled [`Line`]s.

use std::collections::HashMap;

use crate::text::{Color, Line, LineLink, LinkTarget, Style};

mod mxp;
mod mxp_defs;
mod mxp_links;
mod mxp_parse;
mod sgr;
#[cfg(test)]
mod tests;

use mxp::*;
use mxp_defs::*;
use mxp_links::*;
use mxp_parse::*;
use sgr::*;

/// Longest CSI parameter string kept; a longer one is malformed and dropped.
const MAX_PARAMS: usize = 64;
/// Longest MXP tag kept; a longer `<...` is taken as ordinary text.
const MAX_TAG: usize = 1024;
/// How deep the elements a server defines may stand for one another.
const MAX_DEPTH: u8 = 8;

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

    /// Consumes `text`, appending each line completed by a newline to `out`.
    pub fn feed(&mut self, text: &str, out: &mut Vec<Line>) {
        for ch in text.chars() {
            match self.state {
                State::Ground => self.ground(ch, out),
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
                State::Csi => self.csi(ch, out),
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
                State::Tag => self.in_tag(ch, out),
                State::Entity => self.in_entity(ch, out),
            }
        }
    }

    /// Takes one character of ordinary text.
    fn ground(&mut self, ch: char, out: &mut Vec<Line>) {
        match ch {
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
        }
    }

    /// Takes one character of a control sequence.
    fn csi(&mut self, ch: char, out: &mut Vec<Line>) {
        match ch {
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
        }
    }

    /// Takes one character of an MXP tag.
    fn in_tag(&mut self, ch: char, out: &mut Vec<Line>) {
        match ch {
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
        }
    }

    /// Takes one character of an MXP entity.
    fn in_entity(&mut self, ch: char, out: &mut Vec<Line>) {
        match ch {
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
