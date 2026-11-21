//! Colouring of what is typed in the editor besides Luau: regular
//! expressions, the commands a rule sends, and JSON. Each is split into
//! stretches that follow one another and cover the whole text, so that
//! nothing typed is lost or moved, finished or not.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontId};

use crate::theme::Theme;

/// What a stretch of text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Plain,
    /// `(` and `)` of a group, `[` and `]` of a class.
    Bracket,
    /// `\w`, `\d`, `\.`: a character said another way.
    Escape,
    /// `^`, `$`, `\b`: a place, not a character.
    Anchor,
    /// `*`, `+`, `?`, `{2,3}`, and `|`.
    Repeat,
    /// `%1`, `$name`: what is put in when it is sent.
    Placeholder,
    /// `;`, which separates commands on a line.
    Separator,
    Text,
    Number,
    /// `true`, `false`, `null`.
    Constant,
    /// The name of a field of an object.
    Key,
}

/// The stretches of a regular expression.
pub fn regex_tokens(source: &str) -> Vec<(Kind, &str)> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();
    let mut in_class = false;
    while let Some((start, c)) = chars.next() {
        let mut end = start + c.len_utf8();
        let kind = match c {
            '\\' => {
                if let Some((at, next)) = chars.next() {
                    end = at + next.len_utf8();
                    match next {
                        'b' | 'B' | 'A' | 'z' => Kind::Anchor,
                        _ => Kind::Escape,
                    }
                } else {
                    Kind::Escape
                }
            }
            '[' if !in_class => {
                in_class = true;
                Kind::Bracket
            }
            ']' if in_class => {
                in_class = false;
                Kind::Bracket
            }
            _ if in_class => Kind::Plain,
            '(' => {
                // `(?:`, `(?P<name>`, `(?i)`: the opening says how.
                if source[end..].starts_with('?') {
                    let rest = &source[end..];
                    let close = rest.find([':', ')', '>']).map_or(rest.len(), |at| at + 1);
                    for _ in rest[..close].chars() {
                        chars.next();
                    }
                    end += close;
                }
                Kind::Bracket
            }
            ')' => Kind::Bracket,
            '^' | '$' => Kind::Anchor,
            '*' | '+' | '?' | '|' => Kind::Repeat,
            '{' => {
                let rest = &source[end..];
                match rest.find('}') {
                    Some(close)
                        if rest[..close]
                            .chars()
                            .all(|c| c.is_ascii_digit() || c == ',') =>
                    {
                        for _ in rest[..=close].chars() {
                            chars.next();
                        }
                        end += close + 1;
                        Kind::Repeat
                    }
                    _ => Kind::Plain,
                }
            }
            _ => Kind::Plain,
        };
        push(&mut tokens, kind, &source[start..end]);
    }
    tokens
}

/// The stretches of the commands a rule sends: what is put in when it is
/// sent stands out, and so does what separates commands.
pub fn command_tokens(source: &str) -> Vec<(Kind, &str)> {
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < source.len() {
        let rest = &source[at..];
        let c = rest.chars().next().unwrap_or(' ');
        let taken = match c {
            '%' if rest[1..].starts_with(|c: char| c.is_ascii_digit()) => {
                let digits = rest[1..].chars().take_while(char::is_ascii_digit).count();
                Some((Kind::Placeholder, 1 + digits))
            }
            '$' if rest[1..].starts_with(|c: char| c.is_alphanumeric() || c == '_') => {
                let name = rest[1..]
                    .char_indices()
                    .take_while(|(_, c)| c.is_alphanumeric() || matches!(c, '_' | '.'))
                    .last()
                    .map_or(0, |(i, c)| i + c.len_utf8());
                Some((Kind::Placeholder, 1 + name))
            }
            ';' => Some((Kind::Separator, 1)),
            _ => None,
        };
        match taken {
            Some((kind, len)) => {
                push(&mut tokens, kind, &source[at..at + len]);
                at += len;
            }
            None => {
                let len = c.len_utf8();
                push(&mut tokens, Kind::Plain, &source[at..at + len]);
                at += len;
            }
        }
    }
    tokens
}

