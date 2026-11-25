//! Saying more of a mistake than Luau does: of a call to what there is not,
//! which it says is of a nil value, what was called.

use super::*;

/// Where the code of what is run is kept, to be looked at.
#[derive(Default)]
pub(super) struct Sources(HashMap<(String, String), String>);

impl Sources {
    /// Keeps the code that `owner` runs, told in its mistakes as `name`.
    pub(super) fn keep(&mut self, owner: &str, name: &str, code: &str) {
        (self.0).insert((owner.to_owned(), name.to_owned()), code.to_owned());
    }

    pub(super) fn forget(&mut self, owner: &str) {
        self.0.retain(|(known, _), _| known != owner);
    }

    fn line(&self, owner: &str, name: &str, line: usize) -> Option<&str> {
        let code = |owner: &str| self.0.get(&(owner.to_owned(), name.to_owned()));
        // A module is of no script.
        (code(owner).or_else(|| code("")))?
            .lines()
            .nth(line.checked_sub(1)?)
    }
}

/// What Luau says of a call to nothing.
const NIL_CALL: &str = "attempt to call a nil value";

impl Engine {
    /// A mistake, with what was called if it was a call to what there is not.
    pub(super) fn explain(&self, message: String) -> String {
        self.called_nothing(&message).unwrap_or(message)
    }

    fn called_nothing(&self, message: &str) -> Option<String> {
        let place = message
            .strip_suffix(NIL_CALL)?
            .trim_end()
            .strip_suffix(':')?;
        let (name, line) = place.rsplit_once(':')?;
        let state = self.state.borrow();
        let code = state.sources.line(&state.owner, name, line.parse().ok()?)?;
        let calls = calls_in(code);
        drop(state);
        let mut nothing: Vec<&String> =
            (calls.iter()).filter(|call| !self.is_there(call)).collect();
        nothing.dedup();
        match nothing.as_slice() {
            [call] => Some(format!("{place}: there is no function {call}")),
            _ => None,
        }
    }

    /// Whether there is something of this name, or path of names.
    fn is_there(&self, path: &str) -> bool {
        let mut value = Value::Table(self.lua.globals());
        for name in path.split(['.', ':']) {
            value = match value {
                Value::Table(table) => table.get(name).unwrap_or(Value::Nil),
                Value::String(_) => return true,
                _ => return false,
            };
        }
        !value.is_nil()
    }
}

/// What a line of Luau calls, by name or path: `send`, `gauges.set`.
fn calls_in(line: &str) -> Vec<String> {
    const WORDS: [&str; 9] = [
        "and", "or", "not", "if", "elseif", "while", "until", "return", "in",
    ];
    let code = without_texts(line);
    let is_name = |ch: char| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == ':';
    let mut calls = Vec::new();
    for (at, _) in code.match_indices(['(', '"', '{']) {
        let before = code[..at].trim_end();
        let start = before.rfind(|ch| !is_name(ch)).map_or(0, |at| at + 1);
        let name = &before[start..];
        let defined = before[..start].trim_end().ends_with("function");
        let plain = name.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_');
        if plain && !defined && !WORDS.contains(&name) && name != "function" {
            calls.push(name.to_owned());
        }
    }
    calls
}

/// A line of Luau with nothing in its texts, and without its comment.
fn without_texts(line: &str) -> String {
    let mut code = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '-' if chars.peek() == Some(&'-') => break,
            '"' | '\'' | '`' => {
                code.push('"');
                while let Some(inside) = chars.next() {
                    match inside {
                        '\\' => drop(chars.next()),
                        inside if inside == ch => break,
                        _ => {}
                    }
                }
                code.push(')');
            }
            ch => code.push(ch),
        }
    }
    code
}
