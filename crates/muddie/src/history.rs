//! Input history: browsed with the up and down arrows, which only stop at
//! entries starting with what was already typed, and searched with Ctrl+R.

/// Oldest entries are dropped beyond this.
const MAX_ENTRIES: usize = 1000;

#[derive(Default)]
pub struct History {
    entries: Vec<String>,
    /// Index of the entry being shown, or `None` while editing a new line.
    cursor: Option<usize>,
    /// The unsent line that was being typed when browsing started. Browsing
    /// only stops at entries that match it.
    draft: String,
    /// Whether entries must contain the draft (a search) or start with it.
    anywhere: bool,
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

    /// The entries, oldest first.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    fn matches(&self, entry: &str) -> bool {
        if self.anywhere {
            entry.to_lowercase().contains(&self.draft.to_lowercase())
        } else {
            entry.starts_with(&self.draft)
        }
    }

    /// Replaces `input` with the previous entry starting with what was
    /// typed, if there is one.
    pub fn older(&mut self, input: &mut String) {
        self.back(input, false);
    }

    /// Replaces `input` with the previous entry containing what was typed.
    /// Repeating it goes further back.
    pub fn search(&mut self, input: &mut String) {
        self.back(input, true);
    }

    fn back(&mut self, input: &mut String, anywhere: bool) {
        let before = match self.cursor {
            Some(index) => index,
            None => {
                self.draft.clone_from(input);
                self.entries.len()
            }
        };
        self.anywhere = anywhere;
        let found = (0..before)
            .rev()
            .find(|&index| self.matches(&self.entries[index]));
        if let Some(index) = found {
            self.cursor = Some(index);
            input.clone_from(&self.entries[index]);
        }
    }

    /// Replaces `input` with the next matching entry, or with what was
    /// being typed after the newest.
    pub fn newer(&mut self, input: &mut String) {
        let Some(current) = self.cursor else {
            return;
        };
        let found =
            (current + 1..self.entries.len()).find(|&index| self.matches(&self.entries[index]));
        match found {
            Some(index) => {
                self.cursor = Some(index);
                input.clone_from(&self.entries[index]);
            }
            None => {
                self.cursor = None;
                *input = std::mem::take(&mut self.draft);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(lines: &[&str]) -> History {
        let mut history = History::default();
        lines.iter().for_each(|line| history.add(line));
        history
    }

    #[test]
    fn browsing_keeps_the_draft() {
        let mut history = history(&["north", "look", "look"]);
        let mut input = String::new();
        history.older(&mut input);
        assert_eq!(input, "look");
        history.older(&mut input);
        assert_eq!(input, "north");
        history.older(&mut input);
        assert_eq!(input, "north");
        history.newer(&mut input);
        history.newer(&mut input);
        assert_eq!(input, "");
    }

    #[test]
    fn arrows_only_stop_at_entries_starting_with_what_was_typed() {
        let mut history = history(&["say hello", "north", "say bye", "south"]);
        let mut input = "say".to_owned();
        history.older(&mut input);
        assert_eq!(input, "say bye");
        history.older(&mut input);
        assert_eq!(input, "say hello");
        history.newer(&mut input);
        assert_eq!(input, "say bye");
        history.newer(&mut input);
        assert_eq!(input, "say");
        let mut input = "zzz".to_owned();
        history.older(&mut input);
        assert_eq!(input, "zzz");
    }

    #[test]
    fn search_finds_text_anywhere_ignoring_case() {
        let mut history = history(&["tell ana Hello", "north", "say hello there"]);
        let mut input = "HELLO".to_owned();
        history.search(&mut input);
        assert_eq!(input, "say hello there");
        history.search(&mut input);
        assert_eq!(input, "tell ana Hello");
        history.newer(&mut input);
        history.newer(&mut input);
        assert_eq!(input, "HELLO");
    }
}
