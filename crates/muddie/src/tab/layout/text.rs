//! Panels of text: what they are given, and how they show it.

use super::*;

/// The frame of a panel: a rounded well on the window's chrome.
pub(in crate::tab) fn well(left: i8, right: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(theme::current().background)
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .outer_margin(egui::Margin {
            left,
            right,
            top: 4,
            bottom: 4,
        })
}

impl Tab {
    /// The content of the panel with this id, made if it had none.
    pub(in crate::tab) fn text_of(&mut self, id: u32) -> &mut TextContent {
        let index = match self.panels.text.iter().position(|(known, _)| *known == id) {
            Some(index) => index,
            None => {
                let mut content = TextContent::default();
                content
                    .log
                    .restyle(self.settings.scrollback_lines, self.settings.timestamps);
                self.panels.text.push((id, content));
                self.panels.text.len() - 1
            }
        };
        &mut self.panels.text[index].1
    }

    /// The panel a script or a trigger means by `name`, made as a panel
    /// of text if there is none. `map` and `channels` mean the panel that
    /// is the map and the one of the channels, whatever they are called.
    pub(in crate::tab) fn panel_called(&mut self, name: &str) -> u32 {
        let name = name.trim();
        let panels = &self.layout.panels;
        let by_name = panels
            .iter()
            .find(|panel| panel.name.eq_ignore_ascii_case(name));
        let by_role = || match name.to_lowercase().as_str() {
            "map" => panels.iter().find(|panel| panel.kind == Kind::Map),
            "channels" => panels.iter().find(|panel| panel.channels),
            _ => None,
        };
        if let Some(panel) = by_name.or_else(by_role) {
            return panel.id;
        }
        self.layout.add(name, Kind::Text)
    }

    /// Puts `text` in a panel, in place of what it showed; none clears it.
    pub(in crate::tab) fn show_in_panel(&mut self, name: &str, text: Option<&str>) {
        match text {
            Some(text) => {
                let id = self.panel_called(name);
                self.text_of(id).fixed = panels::styled_lines(text);
            }
            None => {
                let known = self
                    .layout
                    .panels
                    .iter()
                    .find(|panel| panel.name.eq_ignore_ascii_case(name.trim()));
                if let Some(id) = known.map(|panel| panel.id) {
                    self.text_of(id).fixed.clear();
                }
            }
        }
    }

    /// Adds a line at the end of a panel.
    pub(in crate::tab) fn add_to_panel(&mut self, name: &str, line: &Line) {
        let id = self.panel_called(name);
        self.text_of(id).log.push(line);
        self.note_unread(id);
    }

    /// Marks a panel that got something while another tab of its place
    /// was the one on show.
    pub(in crate::tab) fn note_unread(&mut self, id: u32) {
        let stack = self.layout.panel(id).map_or(0, |panel| panel.stack);
        let shown = self
            .panels
            .shown_tabs
            .iter()
            .find(|(known, _)| *known == stack)
            .map(|(_, id)| *id);
        if stack != 0
            && shown.is_some_and(|shown| shown != id)
            && !self.panels.unread_tabs.contains(&id)
        {
            self.panels.unread_tabs.push(id);
        }
    }

    /// What a panel was given: text put there whole, centred as a block
    /// since it is often a drawing; a channel, or all of them, if it shows
    /// channels; or else its lines one after another. Returns whether
    /// there was anything.
    pub(in crate::tab) fn text_body(&mut self, ui: &mut egui::Ui, id: u32, quiet: bool) -> bool {
        // The drawing put in it, if there is one, laid out where it is
        // kept: it is not copied to be drawn.
        let galleys: Vec<_> = (self.text_of(id).fixed.iter())
            .map(|line| ui.painter().layout_job(server_job(line)))
            .collect();
        // What a drawing put in it has to fit in, there or yet to come.
        // The map's panel is measured where the map is drawn.
        let map =
            (self.layout.panels.iter()).any(|panel| panel.id == id && panel.kind == Kind::Map);
        if !map {
            let cells = Self::cells(ui, ui.available_size());
            self.panel_measured(id, cells, !galleys.is_empty());
        }
        if !galleys.is_empty() {
            drawing(ui, galleys);
            return true;
        }
        if self.channel_shown_in(ui, id) {
            return true;
        }
        if self.text_of(id).log.len() > 0 {
            if let Some(clicked) = self.text_of(id).log.show(ui, &format!("panel {id}")) {
                self.link_clicked(clicked);
            }
            return true;
        }
        if !quiet {
            let none = tr(
                "Nothing here yet. A trigger can show lines in this panel, and so can a script.",
            );
            let faint = RichText::new(none)
                .font(theme::font(Weight::Regular, 12.5))
                .color(theme::current().text_faint);
            ui.label(faint);
        }
        false
    }

    /// A panel that channels are shown in has a tab for each of them: the
    /// one chosen is shown alone. With all of them in view there is nothing
    /// unseen. Returns whether one alone was shown.
    fn channel_shown_in(&mut self, ui: &mut egui::Ui, id: u32) -> bool {
        let spec = self.layout.panel(id).cloned();
        let here = |channel: &Channel| spec.as_ref().is_some_and(|spec| channel.shown_in(spec));
        if !self.panels.channels.iter().any(here) {
            return false;
        }
        let shown = self.channel_tabs(ui, id);
        let chosen = (self.panels.channels.iter_mut())
            .find(|channel| !shown.is_empty() && channel.name == shown && here(channel));
        let Some(channel) = chosen else {
            (self.panels.channels.iter_mut())
                .filter(|channel| here(channel))
                .for_each(|channel| channel.unread = false);
            return false;
        };
        channel.unread = false;
        let clicked = match channel.log.len() {
            0 => None,
            _ => channel.log.show(ui, &format!("panel {id} channel {shown}")),
        };
        if let Some(clicked) = clicked {
            self.link_clicked(clicked);
        }
        true
    }
}

/// A drawing put in a panel whole, centred as a block, its lines never
/// wrapped: the columns must stay in line.
fn drawing(ui: &mut egui::Ui, galleys: Vec<std::sync::Arc<egui::Galley>>) {
    let widest = galleys
        .iter()
        .map(|galley| galley.size().x)
        .fold(0.0, f32::max);
    let height: f32 = galleys.iter().map(|galley| galley.size().y).sum();
    let left = ((ui.available_width() - widest) / 2.0).max(0.0);
    ui.add_space(((ui.available_height() - height) / 2.0).max(0.0));
    ui.horizontal(|ui| {
        ui.add_space(left);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for galley in galleys {
                ui.add(Label::new(galley).extend());
            }
        });
    });
}
