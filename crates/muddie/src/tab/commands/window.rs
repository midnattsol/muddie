//! The palette over the window: its field, its list, and its keys.

use super::*;
use crate::widgets;

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
        self.say_found(ctx, &items);
        let (area, clicked) = self.palette_card(ctx, &items, shown);
        if let Some(index) = clicked {
            self.palette.focus = true;
            return self.choose(&items[index].clone(), ctx);
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
        // With Shift first: without it, the plain one would take it too.
        let way = pressed(Modifiers::SHIFT, Key::Enter);
        let (tab, escape, enter) = (
            pressed(Modifiers::NONE, Key::Tab),
            pressed(Modifiers::NONE, Key::Escape),
            pressed(Modifiers::NONE, Key::Enter),
        );
        // Escape stops asking, back to the search; else it closes.
        if escape {
            match self.palette.asking.is_some() {
                true => self.stop_asking(),
                false => self.close_palette(),
            }
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
            // While asking, what is typed is the answer; with nothing typed,
            // what is asked is said again.
            if let (true, Some(asking)) = (enter, self.palette.asking.clone()) {
                match self.palette.query.trim().is_empty() {
                    true => self
                        .voice
                        .say_now(&trf("{0}: write it, then Enter.", &[&asking.title])),
                    false => {
                        let answer = self.palette.query.clone();
                        self.answer(&answer);
                    }
                }
                return true;
            }
            // Nothing to do: it is said, and nothing moves.
            if enter || way {
                self.voice.say_now(tr("No matches."));
            }
            return false;
        };
        // Shift and Enter on a place says the way there, without going.
        if way && let Some(Act::Place(name)) = &chosen.act {
            let line = format!("/map way {name}");
            self.palette.query.clear();
            self.run_command_from(&line, true);
            return true;
        }
        if enter || way {
            self.choose(&chosen, ctx);
            return true;
        }
        // While asking, Tab puts the answer chosen in the field, to change.
        if tab && self.palette.asking.is_some() {
            self.palette.query.clone_from(&chosen.usage);
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
        self.say_chosen(items);
    }

    /// Says the one chosen aloud: the group, when it is another than the
    /// one said last; the thing, what it is or is for, and where it is in
    /// the list.
    fn say_chosen(&mut self, items: &[Item]) {
        let Some(chosen) = items.get(self.palette.chosen) else {
            return self.voice.say_now(tr("No matches."));
        };
        let group = chosen.group.clone();
        let mut said = String::new();
        if !group.is_empty() && self.palette.spoken_group.as_ref() != Some(&group) {
            said = format!("{group}. ");
        }
        self.palette.spoken_group = Some(group);
        said.push_str(&chosen.usage);
        if !chosen.about.is_empty() {
            said = format!("{said}. {}", chosen.about);
        }
        let place = trf(
            "{0} of {1}",
            &[
                &(self.palette.chosen + 1).to_string(),
                &items.len().to_string(),
            ],
        );
        self.voice.say_now(&format!("{said}. {place}"));
    }

    /// Once what is typed has been left as it is for a moment, says what it
    /// found first, or that it found nothing: what is seen at a glance.
    fn say_found(&mut self, ctx: &egui::Context, items: &[Item]) {
        const PAUSE: std::time::Duration = std::time::Duration::from_millis(500);
        let Some(typed_at) = self.palette.typed_at else {
            return;
        };
        let waited = typed_at.elapsed();
        if waited < PAUSE {
            return ctx.request_repaint_after(PAUSE - waited);
        }
        self.palette.typed_at = None;
        self.say_chosen(items);
    }

    /// The card at the top of the window: the field; under it what a
    /// command answered, or the list, in groups, around the one chosen; and
    /// the keys at its foot. Returns its area's response, and the row
    /// clicked.
    fn palette_card(
        &mut self,
        ctx: &egui::Context,
        items: &[Item],
        shown: f32,
    ) -> (egui::Response, Option<usize>) {
        let theme = theme::current();
        let screen = ctx.content_rect();
        let width = (screen.width() - 48.0).min(WIDTH);
        let place = egui::pos2(screen.center().x, screen.top() + 64.0 - (1.0 - shown) * 8.0);
        // As tall as the window leaves it, beneath the bar at the top.
        let most = (screen.height() - 220.0).clamp(4.0 * ROW, ROWS as f32 * ROW);
        let card = egui::Frame::new()
            .fill(theme.raised)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(egui::Margin::same(6))
            .shadow(egui::Shadow {
                offset: [0, 12],
                blur: 36,
                spread: 0,
                color: egui::Color32::from_black_alpha(if theme.dark { 120 } else { 40 }),
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
                    let rule = |ui: &mut egui::Ui| {
                        let line = ui.available_rect_before_wrap();
                        let stroke = egui::Stroke::new(1.0, theme.border);
                        ui.painter().hline(line.x_range(), line.top(), stroke);
                    };
                    rule(ui);
                    ui.add_space(6.0);
                    // As tall whatever it shows, a list or a line: it does
                    // not jump as it changes.
                    let body = egui::vec2(ui.available_width(), most);
                    let top = egui::Layout::top_down(egui::Align::Min);
                    ui.allocate_ui_with_layout(body, top, |ui| {
                        ui.set_min_height(most);
                        if let Some((text, problem)) = self.palette.message.clone() {
                            answered(ui, &text, problem);
                        }
                        // What is asked stays in sight over what may answer
                        // it, typed or not.
                        let asked = self.palette.asking.as_ref().map(|a| a.title.clone());
                        if let Some(title) = asked.clone().or_else(|| self.palette.title.clone()) {
                            group_heading(ui, &title, true);
                        }
                        if items.is_empty() && self.palette.message.is_none() {
                            let note = match &asked {
                                Some(_) => tr("Write it, then Enter."),
                                None => tr("No matches."),
                            };
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                widgets::page_note(ui, note);
                            });
                            ui.add_space(10.0);
                        }
                        if !items.is_empty() {
                            // A list to scroll, by the wheel or its bar too, as
                            // tall as a page of it at most.
                            let reveal = std::mem::take(&mut self.palette.reveal);
                            let grouped =
                                self.palette.title.is_none() && self.palette.asking.is_none();
                            egui::ScrollArea::vertical()
                                .id_salt("palette list")
                                .max_height(most)
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    crate::widgets::keep_clear_of_bar(ui);
                                    // How long it is, told to a screen reader.
                                    let found =
                                        trn("{0} result", "{0} results", items.len() as u64);
                                    let list = egui::UiBuilder::new()
                                        .accessibility_role(egui::accesskit::Role::List)
                                        .accessibility_label(found);
                                    ui.scope_builder(list, |ui| {
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        let mut group = None;
                                        for (index, item) in items.iter().enumerate() {
                                            let this = item.group.clone();
                                            if grouped
                                                && !this.is_empty()
                                                && group.as_ref() != Some(&this)
                                            {
                                                group_heading(ui, &this, false);
                                                group = Some(this);
                                            }
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
                        }
                    });
                    ui.add_space(4.0);
                    rule(ui);
                    keys_at_the_foot(
                        ui,
                        items.get(self.palette.chosen),
                        self.palette.asking.is_some(),
                    );
                });
            });
        (area.response, clicked)
    }

    /// What to look for: the field, after the picture of looking, which
    /// keeps the keyboard while the palette is open.
    fn palette_field(&mut self, ui: &mut egui::Ui) {
        let field = egui::Id::unique("palette field");
        let listed = self.palette.results.is_some() && self.palette.query.is_empty();
        let asked = self
            .palette
            .asking
            .as_ref()
            .map(|asking| asking.title.clone());
        let hint = match &asked {
            Some(title) => title.as_str(),
            None if listed || self.palette.message.is_some() => tr("Choose one"),
            None => tr("Search for something to do, or a place"),
        };

        let edit = egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 10,
                bottom: 10,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    // Named for a screen reader: what it asks, while it asks.
                    let label = asked.clone().unwrap_or_else(|| tr("Palette").to_owned());
                    let look = widgets::SearchLook {
                        hint,
                        label: &label,
                        size: 15.0,
                        width: f32::INFINITY,
                        id: Some(field),
                        lock_focus: true,
                    };
                    widgets::search_field(ui, &mut self.palette.query, look)
                })
                .inner
            })
            .inner;
        if edit.changed() {
            self.palette.chosen = 0;
            self.palette.spoken_group = None;
            self.palette.results = None;
            self.palette.title = None;
            self.palette.message = None;
            self.palette.typed_at = Some(Instant::now());
        }
        if std::mem::take(&mut self.palette.cursor_end) {
            let end = egui::text::CCursor::new(self.palette.query.chars().count());
            let mut state = TextEdit::load_state(ui.ctx(), field).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(end)));
            state.store(ui.ctx(), field);
        }
        if std::mem::take(&mut self.palette.focus) || !edit.has_focus() {
            edit.request_focus();
        }
    }
}

