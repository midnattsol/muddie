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
            &trf("{0} options", &[&spec.name.clone()]),
            false,
        );
        let (mut earlier, mut later, mut join, mut gauges) = (false, false, false, false);
        egui::Popup::menu(&more).show(|ui| {
            widgets::menu_width(ui);
            let panel = &mut self.layout.panels[index];
            widgets::menu_heading(ui, tr("Name"));
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let name = ui.add(TextEdit::singleline(&mut panel.name).desired_width(200.0));
                ui.ctx()
                    .accesskit_node_builder(name.id, |node| node.set_label(tr("Name")));
            });
            widgets::menu_separator(ui);
            widgets::menu_heading(ui, tr("Place"));
            for (side, label) in [
                (Side::Left, tr("At the left")),
                (Side::Right, tr("At the right")),
                (Side::Top, tr("Above")),
                (Side::Bottom, tr("Below")),
            ] {
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
            if widgets::menu_item(ui, before, "").clicked() {
                earlier = true;
                ui.close();
            }
            if widgets::menu_item(ui, after, "").clicked() {
                later = true;
                ui.close();
            }
            if spec.stack == 0 {
                if widgets::menu_item(ui, tr("Share the place of the one before, as tabs"), "")
                    .clicked()
                {
                    join = true;
                    ui.close();
                }
            } else if widgets::menu_item(ui, tr("Detach from tabs"), "").clicked() {
                panel.stack = 0;
                ui.close();
            }
            widgets::menu_separator(ui);
            match spec.kind {
                Kind::Text => {
                    widgets::menu_check(
                        ui,
                        &mut panel.channels,
                        tr("Show the MUD's channels here"),
                    );
                }
                Kind::Gauges => {
                    if widgets::menu_item(ui, tr("Edit the gauges"), "").clicked() {
                        gauges = true;
                        ui.close();
                    }
                }
                Kind::Map => self.map_view.menu_items(ui),
            }
            let corners = tr("Full-height sides");
            widgets::menu_check(ui, &mut self.layout.columns_full, corners)
                .on_hover_text(tr("Side panels reach the top and the bottom"));
            widgets::menu_separator(ui);
            let panel = &mut self.layout.panels[index];
            if widgets::menu_item(ui, &trf("Hide {0}", &[&spec.name.clone()]), "").clicked() {
                panel.visible = false;
                ui.close();
            }
            if widgets::menu_item(ui, &trf("Remove {0}", &[&spec.name.clone()]), "").clicked() {
                self.panels.removed = Some(index);
                ui.close();
            }
        });
        if earlier || later {
            self.shift_panel(index, if earlier { -1 } else { 1 });
        }
        if join {
            self.join_before(index);
        }
        if gauges {
            self.open_gauges(spec.id);
        }
    }

    /// The bar of the layout mode: what to do, a way to add a panel, and
    /// the way out.
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
