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
        std::fs::write(path, text)
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
            Kind::Script => None,
        }
    }

    pub fn list_mut(&mut self, kind: Kind) -> Option<&mut dyn RuleList> {
        match kind {
            Kind::Trigger => Some(&mut self.triggers),
            Kind::Alias => Some(&mut self.aliases),
            Kind::Macro => Some(&mut self.macros),
            Kind::Timer => Some(&mut self.timers),
            Kind::Script => None,
        }
    }

    /// What is saved switched off: folders, rules and scripts by their
    /// place, and tags.
    pub fn switched_off(&self) -> Switches {
        let mut switches = Switches::default();
        for folder in self.folders.iter().filter(|folder| !folder.enabled) {
            switches.set_place(folder.kind, &folder.path, false);
        }
        for kind in Kind::ALL {
            let Some(list) = self.list(kind) else {
                continue;
            };
            for index in (0..list.len()).filter(|&index| !list.enabled(index)) {
                switches.set_place(kind, &list.place(index), false);
            }
        }
        for script in self.scripts.iter().filter(|script| !script.enabled) {
            switches.set_place(Kind::Script, &script.path, false);
        }
        for tag in &self.tags_off {
            switches.set_tag(tag, false);
        }
        switches
    }

    /// The folders of a kind, in their order: those kept, then any a
    /// rule is in that is not, with those above it.
    pub fn folder_paths(&self, kind: Kind) -> Vec<String> {
        let mut paths: Vec<String> = (self.folders.iter())
            .filter(|folder| folder.kind == kind)
            .map(|folder| folder.path.clone())
            .collect();
        let held = self.list(kind).map_or_else(Vec::new, |list| {
            (0..list.len())
                .map(|index| list.folder(index).to_owned())
                .collect()
        });
        let mut implied: Vec<String> = (held.iter())
            .filter(|folder| !folder.trim().is_empty())
            .flat_map(|folder| {
                muddie_script::places_up(folder)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|path| !paths.contains(path))
            .collect();
        implied.sort();
        implied.dedup();
        paths.extend(implied);
        paths
    }

    /// Whether a folder is saved switched on, as one is unless it is not.
    pub fn folder_on(&self, kind: Kind, path: &str) -> bool {
        (self.folders.iter())
            .find(|folder| folder.kind == kind && folder.path == path)
            .is_none_or(|folder| folder.enabled)
    }

    /// Sets whether a folder is saved switched on, keeping it if it was not
    /// kept yet.
    pub fn set_folder_on(&mut self, kind: Kind, path: &str, on: bool) {
        match (self.folders.iter_mut()).find(|folder| folder.kind == kind && folder.path == path) {
            Some(folder) => folder.enabled = on,
            None => self.folders.push(Folder {
                kind,
                path: path.to_owned(),
                enabled: on,
            }),
        }
    }

    /// Gives a folder, with all it holds, another place: renamed, or put in
    /// another folder. Its rules and the folders in it go with it.
    pub fn move_folder(&mut self, kind: Kind, from: &str, to: &str) {
        let moved = |path: &str| match path.strip_prefix(from) {
            Some("") => Some(to.to_owned()),
            Some(rest) if rest.starts_with('/') => Some(format!("{to}{rest}")),
            _ => None,
        };
        for folder in self.folders.iter_mut().filter(|folder| folder.kind == kind) {
            if let Some(path) = moved(&folder.path) {
                folder.path = path;
            }
        }
        if let Some(list) = self.list_mut(kind) {
            for index in 0..list.len() {
                if let Some(path) = moved(list.folder(index)) {
                    list.set_folder(index, path);
                }
            }
        }
    }

    /// Takes a folder away, leaving what it held in the folder it was in.
    pub fn remove_folder(&mut self, kind: Kind, path: &str) {
        let parent = path
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent)
            .to_owned();
        self.folders
            .retain(|folder| folder.kind != kind || folder.path != path);
        let inner = |held: &str| -> Option<String> {
            let rest = held.strip_prefix(path)?;
            rest.starts_with('/').then(|| rest.to_owned())
        };
        for folder in self.folders.iter_mut().filter(|folder| folder.kind == kind) {
            if let Some(rest) = inner(&folder.path) {
                folder.path = joined(&parent, &rest[1..]);
            }
        }
        if let Some(list) = self.list_mut(kind) {
            for index in 0..list.len() {
                let held = list.folder(index).to_owned();
                if held == path {
                    list.set_folder(index, parent.clone());
                } else if let Some(rest) = inner(&held) {
                    list.set_folder(index, joined(&parent, &rest[1..]));
                }
            }
        }
    }

    /// The tags there are, each once however it is capitalised, in
    /// alphabetical order.
    pub fn tag_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        let carried = Kind::ALL
            .iter()
            .filter_map(|&kind| self.list(kind))
            .flat_map(|list| (0..list.len()).flat_map(|index| list.tags(index).to_vec()))
            .chain(self.scripts.iter().flat_map(|script| script.tags.clone()))
            .chain(self.tags_off.iter().cloned());
        for tag in carried {
            let key = tag_key(&tag);
            if !key.is_empty() && !names.iter().any(|known| tag_key(known) == key) {
                names.push(tag.trim().to_owned());
            }
        }
        names.sort_by_key(|name| tag_key(name));
        names
    }

    /// Whether a tag is saved switched on.
    pub fn tag_on(&self, tag: &str) -> bool {
        !self.tags_off.contains(&tag_key(tag))
    }

    pub fn set_tag_on(&mut self, tag: &str, on: bool) {
        let key = tag_key(tag);
        self.tags_off.retain(|known| *known != key);
        if !on {
            self.tags_off.push(key);
        }
    }

    /// How a script is kept: switched on and with no tags unless said.
    pub fn script(&self, path: &str) -> ScriptEntry {
        (self.scripts.iter())
            .find(|script| script.path == path)
            .cloned()
            .unwrap_or_else(|| ScriptEntry {
                path: path.to_owned(),
                enabled: true,
                tags: Vec::new(),
            })
    }

    /// Keeps how a script is: only what differs from the usual is kept.
    pub fn set_script(&mut self, entry: ScriptEntry) {
        self.scripts.retain(|script| script.path != entry.path);
        if !entry.enabled || !entry.tags.is_empty() {
            self.scripts.push(entry);
        }
    }

    /// Gives the scripts in a folder, or one script, another place, as
    /// their files were moved.
    pub fn move_scripts(&mut self, from: &str, to: &str) {
        for script in &mut self.scripts {
            match script.path.strip_prefix(from) {
                Some("") => script.path = to.to_owned(),
                Some(rest) if rest.starts_with('/') => script.path = format!("{to}{rest}"),
                _ => {}
            }
        }
        self.move_folder(Kind::Script, from, to);
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
        chunks.extend(
            (self.gauges.iter())
                .filter(|rule| rule.enabled && !rule.value.trim().is_empty())
                .map(Gauge::chunk),
        );
        chunks
    }
}