/// The name of a group of the list, or of what the list answers, over it.
fn group_heading(ui: &mut egui::Ui, text: &str, asked: bool) {
    let theme = theme::current();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        // What is asked, or answered, is written as said; a group's name
        // smaller, in capitals.
        let font = match asked {
            true => theme::font(Weight::Medium, 13.0),
            false => theme::font(Weight::Medium, 11.0),
        };
        let text = match asked {
            true => text.to_owned(),
            false => text.to_uppercase(),
        };
        ui.add(
            egui::Label::new(RichText::new(text).font(font).color(theme.text_faint))
                .selectable(false),
        );
    });
    ui.add_space(4.0);
}

/// What was done from the palette answered, in a line of its own,
/// marked as done or as something that could not be done.
fn answered(ui: &mut egui::Ui, text: &str, problem: bool) {
    let theme = theme::current();
    let (glyph, colour) = match problem {
        true => (widgets::Glyph::Warning, theme.danger),
        false => (widgets::Glyph::Check, theme.good),
    };
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(14, 8))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (mark, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 18.0), egui::Sense::hover());
                widgets::paint_glyph(
                    ui.painter(),
                    mark.center(),
                    egui::Align2::CENTER_CENTER,
                    glyph,
                    13.0,
                    colour,
                );
                let words = RichText::new(text)
                    .font(theme::font(Weight::Regular, 14.0))
                    .color(theme.text);
                ui.add(egui::Label::new(words).wrap());
            });
        });
}

