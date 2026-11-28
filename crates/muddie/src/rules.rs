//! Triggers, aliases, macros, timers and gauges made in the editor
//! instead of in a script. They are kept in `rules.toml` and turned into Luau for the
//! script engine, so they behave exactly like their scripted equivalents.

use std::path::Path;

use serde::{Deserialize, Serialize};

mod folders;
mod kinds;
mod list;
mod luau;
mod merging;
#[cfg(test)]
mod tests;

pub use kinds::*;
pub use list::{RuleList, Ruled, moved_order};
pub use luau::Luau;
pub use merging::same_gauge;
pub use muddie_script::{Kind, Switches, tag_key};

/// Puts a match's groups into a template: `%1`... become the groups.
pub fn fill(template: &str, captures: &regex::Captures) -> String {
    let mut filled = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match (ch, chars.peek().and_then(|next| next.to_digit(10))) {
            ('%', Some(group)) => {
                chars.next();
                filled.push_str(captures.get(group as usize).map_or("", |m| m.as_str()));
            }
            _ => filled.push(ch),
        }
    }
    filled
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    /// The tags switched off, as they are compared.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags_off: Vec<String>,
    #[serde(rename = "trigger")]
    pub triggers: Vec<Trigger>,
    #[serde(rename = "alias")]
    pub aliases: Vec<Alias>,
    #[serde(rename = "macro")]
    pub macros: Vec<Macro>,
    #[serde(rename = "timer")]
    pub timers: Vec<Timer>,
    #[serde(rename = "gauge")]
    pub gauges: Vec<Gauge>,
    /// The folders of each kind, in their order.
    #[serde(rename = "folder", skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<Folder>,
    /// The player's scripts that are switched off or carry tags.
    #[serde(rename = "script", skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<ScriptEntry>,
}

/// The commands of a rule's `send`: one per line, or separated by `;`,
/// as typed ones are: `;;` stands for a `;`.
pub fn commands(send: &str) -> Vec<String> {
    (send.lines())
        .flat_map(|line| muddie_script::split_commands(line, Some(";")))
        .filter(|command| !command.is_empty())
        .collect()
}

/// The commands of a rule's `send` as the Luau that sends them, which is
/// what the rule runs and what switching it to Luau starts from: `%1`...
/// become the match's groups if the rule has a match, and `$name` the
/// variable's value.
pub fn commands_as_luau(send: &str, matched: bool) -> String {
    (commands(send).iter())
        .map(|command| format!("send({})\n", filled_luau(command, matched)))
        .collect()
}

/// A template as the Luau that makes it: `%1`... the match's groups if
/// there is a match, `$name` the variable's value. What a rule sends and
/// what it says are made of it.
pub fn filled_luau(template: &str, matched: bool) -> String {
    let mut pieces: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match (ch, chars.peek().and_then(|next| next.to_digit(10))) {
            ('%', Some(group)) if matched => {
                chars.next();
                if !text.is_empty() {
                    pieces.push(quoted(&std::mem::take(&mut text)));
                }
                pieces.push(format!("(m[{group}] or \"\")"));
            }
            _ => text.push(ch),
        }
    }
    if !text.is_empty() || pieces.is_empty() {
        pieces.push(quoted(&text));
    }
    format!("expand({})", pieces.join(" .. "))
}

/// What every chunk of [`Rules::chunks`] starts with.
const PRELUDE: &str = include_str!("rules/prelude.luau");

impl Rules {
    /// Reads the rules file; a missing one means no rules.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("cannot read {}: {error}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|error| error.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        }
        crate::whole::write(path, text)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    }

    /// The rules of a kind, to go through and change whatever their kind;
    /// scripts are files, not rules.
    pub fn list(&self, kind: Kind) -> Option<&dyn RuleList> {
        match kind {
            Kind::Trigger => Some(&self.triggers),
            Kind::Alias => Some(&self.aliases),
            Kind::Macro => Some(&self.macros),
            Kind::Timer => Some(&self.timers),
            Kind::Gauge => Some(&self.gauges),
            Kind::Script => None,
        }
    }

    pub fn list_mut(&mut self, kind: Kind) -> Option<&mut dyn RuleList> {
        match kind {
            Kind::Trigger => Some(&mut self.triggers),
            Kind::Alias => Some(&mut self.aliases),
            Kind::Macro => Some(&mut self.macros),
            Kind::Timer => Some(&mut self.timers),
            Kind::Gauge => Some(&mut self.gauges),
            Kind::Script => None,
        }
    }

    /// The rules as one Luau script.
    #[cfg(test)]
    pub fn to_luau(&self) -> String {
        let chunks = self
            .chunks()
            .into_iter()
            .map(|(_, code)| format!("do\n{code}end\n"));
        chunks.collect()
    }

    /// The rules as Luau, one chunk per rule complete enough to run, each
    /// with a name to report errors under. Those switched off are made all
    /// the same, and switched off: a script can switch them on.
    pub fn chunks(&self) -> Vec<(String, String)> {
        let mut chunks: Vec<(String, String)> = Vec::new();
        chunks.extend(rule_chunks(&self.triggers));
        chunks.extend(rule_chunks(&self.aliases));
        chunks.extend(rule_chunks(&self.macros));
        chunks.extend(rule_chunks(&self.timers));
        let gauges = &self.gauges;
        chunks.extend(
            (0..gauges.len())
                .filter(|&index| gauges[index].complete())
                .map(|index| gauges[index].chunk(&RuleList::place(gauges, index))),
        );
        chunks
    }
}

/// The chunks of the rules of one kind complete enough to run.
fn rule_chunks<T: luau::Luau>(rules: &Vec<Rule<T>>) -> Vec<(String, String)> {
    (0..rules.len())
        .filter(|&index| rules[index].when.complete())
        .map(|index| rules[index].chunk(&RuleList::place(rules, index)))
        .collect()
}

/// `text` as a Luau string literal.
fn quoted(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('"');
    for ch in text.chars() {
        match ch {
            '"' => literal.push_str("\\\""),
            '\\' => literal.push_str("\\\\"),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            ch if ch.is_control() => literal.push_str(&format!("\\u{{{:x}}}", ch as u32)),
            ch => literal.push(ch),
        }
    }
    literal.push('"');
    literal
}

/// `text` as a regular expression matching it literally.
fn regex_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}
