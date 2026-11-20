//! What the MUD sends besides text, GMCP and MSDP, kept to be looked at:
//! to know what there is before writing a gauge or a script for it.

use std::collections::VecDeque;

use serde_json::Value;

use super::*;
use crate::theme::Weight;
use crate::widgets;

mod gauge;
mod received;
mod view;

pub use received::{Leaf, Seen, leaves, likely_max};

/// How many earlier messages of each name are kept.
const KEPT: usize = 12;

/// How many different things are kept at most.
const MOST: usize = 400;

impl Tab {}

/// The view of what the MUD sends: what was seen, which is chosen, how it
/// is filtered, and whether it is kept outside the view.
pub(super) struct DataView {
    /// What the MUD has sent besides text, by name, to look at.
    pub(super) seen: Vec<data::Seen>,
    /// The one chosen in the list, and what the list is narrowed to.
    pub(super) selected: usize,
    pub(super) filter: String,
    /// The protocol whose data alone is listed, if one was chosen.
    pub(super) protocol: Option<String>,
    /// Whether what arrives is kept while it is not on screen,
    /// and what the connection was last told about tracing.
    pub(super) keep: bool,
    pub(super) trace_sent: Option<bool>,
}

impl Default for DataView {
    fn default() -> Self {
        Self {
            seen: Vec::new(),
            selected: 0,
            filter: String::new(),
            protocol: None,
            keep: false,
            trace_sent: Some(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_laid_out_as_values_with_their_paths() {
        let value: Value = serde_json::from_str(
            r#"{"health": {"current": 25, "max": "100"}, "hp": 3, "maxhp": 9, "name": "Ana", "bag": [5, "sword"]}"#,
        )
        .unwrap();
        let mut all = Vec::new();
        leaves("Char.Vitals", &value, true, &mut all);
        let shown: Vec<(&str, &str, bool)> = all
            .iter()
            .map(|leaf| (leaf.path.as_str(), leaf.shown.as_str(), leaf.gauge))
            .collect();
        assert_eq!(
            shown,
            [
                ("Char.Vitals.bag[1]", "5", false),
                ("Char.Vitals.bag[2]", "sword", false),
                ("Char.Vitals.health.current", "25", true),
                ("Char.Vitals.health.max", "100", true),
                ("Char.Vitals.hp", "3", true),
                ("Char.Vitals.maxhp", "9", true),
                ("Char.Vitals.name", "Ana", false),
            ]
        );
        // The maximum that goes with a value is found beside it.
        assert_eq!(
            likely_max("Char.Vitals.health.current", &all).unwrap(),
            "Char.Vitals.health.max"
        );
        assert_eq!(
            likely_max("Char.Vitals.hp", &all).unwrap(),
            "Char.Vitals.maxhp"
        );
        assert_eq!(likely_max("Char.Vitals.maxhp", &all), None);
    }
}