/// The keys of the palette, at its foot, each with what it does: those
/// that do something with the one chosen, and Tab while it asks.
fn keys_at_the_foot(ui: &mut egui::Ui, chosen: Option<&Item>, asking: bool) {
    let theme = theme::current();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        let place = chosen.is_some_and(|item| matches!(item.act, Some(Act::Place(_))));
        let more = asking && chosen.is_some();
        let keys = [
            Some(("↑ ↓", tr("choose"))),
            chosen.map(|_| (tr("Enter"), if place { tr("go") } else { tr("run") })),
            place.then(|| (tr("Shift+Enter"), tr("the way"))),
            more.then(|| ("Tab", tr("complete"))),
            Some((tr("Escape"), tr("close"))),
        ];
        for (keys, does) in keys.into_iter().flatten() {
            let chip = RichText::new(keys)
                .font(theme::font(Weight::Medium, 11.0))
                .color(theme.text_dim);
            egui::Frame::new()
                .stroke(egui::Stroke::new(1.0, theme.border))
                .corner_radius(egui::CornerRadius::same(4))
                .inner_margin(egui::Margin::symmetric(5, 1))
                .show(ui, |ui| ui.add(egui::Label::new(chip).selectable(false)));
            let words = RichText::new(does)
                .font(theme::font(Weight::Regular, 11.5))
                .color(theme.text_faint);
            ui.add(egui::Label::new(words).selectable(false));
            ui.add_space(8.0);
        }
    });
    ui.add_space(6.0);
}

