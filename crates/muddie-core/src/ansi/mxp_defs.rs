//! What the server defines with MXP: its own elements, attribute lists,
//! entities and line tags.

use super::*;

/// Longest MXP entity name, as in `&name;`.
pub(crate) const MAX_ENTITY: usize = 32;

/// How many elements, and how many entities, a server may define.
pub(crate) const MAX_DEFINED: usize = 512;

/// An element the server defined: the tags it stands for, what it takes
/// with its defaults, and what it is flagged as.
#[derive(Debug, Clone, Default)]
pub(crate) struct Element {
    pub(crate) definition: String,
    pub(crate) attributes: Vec<(String, String)>,
    /// It has no closing tag.
    pub(crate) empty: bool,
    /// It may be used in text that is not the server's own.
    pub(crate) open: bool,
    pub(crate) flag: String,
}

/// What a numbered kind of line is shown as.
#[derive(Debug, Clone, Default)]
pub(crate) struct LineTag {
    pub(crate) element: String,
    pub(crate) fore: Option<Color>,
    pub(crate) back: Option<Color>,
    pub(crate) gag: bool,
    pub(crate) frame: String,
}

impl Assembler {
    /// Takes a definition: `!ELEMENT`, `!ATTLIST`, `!ENTITY` or `!TAG`,
    /// given without the `!`.
    pub(crate) fn define(&mut self, definition: &str) {
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
    pub(crate) fn expand(&self, text: &str) -> String {
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
    pub(crate) fn entity_value(&self, name: &str) -> Option<String> {
        if let Some(value) = self.entities.get(&name.to_lowercase()) {
            return Some(value.clone());
        }
        entity(name).map(String::from)
    }

    /// Marks a line as one of a numbered kind, `ESC [ n z` with a number
    /// from 10: the name of a room, its description, its exits, a
    /// prompt, or what the server said the number means.
    pub(crate) fn line_tag(&mut self, number: u8, out: &mut Vec<Line>) {
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
}
