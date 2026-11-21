//! Undoing and redoing what is changed in the rules in the editor: every
//! change is a step, but typing in one field without stopping is one.

use super::*;
use crate::rules::Rules;
use std::time::Duration;

/// How many steps are kept to go back.
const MOST_STEPS: usize = 100;

/// Typing in one field with no longer a pause than this is one step.
const TYPING: Duration = Duration::from_millis(1500);

/// The rules as they were before a change, and where the editor was.
pub(in crate::tab) struct Step {
    rules: Rules,
    section: usize,
    selected: usize,
    folder: Option<String>,
    /// The field typed in, when the change was typing: what is typed in it
    /// next, soon enough, is the same step.
    typing: Option<egui::Id>,
    at: Instant,
}

/// The steps to undo, newest last, and those undone, to redo.
#[derive(Default)]
pub(in crate::tab) struct History {
    past: Vec<Step>,
    future: Vec<Step>,
}

impl History {
    /// How many steps there are to undo.
    pub(in crate::tab) fn len(&self) -> usize {
        self.past.len()
    }

    /// Forgets every step: the rules edited are others.
    pub(in crate::tab) fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
    }
}

impl Tab {
    /// Keeps the rules as they were before a change, as a step to undo; a
    /// change made by typing on in the same field joins the step before.
    /// What was undone can no longer be redone.
    pub(in crate::tab) fn record_step(&mut self, before: Rules, typing: Option<egui::Id>) {
        let history = &mut self.editor.history;
        history.future.clear();
        if let Some(last) = history.past.last_mut()
            && typing.is_some()
            && last.typing == typing
            && last.at.elapsed() < TYPING
        {
            last.at = Instant::now();
            return;
        }
        history.past.push(Step {
            rules: before,
            section: self.editor.section,
            selected: self.editor.selected[self.editor.section],
            folder: self.editor.folder.clone(),
            typing,
            at: Instant::now(),
        });
        if history.past.len() > MOST_STEPS {
            history.past.remove(0);
        }
        // A deletion's bar is about the deletion: once something else is
        // done, Undo would undo that, and the bar goes.
        if let Some(undo) = &self.editor.undo
            && let undo::Undone::Step(steps) = undo.what
            && history.past.len() > steps
        {
            self.editor.undo = None;
        }
    }

    /// Undoes the last step, or redoes the last undone (`again`): the rules
    /// are as they were, what changed is chosen, and it is said.
    pub(in crate::tab) fn undo_step(&mut self, again: bool) {
        let history = &mut self.editor.history;
        let Some(step) = (match again {
            true => history.future.pop(),
            false => history.past.pop(),
        }) else {
            self.voice.say_now(match again {
                true => tr("Nothing to redo."),
                false => tr("Nothing to undo."),
            });
            return;
        };
        let now = std::mem::replace(&mut self.editor.draft, step.rules);
        let changed = changed_rule(&now, &self.editor.draft);
        let folder = folder_back(&now, &self.editor.draft);
        let back = Step {
            rules: now,
            section: self.editor.section,
            selected: self.editor.selected[self.editor.section],
            folder: self.editor.folder.clone(),
            typing: None,
            at: Instant::now(),
        };
        let history = &mut self.editor.history;
        match again {
            true => history.past.push(back),
            false => history.future.push(back),
        }
        // Not a change of its own: the autosave sees it as known.
        self.editor.seen = self.editor.draft.clone();
        self.editor.settle.changed();
        self.editor.undo = None;
        // What changed is chosen, to be seen; else where the editor was.
        self.editor.marked.clear();
        let title = match (folder, changed) {
            // A folder had back is what is chosen, not what is in it.
            (Some((section, path)), _) => {
                self.editor.section = section;
                self.choose_folder(&path);
                Some(name_of(&path).to_owned())
            }
            (None, Some((section, index))) => {
                self.editor.section = section;
                self.editor.folder = None;
                let len = self.section_len();
                self.editor.selected[section] = index.min(len.saturating_sub(1));
                self.automation_rows()
                    .get(self.editor.selected[section])
                    .map(|row| row.title.clone())
            }
            (None, None) => {
                self.editor.section = step.section;
                self.editor.selected[step.section] = step.selected;
                self.editor.folder = step.folder;
                None
            }
        };
        self.editor.reveal = true;
        let said = match (again, title) {
            (false, Some(title)) => trf("Undone: {0}.", &[&title]),
            (true, Some(title)) => trf("Redone: {0}.", &[&title]),
            (false, None) => tr("Undone.").to_owned(),
            (true, None) => tr("Redone.").to_owned(),
        };
        self.voice.say_now(&said);
    }
}

/// A folder there is in `b` and not in `a`, with the section it is of.
fn folder_back(a: &Rules, b: &Rules) -> Option<(usize, String)> {
    [TRIGGERS, ALIASES, MACROS, TIMERS]
        .into_iter()
        .find_map(|section| {
            let kind = section_kind(section)?;
            let known = a.folder_paths(kind);
            let path = b
                .folder_paths(kind)
                .into_iter()
                .find(|path| !known.contains(path))?;
            Some((section, path))
        })
}

/// The section and the place of the first rule that differs between two
/// versions of the rules, if one does: one added or taken away, or
/// changed.
fn changed_rule(a: &Rules, b: &Rules) -> Option<(usize, usize)> {
    fn first<T: PartialEq>(a: &[T], b: &[T]) -> Option<usize> {
        let differs = (0..a.len().min(b.len())).find(|&index| a[index] != b[index]);
        match differs {
            Some(index) => Some(index),
            None if a.len() != b.len() => Some(a.len().min(b.len())),
            None => None,
        }
    }
    [
        (TRIGGERS, first(&a.triggers, &b.triggers)),
        (ALIASES, first(&a.aliases, &b.aliases)),
        (MACROS, first(&a.macros, &b.macros)),
        (TIMERS, first(&a.timers, &b.timers)),
        (GAUGES, first(&a.gauges, &b.gauges)),
    ]
    .into_iter()
    .find_map(|(section, index)| index.map(|index| (section, index)))
}
