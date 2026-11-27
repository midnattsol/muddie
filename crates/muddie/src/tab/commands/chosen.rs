//! What a script offers to choose from, in the palette: each thing it
//! offers, to read and to choose, and what was chosen told back to it.

use super::*;

/// What a script offers to choose from, while the palette asks.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::tab) struct Choice {
    /// What the script is told back by.
    id: u64,
    /// What each says, and what more it says of itself.
    items: Vec<(String, String)>,
}

impl Tab {
    /// Offers what a script asks to choose from, under `title`, in the
    /// palette: one it offered before and not yet chosen from is left,
    /// none chosen.
    pub(in crate::tab) fn start_choice(
        &mut self,
        id: u64,
        title: &str,
        items: Vec<(String, String)>,
    ) {
        self.end_choice(None);
        let title = match title.is_empty() {
            true => tr("Choose one"),
            false => title,
        };
        self.palette.choice = Some(Choice { id, items });
        self.ask(Asking::new(title, "", Offers::Choice));
    }

    /// What is offered to choose from, as typed: each by what it says,
    /// chosen by where it is in the list.
    pub(super) fn choice_items(&self, query: &str) -> Vec<Item> {
        let Some(choice) = &self.palette.choice else {
            return Vec::new();
        };
        (choice.items.iter().enumerate())
            .filter(|(_, (text, about))| {
                text.to_lowercase().contains(query) || about.to_lowercase().contains(query)
            })
            .map(|(index, (text, about))| Item {
                insert: String::new(),
                usage: text.clone(),
                group: String::new(),
                about: about.clone(),
                act: Some(Act::Chose(index)),
            })
            .collect()
    }

    /// Tells the script what was chosen from what it offered, by where it
    /// is in the list; or that nothing was.
    pub(super) fn end_choice(&mut self, chosen: Option<usize>) {
        let Some(choice) = self.palette.choice.take() else {
            return;
        };
        let picked = chosen
            .and_then(|index| (choice.items.get(index)).map(|(text, _)| (index, text.as_str())));
        self.scripts.chosen(choice.id, picked);
        self.run_actions();
    }

    /// Once the palette no longer asks to choose, as it was closed or
    /// something else asked, nothing was chosen.
    pub(super) fn choice_left(&mut self) {
        let asks =
            (self.palette.asking.as_ref()).is_some_and(|asking| asking.offers == Offers::Choice);
        if self.palette.choice.is_some() && !(self.palette.open && asks) {
            self.end_choice(None);
        }
    }
}
