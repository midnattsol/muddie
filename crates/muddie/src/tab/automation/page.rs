//! The editor's page: its sections at the left, the list of what there is
//! in the one chosen, and the details of the one chosen in the list.

use super::*;

/// Where the parts of the editor's page go.
struct Columns {
    sections: egui::Rect,
    list: egui::Rect,
    handle: egui::Rect,
    detail: egui::Rect,
}

impl Tab {
    /// The automation view, which takes the session's whole space: the
    /// sections, the list of the current one, and the selected item.
    /// The editor's page: the sections standing alone at the left, then
    /// the list and the details as two cards, with the gap between them to
    /// drag to trade their widths. Escape goes back to the game, unless it
    /// is cancelling a key.
    pub(in crate::tab) fn automation_page(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        let theme = theme::current();
        // Escape first gives up a key being recorded, a field being typed
        // in, and several chosen; only then the editor. Whether a field was
        // being typed in is as it was before this frame: Escape has already
        // taken it from the field.
        let leaving = !self.editor.recording_key
            && !self.editor.was_typing
            && self.editor.marked.len() < 2
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
        let page = egui::Frame::new()
            .fill(theme.surface)
            .inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 2,
                bottom: 8,
            });
        CentralPanel::default().frame(page).show(ui, |ui| {
            let columns = self.columns(ui.available_rect_before_wrap());
            self.pane_handle(ui, columns.handle);
            let card = egui::CornerRadius::same(14);
            ui.painter().rect_filled(columns.list, card, theme.field);
            ui.painter().rect_filled(columns.detail, card, theme.field);
            self.pane(
                ui,
                "sections",
                columns.sections.shrink2(egui::vec2(2.0, 8.0)),
                false,
                |tab, ui| tab.automation_sections(ui),
            );
            self.pane(
                ui,
                "list",
                columns.list.shrink2(egui::vec2(10.0, 12.0)),
                true,
                |tab, ui| tab.automation_list(ui),
            );
            let inner = columns.detail.shrink2(egui::vec2(26.0, 18.0));
            self.pane(ui, "detail", inner, true, |tab, ui| {
                tab.undo_bar(ui);
                tab.editor.detail_height = inner.height();
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| tab.automation_detail(ui));
            });
        });
        self.editor.was_typing = ui.memory(|memory| memory.focused().is_some());
        self.rules_autosave(ctx, !leaving);
    }

    /// Where the sections, the list, the handle between it and the details,
    /// and the details go, with the list as wide as it was left within
    /// reason.
    fn columns(&mut self, full: egui::Rect) -> Columns {
        const SECTIONS: f32 = 208.0;
        const AIR: f32 = 22.0;
        const HANDLE: f32 = 12.0;
        let most = (full.width() - SECTIONS - AIR - HANDLE - 460.0).max(240.0);
        self.editor.panes[1] = self.editor.panes[1].clamp(240.0, most);
        let column = |left: f32, width: f32| {
            egui::Rect::from_min_max(
                egui::pos2(left, full.top()),
                egui::pos2(left + width, full.bottom()),
            )
        };
        let sections = column(full.left(), SECTIONS);
        let list = column(sections.right() + AIR, self.editor.panes[1]);
        let handle = column(list.right(), HANDLE);
        let detail = column(handle.right(), full.right() - handle.right());
        Columns {
            sections,
            list,
            handle,
            detail,
        }
    }

    /// The gap between the list and the details, dragged to trade their
    /// widths, with a short grip only while it can be taken.
    fn pane_handle(&mut self, ui: &mut egui::Ui, handle: egui::Rect) {
        let id = ui.make_persistent_id("automation handle");
        let response = ui
            .interact(handle, id, egui::Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Splitter, true, tr("Resize")));
        if response.dragged() {
            self.editor.panes[1] += response.drag_delta().x;
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if response.hovered() || response.dragged() {
            let grip = handle.center();
            let stroke = egui::Stroke::new(3.0, theme::current().accent);
            ui.painter().line_segment(
                [grip - egui::vec2(0.0, 18.0), grip + egui::vec2(0.0, 18.0)],
                stroke,
            );
        }
    }

    /// One pane of the page, under its own name, or what one remembers (how
    /// far it is scrolled, say) would be the other's too. On a card, fields
    /// stand out from it.
    fn pane(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        rect: egui::Rect,
        raised: bool,
        add: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        let builder = egui::UiBuilder::new()
            .id_salt(("automation pane", name))
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min));
        ui.scope_builder(builder, |ui| {
            ui.set_clip_rect(rect.intersect(ui.clip_rect()));
            if raised {
                let surface = theme::current().surface;
                ui.visuals_mut().text_edit_bg_color = Some(surface);
                ui.visuals_mut().extreme_bg_color = surface;
            }
            add(self, ui);
        });
    }

    pub(in crate::tab) fn automation_sections(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 4.0;
        let heading = RichText::new(tr("Editor"))
            .font(theme::font(Weight::Semibold, 17.0))
            .color(theme::current().text);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.add(Label::new(heading).selectable(false));
        });
        ui.add_space(16.0);
        let counts = [
            self.editor.draft.triggers.len(),
            self.editor.draft.aliases.len(),
            self.editor.draft.macros.len(),
            self.editor.draft.timers.len(),
            self.editor.draft.gauges.len(),
            self.scripts.variable_count(),
            self.editor.script_files.len(),
        ];
        let names = [
            tr("Triggers"),
            tr("Aliases"),
            tr("Macros"),
            tr("Timers"),
            tr("Gauges"),
            tr("Variables"),
            tr("Scripts"),
        ];
        let data = (DATA, tr("Incoming data"), self.data_view.seen.len());
        let rules = names
            .into_iter()
            .enumerate()
            .map(|(section, name)| (section, name, counts[section]));
        for (section, name, count) in rules.chain([data]) {
            // What the rules are made from stands apart from them.
            if section == DATA {
                ui.add_space(10.0);
            }
            let selected = self.editor.section == section;
            if widgets::nav_item(ui, selected, name, Some(count)).clicked() {
                self.flush_script();
                self.editor.section = section;
                self.editor.marked.clear();
                self.editor.sample.clear();
                self.editor.recording_key = false;
            }
        }
        self.sidebar_tags(ui);
        ui.add_space(18.0);
        self.sidebar_foot(ui);
    }

    /// The foot of the sections: restarting the scripts, with when that
    /// was last done and what failed then. What is edited is saved by
    /// itself, and only saving that fails is said, as any failure is.
    fn sidebar_foot(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        let small = |text: &str, colour| {
            RichText::new(text)
                .font(theme::font(Weight::Regular, 11.5))
                .color(colour)
        };
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let restart = widgets::button(ui, tr("Restart scripts")).on_hover_text(tr(
                "Runs every script and rule again, as when a session starts. The variables are kept.",
            ));
            if restart.clicked() {
                self.restart_scripts();
            }
        });
        let Some(at) = &self.editor.restarted else {
            return;
        };
        let (said, colour) = match self.editor.failed.len() {
            0 => (trf("Restarted at {0}", &[at]), theme.text_faint),
            count => (
                trn(
                    "Restarted at {1}; {0} failed",
                    "Restarted at {1}; {0} failed",
                    count as u64,
                )
                .replace("{1}", at),
                theme.danger,
            ),
        };
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(small(&said, colour));
        });
    }
}
