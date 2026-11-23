//! Panels of gauges, and changing a gauge where it is shown.

use super::*;
use crate::panels;
use muddie_script::PanelOf;

/// Whether `panel` is the first of the panels of gauges, the one that
/// shows the gauges put in none.
pub(in crate::tab) fn panels_first(layout: &Layout, panel: u32) -> bool {
    let first = layout.panels.iter().find(|spec| spec.kind == Kind::Gauges);
    first.is_some_and(|spec| spec.id == panel)
}

impl Tab {
    /// The panel of gauges called `name`, for gauges that name it, as a
    /// plugin's, which cannot know the panels' ids: one of that name; else
    /// one no gauge is shown in, as the one a profile comes with, called
    /// so; else a new one. `used` says whether a panel has gauges in it.
    pub(in crate::tab) fn gauges_panel_called(
        &mut self,
        name: &str,
        used: impl Fn(u32) -> bool,
    ) -> u32 {
        let name = name.trim();
        let of_gauges = |panel: &&mut PanelSpec| panel.kind == Kind::Gauges;
        let panels = &mut self.layout.panels;
        if let Some(panel) = (panels.iter_mut())
            .filter(of_gauges)
            .find(|panel| panel.name.eq_ignore_ascii_case(name))
        {
            return panel.id;
        }
        if let Some(panel) = (panels.iter_mut())
            .filter(of_gauges)
            .find(|panel| !used(panel.id))
        {
            panel.name = name.to_owned();
            return panel.id;
        }
        self.layout.add(name, Kind::Gauges)
    }

    /// The panel of gauges called `name`, made if there is none: where a
    /// script shows a gauge, as a channel's tab, by the panel's name.
    pub(in crate::tab) fn gauges_panel_named(&mut self, name: &str) -> u32 {
        let found = (self.layout.panels.iter()).find(|panel| {
            panel.kind == Kind::Gauges && panel.name.eq_ignore_ascii_case(name.trim())
        });
        match found {
            Some(panel) => panel.id,
            None => self.layout.add(name.trim(), Kind::Gauges),
        }
    }

    /// Whether a gauge belongs in the panel `id`: the one it was put in,
    /// by its id or its name, or the first of the gauges' panels if it was
    /// put in none, as those the MUD's vitals make are.
    pub(in crate::tab) fn gauge_is_in(&self, gauge: &panels::Gauge, id: u32) -> bool {
        let of_gauges = |panel: &&PanelSpec| panel.kind == Kind::Gauges;
        let own = match &gauge.panel {
            Some(PanelOf::Id(own)) => self.layout.panel(*own).filter(of_gauges),
            Some(PanelOf::Named(name)) => (self.layout.panels.iter())
                .filter(of_gauges)
                .find(|panel| panel.name.eq_ignore_ascii_case(name.trim())),
            None => None,
        };
        let home = own.or_else(|| self.layout.panels.iter().find(of_gauges));
        home.is_some_and(|panel| panel.id == id)
    }

    /// Whether a gauge is switched on: its place, its folders and its tags.
    pub(in crate::tab) fn gauge_on(&self, gauge: &panels::Gauge) -> bool {
        (self.scripts.switches()).active(muddie_script::Kind::Gauge, &gauge.belongs)
    }

    /// The gauges switched on, said, as Alt+0 says them.
    pub(in crate::tab) fn gauges_said(&self) -> String {
        self.gauges.summary_of(|gauge| self.gauge_on(gauge))
    }

    /// Where the gauge of this key is among those of the editor, if it is
    /// one of them: its key is its place.
    fn gauge_made(&self, key: &str) -> Option<usize> {
        let gauges = &self.rules.gauges;
        (0..gauges.len()).find(|&index| crate::rules::RuleList::place(gauges, index) == key)
    }

    /// The gauges of a panel in a row, or how to get some.
    pub(in crate::tab) fn gauges_body(&mut self, ui: &mut egui::Ui, id: u32) {
        // In the order they have in the editor, whichever the MUD
        // reported first.
        let places: Vec<String> = (0..self.rules.gauges.len())
            .map(|index| crate::rules::RuleList::place(&self.rules.gauges, index))
            .collect();
        (self.gauges).arrange(|key| places.iter().position(|place| place == key));
        let (any, pressed) = self.gauges.show(
            ui,
            |gauge| self.gauge_on(gauge) && self.gauge_is_in(gauge, id),
            |gauge| self.gauge_made(&gauge.key).is_some(),
        );
        if let Some(key) = pressed {
            self.edit_gauge_here(&key);
        }
        if any {
            // The way to change them, at the corner, out of their way.
            return;
        }
        ui.horizontal_wrapped(|ui| {
            widgets::page_note(ui, tr("No gauges here yet."));
            if widgets::quiet_button(ui, tr("Add gauges")).clicked() {
                self.open_gauges(id);
            }
        });
    }

    /// Opens the details of a gauge made in the editor over the game, to
    /// change it where it is shown.
    pub(in crate::tab) fn edit_gauge_here(&mut self, key: &str) {
        let Some(index) = self.gauge_made(key) else {
            return;
        };
        if !self.editor.open {
            self.editor.draft = self.rules.clone();
            self.editor.seen = self.rules.clone();
        }
        self.editor.gauge_here = Some(index);
    }

    /// The details of the gauge opened over the game: the same as in the
    /// editor, saved the same way, with the way to the editor.
    pub(in crate::tab) fn gauge_window(&mut self, ctx: &egui::Context) {
        let Some(index) = self.editor.gauge_here else {
            return;
        };
        if index >= self.editor.draft.gauges.len() || self.editor.open {
            self.editor.gauge_here = None;
            return;
        }
        let (mut open, mut delete, mut editor) = (true, false, false);
        let name = self.editor.draft.gauges[index].shown_name().to_owned();
        widgets::dialog(
            ctx,
            "gauge here",
            &name,
            &mut open,
            crate::theme::size::DIALOG_M,
            |ui| {
                let room = (ctx.content_rect().height() - 180.0).max(240.0);
                egui::ScrollArea::vertical()
                    .max_height(room)
                    .show(ui, |ui| {
                        widgets::keep_clear_of_bar(ui);
                        delete = self.gauge_detail(ui, index);
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    editor = widgets::quiet_button(ui, tr("Open in the editor")).clicked();
                });
            },
        );
        if delete {
            self.editor.draft.gauges.remove(index);
            open = false;
        }
        if editor {
            open = false;
        }
        if !open {
            self.editor.gauge_here = None;
        }
        // Saved as the editor saves: when the changes stop, and on closing.
        self.rules_autosave(ctx, open);
        if editor {
            self.open_automation(automation::GAUGES);
            self.select_rule(automation::GAUGES, index);
        }
    }

    /// Goes to the gauges in the editor, on the first of this panel's.
    pub(in crate::tab) fn open_gauges(&mut self, panel: u32) {
        self.open_automation(automation::GAUGES);
        let first = panels_first(&self.layout, panel);
        let here =
            |gauge: &crate::rules::Gauge| gauge.panel == panel || (first && gauge.panel == 0);
        if let Some(index) = self.editor.draft.gauges.iter().position(here) {
            self.select_rule(automation::GAUGES, index);
        }
    }
}
