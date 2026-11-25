//! The rows of the list of rules: what a row is, how each kind of rule becomes
//! one, and which rows are wanted.

use super::*;

/// What the list shows of one thing of a section.
pub(in crate::tab) struct Row {
    pub(in crate::tab) title: String,
    /// What it does in a line, in words: what a search looks in, and what
    /// a screen reader is told.
    pub(in crate::tab) detail: String,
    /// The same, part by part, each with its picture, as it is shown.
    pub(in crate::tab) parts: Vec<(widgets::Glyph, String)>,
    /// Its own switch.
    pub(in crate::tab) on: bool,
    /// The folder it is in; empty for none.
    pub(in crate::tab) folder: String,
    pub(in crate::tab) tags: Vec<String>,
    /// It failed when last run.
    pub(in crate::tab) problem: bool,
}

/// A line of the list: a thing, by its place in the section, or a folder,
/// by its path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(in crate::tab) enum Spot {
    Row(usize),
    Folder(String),
}

impl Tab {
    /// What the list shows of each thing of the current section, in order.
    pub(in crate::tab) fn automation_rows(&self) -> Vec<Row> {
        let (draft, failed) = (&self.editor.draft, &self.editor.failed);
        match self.editor.section {
            TRIGGERS => rule_rows(&draft.triggers, failed, crate::rules::Rule::summary),
            ALIASES => rule_rows(&draft.aliases, failed, |rule| {
                rule.does().into_iter().collect()
            }),
            MACROS => rule_rows(&draft.macros, failed, |rule| {
                let key = (Part::Key, rule.when.key.trim().to_uppercase());
                std::iter::once(key).chain(rule.does()).collect()
            }),
            TIMERS => rule_rows(&draft.timers, failed, |rule| {
                let how = if rule.when.repeat {
                    tr("every")
                } else {
                    tr("after")
                };
                let time = (Part::Time, format!("{how} {}", rule.when.interval()));
                std::iter::once(time).chain(rule.does()).collect()
            }),
            GAUGES => (draft.gauges.iter())
                .map(|rule| gauge_row(rule, failed))
                .collect(),
            VARIABLES => (self.scripts.variables().into_iter())
                .map(variable_row)
                .collect(),
            _ => (self.editor.script_files.iter())
                .map(|file| self.script_row(file))
                .collect(),
        }
    }

    /// A script: its file's name, where it runs, and how it is kept.
    fn script_row(&self, file: &std::path::Path) -> Row {
        let place = self.script_place(file);
        let kept = self.editor.draft.script(&place);
        let detail = match self.is_own_script(file) {
            true => tr("In this profile"),
            false => tr("In every session"),
        };
        Row {
            title: title_of(file),
            detail: detail.to_owned(),
            parts: Vec::new(),
            on: kept.enabled,
            folder: place
                .rsplit_once('/')
                .map_or("", |(folder, _)| folder)
                .to_owned(),
            tags: kept.tags,
            problem: self.editor.failed.contains(&title_of(file)),
        }
    }

    /// Whether a row is shown, by what is searched for and the tag chosen.
    pub(super) fn wanted(&self, row: &Row) -> bool {
        let filter = self.editor.filter.trim().to_lowercase();
        let found = filter.is_empty()
            || row.title.to_lowercase().contains(&filter)
            || row.detail.to_lowercase().contains(&filter)
            || row
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&filter));
        let tagged = (self.tag_filter())
            .is_none_or(|wanted| row.tags.iter().any(|tag| tag_key(tag) == tag_key(wanted)));
        found && tagged
    }

    /// The tag the list is narrowed to, where things carry tags.
    fn tag_filter(&self) -> Option<&str> {
        let tagged = self.editor.section.kind().is_some();
        self.editor.tag_filter.as_deref().filter(|_| tagged)
    }

    /// Whether the list is narrowed, so that what is in it cannot be put
    /// in order by dragging.
    pub(in crate::tab) fn narrowed(&self) -> bool {
        !self.editor.filter.trim().is_empty() || self.tag_filter().is_some()
    }
}

/// A part of what a rule does as it is shown: its picture, and its words.
fn shown_part(part: Part, text: String) -> (widgets::Glyph, String) {
    use widgets::Glyph;
    match part {
        Part::Send => (Glyph::Send, text),
        Part::Script => (Glyph::Code, text),
        Part::Speak => (Glyph::Say, text),
        Part::Sound => (Glyph::Sound, text),
        Part::Highlight => (Glyph::Colour, text),
        Part::Panel => (Glyph::Panel, text),
        Part::Gag => (Glyph::Hide, tr("hides the line").to_owned()),
        Part::Silence => (Glyph::Say, tr("does not read it aloud").to_owned()),
        Part::Key => (Glyph::Key, text),
        Part::Time => (Glyph::Timer, text),
    }
}

/// What a rule is called in the list, and wherever it is named: its name,
/// or what sets it off, or that it is new.
pub(in crate::tab) fn rule_title<T: crate::rules::When>(
    rules: &Vec<crate::rules::Rule<T>>,
    index: usize,
) -> String {
    match RuleList::title(rules, index) {
        title if title.is_empty() => tr(T::NEW).to_owned(),
        title => title,
    }
}

/// The rows of rules of one kind, with what each does in a line.
fn rule_rows<T: crate::rules::Luau>(
    rules: &Vec<crate::rules::Rule<T>>,
    failed: &[String],
    detail: impl Fn(&crate::rules::Rule<T>) -> Vec<(Part, String)>,
) -> Vec<Row> {
    (0..rules.len())
        .map(|index| {
            let title = rule_title(rules, index);
            let rule = &rules[index];
            let parts: Vec<(widgets::Glyph, String)> = (detail(rule).into_iter())
                .filter(|(part, text)| {
                    matches!(part, Part::Gag | Part::Silence) || !text.is_empty()
                })
                .map(|(part, text)| shown_part(part, text))
                .collect();
            Row {
                title,
                detail: (parts.iter())
                    .map(|(_, text)| text.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                parts,
                on: rule.enabled,
                folder: rule.folder.trim().to_owned(),
                tags: rule.tags.clone(),
                // Its errors say they are its by what it is called in them.
                problem: failed.contains(&rule.reported_as()),
            }
        })
        .collect()
}

fn gauge_row(rule: &crate::rules::Gauge, failed: &[String]) -> Row {
    let kind = crate::rules::KindName::shown(rule.kind);
    let title = match rule.shown_name() {
        "" => tr("New gauge").to_owned(),
        name => name.to_owned(),
    };
    Row {
        title,
        detail: format!("{kind}  ·  {}", rule.value.trim()),
        parts: Vec::new(),
        on: rule.enabled,
        folder: rule.folder.trim().to_owned(),
        tags: rule.tags.clone(),
        problem: failed.contains(&rule.reported_as()),
    }
}

fn variable_row((name, value): (String, serde_json::Value)) -> Row {
    let held = shown_value(&value).replace('\n', " ");
    Row {
        title: name,
        detail: format!("{}  ·  {held}", TYPES[kind_of(&value)]),
        parts: Vec::new(),
        on: true,
        folder: String::new(),
        tags: Vec::new(),
        problem: false,
    }
}

/// What a script is called in the list, and in what its errors say: its
/// file's name.
pub(in crate::tab) fn title_of(file: &std::path::Path) -> String {
    (file.file_name().unwrap_or_default().to_string_lossy()).into_owned()
}
