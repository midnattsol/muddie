//! The editor's page: its sections at the left, the list of what there is
//! in the one chosen, and the details of the one chosen in the list.

use super::*;

/// How wide the details of a rule are at most: wider, lines are long to
/// read and controls far from their names.
const DETAIL_WIDTH: f32 = 760.0;

/// A section of the editor by its name.
pub(in crate::tab) fn section_name(section: usize) -> &'static str {
    match section {
        TRIGGERS => tr("Triggers"),
        ALIASES => tr("Aliases"),
        MACROS => tr("Macros"),
        TIMERS => tr("Timers"),
        GAUGES => tr("Gauges"),
        VARIABLES => tr("Variables"),
        SCRIPTS => tr("Scripts"),
        _ => tr("Incoming data"),
    }
}

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
        // in, several chosen, a menu open and a drag; only then the editor.
        // Whether a field was being typed in is as it was before this
        // frame: Escape has already taken it from the field.
        self.history_keys(ui);
        let leaving = !self.editor.recording_key
            && !self.editor.was_typing
            && self.editor.marked.len() < 2
            && self.editor.dragged.is_none()
            && widgets::take_escape(ui);
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
            let card = egui::CornerRadius::same(14);
            ui.painter().rect_filled(columns.list, card, theme.field);
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
            // Between the list and the details, where the keyboard goes.
            self.pane_handle(ui, columns.handle);
            // The details stand on the page, in cards as the settings
            // have, no wider than reads well.
            let inner = columns.detail.shrink2(egui::vec2(14.0, 6.0));
            self.pane(ui, "detail", inner, false, |tab, ui| {
                tab.unprofiled_banner(ui);
                tab.undo_bar(ui);
                tab.editor.detail_height = inner.height();
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        widgets::keep_clear_of_bar(ui);
                        ui.set_max_width(ui.available_width().min(DETAIL_WIDTH));
                        let top = ui.cursor().top();
                        tab.automation_detail(ui);
                        // What is left empty under the details, for an
                        // editor of code to take.
                        let used = ui.cursor().top() - top;
                        tab.editor.code.spare = ui.clip_rect().height() - used - 12.0;
                    });
            });
        });
        // An editor of code that gave Tab back after Escape has the
        // keyboard but is not typed in: Escape again leaves the editor.
        let focused = ui.memory(|memory| memory.focused());
        self.editor.was_typing = focused.is_some() && focused != widgets::escaped_editor(ui.ctx());
        self.rules_autosave(ctx, !leaving);
    }

    /// Ctrl+Z undoes the last change to the rules, and Ctrl+Shift+Z or
    /// Ctrl+Y redoes it: in a field of a rule too, where the typing is a
    /// step. Where what is typed is not a rule (a script, a variable),
    /// the field's own Ctrl+Z undoes it; and a script or a variable deleted
    /// is had back from its bar. The keyboard stays in the list if it was.
    fn history_keys(&mut self, ui: &mut egui::Ui) {
        let section = self.editor.section;
        let field = automation::typing_in(ui.ctx()).is_some();
        let rules = matches!(section, TRIGGERS | ALIASES | MACROS | TIMERS | GAUGES);
        let disk_bar = (self.editor.undo.as_ref())
            .is_some_and(|undo| !matches!(undo.what, super::undo::Undone::Step(_)));
        if self.editor.recording_key || (field && !rules) {
            return;
        }
        let (again, back) = ui.input_mut(|input| {
            let again = input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                || input.consume_key(Modifiers::COMMAND, Key::Y);
            let back = !disk_bar && input.consume_key(Modifiers::COMMAND, Key::Z);
            (again, back)
        });
        if !again && !back {
            return;
        }
        let focused = ui.memory(|memory| memory.focused());
        let in_list = (self.current_spot())
            .is_some_and(|spot| focused == Some(list::item_id(section, &spot)));
        self.undo_step(again);
        if in_list && let Some(spot) = self.current_spot() {
            let id = list::item_id(self.editor.section, &spot);
            ui.memory_mut(|memory| memory.request_focus(id));
        }
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
        let edge = widgets::edge(ui, handle, id, tr("Width of the list"), true);
        self.editor.panes[1] += edge.moved;
        if edge.by_keys {
            let width = (self.editor.panes[1].round() as u32).to_string();
            self.voice.say_now(&trf("{0} points", &[&width]));
        }
        let response = edge.response;
        if response.hovered() || response.dragged() || response.has_focus() {
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
        let data = (DATA, section_name(DATA), self.data_view.seen.len());
        let rules =
            (0..counts.len()).map(|section| (section, section_name(section), counts[section]));
        for (section, name, count) in rules.chain([data]) {
            // What the rules are made from stands apart from them.
            if section == DATA {
                ui.add_space(10.0);
            }
            let selected = self.editor.section == section;
            let item = widgets::nav_item(ui, selected, name, Some(count));
            if selected
                && std::mem::take(&mut self.editor.focus_section)
                && widgets::keyboard_in_use(ui.ctx())
            {
                item.request_focus();
            }
            if item.clicked() {
                self.flush_script();
                self.editor.section = section;
                self.editor.folder = None;
                self.reveal_chosen();
                self.editor.marked.clear();
                self.editor.sample.clear();
                self.editor.recording_key = false;
            }
        }
        self.sidebar_tags(ui);
        self.sidebar_foot(ui);
    }

    /// The foot of the sections, kept at the bottom: how the scripts are,
    /// when they were last started over and what failed then, and the
    /// button that starts them over. What is edited is saved by itself,
    /// and only saving that fails is said, as any failure is.
    fn sidebar_foot(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        const TALL: f32 = 58.0;
        let rest = ui.available_rect_before_wrap();
        let place = egui::Rect::from_min_max(
            egui::pos2(rest.left(), (rest.bottom() - TALL).max(rest.top() + 12.0)),
            rest.max,
        );
        let mut foot = ui.new_child(egui::UiBuilder::new().max_rect(place));
        let failed = self.editor.failed.len();
        let (state, colour) = match failed {
            0 => (tr("Scripts running").to_owned(), theme.good),
            count => (
                trn("{0} script failed", "{0} scripts failed", count as u64),
                theme.danger,
            ),
        };
        let when = (self.editor.restarted.as_ref()).map(|at| trf("Restarted at {0}", &[at]));
        let mut restart = false;
        crate::widgets::inset(theme.field)
            .show(&mut foot, |ui| {
                ui.set_width(ui.available_width());
                let id = ui.make_persistent_id("sidebar foot");
                widgets::right_last(
                    ui,
                    id,
        &mut (),
                    |_, ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.horizontal(|ui| {
                                let size = egui::vec2(8.0, 8.0);
                                let (dot, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                ui.painter().circle_filled(dot.center(), 3.5, colour);
                                let state = RichText::new(&state)
                                    .font(theme::font(Weight::Medium, 12.5))
                                    .color(theme.text);
                                ui.add(Label::new(state).selectable(false));
                            });
                            if let Some(when) = &when {
                                let when = RichText::new(when)
                                    .font(theme::font(Weight::Regular, 11.5))
                                    .color(theme.text_faint);
                                ui.add(Label::new(when).selectable(false));
                            }
                        });
                    },
                    |_, ui| {
                        restart = widgets::glyph_button(ui, widgets::Glyph::Restart, tr("Restart scripts"))
                            .on_hover_text(tr(
                                "Runs every script and rule again, as when a session starts. The variables are kept.",
                            ))
                            .clicked();
                    },
                );
            });
        if restart {
            self.restart_scripts();
        }
    }
}