/// A row of the palette's list: its picture, what it is, whole, and for a
/// command what it is for after it, cut short if it must be; the keys that
/// do the same at the right. Marked when chosen.
fn palette_row(ui: &mut egui::Ui, item: &Item, chosen: bool) -> egui::Response {
    let theme = theme::current();
    let (rect, row) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW), egui::Sense::click());
    let about = item.about.as_str();
    row.widget_info(|| {
        let usage = &item.usage;
        let said = match about.is_empty() {
            true => usage.clone(),
            false => format!("{usage}. {about}"),
        };
        egui::WidgetInfo::selected(egui::Role::ListItem, true, chosen, said)
    });
    if chosen || row.hovered() {
        let fill = match chosen {
            true => theme.accent.gamma_multiply(0.18),
            false => theme.hover.gamma_multiply(0.6),
        };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), fill);
    }
    let inside = rect.shrink2(egui::vec2(14.0, 0.0));
    let (glyph, keys) = look_of(item);
    let painter = ui.painter().with_clip_rect(inside);
    // The keys first, at the right, for the rest to know its room.
    let mut end = inside.right();
    if !keys.is_empty() {
        let shown = painter.text(
            egui::pos2(end - 5.0, inside.center().y),
            egui::Align2::RIGHT_CENTER,
            keys,
            theme::font(Weight::Medium, 11.5),
            theme.text_dim,
        );
        let chip = shown.expand2(egui::vec2(5.0, 2.0));
        painter.rect_stroke(
            chip,
            4.0,
            egui::Stroke::new(1.0, theme.border),
            egui::StrokeKind::Inside,
        );
        end = chip.left() - 12.0;
    }
    widgets::paint_glyph(
        &painter,
        inside.left_center() + egui::vec2(8.0, 0.0),
        egui::Align2::CENTER_CENTER,
        glyph,
        13.0,
        if chosen {
            theme.accent
        } else {
            theme.text_faint
        },
    );
    // What it is, whole: the words of a command typed as they are, and
    // what it is given dimmer.
    let start = inside.left() + 28.0;
    let mut job = egui::text::LayoutJob::default();
    let typed = theme::font(Weight::Medium, 14.0);
    for (index, word) in item.usage.split(' ').enumerate() {
        let given = word.starts_with(['<', '[']);
        let colour = match given {
            true => theme.text_faint,
            false => theme.text,
        };
        let space = if index == 0 { "" } else { " " };
        job.append(
            &format!("{space}{word}"),
            0.0,
            egui::TextFormat::simple(typed.clone(), colour),
        );
    }
    let usage = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let at = egui::pos2(start, inside.center().y - usage.size().y / 2.0);
    let width = usage.size().x;
    ui.painter()
        .with_clip_rect(inside.with_max_x(end))
        .galley(at, usage, theme.text);
    // What it is for, in what room is left, cut short with an ellipsis.
    let room = end - (start + width + 14.0);
    if !about.is_empty() && room > 40.0 {
        let mut job = egui::text::LayoutJob::single_section(
            about.to_owned(),
            egui::TextFormat::simple(theme::font(Weight::Regular, 13.0), theme.text_faint),
        );
        job.wrap = egui::text::TextWrapping {
            max_width: room,
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
        let about = ui.fonts_mut(|fonts| fonts.layout_job(job));
        let at = egui::pos2(
            start + width + 14.0,
            inside.center().y - about.size().y / 2.0,
        );
        ui.painter().galley(at, about, theme.text_faint);
    }
    row
}

/// The picture of something the palette lists, by what it does, and the
/// keys that do the same, if there are some.
fn look_of(item: &Item) -> (widgets::Glyph, &'static str) {
    use widgets::Glyph;
    let Some(act) = &item.act else {
        return (Glyph::Script, "");
    };
    match act {
        Act::View(View::Game) => (Glyph::Send, ""),
        Act::View(View::Editor) => (Glyph::Code, crate::shortcuts::SWITCH_VIEW.written),
        Act::View(View::Layout) => (Glyph::Panel, ""),
        Act::View(View::Data) => (Glyph::Data, crate::shortcuts::DATA_VIEW.written),
        Act::Section(_) => (Glyph::Code, ""),
        Act::App(request) => match request {
            AppRequest::NewSession => (Glyph::Plus, crate::shortcuts::NEW_SESSION.written),
            AppRequest::CloseSession => (Glyph::Close, crate::shortcuts::CLOSE_SESSION.written),
            AppRequest::Settings(_) => (Glyph::Settings, crate::shortcuts::SETTINGS.written),
            AppRequest::Shortcuts => (Glyph::Key, ""),
            AppRequest::Profiles
            | AppRequest::Profile(..)
            | AppRequest::SaveAsProfile
            | AppRequest::Play(_) => (Glyph::Profile, ""),
            _ => (Glyph::Check, ""),
        },
        Act::Reconnect => (Glyph::Restart, crate::shortcuts::RECONNECT.written),
        Act::Disconnect => (Glyph::Close, ""),
        Act::Find => (Glyph::Search, crate::shortcuts::FIND.written),
        Act::CopyOutput => (Glyph::Copy, ""),
        Act::Log => (Glyph::Text, ""),
        Act::Speech | Act::ReadChannel(..) => (Glyph::Say, ""),
        Act::Panel(_) => (Glyph::Panel, ""),
        Act::Place(_) | Act::MapAction(_) | Act::Look(_) | Act::AllAreas => (Glyph::Map, ""),
        Act::Ask(_) | Act::Answer(_) => (Glyph::Search, ""),
        Act::Run(command) if command.starts_with("/map") => (Glyph::Map, ""),
        Act::Run(_) => (Glyph::Check, ""),
    }
}

/// How wide the palette is at most, how tall a row of the list is, and how
/// many make a page of it.
const WIDTH: f32 = 720.0;
const ROW: f32 = 36.0;
const ROWS: usize = 12;

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
