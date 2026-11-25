//! Colouring of Luau source for the script editors.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontId};

use crate::theme::Theme;

/// What a stretch of source is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Plain,
    Comment,
    Text,
    Number,
    Keyword,
    /// A name of the language's own values: `true`, `nil`...
    Constant,
    /// A name being called.
    Call,
}

const KEYWORDS: [&str; 20] = [
    "and", "break", "continue", "do", "else", "elseif", "end", "for", "function", "if", "in",
    "local", "not", "or", "repeat", "return", "then", "until", "while", "export",
];
const CONSTANTS: [&str; 4] = ["true", "false", "nil", "self"];

/// Splits `source` into stretches of one kind each. The stretches follow
/// one another and cover the whole source, so unfinished strings and
/// comments, as they are while being typed, simply run to the end.
pub fn tokens(source: &str) -> Vec<(Kind, &str)> {
    let bytes = source.as_bytes();
    let mut tokens: Vec<(Kind, &str)> = Vec::new();
    let mut push = |kind: Kind, from: usize, to: usize| {
        if from < to {
            tokens.push((kind, &source[from..to]));
        }
    };
    let mut plain_from = 0;
    let mut at = 0;
    while at < bytes.len() {
        let Some((kind, end)) = token_at(source, at) else {
            // Punctuation, spaces and anything else, one character at a time.
            let width = source[at..].chars().next().map_or(1, char::len_utf8);
            at += width;
            continue;
        };
        // An escape at the very end can point past it.
        let end = (at + 1..=end.min(bytes.len()))
            .rev()
            .find(|&end| source.is_char_boundary(end))
            .unwrap_or(bytes.len());
        if kind == Kind::Plain {
            at = end;
            continue;
        }
        push(Kind::Plain, plain_from, at);
        push(kind, at, end);
        plain_from = end;
        at = end;
    }
    push(Kind::Plain, plain_from, source.len());
    tokens
}

/// The kind and the end of the stretch that starts at `at`, or `None`
/// for punctuation, spaces and anything else.
fn token_at(source: &str, at: usize) -> Option<(Kind, usize)> {
    let bytes = source.as_bytes();
    let byte = bytes[at];
    Some(if source[at..].starts_with("--") {
        let end = long_bracket(source, at + 2).unwrap_or_else(|| {
            source[at..]
                .find('\n')
                .map_or(source.len(), |line_end| at + line_end)
        });
        (Kind::Comment, end)
    } else if byte == b'"' || byte == b'\'' || byte == b'`' {
        let mut end = at + 1;
        while end < bytes.len() && bytes[end] != byte && bytes[end] != b'\n' {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        (Kind::Text, (end + 1).min(bytes.len()))
    } else if let Some(end) = long_bracket(source, at) {
        (Kind::Text, end)
    } else if byte.is_ascii_digit() {
        let mut end = at + 1;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'.' || bytes[end] == b'_')
        {
            end += 1;
        }
        (Kind::Number, end)
    } else if byte.is_ascii_alphabetic() || byte == b'_' {
        word_at(source, at)
    } else {
        return None;
    })
}

/// The end of a long bracket `[[...]]` or `[==[...]==]` opened at `at`.
fn long_bracket(source: &str, at: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut level = at + 1;
    while bytes.get(level) == Some(&b'=') {
        level += 1;
    }
    if bytes.get(at) != Some(&b'[') || bytes.get(level) != Some(&b'[') {
        return None;
    }
    let close = format!("]{}]", "=".repeat(level - at - 1));
    let end = source[level..]
        .find(&close)
        .map(|found| level + found + close.len());
    Some(end.unwrap_or(source.len()))
}

/// The kind and the end of the name that starts at `at`.
fn word_at(source: &str, at: usize) -> (Kind, usize) {
    let bytes = source.as_bytes();
    let mut end = at + 1;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    let word = &source[at..end];
    let called = source[end..]
        .trim_start_matches([' ', '\t'])
        .starts_with(['(', '"', '{']);
    let kind = if KEYWORDS.contains(&word) {
        Kind::Keyword
    } else if CONSTANTS.contains(&word) {
        Kind::Constant
    } else if called {
        Kind::Call
    } else {
        Kind::Plain
    };
    (kind, end)
}

/// `source` laid out with each kind in its colour.
pub fn highlight(source: &str, font: &FontId, theme: &Theme) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (kind, text) in tokens(source) {
        let colour: Color32 = match kind {
            Kind::Plain => theme.text,
            Kind::Comment => theme.text_faint,
            Kind::Text => theme.terminal[2],
            Kind::Number => theme.terminal[3],
            Kind::Keyword => theme.terminal[5],
            Kind::Constant => theme.terminal[3],
            Kind::Call => theme.terminal[4],
        };
        let mut format = TextFormat::simple(font.clone(), colour);
        format.italics = kind == Kind::Comment;
        job.append(text, 0.0, format);
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<(Kind, &str)> {
        let tokens = tokens(source);
        // Nothing is lost or reordered, whatever the source.
        let joined: String = tokens.iter().map(|(_, text)| *text).collect();
        assert_eq!(joined, source);
        tokens
            .into_iter()
            .filter(|(kind, _)| *kind != Kind::Plain)
            .collect()
    }

    #[test]
    fn source_is_told_apart() {
        use Kind::*;
        let source =
            "local n = 42 -- answer\nif m[1] ~= \"you\" then send('hi ' .. name) end\nreturn nil";
        assert_eq!(
            kinds(source),
            [
                (Keyword, "local"),
                (Number, "42"),
                (Comment, "-- answer"),
                (Keyword, "if"),
                (Number, "1"),
                (Text, "\"you\""),
                (Keyword, "then"),
                (Call, "send"),
                (Text, "'hi '"),
                (Keyword, "end"),
                (Keyword, "return"),
                (Constant, "nil"),
            ]
        );
        assert_eq!(
            kinds("x = [[two\nlines]] --[==[ long\ncomment ]==] y"),
            [
                (Text, "[[two\nlines]]"),
                (Comment, "--[==[ long\ncomment ]==]")
            ]
        );
    }

    #[test]
    fn unfinished_source_is_coloured_as_far_as_it_goes() {
        use Kind::*;
        assert_eq!(kinds("echo(\"ñandú"), [(Call, "echo"), (Text, "\"ñandú")]);
        assert_eq!(kinds("s = \"a\\"), [(Text, "\"a\\")]);
        assert_eq!(kinds("--[[ never closed"), [(Comment, "--[[ never closed")]);
        assert_eq!(kinds("t[[1"), [(Text, "[[1")]);
        assert_eq!(kinds(""), []);
    }
}
