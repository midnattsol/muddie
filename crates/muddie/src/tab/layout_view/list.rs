//! The list of panels in the layout view: by side, each panel a line.

use super::*;

impl Tab {
    /// The list: its heading with the button that adds a panel, then the
    /// panels under the side each is on.
    pub(super) fn layout_list(&mut self, ui: &mut egui::Ui, listed: &[u32]) {
        ui.horizontal(|ui| {
            let heading = RichText::new(tr("Layout"))
                .font(theme::font(Weight::Semibold, 17.0))
                .color(theme::current().text);
            ui.add_space(6.0);
            let shown = ui.add(Label::new(heading).selectable(false));
            widgets::heading_role(ui, &shown, tr("Layout"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let add = widgets::sign_button(ui, "+", tr("New panel"), true);
                crate::widgets::menu_of(&add).show(|ui| {
                    widgets::menu_width(ui);
                    let mut kinds = vec![(layout::Kind::Text, tr("Panel of text"))];
                    if !self.layout.has(layout::Kind::Map) {
                        kinds.push((layout::Kind::Map, tr("Map")));
                    }
                    kinds.push((layout::Kind::Gauges, tr("Gauges")));
                    for (kind, name) in kinds {
                        if widgets::menu_item(ui, name, "").clicked() {
                            self.add_panel(kind);
                            self.layout_view.chosen =
                                self.layout.panels.last().map(|panel| panel.id);
                            self.layout_view.focus = true;
                            ui.close();
                        }
                    }
                });
            });
        });
        ui.add_space(10.0);
        self.layout_keys(ui, listed);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                widgets::keep_clear_of_bar(ui);
                if listed.is_empty() {
                    widgets::page_note(ui, tr("There are no panels: + adds one."));
                    return;
                }
                let whole = egui::UiBuilder::new()
                    .accessibility_role(egui::accesskit::Role::List)
                    .accessibility_label(tr("Panels"));
                ui.scope_builder(whole, |ui| {
                    ui.spacing_mut().item_spacing.y = widgets::LINE_GAP;
                    for side in SIDES {
                        let members: Vec<PanelSpec> = (self.layout.panels.iter())
                            .filter(|panel| panel.side == side)
                            .cloned()
                            .collect();
                        if members.is_empty() {
                            continue;
                        }
                        let mut caption = side_name(side).to_owned();
                        if self.layout.collapsed.contains(&side) {
                            caption = trf("{0}, collapsed", &[&caption]);
                        }
                        widgets::section_caption(ui, &caption);
                        for panel in &members {
                            self.layout_row(ui, panel);
                        }
                    }
                });
            });
    }

    /// One panel's line: its name, and what it is and how it is shown
    /// under it. Tab stops at the one chosen; the arrows go through the
    /// rest.
    pub(super) fn layout_row(&mut self, ui: &mut egui::Ui, panel: &PanelSpec) {
        let chosen = self.layout_view.chosen == Some(panel.id);
        let name = panel.shown_name();
        // What it is, unless its name says so already.
        let mut about = Vec::new();
        if name != kind_name(panel.kind) {
            about.push(kind_name(panel.kind).to_owned());
        }
        if panel.kind == layout::Kind::Text && panel.channels {
            about.push(tr("the MUD's channels").to_owned());
        }
        let sharing: Vec<String> = (self.layout.panels.iter())
            .filter(|other| {
                other.id != panel.id
                    && panel.stack != 0
                    && other.stack == panel.stack
                    && other.side == panel.side
            })
            .map(PanelSpec::shown_name)
            .collect();
        if !sharing.is_empty() {
            about.push(trf("a tab with {0}", &[&sharing.join(", ")]));
        }
        if !panel.visible {
            about.push(tr("hidden").to_owned());
        }
        let detail = about.join(", ");
        let row = widgets::ListRow {
            title: &name,
            detail: &detail,
            selected: chosen,
            active: true,
            key: Some(row_id(panel.id)),
            current: chosen,
            ..widgets::ListRow::default()
        };
        let line = row.show(ui);
        // Its side, which its caption shows, said with it: the arrows go
        // from one side's panels to the next without the caption.
        let side = side_name(panel.side);
        (ui.ctx()).accesskit_node_builder(line.id, |node| {
            let said = match node.description() {
                Some(about) if !about.is_empty() => format!("{about}, {side}"),
                _ => side.to_owned(),
            };
            node.set_description(said);
        });
        widgets::hold_arrows(ui, &line);
        // Gone to by the keys, it is brought into sight.
        if chosen && std::mem::take(&mut self.layout_view.reveal) {
            line.scroll_to_me(None);
        }
        if chosen
            && std::mem::take(&mut self.layout_view.focus)
            && widgets::keyboard_in_use(ui.ctx())
        {
            line.request_focus();
        }
        if line.clicked() {
            self.layout_view.chosen = Some(panel.id);
        }
    }
}
