//! Reading an MXP tag: its name and attributes, and the entities in text.

/// Whether `text` ends inside a quoted string.
pub(crate) fn ends_in_quotes(text: &str) -> bool {
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

pub(crate) fn is_formatting(name: &str) -> bool {
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
pub(crate) fn parse_tag(tag: &str) -> (String, Vec<(Option<String>, String)>) {
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
pub(crate) fn attribute_list(list: &str) -> Vec<(String, String)> {
    (list.split_whitespace())
        .map(|attribute| match attribute.split_once('=') {
            Some((name, default)) => (name.to_lowercase(), default.to_owned()),
            None => (attribute.to_lowercase(), String::new()),
        })
        .collect()
}

/// `text` with each `&name;` in it, whatever the case of its letters,
/// replaced by `value`. Only the letters of English change case, so that
/// the text lowered is as long as the text, byte by byte.
pub(crate) fn replace_entity(text: &str, name: &str, value: &str) -> String {
    let wanted = format!("&{};", name.to_ascii_lowercase());
    let lower = text.to_ascii_lowercase();
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
pub(crate) fn tags_in(definition: &str) -> Vec<String> {
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
pub(crate) fn entity(name: &str) -> Option<char> {
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

/// What a tag was given: by name, or by where it is among those given
/// without one.
pub(crate) struct Attributes(pub(crate) Vec<(Option<String>, String)>);

impl Attributes {
    /// The one called `key`, or else the one without a name at `position`.
    pub(crate) fn get(&self, key: &str, position: usize) -> Option<&str> {
        let named = self.0.iter().find(|(name, _)| name.as_deref() == Some(key));
        let positional = self
            .0
            .iter()
            .filter(|(name, _)| name.is_none())
            .nth(position);
        named.or(positional).map(|(_, value)| value.as_str())
    }

    /// The same, as text: empty if not given.
    pub(crate) fn text(&self, key: &str, position: usize) -> String {
        self.get(key, position).unwrap_or_default().to_owned()
    }

    /// Whether a word was given, by name or as a value of its own.
    pub(crate) fn has(&self, word: &str) -> bool {
        (self.0.iter()).any(|(name, value)| {
            name.as_deref() == Some(word) || (name.is_none() && value.eq_ignore_ascii_case(word))
        })
    }
}
