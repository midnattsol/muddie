//! A panel's menu, adding panels, and showing the map.

use super::*;

impl Tab {
    /// What a panel's menu has: its name, where it goes, how it shares
    /// its place, what goes with its kind, and taking it away.
    pub(in crate::tab) fn panel_menu(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(spec) = self.layout.panels.get(index).cloned() else {
            return;
        };
        let more = widgets::icon_button(
            ui,
            widgets::Icon::More,
            &trf("{0} options", &[&spec.name]),
            false,
        );
        let mut asked = None;
        egui::Popup::menu(&more).show(|ui| {
            widgets::menu_width(ui);
            let panel = &mut self.layout.panels[index];
            name_field(ui, &mut panel.name);
            widgets::menu_separator(ui);
            asked = place_items(ui, panel).or(asked.take());
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
            widgets::menu_check(ui, &mut self.layout.columns_full, tr("Full-height sides"))
                .on_hover_text(tr("Side panels reach the top and the bottom"));
            widgets::menu_separator(ui);
            asked = closing_items(ui, &spec.name).or(asked.take());
        });
        match asked {
            Some(PanelAsk::Earlier) => self.shift_panel(index, -1),
            Some(PanelAsk::Later) => self.shift_panel(index, 1),
            Some(PanelAsk::Join) => self.join_before(index),
            Some(PanelAsk::Gauges) => self.open_gauges(spec.id),
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
    }

    /// Whether the map has its panel on screen.
    pub fn map_shown(&self) -> bool {
        (self.layout.panels.iter()).any(|panel| panel.kind == Kind::Map && panel.visible)
    }

    /// Shows or hides the map's panel, making one if there is none.
    pub(in crate::tab) fn set_map_shown(&mut self, shown: bool) {
        match self
            .layout
            .panels
            .iter_mut()
            .find(|panel| panel.kind == Kind::Map)
        {
            Some(panel) => panel.visible = shown,
            None if shown => {
                let id = self.layout.next_id();
                self.layout.panels.push(PanelSpec {
                    id,
                    name: "Map".to_owned(),
                    kind: Kind::Map,
                    size: 330.0,
                    ..PanelSpec::default()
                });
            }
            None => {}
        }
    }
}

/// What a panel's menu asks to be done once it is closed.
enum PanelAsk {
    Earlier,
    Later,
    Join,
    Gauges,
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

/// The side it goes on, moving it a step along that side, and sharing the
/// place of the one before as tabs, or leaving them.
fn place_items(ui: &mut egui::Ui, panel: &mut PanelSpec) -> Option<PanelAsk> {
    widgets::menu_heading(ui, tr("Place"));
    let sides = [
        (Side::Left, tr("At the left")),
        (Side::Right, tr("At the right")),
        (Side::Top, tr("Above")),
        (Side::Bottom, tr("Below")),
    ];
    for (side, label) in sides {
        let mut here = panel.side == side;
        if widgets::menu_check(ui, &mut here, label).clicked() && panel.side != side {
            if panel.side.is_beside() != side.is_beside() {
                panel.size = if side.is_beside() { 300.0 } else { 120.0 };
            }
            panel.side = side;
            ui.close();
        }
    }
    let (before, after) = match panel.side.is_beside() {
        true => (tr("Move up"), tr("Move down")),
        false => (tr("Move left"), tr("Move right")),
    };
    let mut asked = None;
    let mut item = |ui: &mut egui::Ui, label: &str, what: PanelAsk| {
        if widgets::menu_item(ui, label, "").clicked() {
            asked = Some(what);
            ui.close();
        }
    };
    item(ui, before, PanelAsk::Earlier);
    item(ui, after, PanelAsk::Later);
    if panel.stack == 0 {
        item(
            ui,
            tr("Share the place of the one before, as tabs"),
            PanelAsk::Join,
        );
    } else if widgets::menu_item(ui, tr("Detach from tabs"), "").clicked() {
        panel.stack = 0;
        ui.close();
    }
    asked
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
