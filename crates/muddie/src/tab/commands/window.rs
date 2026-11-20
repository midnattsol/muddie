//! The palette over the window: its field, its list, and its keys.

use super::*;

impl Tab {
    /// The palette over the window while it is open: the keys first, then
    /// its card, then a click on one of its rows or anywhere else.
    pub(in crate::tab) fn palette(&mut self, ctx: &egui::Context) {
        let shown =
            ctx.animate_bool_with_time(egui::Id::unique("palette shown"), self.palette.open, 0.1);
        if !self.palette.open {
            return;
        }
        let items = self.palette_items();
        self.palette.chosen = self.palette.chosen.min(items.len().saturating_sub(1));
        if self.palette_keys(ctx, &items) {
            return;
        }
        let (area, clicked) = self.palette_card(ctx, &items, shown);
        if let Some(index) = clicked {
            self.palette.focus = true;
            return self.choose(&items[index].clone(), false, ctx);
        }
        // A click anywhere else closes it; not the one that opened it,
        // from the field at the top, in the frame it opened.
        let fresh = std::mem::take(&mut self.palette.fresh);
        if !fresh && area.clicked_elsewhere() {
            self.close_palette();
        }
    }

    /// Its keys, before the field in it gets them: Escape closes it; the
    /// arrows choose one up or down, round the ends, Page Up and Page Down
    /// a page, stopping at the ends, Ctrl+Home and Ctrl+End the first and
    /// the last, which is said aloud with where it is; Enter takes the one
    /// chosen and Tab completes it. Home and End are the field's. Returns
    /// whether it is done with for this frame.
    fn palette_keys(&mut self, ctx: &egui::Context, items: &[Item]) -> bool {
        let pressed = |modifiers: Modifiers, key: Key| {
            ctx.input_mut(|input| input.consume_key(modifiers, key))
        };
        // Those with Ctrl first: without it, the plain ones would take
        // them too.
        let (first, last) = (
            pressed(Modifiers::COMMAND, Key::Home),
            pressed(Modifiers::COMMAND, Key::End),
        );
        let (up, down) = (
            pressed(Modifiers::NONE, Key::ArrowUp),
            pressed(Modifiers::NONE, Key::ArrowDown),
        );
        let (page_up, page_down) = (
            pressed(Modifiers::NONE, Key::PageUp),
            pressed(Modifiers::NONE, Key::PageDown),
        );
        let (tab, escape, enter) = (
            pressed(Modifiers::NONE, Key::Tab),
            pressed(Modifiers::NONE, Key::Escape),
            pressed(Modifiers::NONE, Key::Enter),
        );
        if escape {
            self.close_palette();
            return true;
        }
        let step = match (up, down, page_up, page_down, first, last) {
            (true, ..) => Some(Step::One(true)),
            (_, true, ..) => Some(Step::One(false)),
            (_, _, true, ..) => Some(Step::Page(true)),
            (_, _, _, true, ..) => Some(Step::Page(false)),
            (_, _, _, _, true, _) => Some(Step::End(true)),
            (_, _, _, _, _, true) => Some(Step::End(false)),
            _ => None,
        };
        if let (Some(step), false) = (step, items.is_empty()) {
            self.move_choice(items, step);
        }
        let Some(chosen) = items.get(self.palette.chosen).cloned() else {
            return false;
        };
        let typed = format!("/{}", self.palette.query.trim_start_matches('/'));
        let typed_more = typed.trim_end().len() > chosen.insert.trim_end().len();
        if enter {
            self.choose(&chosen, typed_more, ctx);
            return true;
        }
        // Tab completes, and there is nothing to complete in a list.
        let listed = self.palette.results.is_some() && self.palette.query.is_empty();
        if tab && !listed && !typed_more {
            self.palette.query = chosen.insert.trim_start_matches('/').to_owned();
            self.palette.chosen = 0;
            self.palette.cursor_end = true;
        }
        false
    }

    /// Chooses another as `step` says, brings it into view, and says it
    /// aloud with where it is in the list, for those who do not see it.
    fn move_choice(&mut self, items: &[Item], step: Step) {
        let (last, chosen) = (items.len() - 1, self.palette.chosen);
        self.palette.chosen = match step {
            Step::One(true) if chosen == 0 => last,
            Step::One(true) => chosen - 1,
            Step::One(false) if chosen == last => 0,
            Step::One(false) => chosen + 1,
            Step::Page(true) => chosen.saturating_sub(ROWS - 1),
            Step::Page(false) => (chosen + ROWS - 1).min(last),
            Step::End(true) => 0,
            Step::End(false) => last,
        };
        self.palette.reveal = true;
        let chosen = &items[self.palette.chosen];
        let place = trf(
            "{0} of {1}",
            &[
                &(self.palette.chosen + 1).to_string(),
                &items.len().to_string(),
            ],
        );
        self.voice.say_now(&format!(
            "{}. {}. {place}",
            shown_usage(&chosen.usage),
            chosen.about
        ));
    }

