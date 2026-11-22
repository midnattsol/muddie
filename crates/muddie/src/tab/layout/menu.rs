//! A panel's menu, adding panels, and showing the map.

use super::*;

impl Tab {
    /// What a panel's menu has: naming it again, what goes with its kind,
    /// and hiding or removing it. Where it goes, its order and sharing a
    /// place as tabs are done by dragging it by its handle, or in Layout.
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
            if widgets::menu_item(ui, tr("Rename…"), "").clicked() {
                asked = Some(PanelAsk::Rename);
                ui.close();
            }
            widgets::menu_separator(ui);
            let panel = &mut self.layout.panels[index];
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
                Kind::Map => {
                    self.map_view.menu_items(ui);
                    // The last room the map added, while there is one.
                    if self.map.borrow().last_change().is_some()
                        && widgets::menu_item(ui, tr("Undo the map's last change"), "").clicked()
                    {
                        asked = Some(PanelAsk::UndoChange);
                        ui.close();
                    }
                    // What the scripts offer to do with the map, under the
                    // name of each that offers something.
                    let tools = self.scripts.map_tools();
                    let mut by = None;
                    for tool in tools {
                        if by.as_ref() != Some(&tool.by) {
                            widgets::menu_separator(ui);
                            widgets::menu_heading(ui, &tool.by);
                            by = Some(tool.by.clone());
                        }
                        let chosen = match tool.checked {
                            Some(mut on) => widgets::menu_check(ui, &mut on, &tool.label),
                            None => widgets::menu_item(ui, &tool.label, ""),
                        };
                        if chosen.clicked() {
                            asked = Some(PanelAsk::MapTool(tool.label));
                            ui.close();
                        }
                    }
                }
            }
            widgets::menu_separator(ui);
            asked = closing_items(ui, &spec.shown_name()).or(asked.take());
        });
        match asked {
            Some(PanelAsk::Rename) => self.rename_panel(spec.id),
            Some(PanelAsk::Gauges) => self.open_gauges(spec.id),
            Some(PanelAsk::Hide) => self.layout.panels[index].visible = false,
            Some(PanelAsk::Remove) => self.panels.removed = Some(index),
            Some(PanelAsk::MapTool(label)) => self.use_map_tool(&label),
            Some(PanelAsk::UndoChange) => self.undo_change(),
            None => {}
        }
    }

    /// Starts naming a panel again, in its header: it is shown first, if
    /// it was hidden or behind another as a tab.
    pub(in crate::tab) fn rename_panel(&mut self, id: u32) {
        let Some(panel) = self.layout.panel_mut(id) else {
            return;
        };
        panel.visible = true;
        let (side, stack, name) = (panel.side, panel.stack, panel.shown_name());
        self.layout.reveal(side);
        self.set_view(0);
        self.panels.shown_tabs.retain(|(known, _)| *known != stack);
        self.panels.shown_tabs.push((stack, id));
        self.panels.renaming = Some((id, name, true));
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
        let name = match kind {
            Kind::Text => free("Panel", false),
            Kind::Map => "Map".to_owned(),
            Kind::Gauges => free("Gauges", true),
        };
        let (side, size) = kind.home();
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
            None if shown => self.add_panel(Kind::Map),
            None => {}
        }
    }
}

/// What a panel's menu asks to be done once it is closed.
enum PanelAsk {
    Rename,
    Gauges,
    Hide,
    Remove,
    /// What a script offered to do with the map, by what it is chosen by.
    MapTool(String),
    UndoChange,
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