/// The stretches of JSON: strings, a string that names a field, numbers and
/// constants.
pub fn json_tokens(source: &str) -> Vec<(Kind, &str)> {
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < source.len() {
        let rest = &source[at..];
        let c = rest.chars().next().unwrap_or(' ');
        let (kind, len) = match c {
            '"' => {
                let mut escaped = false;
                let close = rest[1..]
                    .char_indices()
                    .find(|(_, c)| {
                        let end = !escaped && *c == '"';
                        escaped = !escaped && *c == '\\';
                        end
                    })
                    .map_or(rest.len(), |(i, _)| i + 2);
                // A string followed by `:` names a field.
                let after = rest[close..].trim_start();
                let kind = if after.starts_with(':') {
                    Kind::Key
                } else {
                    Kind::Text
                };
                (kind, close)
            }
            '-' | '0'..='9' => {
                let len = rest
                    .char_indices()
                    .take_while(|(i, c)| {
                        c.is_ascii_digit()
                            || matches!(c, '.' | 'e' | 'E' | '+')
                            || (*i == 0 && *c == '-')
                    })
                    .last()
                    .map_or(1, |(i, c)| i + c.len_utf8());
                (Kind::Number, len)
            }
            _ if rest.starts_with("true") => (Kind::Constant, 4),
            _ if rest.starts_with("false") => (Kind::Constant, 5),
            _ if rest.starts_with("null") => (Kind::Constant, 4),
            _ => (Kind::Plain, c.len_utf8()),
        };
        push(&mut tokens, kind, &source[at..at + len]);
        at += len;
    }
    tokens
}

/// Adds a stretch, unless it is empty.
fn push<'a>(tokens: &mut Vec<(Kind, &'a str)>, kind: Kind, text: &'a str) {
    if !text.is_empty() {
        tokens.push((kind, text));
    }
}

/// The colour of a kind of stretch.
fn colour(kind: Kind, theme: &Theme) -> Color32 {
    match kind {
        Kind::Plain => theme.text,
        Kind::Bracket => theme.terminal[4],
        Kind::Escape => theme.terminal[6],
        Kind::Anchor => theme.terminal[5],
        Kind::Repeat => theme.terminal[3],
        Kind::Placeholder => theme.terminal[4],
        Kind::Separator => theme.text_faint,
        Kind::Text => theme.terminal[2],
        Kind::Number => theme.terminal[3],
        Kind::Constant => theme.terminal[5],
        Kind::Key => theme.terminal[4],
    }
}

/// Lays out stretches in their colours.
pub fn layout(tokens: &[(Kind, &str)], font: &FontId, theme: &Theme) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (kind, text) in tokens {
        job.append(
            text,
            0.0,
            TextFormat::simple(font.clone(), colour(*kind, theme)),
        );
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whole(tokens: &[(Kind, &str)]) -> String {
        tokens.iter().map(|(_, text)| *text).collect()
    }

    fn marked<'a>(tokens: &[(Kind, &'a str)]) -> Vec<(Kind, &'a str)> {
        tokens
            .iter()
            .filter(|(kind, _)| *kind != Kind::Plain)
            .copied()
            .collect()
    }

    #[test]
    fn a_regular_expression_is_coloured_by_what_each_part_does() {
        let source = r"^(\w+) attacks (?:you|me)\.{2,3}[a-z]*$";
        let tokens = regex_tokens(source);
        assert_eq!(whole(&tokens), source);
        assert_eq!(
            marked(&tokens),
            [
                (Kind::Anchor, "^"),
                (Kind::Bracket, "("),
                (Kind::Escape, r"\w"),
                (Kind::Repeat, "+"),
                (Kind::Bracket, ")"),
                (Kind::Bracket, "(?:"),
                (Kind::Repeat, "|"),
                (Kind::Bracket, ")"),
                (Kind::Escape, r"\."),
                (Kind::Repeat, "{2,3}"),
                (Kind::Bracket, "["),
                (Kind::Bracket, "]"),
                (Kind::Repeat, "*"),
                (Kind::Anchor, "$"),
            ]
        );
        // Unfinished, as while it is typed, nothing is lost.
        for source in [r"\", "(", "(?", "[abc", "a{2", "{", "ñ(\\é)"] {
            assert_eq!(whole(&regex_tokens(source)), source);
        }
    }

    #[test]
    fn what_is_put_in_a_command_stands_out() {
        let source = "kill %1; say $target.name hi %";
        let tokens = command_tokens(source);
        assert_eq!(whole(&tokens), source);
        assert_eq!(
            marked(&tokens),
            [
                (Kind::Placeholder, "%1"),
                (Kind::Separator, ";"),
                (Kind::Placeholder, "$target.name"),
            ]
        );
    }

    #[test]
    fn json_is_coloured_by_what_each_value_is() {
        let source = r#"{"hp": 80, "name": "Ana \"la\"", "ok": true, "x": -1.5e3, "n": null}"#;
        let tokens = json_tokens(source);
        assert_eq!(whole(&tokens), source);
        assert_eq!(
            marked(&tokens),
            [
                (Kind::Key, r#""hp""#),
                (Kind::Number, "80"),
                (Kind::Key, r#""name""#),
                (Kind::Text, r#""Ana \"la\"""#),
                (Kind::Key, r#""ok""#),
                (Kind::Constant, "true"),
                (Kind::Key, r#""x""#),
                (Kind::Number, "-1.5e3"),
                (Kind::Key, r#""n""#),
                (Kind::Constant, "null"),
            ]
        );
        for source in [r#"{"a"#, r#""\"#, "-", "tru"] {
            assert_eq!(whole(&json_tokens(source)), source);
        }
    }
}
