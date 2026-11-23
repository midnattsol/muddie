//! Triggers, aliases, macros, timers and gauges made in the editor
//! instead of in a script. They are kept in `rules.toml` and turned into Luau for the
//! script engine, so they behave exactly like their scripted equivalents.

use std::path::Path;

use serde::{Deserialize, Serialize};

mod kinds;
mod luau;
#[cfg(test)]
mod tests;

pub use kinds::*;
pub use luau::Luau;
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

    /// Puts a folder before or `after` another, among those in the same
    /// folder: the folders are shown in the order they are kept.
    pub fn place_folder(&mut self, kind: Kind, path: &str, beside: &str, after: bool) {
        // Every folder kept, those only implied by what is in them too, so
        // that each has a place in the order.
        for known in self.folder_paths(kind) {
            if !(self.folders.iter()).any(|folder| folder.kind == kind && folder.path == known) {
                self.set_folder_on(kind, &known, true);
            }
        }
        let at = |folders: &[Folder], path: &str| {
            (folders.iter()).position(|folder| folder.kind == kind && folder.path == path)
        };
        let Some(from) = at(&self.folders, path) else {
            return;
        };
        let folder = self.folders.remove(from);
        match at(&self.folders, beside) {
            Some(to) => self.folders.insert(to + usize::from(after), folder),
            None => self.folders.push(folder),
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
        let gauges = &self.gauges;
        chunks.extend(
            (0..gauges.len())
                .filter(|&index| gauges[index].complete())
                .map(|index| gauges[index].chunk(&RuleList::place(gauges, index))),
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

impl Rules {
    /// Takes in the rules `theirs` brings, of every kind, as a plugin
    /// installs them: what is there already is kept, or, `again`, put
    /// back as it comes; what is not is added. A rule is the same as one
    /// brought by its folder and its name; a gauge, as plugins installed
    /// them before they had folders, also one of its name in none, which
    /// goes in the folder.
    pub fn bring(&mut self, theirs: &Rules, again: bool) {
        bring(&mut self.triggers, &theirs.triggers, again, same_rule);
        bring(&mut self.aliases, &theirs.aliases, again, same_rule);
        bring(&mut self.macros, &theirs.macros, again, same_rule);
        bring(&mut self.timers, &theirs.timers, again, same_rule);
        bring(&mut self.gauges, &theirs.gauges, again, same_gauge);
    }

    /// Takes out a folder and all in it, of every kind: what a plugin
    /// brought, as it is removed.
    pub fn drop_folder(&mut self, top: &str) {
        let inside = |folder: &str| folder.split('/').next() == Some(top);
        for kind in Kind::ALL {
            if let Some(list) = self.list_mut(kind) {
                for index in (0..list.len()).rev() {
                    if inside(list.folder(index)) {
                        list.remove(index);
                    }
                }
            }
        }
        self.folders.retain(|folder| !inside(&folder.path));
    }

    /// Puts every rule in `folder`, or in the folder of that name inside it
    /// where it was in one: what a plugin brings, in a folder of its name.
    pub fn put_in(&mut self, folder: &str) {
        for kind in Kind::ALL {
            if let Some(list) = self.list_mut(kind) {
                for index in 0..list.len() {
                    let inner = list.folder(index).trim().to_owned();
                    let path = match inner.as_str() {
                        "" => folder.to_owned(),
                        inner => joined(folder, inner),
                    };
                    list.set_folder(index, path);
                }
            }
        }
    }

    /// Switches a folder on or off, of every kind.
    pub fn set_folder_on_everywhere(&mut self, path: &str, on: bool) {
        for kind in Kind::ALL {
            self.set_folder_on(kind, path, on);
        }
    }
}

/// Whether a rule is one a plugin brings: in the same folder, under the
/// same name.
fn same_rule<R: Ruled>(known: &R, theirs: &R) -> bool {
    known.title() == theirs.title() && known.folder().trim() == theirs.folder().trim()
}

/// Whether a gauge is one a plugin brings: in the same folder, under the
/// same name; or, as plugins installed them before they had folders, in
/// none.
pub fn same_gauge(known: &Gauge, theirs: &Gauge) -> bool {
    same_rule(known, theirs) || (known.folder.trim().is_empty() && known.title() == theirs.title())
}

/// Takes in the rules of one kind a plugin brings (see [`Rules::bring`]).
fn bring<R: Ruled + Clone>(
    mine: &mut Vec<R>,
    theirs: &[R],
    again: bool,
    same: impl Fn(&R, &R) -> bool,
) {
    for rule in theirs {
        match mine.iter().position(|known| same(known, rule)) {
            Some(at) if again => mine[at] = rule.clone(),
            Some(at) => *mine[at].head().folder = rule.folder().to_owned(),
            None => mine.push(rule.clone()),
        }
    }
}

/// One rule, whatever its kind: what [`RuleList`] needs of each.
pub trait Ruled {
    /// What it is called: its name, or else what it is made of.
    fn title(&self) -> String;
    fn head(&mut self) -> Head<'_>;
    fn is_on(&self) -> bool;
    fn folder(&self) -> &str;
    fn tags(&self) -> &[String];
    /// A new one, in a folder.
    fn new_in(folder: &str) -> Self;
}

impl<T: When> Ruled for Rule<T> {
    fn title(&self) -> String {
        Rule::title(self)
    }
    fn head(&mut self) -> Head<'_> {
        Head {
            name: &mut self.name,
            enabled: &mut self.enabled,
            folder: &mut self.folder,
            tags: &mut self.tags,
        }
    }
    fn is_on(&self) -> bool {
        self.enabled
    }
    fn folder(&self) -> &str {
        &self.folder
    }
    fn tags(&self) -> &[String] {
        &self.tags
    }
    fn new_in(folder: &str) -> Self {
        Rule {
            folder: folder.to_owned(),
            ..Rule::default()
        }
    }
}

impl Ruled for Gauge {
    fn title(&self) -> String {
        self.shown_name().to_owned()
    }
    fn head(&mut self) -> Head<'_> {
        Head {
            name: &mut self.name,
            enabled: &mut self.enabled,
            folder: &mut self.folder,
            tags: &mut self.tags,
        }
    }
    fn is_on(&self) -> bool {
        self.enabled
    }
    fn folder(&self) -> &str {
        &self.folder
    }
    fn tags(&self) -> &[String] {
        &self.tags
    }
    fn new_in(folder: &str) -> Self {
        Gauge {
            folder: folder.to_owned(),
            ..Gauge::starter()
        }
    }
}

impl<R: Ruled> RuleList for Vec<R> {
    fn len(&self) -> usize {
        <[R]>::len(self)
    }
    fn title(&self, index: usize) -> String {
        self[index].title()
    }
    /// Where it is among those of its kind, as `Folder/Title`, which is
    /// what it is switched by: one called as one before it in the same
    /// folder is told apart by a number, `Title #2`.
    fn place(&self, index: usize) -> String {
        let rule = &self[index];
        let (folder, title) = (Ruled::folder(rule).trim(), rule.title());
        let alike = (self[..index].iter())
            .filter(|other| Ruled::folder(*other).trim() == folder && other.title() == title)
            .count();
        match alike {
            0 => joined(folder, &title),
            n => joined(folder, &format!("{title} #{}", n + 1)),
        }
    }
    fn enabled(&self, index: usize) -> bool {
        self[index].is_on()
    }
    fn folder(&self, index: usize) -> &str {
        Ruled::folder(&self[index])
    }
    fn set_folder(&mut self, index: usize, folder: String) {
        *self[index].head().folder = folder;
    }
    fn tags(&self, index: usize) -> &[String] {
        Ruled::tags(&self[index])
    }
    fn head_mut(&mut self, index: usize) -> Head<'_> {
        self[index].head()
    }
    fn add(&mut self, folder: &str) -> usize {
        self.push(R::new_in(folder));
        <[R]>::len(self) - 1
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