    /// The card at the top of the window: the field, and a window of the
    /// list around the one chosen. Returns its area's response, and the row
    /// clicked.
    fn palette_card(
        &mut self,
        ctx: &egui::Context,
        items: &[Item],
        shown: f32,
    ) -> (egui::Response, Option<usize>) {
        let theme = theme::current();
        let screen = ctx.content_rect();
        let width = (screen.width() - 32.0).min(560.0);
        let place = egui::pos2(screen.center().x, screen.top() + 84.0 - (1.0 - shown) * 8.0);
        let card = egui::Frame::new()
            .fill(theme.raised)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(egui::Margin::same(6))
            .shadow(egui::Shadow {
                offset: [0, 10],
                blur: 30,
                spread: 0,
                color: egui::Color32::from_black_alpha(if theme.dark { 110 } else { 36 }),
            });
        let mut clicked = None;
        let area = egui::Area::new(egui::Id::unique("palette"))
            .order(egui::Order::Foreground)
            .fixed_pos(place)
            .pivot(egui::Align2::CENTER_TOP)
            .show(ctx, |ui| {
                ui.set_opacity(shown);
                card.show(ui, |ui| {
                    ui.set_width(width - 12.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    self.palette_field(ui, items.len());
                    if items.is_empty() {
                        return;
                    }
                    let line = ui.available_rect_before_wrap();
                    ui.painter().hline(
                        line.x_range(),
                        line.top(),
                        egui::Stroke::new(1.0, theme.border),
                    );
                    ui.add_space(6.0);
                    // A list to scroll, by the wheel or its bar too, as tall
                    // as a page of it at most: its bar and its faded edges
                    // say when there is more.
                    let reveal = std::mem::take(&mut self.palette.reveal);
                    egui::ScrollArea::vertical()
                        .id_salt("palette list")
                        .max_height(ROWS as f32 * ROW)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (index, item) in items.iter().enumerate() {
                                let chosen = index == self.palette.chosen;
                                let row = palette_row(ui, item, chosen);
                                if row.clicked() {
                                    clicked = Some(index);
                                }
                                if chosen && reveal {
                                    row.scroll_to_me(None);
                                }
                            }
                        });
                });
            });
        (area.response, clicked)
    }

    /// What to look for: the field, which keeps the keyboard while the
    /// palette is open.
    fn palette_field(&mut self, ui: &mut egui::Ui, count: usize) {
        let theme = theme::current();
        let field = egui::Id::unique("palette field");
        let listed = self.palette.results.is_some() && self.palette.query.is_empty();
        let hint = if listed {
            tr("Choose one, or type a command")
        } else {
            tr("Go to, open or run anything")
        };
        let hint = RichText::new(hint)
            .font(theme::font(Weight::Regular, 14.5))
            .color(theme.text_faint);
        let edit = egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // How many there are, at the right, while there are some.
                    let shown = match count {
                        0 => String::new(),
                        count => trn("{0} result", "{0} results", count as u64),
                    };
                    let counted =
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(shown)
                                    .font(theme::font(Weight::Regular, 12.0))
                                    .color(theme.text_faint),
                            );
                            TextEdit::singleline(&mut self.palette.query)
                                .id(field)
                                .frame(egui::Frame::NONE)
                                .font(theme::font(Weight::Regular, 14.5))
                                .hint_text(hint)
                                .lock_focus(true)
                                .desired_width(f32::INFINITY)
                                .show(ui)
                        });
                    counted.inner
                })
                .inner
            })
            .inner;
        if edit.response.changed() {
            self.palette.chosen = 0;
            self.palette.results = None;
        }
        if std::mem::take(&mut self.palette.cursor_end) {
            let end = egui::text::CCursor::new(self.palette.query.chars().count());
            let mut state = edit.state.clone();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(end)));
            state.store(ui.ctx(), field);
        }
        if std::mem::take(&mut self.palette.focus) || !edit.response.has_focus() {
            edit.response.request_focus();
        }
    }
}

/// A row of the palette's list: the thing itself at the left, what it is
/// at the right, marked when chosen.
fn palette_row(ui: &mut egui::Ui, item: &Item, chosen: bool) -> egui::Response {
    let theme = theme::current();
    let (rect, row) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW), egui::Sense::click());
    let usage = shown_usage(&item.usage);
    row.widget_info(|| {
        let said = match item.about.is_empty() {
            true => usage.to_owned(),
            false => format!("{usage}. {}", item.about),
        };
        egui::WidgetInfo::selected(egui::Role::ListItem, true, chosen, said)
    });
    if chosen || row.hovered() {
        let fill = if chosen {
            theme.hover
        } else {
            theme.hover.gamma_multiply(0.5)
        };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), fill);
    }
    let inside = rect.shrink2(egui::vec2(10.0, 0.0));
    let about = ui.painter().with_clip_rect(inside).text(
        inside.right_center(),
        egui::Align2::RIGHT_CENTER,
        &item.about,
        theme::font(Weight::Regular, 12.5),
        theme.text_faint,
    );
    let room = egui::Rect::from_min_max(inside.min, egui::pos2(about.left() - 14.0, inside.max.y));
    ui.painter().with_clip_rect(room).text(
        inside.left_center(),
        egui::Align2::LEFT_CENTER,
        usage,
        theme::font(Weight::Regular, 13.5),
        theme.text,
    );
    row
}

/// How tall a row of the list is, and how many make a page of it.
const ROW: f32 = 32.0;
const ROWS: usize = 8;

/// How the keys move the choice in the list.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// One up, or down, round the ends.
    One(bool),
    /// A page up, or down, stopping at the ends.
    Page(bool),
    /// The first, or the last.
    End(bool),
}
