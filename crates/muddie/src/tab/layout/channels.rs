//! The MUD's channels: each in its tab, in the panel that shows them.

use super::*;

/// How many channels are kept apart; what is said on more goes with the
/// rest only.
pub(in crate::tab) const MOST_CHANNELS: usize = 64;

impl Tab {
    /// Adds what was said on a channel to the panels that show them.
    pub(in crate::tab) fn channel_line(&mut self, line: &Line) {
        let ids: Vec<u32> = (self.layout.panels.iter())
            .filter(|panel| panel.channels)
            .map(|panel| panel.id)
            .collect();
        for id in ids {
            self.text_of(id).log.push(line);
            self.note_unread(id);
        }
    }

    /// Takes lines said on a channel of the MUD's: they go with the rest
    /// of what is said, and under the channel's own tab. With no lines,
    /// the channel is made known, to have its tab.
    pub(in crate::tab) fn channel_said(&mut self, name: &str, panel: &str, lines: Vec<Line>) {
        let (name, panel) = (name.trim(), panel.trim());
        if panel.is_empty() {
            self.ensure_channels_panel();
        } else {
            // The panel named: the one of that name, made if there is
            // none, and left where it is and as it is if there is.
            self.panel_called(panel);
        }
        let index = self.channel_index(name, panel);
        if lines.is_empty() {
            return;
        }
        let seen = self.channel_in_sight(name, panel);
        for line in &lines {
            // With the rest of what is said there, and under its own tab.
            match panel.is_empty() {
                true => self.channel_line(line),
                false => self.add_to_panel(panel, line),
            }
            if let Some(index) = index {
                self.panels.channels[index].log.push(line);
            }
        }
        if let Some(index) = index {
            self.panels.channels[index].unread |= !seen;
        }
    }

    /// With nowhere to show the channels, they are given a panel: one of
    /// text already called Channels, which is then marked to show them, or
    /// else a new one under the map, as the last of those at its side. One
    /// that is hidden is somewhere, and is left as it is.
    fn ensure_channels_panel(&mut self) {
        if self.layout.panels.iter().any(|panel| panel.channels) {
            return;
        }
        let named = (self.layout.panels.iter_mut())
            .find(|panel| panel.kind == Kind::Text && panel.name.eq_ignore_ascii_case("Channels"));
        if let Some(panel) = named {
            panel.channels = true;
            return;
        }
        let id = self.layout.next_id();
        let taken = |name: &str| self.layout.panels.iter().any(|panel| panel.name == name);
        let name = ["Channels", "Channels 2", "Channels 3"]
            .into_iter()
            .find(|name| !taken(name))
            .unwrap_or("Channels");
        let side = (self.layout.panels.iter())
            .find(|panel| panel.kind == Kind::Map)
            .map_or(Side::Right, |map| map.side);
        let size = (self.layout.panels.iter())
            .find(|panel| panel.side == side)
            .map_or(330.0, |panel| panel.size);
        self.layout.panels.push(PanelSpec {
            id,
            name: name.to_owned(),
            kind: Kind::Text,
            side,
            size,
            channels: true,
            ..PanelSpec::default()
        });
    }

    /// The channel of this name in this panel, made if it is not known yet
    /// and there is room for one more; none for one with no name.
    fn channel_index(&mut self, name: &str, panel: &str) -> Option<usize> {
        let known = (self.panels.channels.iter()).position(|channel| {
            channel.name.eq_ignore_ascii_case(name) && channel.panel.eq_ignore_ascii_case(panel)
        });
        if known.is_some() || name.is_empty() || self.panels.channels.len() >= MOST_CHANNELS {
            return known;
        }
        let mut log = Output::default();
        log.restyle(self.settings.scrollback_lines, self.settings.timestamps);
        self.panels.channels.push(Channel {
            name: name.to_owned(),
            panel: panel.to_owned(),
            log,
            unread: false,
        });
        Some(self.panels.channels.len() - 1)
    }

    /// Whether what is said on a channel is seen as it comes: where a panel
    /// it is shown in is on this channel, or on all of them.
    fn channel_in_sight(&self, name: &str, panel: &str) -> bool {
        (self.layout.panels.iter()).any(|spec| {
            let hosts = match panel.is_empty() {
                true => spec.channels,
                false => spec.name.eq_ignore_ascii_case(panel),
            };
            let shown = self
                .panels
                .channel_shown
                .iter()
                .find(|(id, _)| *id == spec.id);
            let on_it =
                shown.is_none_or(|(_, shown)| shown.is_empty() || shown.eq_ignore_ascii_case(name));
            hosts && spec.visible && on_it
        })
    }

    /// The tabs of a panel of channels: all, and one for each channel,
    /// marked if something was said on it that has not been looked at.
    /// Returns the channel chosen; none for all.
    pub(in crate::tab) fn channel_tabs(&mut self, ui: &mut egui::Ui, id: u32) -> String {
        let known = self
            .panels
            .channel_shown
            .iter()
            .find(|(panel, _)| *panel == id);
        let mut shown = known.map(|(_, shown)| shown.clone()).unwrap_or_default();
        let spec = self.layout.panel(id).cloned();
        let here = |channel: &&Channel| spec.as_ref().is_some_and(|spec| channel.shown_in(spec));
        let names: Vec<(String, bool)> = std::iter::once((String::new(), false))
            .chain(
                (self.panels.channels.iter())
                    .filter(here)
                    .map(|channel| (channel.name.clone(), channel.unread)),
            )
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
            for (name, unread) in &names {
                let label = if name.is_empty() {
                    tr("All")
                } else {
                    name.as_str()
                };
                if widgets::tab(ui, label, *name == shown, *unread).clicked() {
                    shown.clone_from(name);
                }
            }
        });
        ui.add_space(2.0);
        self.panels.channel_shown.retain(|(panel, _)| *panel != id);
        self.panels.channel_shown.push((id, shown.clone()));
        shown
    }
}
