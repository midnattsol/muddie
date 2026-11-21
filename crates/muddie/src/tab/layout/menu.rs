//! A panel's menu, adding panels, and showing the map.

use super::*;

impl Tab {
    /// What a panel's menu has: its name, what goes with its kind, and
    /// hiding or removing it. Where it goes, its order and sharing a place
    /// as tabs are done by dragging it by its handle.
    pub(in crate::tab) fn panel_menu(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(spec) = self.layout.panels.get(index).cloned() else {
            return;
        };
        let more = widgets::icon_button(
            ui,
            widgets::Icon::More,
            &trf("{0} options", &[&spec.shown_name()]),
            false,
        );
        let mut asked = None;
        crate::widgets::menu_of(&more).show(|ui| {
            widgets::menu_width(ui);
            let panel = &mut self.layout.panels[index];
            name_field(ui, &mut panel.name);
            widgets::menu_separator(ui);
            match spec.kind {
                Kind::Text => drop(widgets::menu_check(
                    ui,
                    &mut panel.channels,
                    tr("Show the MUD's channels here"),
                )),
                Kind::Gauges => {
                    if widgets::menu_item(ui, tr("Edit the gauges"), "").clicked() {
                        asked = Some(PanelAsk::Gauges);
                        ui.close();
                    }
                }
                Kind::Map => self.map_view.menu_items(ui),
            }
            if widgets::menu_item(ui, tr("Arrange the panels…"), "").clicked() {
                asked = Some(PanelAsk::Arrange);
                ui.close();
            }
            widgets::menu_separator(ui);
            asked = closing_items(ui, &spec.shown_name()).or(asked.take());
        });
        match asked {
            Some(PanelAsk::Gauges) => self.open_gauges(spec.id),
            Some(PanelAsk::Arrange) => self.open_arrange(Some(spec.id)),
            Some(PanelAsk::Hide) => self.layout.panels[index].visible = false,
            Some(PanelAsk::Remove) => self.panels.removed = Some(index),
            None => {}
        }
    }

    /// Adds a panel of a kind, under a name not taken, where that kind
    /// usually goes.
    pub(in crate::tab) fn add_panel(&mut self, kind: Kind) {
        let id = self.layout.next_id();
        // A name not taken: the plain one, then numbered.
        let free = |base: &str, first: bool| -> String {
            let taken = |name: &String| self.layout.panels.iter().any(|panel| panel.name == *name);
            let plain = first.then(|| base.to_owned()).into_iter();
            let numbered = (if first { 2 } else { 1 }..).map(|number| format!("{base} {number}"));
            plain
                .chain(numbered)
                .take(300)
                .find(|name| !taken(name))
                .unwrap_or_default()
        };
        let (name, side, size) = match kind {
            Kind::Text => (free("Panel", false), Side::Right, 280.0),
            Kind::Map => ("Map".to_owned(), Side::Right, 330.0),
            Kind::Gauges => (free("Gauges", true), Side::Bottom, 96.0),
        };
        self.layout.panels.push(PanelSpec {
            id,
            name,
            kind,
            side,
            size,
            ..PanelSpec::default()
        });
        self.layout.reveal(side);
    }

    /// Whether the map has its panel on screen.
    pub fn map_shown(&self) -> bool {
        (self.layout.panels.iter())
            .any(|panel| panel.kind == Kind::Map && self.layout.on_screen(panel))
    }

    /// Shows or hides the map's panel, making one if there is none.
    pub(in crate::tab) fn set_map_shown(&mut self, shown: bool) {
        match self
            .layout
            .panels
            .iter_mut()
            .find(|panel| panel.kind == Kind::Map)
        {
            Some(panel) => {
                panel.visible = shown;
                let side = panel.side;
                if shown {
                    self.layout.reveal(side);
                }
            }
            None if shown => {
                let id = self.layout.next_id();
                self.layout.panels.push(PanelSpec {
                    id,
                    name: "Map".to_owned(),
                    kind: Kind::Map,
                    size: 330.0,
                    ..PanelSpec::default()
                });
                self.layout.reveal(PanelSpec::default().side);
            }
            None => {}
        }
    }
}

/// What a panel's menu asks to be done once it is closed.
enum PanelAsk {
    Gauges,
    Arrange,
    Hide,
    Remove,
}

/// The panel's name, to change.
fn name_field(ui: &mut egui::Ui, name: &mut String) {
    widgets::menu_heading(ui, tr("Name"));
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let field = ui.add(TextEdit::singleline(name).desired_width(200.0));
        ui.ctx()
            .accesskit_node_builder(field.id, |node| node.set_label(tr("Name")));
    });
}

/// Hiding the panel, and removing it.
fn closing_items(ui: &mut egui::Ui, name: &str) -> Option<PanelAsk> {
    let mut asked = None;
    if widgets::menu_item(ui, &trf("Hide {0}", &[name]), "").clicked() {
        asked = Some(PanelAsk::Hide);
        ui.close();
    }
    if widgets::menu_item(ui, &trf("Remove {0}", &[name]), "").clicked() {
        asked = Some(PanelAsk::Remove);
        ui.close();
    }
    asked
}
