//! The command line, and what is typed in it.

use super::*;

/// How long unterminated text must stay unchanged before triggers see it
/// as a line. Servers that do not mark their prompts leave them like that.
pub(super) const PARTIAL_SETTLE: Duration = Duration::from_millis(400);

impl Tab {
    pub(super) fn submit(&mut self) {
        let line = std::mem::take(&mut self.input);
        // The command stays, selected, to be sent again or typed over.
        // Never a password.
        let secret = self.typing_secret();
        self.asks_password = false;
        let keep = self.settings.keep_command && !secret && !line.is_empty();
        self.kept_input = keep.then(|| line.clone());
        if let Some(kept) = &self.kept_input {
            self.input.clone_from(kept);
            self.select_input = true;
        }
        // What follows answers this command, so drop what was still queued.
        self.voice.stop();
        if secret {
            // A password: scripts and the history never see it, nor is it
            // told as sent.
            self.send_secret(line);
        } else {
            // All of it is for the MUD, through the aliases: what the
            // client does is done from the palette.
            self.history.add(&line);
            self.scripts.on_input(&line);
            self.run_actions();
        }
    }

    /// Whether what is typed now is a password: the server hides it, or
    /// it has just asked for one.
    pub(super) fn typing_secret(&self) -> bool {
        self.input_hidden || self.asks_password
    }

