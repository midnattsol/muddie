//! Triggers, aliases and keys made in the interface instead of in a
//! script. They are kept in `rules.toml` and turned into Luau for the
//! script engine, so they behave exactly like their scripted equivalents.

use std::path::Path;

use serde::{Deserialize, Serialize};

mod kinds;
mod luau;
#[cfg(test)]
mod tests;

pub use kinds::*;

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
    #[serde(rename = "trigger")]
    pub triggers: Vec<Trigger>,
    #[serde(rename = "alias")]
    pub aliases: Vec<Alias>,
    #[serde(rename = "key")]
    pub keys: Vec<KeyBinding>,
    #[serde(rename = "timer")]
    pub timers: Vec<Timer>,
    #[serde(rename = "group")]
    pub groups: Vec<Group>,
    #[serde(rename = "gauge")]
    pub gauges: Vec<Gauge>,
    /// The scripts that are switched off, by [`script_key`]: they are
    /// kept, and not run.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scripts_off: Vec<String>,
}

/// What a script file is known by among those switched off: its name, or
/// for the one of every session, which is not in a `scripts` folder,
/// its name after `every/`.
pub fn script_key(file: &std::path::Path) -> String {
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    match file.parent().is_some_and(|dir| dir.ends_with("scripts")) {
        true => name.into_owned(),
        false => format!("every/{name}"),
    }
}

/// The commands of a rule's `send`: one per line, or separated by `;`.
pub fn commands(send: &str) -> impl Iterator<Item = &str> {
    send.split([';', '\n'])
        .map(str::trim)
        .filter(|command| !command.is_empty())
}

/// The commands of a rule's `send` as the Luau that sends them: `%1`...
/// become the match's groups if the rule has a match.
pub fn commands_as_lua(send: &str, matched: bool) -> String {
    let mut code = String::new();
    for command in commands(send) {
        let mut pieces: Vec<String> = Vec::new();
        let mut text = String::new();
        let mut chars = command.chars().peekable();
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
        code.push_str(&format!("send({})\n", pieces.join(" .. ")));
    }
    code
}

/// What every chunk of [`Rules::chunks`] starts with.
const PRELUDE: &str = include_str!("rules/prelude.luau");

impl Rules {
    /// Reads the rules file; a missing one means no rules.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let loaded: Result<Self, _> = toml::from_str(&text);
                let mut rules = loaded.map_err(|error| format!("{}: {error}", path.display()))?;
                rules.migrate();
                Ok(rules)
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
        std::fs::write(path, text)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    }

    /// Brings rules written by earlier versions up to date: a rule does
    /// its commands or its script, so one that had both becomes a script
    /// that starts by sending the commands.
    pub fn migrate(&mut self) {
        let merge = |send: &mut String, script: &mut String, lua: &mut bool, matched: bool| {
            if !*lua && !script.trim().is_empty() {
                *script = format!("{}{script}", commands_as_lua(send, matched));
                send.clear();
                *lua = true;
            }
        };
        for rule in &mut self.triggers {
            merge(&mut rule.send, &mut rule.script, &mut rule.lua, true);
        }
        for rule in &mut self.aliases {
            merge(&mut rule.send, &mut rule.script, &mut rule.lua, true);
        }
        for rule in &mut self.keys {
            merge(&mut rule.send, &mut rule.script, &mut rule.lua, false);
        }
        // Gauges had a switch for cells and one level to warn under.
        for gauge in &mut self.gauges {
            if std::mem::take(&mut gauge.segments) && gauge.kind == GaugeKind::Bar {
                gauge.kind = GaugeKind::Cells;
            }
            let low = std::mem::take(&mut gauge.low);
            if low > 0 && gauge.steps.is_empty() {
                gauge.steps.push(Step {
                    under: low,
                    colour: "red".to_owned(),
                });
            }
        }
    }

    /// The groups there are, in alphabetical order: those a rule is in
    /// and those with a state of their own.
    pub fn group_names(&self) -> Vec<String> {
        let rules = (self.triggers.iter().map(|rule| &rule.group))
            .chain(self.aliases.iter().map(|rule| &rule.group))
            .chain(self.keys.iter().map(|rule| &rule.group))
            .chain(self.timers.iter().map(|rule| &rule.group));
        let mut names: Vec<String> = rules
            .chain(self.groups.iter().map(|group| &group.name))
            .cloned()
            .collect();
        names.retain(|name| !name.trim().is_empty());
        names.sort_by_key(|name| name.to_lowercase());
        names.dedup();
        names
    }

    /// Whether a group starts a session switched on.
    pub fn group_enabled(&self, name: &str) -> bool {
        self.groups
            .iter()
            .find(|group| group.name == name)
            .is_none_or(|group| group.enabled)
    }

    /// Sets whether a group starts a session switched on.
    pub fn set_group_enabled(&mut self, name: &str, enabled: bool) {
        self.groups.retain(|group| group.name != name);
        // Only what differs from the usual is worth keeping.
        if !enabled {
            self.groups.push(Group {
                name: name.to_owned(),
                enabled,
            });
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

    /// The rules as Luau, one chunk per rule switched on, each with a name
    /// to report errors under; the groups that start switched off first.
    pub fn chunks(&self) -> Vec<(String, String)> {
        let mut chunks = Vec::new();
        let off: String = (self.groups.iter())
            .filter(|group| !group.enabled)
            .map(|group| format!("group.disable({})\n", quoted(group.name.trim())))
            .collect();
        if !off.is_empty() {
            chunks.push(("groups".to_owned(), off));
        }
        let on = |enabled: bool, given: &str| enabled && !given.trim().is_empty();
        chunks.extend(
            (self.triggers.iter())
                .filter(|rule| on(rule.enabled, &rule.pattern))
                .map(Trigger::chunk),
        );
        chunks.extend(
            (self.aliases.iter())
                .filter(|rule| on(rule.enabled, &rule.pattern))
                .map(Alias::chunk),
        );
        chunks.extend(
            (self.keys.iter())
                .filter(|rule| on(rule.enabled, &rule.key))
                .map(KeyBinding::chunk),
        );
        chunks.extend(
            (self.timers.iter())
                .filter(|rule| rule.enabled && rule.tenths > 0)
                .map(Timer::chunk),
        );
        chunks.extend(
            (self.gauges.iter())
                .filter(|rule| on(rule.enabled, &rule.value))
                .map(Gauge::chunk),
        );
        chunks
    }
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
