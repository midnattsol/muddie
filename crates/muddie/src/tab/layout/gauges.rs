//! Panels of gauges, and changing a gauge where it is shown.

use super::*;

/// Whether `panel` is the first of the panels of gauges, the one that
/// shows the gauges put in none.
pub(in crate::tab) fn panels_first(layout: &Layout, panel: u32) -> bool {
    let first = layout.panels.iter().find(|spec| spec.kind == Kind::Gauges);
    first.is_some_and(|spec| spec.id == panel)
}

impl Tab {
    /// Whether the gauge called `name` belongs in the panel `id`: the
    /// one it was put in, or the first of the gauges' panels if it was put
    /// in none, as those the MUD's vitals and scripts make are.
    pub(in crate::tab) fn gauge_is_in(&self, name: &str, id: u32) -> bool {
        let of_gauges = |panel: &&PanelSpec| panel.kind == Kind::Gauges;
        // Its own panel if that is one of gauges, else the first of them.
        let made = (self.rules.gauges.iter()).find(|gauge| gauge.shown_name() == name);
        let own = made.and_then(|gauge| self.layout.panel(gauge.panel).filter(of_gauges));
        let home = own.or_else(|| self.layout.panels.iter().find(of_gauges));
        home.is_some_and(|panel| panel.id == id)
    }

    /// The gauges of a panel in a row, or how to get some.
    pub(in crate::tab) fn gauges_body(&mut self, ui: &mut egui::Ui, id: u32) {
        // In the order they have in the editor, whichever the MUD
        // reported first.
        let made = &self.rules.gauges;
        (self.gauges).arrange(|name| made.iter().position(|gauge| gauge.shown_name() == name));
        let made = |name: &str| {
            self.rules
                .gauges
                .iter()
                .any(|gauge| gauge.shown_name() == name)
        };
        let (any, pressed) = self
            .gauges
            .show(ui, |name| self.gauge_is_in(name, id), made);
        if let Some(name) = pressed {
            self.edit_gauge_here(&name);
        }
        if any {
            // The way to change them, at the corner, out of their way.
            return;
        }
        let theme = theme::current();
        ui.horizontal_wrapped(|ui| {
            let none = tr("No gauges here yet.");
            ui.label(
                RichText::new(none)
                    .font(theme::font(Weight::Regular, 12.5))
                    .color(theme.text_faint),
            );
            if widgets::quiet_button(ui, tr("Add gauges")).clicked() {
                self.open_gauges(id);
            }
        });
    }

    /// Opens the details of a gauge made in the editor over the game, to
    /// change it where it is shown.
    pub(in crate::tab) fn edit_gauge_here(&mut self, name: &str) {
        let Some(index) = (self.rules.gauges.iter()).position(|gauge| gauge.shown_name() == name)
        else {
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
                        delete = self.gauge_detail(ui, index);
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    editor = widgets::quiet_button(ui, tr("Open in the editor")).clicked();
                });
            },
        );
        if delete {
            let gone = self.editor.draft.gauges.remove(index);
            self.gauges.set(gone.shown_name(), None);
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
