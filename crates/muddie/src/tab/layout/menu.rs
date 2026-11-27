//! A panel's menu, adding panels, and showing the map.

use super::*;

impl Tab {
    /// What a panel's menu has: naming it again, moving it in Layout, what
    /// goes with its kind, and hiding or removing it. Where it goes, its
    /// order and sharing a place as tabs are done by dragging it by its
    /// handle too.
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
            if widgets::menu_item(ui, tr("Move in Layout…"), "").clicked() {
                asked = Some(PanelAsk::Arrange);
                ui.close();
            }
            widgets::menu_separator(ui);
            let panel = &mut self.layout.panels[index];
            match spec.kind {
                Kind::Text => text_menu_items(ui, panel),
                Kind::Gauges => {
                    if widgets::menu_item(ui, tr("Edit the gauges"), "").clicked() {
                        asked = Some(PanelAsk::Gauges);
                        ui.close();
                    }
                }
                Kind::Map => self.map_menu_items(ui, &mut asked),
            }
            widgets::menu_separator(ui);
            asked = closing_items(ui, &spec.shown_name()).or(asked.take());
        });
        match asked {
            Some(PanelAsk::Rename) => self.rename_panel(spec.id),
            Some(PanelAsk::Arrange) => self.open_layout_view(Some(spec.id)),
            Some(PanelAsk::Gauges) => self.open_gauges(spec.id),
            Some(PanelAsk::Hide) => self.show_panel(spec.id, false),
            Some(PanelAsk::Remove) => self.panels.removed = Some(index),
            Some(PanelAsk::MapAction(label)) => self.use_map_action(&label),
            Some(PanelAsk::UndoChange) => drop(self.run_command("/map undo")),
            Some(PanelAsk::Appearance) => self.open_map_appearance(),
            Some(PanelAsk::ShowArea) => self.ask(crate::tab::commands::Asking::area_to_show()),
            None => {}
        }
    }

    /// What the menu of the map's panel has: the map view's own, another
    /// area to show, its appearance, undoing its last change, and what the
    /// scripts offer.
    fn map_menu_items(&mut self, ui: &mut egui::Ui, asked: &mut Option<PanelAsk>) {
        self.map_view.menu_items(ui);
        // Another area, to look at: asked for in the palette.
        let several = self.map_view.area_count(&self.map.borrow()) > 1;
        if several && widgets::menu_item(ui, tr("Show an area…"), "").clicked() {
            *asked = Some(PanelAsk::ShowArea);
            ui.close();
        }
        if widgets::menu_item(ui, tr("Map appearance…"), "").clicked() {
            *asked = Some(PanelAsk::Appearance);
            ui.close();
        }
        // The last room the map added, while there is one.
        if self.map.borrow().last_change().is_some()
            && widgets::menu_item(ui, tr("Undo the map's last change"), "").clicked()
        {
            *asked = Some(PanelAsk::UndoChange);
            ui.close();
        }
        self.script_map_items(ui, asked);
    }

    /// What the scripts offer to do with the map, under the name of each
    /// that offers something.
    fn script_map_items(&self, ui: &mut egui::Ui, asked: &mut Option<PanelAsk>) {
        let offered = (self.scripts.map_actions().into_iter()).filter(|tool| tool.on_map);
        let mut by = None;
        for tool in offered {
            let name = self.offered_by(&tool.owner);
            if by.as_ref() != Some(&name) {
                widgets::menu_separator(ui);
                widgets::menu_heading(ui, &name);
                by = Some(name);
            }
            let chosen = match tool.checked {
                Some(mut on) => widgets::menu_check(ui, &mut on, &tool.label),
                None => widgets::menu_item(ui, &tool.label, ""),
            };
            if chosen.clicked() {
                *asked = Some(PanelAsk::MapAction(tool.label));
                ui.close();
            }
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
        self.set_view(View::Game);
        self.panels.shown_tabs.retain(|(known, _)| *known != stack);
        self.panels.shown_tabs.push((stack, id));
        self.panels.renaming = Some((id, name, true));
    }

    /// Names a panel again, as typed: a name with nothing in it is not one,
    /// and one another panel has is said, as scripts tell panels apart by
    /// their names.
    pub(in crate::tab) fn rename_to(&mut self, id: u32, typed: &str) {
        let typed = typed.trim();
        let Some(panel) = self.layout.panel(id) else {
            return;
        };
        if typed.is_empty() || typed == panel.shown_name() {
            return;
        }
        let taken = (self.layout.panels.iter())
            .any(|other| other.id != id && other.shown_name().eq_ignore_ascii_case(typed));
        if taken {
            return self.refuse(&trf("There is a panel called {0} already.", &[typed]));
        }
        let Some(panel) = self.layout.panel_mut(id) else {
            return;
        };
        let before = std::mem::replace(&mut panel.name, typed.to_owned());
        // The gauges put in it go with it, as they are put in by its name.
        if panel.kind == Kind::Gauges {
            let renamed = typed.to_owned();
            self.change_rules(|rules| {
                for gauge in &mut rules.gauges {
                    if gauge.panel.trim().eq_ignore_ascii_case(before.trim()) {
                        gauge.panel.clone_from(&renamed);
                    }
                }
            });
        }
    }

    /// Adds a panel of a kind, under a name not taken, where that kind
    /// usually goes.
    pub(in crate::tab) fn add_panel(&mut self, kind: Kind) {
        let name = match kind {
            Kind::Text => self.layout.free_name("Panel", false),
            Kind::Map => "Map".to_owned(),
            Kind::Gauges => self.layout.free_name("Gauges", true),
        };
        self.layout.add(&name, kind);
    }

    /// Whether the map has its panel on screen.
    pub fn map_shown(&self) -> bool {
        (self.layout.panels.iter())
            .any(|panel| panel.kind == Kind::Map && self.layout.on_screen(panel))
    }

    /// Shows or hides the map's panel, making one if there is none.
    pub(in crate::tab) fn set_map_shown(&mut self, shown: bool) {
        let map = (self.layout.panels.iter()).find(|panel| panel.kind == Kind::Map);
        match map.map(|panel| panel.id) {
            Some(id) => self.layout.set_shown(id, shown),
            None if shown => self.add_panel(Kind::Map),
            None => {}
        }
    }
}

/// What a panel's menu asks to be done once it is closed.
enum PanelAsk {
    Rename,
    /// Moving it, in Layout, where the keyboard moves panels.
    Arrange,
    Gauges,
    Hide,
    Remove,
    /// What a script offered to do with the map, by what it is chosen by.
    MapAction(String),
    UndoChange,
    Appearance,
    ShowArea,
}

/// What the menu of a text panel has: whether it shows the MUD's channels,
/// and, while it does not, whether it is read aloud.
fn text_menu_items(ui: &mut egui::Ui, panel: &mut PanelSpec) {
    widgets::menu_check(ui, &mut panel.channels, tr("Show the MUD's channels here"));
    // The channels have their own, each, in their tabs.
    if !panel.channels {
        let mut aloud = !panel.quiet;
        if widgets::menu_check(ui, &mut aloud, tr("Read aloud")).clicked() {
            panel.quiet = !aloud;
        }
    }
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
