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
        let name = name.trim();
        if panel.trim().is_empty() {
            self.ensure_channels_panel();
        } else {
            // The panel named: the one of that name, made if there is
            // none, and left where it is and as it is if there is.
            self.panel_called(panel.trim());
        }
        self.join_channels();
        let panel = self.channel_place(panel);
        let index = self.channel_index(name, &panel);
        if lines.is_empty() {
            return;
        }
        let seen = self.channel_in_sight(name, &panel);
        for line in &lines {
            // With the rest of what is said there, and under its own tab.
            match panel.is_empty() {
                true => self.channel_line(line),
                false => self.add_to_panel(&panel, line),
            }
            if let Some(index) = index {
                self.panels.channels[index].log.push(line);
            }
        }
        if let Some(index) = index {
            self.panels.channels[index].unread |= !seen;
        }
        self.read_channel(name, &lines);
    }

    /// Says aloud what was said on a channel, after its name, unless the
    /// channels are not read or this one is kept quiet.
    fn read_channel(&mut self, name: &str, lines: &[Line]) {
        if !self.settings.speech_channels || self.channel_quiet(name) {
            return;
        }
        for line in lines {
            let text = line.text();
            if !text.trim().is_empty() {
                self.voice.say(&format!("{name}: {}", text.trim()));
            }
        }
    }

    /// Whether a channel is kept from being read aloud.
    fn channel_quiet(&self, name: &str) -> bool {
        (self.prefs.quiet_channels.iter()).any(|quiet| quiet.eq_ignore_ascii_case(name))
    }

    /// Reads a channel aloud, or keeps it quiet, and keeps that with the
    /// profile.
    fn set_channel_quiet(&mut self, name: &str, quiet: bool) {
        self.prefs
            .quiet_channels
            .retain(|known| !known.eq_ignore_ascii_case(name));
        if quiet {
            self.prefs.quiet_channels.push(name.to_owned());
        }
        let prefs = self.prefs.clone();
        self.keep_in_profile(move |profile| {
            let kept = prefs.quiet_channels.clone();
            profile.session.get_or_insert(prefs).quiet_channels = kept;
        });
    }

    /// Where a channel goes, by the panel named: a panel that shows the
    /// channels anyway is the panel of channels, given as no name, so that
    /// a channel sent there by name and one sent to the panel of channels
    /// are one and the same.
    pub(in crate::tab) fn channel_place(&self, panel: &str) -> String {
        let panel = panel.trim();
        let channels = |spec: &PanelSpec| spec.channels && spec.name.eq_ignore_ascii_case(panel);
        match self.layout.panels.iter().any(channels) {
            true => String::new(),
            false => panel.to_owned(),
        }
    }

    /// Makes one of the channels that are the same once a panel shows the
    /// channels: the first is kept, and what the other was marked is
    /// added to it.
    fn join_channels(&mut self) {
        let places: Vec<String> = (self.panels.channels.iter())
            .map(|channel| self.channel_place(&channel.panel))
            .collect();
        for (channel, place) in self.panels.channels.iter_mut().zip(places) {
            channel.panel = place;
        }
        let mut index = 0;
        while index < self.panels.channels.len() {
            let same = |other: &Channel, this: &Channel| {
                other.name.eq_ignore_ascii_case(&this.name)
                    && other.panel.eq_ignore_ascii_case(&this.panel)
            };
            let this = &self.panels.channels[index];
            match (self.panels.channels[..index].iter()).position(|other| same(other, this)) {
                Some(first) => {
                    let gone = self.panels.channels.remove(index);
                    self.panels.channels[first].unread |= gone.unread;
                }
                None => index += 1,
            }
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
            hosts && self.layout.on_screen(spec) && on_it
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
                let tab = widgets::tab(ui, label, *name == shown, *unread);
                if tab.clicked() {
                    shown.clone_from(name);
                }
                // Each channel can be kept from being read aloud.
                if !name.is_empty() {
                    let mut read = !self.channel_quiet(name);
                    let mut changed = false;
                    tab.context_menu(|ui| {
                        widgets::menu_width(ui);
                        changed = widgets::menu_check(ui, &mut read, tr("Read aloud")).clicked();
                        if changed {
                            ui.close();
                        }
                    });
                    if changed {
                        self.set_channel_quiet(name, !read);
                    }
                }
            }
        });
        ui.add_space(2.0);
        self.panels.channel_shown.retain(|(panel, _)| *panel != id);
        self.panels.channel_shown.push((id, shown.clone()));
        shown
    }
}
