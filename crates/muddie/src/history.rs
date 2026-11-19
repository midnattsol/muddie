//! Input history, browsed with the up and down arrows.

/// Oldest entries are dropped beyond this.
const MAX_ENTRIES: usize = 1000;

#[derive(Default)]
pub struct History {
    entries: Vec<String>,
    /// Index of the entry being shown, or `None` while editing a new line.
    cursor: Option<usize>,
    /// The unsent line that was being typed when browsing started.
    draft: String,
}

impl History {
    /// Records a sent line and ends any browsing.
    pub fn add(&mut self, line: &str) {
        self.cursor = None;
        self.draft.clear();
        if line.is_empty() || self.entries.last().is_some_and(|last| last == line) {
            return;
        }
        if self.entries.len() == MAX_ENTRIES {
            self.entries.remove(0);
        }
        self.entries.push(line.to_owned());
    }

    /// Replaces `input` with the previous entry, if there is one.
    pub fn older(&mut self, input: &mut String) {
        let index = match self.cursor {
            None if self.entries.is_empty() => return,
            None => {
                self.draft = std::mem::take(input);
                self.entries.len() - 1
            }
            Some(0) => return,
            Some(index) => index - 1,
        };
        self.cursor = Some(index);
        input.clone_from(&self.entries[index]);
    }

    /// Replaces `input` with the next entry, or the draft after the newest.
    pub fn newer(&mut self, input: &mut String) {
        let Some(index) = self.cursor else {
            return;
        };
        if index + 1 < self.entries.len() {
            self.cursor = Some(index + 1);
            input.clone_from(&self.entries[index + 1]);
        } else {
            self.cursor = None;
            *input = std::mem::take(&mut self.draft);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browsing_keeps_the_draft() {
        let mut history = History::default();
        history.add("north");
        history.add("look");
        history.add("look");
        let mut input = "sa".to_owned();
        history.older(&mut input);
        assert_eq!(input, "look");
        history.older(&mut input);
        assert_eq!(input, "north");
        history.older(&mut input);
        assert_eq!(input, "north");
        history.newer(&mut input);
        history.newer(&mut input);
        assert_eq!(input, "sa");
        history.newer(&mut input);
        assert_eq!(input, "sa");
    }
}
