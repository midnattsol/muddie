//! Making a gauge of a value received.

use super::*;

impl Tab {
    /// Makes a gauge that follows the value at `path`, with the maximum
    /// that seems to go with it, and opens it to be finished.
    pub(in crate::tab) fn gauge_from(&mut self, path: &str, all: &[Leaf]) {
        if !self
            .layout
            .panels
            .iter()
            .any(|panel| panel.kind == layout::Kind::Gauges)
        {
            self.add_panel(layout::Kind::Gauges);
        }
        // Made in the editor, and shown there to be finished.
        self.open_automation(automation::GAUGES);
        // Named after what it is: `health`, not the `current` inside it.
        let mut parts = path.rsplit('.');
        let last = parts.next().unwrap_or(path);
        let plain = matches!(
            last.to_lowercase().as_str(),
            "current" | "cur" | "value" | "now"
        );
        let name = if plain {
            parts.next().unwrap_or(last)
        } else {
            last
        };
        self.editor.draft.gauges.push(crate::rules::Gauge {
            name: name.to_owned(),
            value: path.to_owned(),
            max: likely_max(path, all).unwrap_or_default(),
            ..crate::rules::Gauge::starter()
        });
        self.select_rule(automation::GAUGES, self.editor.draft.gauges.len() - 1);
    }
}