/// The order of `len` things once those at `moving` are taken out and put,
/// in the order they were, before what was at `to`; `len` puts them last.
pub fn moved_order(len: usize, moving: &[usize], to: usize) -> Vec<usize> {
    let mut moving: Vec<usize> = moving
        .iter()
        .copied()
        .filter(|&index| index < len)
        .collect();
    moving.sort_unstable();
    moving.dedup();
    let staying = (0..len).filter(|index| moving.binary_search(index).is_err());
    let before = (0..to.min(len))
        .filter(|index| moving.binary_search(index).is_err())
        .count();
    let mut order: Vec<usize> = staying.collect();
    order.splice(before..before, moving);
    order
}

/// Puts things in a new order: the one at `order[0]` first, and so on.
pub fn reorder<T>(items: &mut Vec<T>, order: &[usize]) {
    let mut taken: Vec<Option<T>> = std::mem::take(items).into_iter().map(Some).collect();
    items.extend(
        order
            .iter()
            .filter_map(|&index| taken.get_mut(index)?.take()),
    );
}

/// The chunks of the rules of one kind complete enough to run.
fn rule_chunks<T: luau::Luau>(rules: &Vec<Rule<T>>) -> Vec<(String, String)> {
    (0..rules.len())
        .filter(|&index| rules[index].when.complete())
        .map(|index| rules[index].chunk(&RuleList::place(rules, index)))
        .collect()
}

/// What every kind of rule has, to be changed in the editor.
pub struct Head<'a> {
    pub name: &'a mut String,
    pub enabled: &'a mut bool,
    pub folder: &'a mut String,
    pub tags: &'a mut Vec<String>,
}

/// The rules of one kind, whatever it is: what the editor does with all
/// of them alike.
pub trait RuleList {
    fn len(&self) -> usize;
    fn title(&self, index: usize) -> String;
    fn place(&self, index: usize) -> String;
    fn enabled(&self, index: usize) -> bool;
    fn folder(&self, index: usize) -> &str;
    fn set_folder(&mut self, index: usize, folder: String);
    fn tags(&self, index: usize) -> &[String];
    /// What every kind has, to be changed: its name, its switch, its
    /// folder and its tags.
    fn head_mut(&mut self, index: usize) -> Head<'_>;
    /// Adds a new one, in a folder; returns where it is.
    fn add(&mut self, folder: &str) -> usize;
    fn remove(&mut self, index: usize);
    /// Puts them in a new order: the one at `order[0]` first, and so on.
    fn reorder(&mut self, order: &[usize]);
}

impl<T: When> RuleList for Vec<Rule<T>> {
    fn len(&self) -> usize {
        <[Rule<T>]>::len(self)
    }
    fn title(&self, index: usize) -> String {
        self[index].title()
    }
    /// Where it is among those of its kind, as `Folder/Title`, which is
    /// what it is switched by: one called as one before it in the same
    /// folder is told apart by a number, `Title #2`.
    fn place(&self, index: usize) -> String {
        let rule = &self[index];
        let (folder, title) = (rule.folder.trim(), rule.title());
        let alike = (self[..index].iter())
            .filter(|other| other.folder.trim() == folder && other.title() == title)
            .count();
        match alike {
            0 => joined(folder, &title),
            n => joined(folder, &format!("{title} #{}", n + 1)),
        }
    }
    fn enabled(&self, index: usize) -> bool {
        self[index].enabled
    }
    fn folder(&self, index: usize) -> &str {
        &self[index].folder
    }
    fn set_folder(&mut self, index: usize, folder: String) {
        self[index].folder = folder;
    }
    fn tags(&self, index: usize) -> &[String] {
        &self[index].tags
    }
    fn head_mut(&mut self, index: usize) -> Head<'_> {
        let rule = &mut self[index];
        Head {
            name: &mut rule.name,
            enabled: &mut rule.enabled,
            folder: &mut rule.folder,
            tags: &mut rule.tags,
        }
    }
    fn add(&mut self, folder: &str) -> usize {
        self.push(Rule {
            folder: folder.to_owned(),
            ..Rule::default()
        });
        <[Rule<T>]>::len(self) - 1
    }
    fn remove(&mut self, index: usize) {
        Vec::remove(self, index);
    }
    fn reorder(&mut self, order: &[usize]) {
        reorder(self, order);
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