    /// The command line: a field with the prompt mark before it, Tab to
    /// complete, and the arrows through the history.
    pub(super) fn input_line(&mut self, ui: &mut egui::Ui) {
        let id = ui.make_persistent_id("command input");
        let focused = ui.memory(|memory| memory.has_focus(id));
        // While walking, Escape in the empty line stops the walk at once,
        // unless it closes something open over it first. The line had the
        // keyboard in the frame before: Escape takes it away before this.
        let had_it = std::mem::replace(&mut self.line_focused, focused);
        if (focused || had_it)
            && self.walk.is_some()
            && self.input.is_empty()
            && crate::widgets::take_escape(ui)
        {
            self.stop_walking();
            ui.memory_mut(|memory| memory.request_focus(id));
        }
        let completing = self.complete_word(ui, focused);
        // A click on what takes no keyboard (the output, to choose text in
        // it; a button) leaves the keyboard here: given up and taken back,
        // the line would be drawn a frame without it, and its choice
        // blink. What takes the keyboard, a field, takes it all the same.
        let surrender = |ui: &egui::Ui, how| {
            ui.ctx()
                .options_mut(|options| options.input_options.surrender_focus_on = how)
        };
        if focused {
            surrender(ui, egui::SurrenderFocusOn::Never);
        }
        let mut output = self.command_field(ui, id, focused);
        surrender(ui, egui::SurrenderFocusOn::Clicks);
        self.place_cursor(ui, id, &mut output, completing);
        let response = output.response;
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(tr("Command")));
        if response.has_focus() {
            self.line_keys(ui);
        }
        let submitted = response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        if submitted {
            self.submit();
        }
        // The input owns the keyboard unless the user moved focus elsewhere,
        // or a dialog over it has it.
        let dialog = ui.memory(|memory| memory.top_modal_layer().is_some());
        if !self.palette.open
            && !dialog
            && (submitted || ui.memory(|memory| memory.focused().is_none()))
        {
            // A command kept and not touched is chosen whole again: a click
            // elsewhere, to read or copy, took the choice away with the
            // keyboard. A click in the line puts the cursor there instead.
            if !submitted && self.kept_input.as_ref() == Some(&self.input) {
                self.select_input = true;
            }
            response.request_focus();
        }
    }

    /// With Tab, completes the word being typed from the latest output,
    /// then what was typed, then what scripts offer. With nothing typed
    /// Tab is left alone, to move between controls as usual. Returns
    /// whether it completed.
    fn complete_word(&mut self, ui: &mut egui::Ui, focused: bool) -> bool {
        let completing = focused
            && !self.input.is_empty()
            && !self.typing_secret()
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Tab));
        if !completing {
            return false;
        }
        let scripted = self.scripts.completions();
        let rows = (0..self.output.len()).rev().take(200);
        let seen: Vec<&str> = rows
            .flat_map(|row| complete::words(self.output.text(row)))
            .collect();
        let typed = self
            .history
            .entries()
            .iter()
            .rev()
            .flat_map(|entry| complete::words(entry));
        let given = scripted.iter().map(String::as_str);
        self.completion
            .complete(&mut self.input, seen.into_iter().chain(typed).chain(given));
        true
    }

    /// The field itself, in its frame, with the prompt mark drawn before
    /// it. Returns where the frame is, and what the field gave.
    fn command_field(
        &mut self,
        ui: &mut egui::Ui,
        id: egui::Id,
        focused: bool,
    ) -> egui::text_edit::TextEditOutput {
        let theme = theme::current();
        let hint = RichText::new(tr("Type a command"))
            .font(theme::output_font())
            .color(theme.text_faint);
        let edge = if focused {
            theme.accent.gamma_multiply(0.7)
        } else {
            theme.border
        };
        let frame = egui::Frame::new()
            .fill(theme.background)
            .stroke(egui::Stroke::new(1.0, edge))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::symmetric(14, 8));
        let has_text = !self.input.is_empty();
        let shown = frame.show(ui, |ui| {
            let height = ui.fonts_mut(|fonts| fonts.row_height(&theme::output_font())) + 4.0;
            let row = egui::vec2(ui.available_width(), height);
            let centred = egui::Layout::left_to_right(egui::Align::Center);
            ui.allocate_ui_with_layout(row, centred, |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (mark, _) =
                    ui.allocate_exact_size(egui::vec2(8.0, height), egui::Sense::hover());
                let secret = self.typing_secret();
                let edit = TextEdit::singleline(&mut self.input)
                    .id(id)
                    .frame(egui::Frame::NONE)
                    .font(theme::output_font())
                    .password(secret)
                    // Keeps Tab from moving to the next control while there
                    // is a word to complete.
                    .lock_focus(has_text)
                    // In the typeface of what will be typed, so it sits on
                    // the same line.
                    .hint_text(hint)
                    .align(egui::Align2::LEFT_CENTER)
                    .desired_width(f32::INFINITY)
                    .show(ui);
                prompt_mark(ui, mark, edit.response.rect.center().y);
                edit
            })
            .inner
        });
        shown.inner
    }

    /// Where the cursor goes: over all of a command kept to send again,
    /// after a word just completed, or at the end when asked.
    fn place_cursor(
        &mut self,
        ui: &egui::Ui,
        id: egui::Id,
        output: &mut egui::text_edit::TextEditOutput,
        completing: bool,
    ) {
        let end = egui::text::CCursor::new(self.input.chars().count());
        if std::mem::take(&mut self.select_input) {
            let all = egui::text::CCursorRange::two(egui::text::CCursor::new(0), end);
            output.state.cursor.set_char_range(Some(all));
            output.state.clone().store(ui.ctx(), id);
        }
        // Once it is typed over or edited, it is a new command.
        if self
            .kept_input
            .as_ref()
            .is_some_and(|kept| *kept != self.input)
        {
            self.kept_input = None;
        }
        if completing || std::mem::take(&mut self.cursor_to_end) {
            output
                .state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(end)));
            output.state.clone().store(ui.ctx(), id);
        }
    }

    /// The keys of the line: the arrows through the history, Ctrl+R to
    /// search it, Ctrl+F to search the output.
    fn line_keys(&mut self, ui: &mut egui::Ui) {
        let (up, down, search, find) = ui.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
                crate::shortcuts::HISTORY_SEARCH.pressed(input),
                crate::shortcuts::FIND.pressed(input),
            )
        });
        // A command left from before is the newest in the history: the
        // arrows start from it, not from what its text happens to match.
        if (up || down || search) && self.kept_input.take().is_some() {
            self.input.clear();
            if up {
                self.history.older(&mut self.input);
            }
        }
        if up {
            self.history.older(&mut self.input);
        } else if down {
            self.history.newer(&mut self.input);
        } else if search {
            self.history.search(&mut self.input);
        }
        if find {
            self.find.open = true;
            self.find.wants_focus = true;
        }
    }

    /// The bar for searching the scrollback, shown above the input line.
    pub(super) fn find_bar(&mut self, ui: &mut egui::Ui) {
        let theme = crate::theme::current();
        let mut asked = None;
        let mut close = false;
        ui.horizontal(|ui| {
            crate::widgets::inset(theme.field)
                .stroke(egui::Stroke::new(1.0, theme.border))
                .inner_margin(egui::Margin::symmetric(10, 2))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let look = crate::widgets::SearchLook {
                        hint: tr("Find in the output"),
                        label: tr("Find"),
                        size: 13.0,
                        width: 240.0,
                        id: None,
                        lock_focus: false,
                    };
                    let field = crate::widgets::search_field(ui, &mut self.find.query, look);
                    if field.changed() {
                        self.find.place = None;
                    }
                    if std::mem::take(&mut self.find.wants_focus) {
                        field.request_focus();
                    }
                    let enter =
                        field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
                    let shift = ui.input(|input| input.modifiers.shift);
                    // How many there are, once something was found.
                    let count = match self.find.place {
                        Some((at, of)) => trf("{0} of {1}", &[&at.to_string(), &of.to_string()]),
                        None => String::new(),
                    };
                    let counted = RichText::new(count)
                        .font(crate::theme::font(crate::theme::Weight::Regular, 12.0))
                        .color(theme.text_faint);
                    ui.add(Label::new(counted).selectable(false));
                    use crate::widgets::{Glyph, glyph_button};
                    let previous = glyph_button(ui, Glyph::Up, tr("Previous")).clicked();
                    let next = glyph_button(ui, Glyph::Down, tr("Next")).clicked();
                    close = glyph_button(ui, Glyph::Close, tr("Close")).clicked();
                    if previous || (enter && !shift) {
                        asked = Some((false, enter));
                    } else if next || (enter && shift) {
                        asked = Some((true, enter));
                    }
                    if enter {
                        field.request_focus();
                    }
                });
        });
        if let Some((forward, _)) = asked {
            let found = self.output.find(&self.find.query, forward);
            self.find.place = self.output.found_place(&self.find.query);
            match found {
                Some(row) => {
                    let text = self.output.text(row).trim().to_owned();
                    self.voice.say_now(&text);
                }
                None => self.voice.say_now(tr("Not found.")),
            }
        }
        let escape = crate::widgets::take_escape(ui);
        if close || escape {
            self.find.open = false;
            self.find.place = None;
            self.output.end_find();
        }
    }

    /// Runs the triggers on unterminated text that has stopped changing,
    /// which is how a prompt looks when the server does not mark it.
    pub(super) fn check_partial(&mut self, ctx: &egui::Context) {
        let Some((_, since)) = &self.pending_partial else {
            return;
        };
        let waited = since.elapsed();
        if waited < PARTIAL_SETTLE {
            return ctx.request_repaint_after(PARTIAL_SETTLE - waited);
        }
        let Some((line, _)) = self.pending_partial.take() else {
            return;
        };
        let text = line.text();
        let shown = self
            .run_triggers(line)
            .then(|| self.dispatched.clone())
            .flatten();
        match &shown {
            Some(shown) => self.output.set_partial(shown),
            None => {
                self.output.clear_partial();
                self.voice.skip_partial(&text);
            }
        }
        self.settled_partial = Some((text, shown));
        self.run_actions();
        self.login_name();
    }
}

/// The prompt mark, drawn rather than typed, level with the middle of the
/// lowercase letters of the field whose middle is `line`.
fn prompt_mark(ui: &egui::Ui, mark: egui::Rect, line: f32) {
    let x_height = ui.fonts_mut(|fonts| fonts.row_height(&theme::output_font())) * 0.08;
    let (centre, arm) = (egui::pos2(mark.center().x, line + x_height), 4.0);
    let chevron = vec![
        centre + egui::vec2(-arm * 0.5, -arm),
        centre + egui::vec2(arm * 0.5, 0.0),
        centre + egui::vec2(-arm * 0.5, arm),
    ];
    let stroke = egui::Stroke::new(1.6, theme::current().accent);
    ui.painter().add(egui::Shape::line(chevron, stroke));
}

/// Searching the output: whether the bar is open, and what is searched for.
#[derive(Default)]
pub(super) struct Find {
    /// The find bar is showing, and what it searches for.
    pub(super) open: bool,
    pub(super) query: String,
    /// Give the find bar the keyboard on the next frame.
    pub(super) wants_focus: bool,
    /// Where the line found is among those found, and how many there are.
    pub(super) place: Option<(usize, usize)>,
}
