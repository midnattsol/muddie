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

    /// Its keys, before the field in it gets them: Escape closes it, the
    /// arrows choose, which is said aloud, Enter takes the one chosen and
    /// Tab completes it. Returns whether it is done with for this frame.
    fn palette_keys(&mut self, ctx: &egui::Context, items: &[Item]) -> bool {
        let (up, down, tab, escape, enter) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
                input.consume_key(Modifiers::NONE, Key::Tab),
                input.consume_key(Modifiers::NONE, Key::Escape),
                input.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if escape {
            self.close_palette();
            return true;
        }
        if (up || down) && !items.is_empty() {
            self.move_choice(items, up);
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

    /// Chooses the one above or below, round the ends, and says it aloud
    /// for those who do not see the list.
    fn move_choice(&mut self, items: &[Item], up: bool) {
        let last = items.len() - 1;
        self.palette.chosen = match (up, self.palette.chosen) {
            (true, 0) => last,
            (true, chosen) => chosen - 1,
            (false, chosen) if chosen == last => 0,
            (false, chosen) => chosen + 1,
        };
        let chosen = &items[self.palette.chosen];
        self.voice.stop();
        self.voice
            .say(&format!("{}. {}", shown_usage(&chosen.usage), chosen.about));
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
        const ROWS: usize = 8;
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
                    self.palette_field(ui);
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
                    let first = (self.palette.chosen.saturating_sub(ROWS - 1))
                        .min(items.len().saturating_sub(ROWS));
                    for (index, item) in items.iter().enumerate().skip(first).take(ROWS) {
                        if palette_row(ui, item, index == self.palette.chosen) {
                            clicked = Some(index);
                        }
                    }
                });
            });
        (area.response, clicked)
    }

    /// What to look for: the field, which keeps the keyboard while the
    /// palette is open.
    fn palette_field(&mut self, ui: &mut egui::Ui) {
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
                TextEdit::singleline(&mut self.palette.query)
                    .id(field)
                    .frame(egui::Frame::NONE)
                    .font(theme::font(Weight::Regular, 14.5))
                    .hint_text(hint)
                    .lock_focus(true)
                    .desired_width(f32::INFINITY)
                    .show(ui)
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
/// at the right, marked when chosen. Returns whether it was clicked.
fn palette_row(ui: &mut egui::Ui, item: &Item, chosen: bool) -> bool {
    const ROW: f32 = 32.0;
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
    row.clicked()
}
