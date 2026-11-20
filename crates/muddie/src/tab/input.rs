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
        // Never a password, and not a command of the client: that is not
        // something to send again and again.
        let keep = self.settings.keep_command
            && !self.input_hidden
            && !line.is_empty()
            && client_command(&line).is_none();
        self.kept_input = keep.then(|| line.clone());
        if let Some(kept) = &self.kept_input {
            self.input.clone_from(kept);
            self.select_input = true;
        }
        // What follows answers this command, so drop what was still queued.
        self.voice.stop();
        if self.input_hidden {
            // A password: scripts and the history never see it.
            self.send(line);
        } else {
            self.history.add(&line);
            // A command of the client answers at the command line.
            if !self.run_command(&line) {
                self.scripts.on_input(&line);
                self.run_actions();
            }
        }
    }

    /// The command line: a field with the prompt mark before it, Tab to
    /// complete, the arrows through the history, and what the last command
    /// said where its hint would be.
    pub(super) fn input_line(&mut self, ui: &mut egui::Ui) {
        let id = ui.make_persistent_id("command input");
        let focused = ui.memory(|memory| memory.has_focus(id));
        let completing = self.complete_word(ui, focused);
        let has_text = !self.input.is_empty();
        // What the last command said takes the place of the hint; with
        // something typed, it is said at the other end of the line.
        let said = self.bar_said(ui);
        let (bar, mut output) = self.command_field(ui, id, focused, said.clone());
        self.place_cursor(ui, id, &mut output, completing);
        let response = output.response;
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label(tr("Command")));
        if response.has_focus() {
            self.line_keys(ui);
        }
        if has_text && let Some((text, colour)) = said {
            said_at_the_end(ui, bar, text, colour);
        }
        // Typing puts away what the last command answered.
        if response.changed() {
            self.palette.bar_note = None;
        }
        let submitted = response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        if submitted {
            self.submit();
        }
        // The input owns the keyboard unless the user moved focus elsewhere.
        if !self.palette.open && (submitted || ui.memory(|memory| memory.focused().is_none())) {
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
            && !self.input_hidden
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Tab));
        if !completing {
            return false;
        }
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
        let given =
            (self.completions.iter()).flat_map(|(_, words)| words.iter().map(String::as_str));
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
        said: Option<(String, egui::Color32)>,
    ) -> (egui::Rect, egui::text_edit::TextEditOutput) {
        let theme = theme::current();
        let hint = match said {
            Some((text, colour)) => RichText::new(text).font(palette::font()).color(colour),
            None => RichText::new(tr("Type a command"))
                .font(palette::font())
                .color(theme.text_faint),
        };
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
            let height = ui.fonts_mut(|fonts| fonts.row_height(&palette::font())) + 4.0;
            let row = egui::vec2(ui.available_width(), height);
            let centred = egui::Layout::left_to_right(egui::Align::Center);
            ui.allocate_ui_with_layout(row, centred, |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (mark, _) =
                    ui.allocate_exact_size(egui::vec2(8.0, height), egui::Sense::hover());
                let edit = TextEdit::singleline(&mut self.input)
                    .id(id)
                    .frame(egui::Frame::NONE)
                    .font(palette::font())
                    .password(self.input_hidden)
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
        (shown.response.rect, shown.inner)
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
                input.consume_key(Modifiers::CTRL, Key::R),
                input.consume_key(Modifiers::CTRL, Key::F),
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
        ui.horizontal(|ui| {
            let label = ui.label(tr("Find"));
            let field = ui
                .add(TextEdit::singleline(&mut self.find.query).desired_width(260.0))
                .labelled_by(label.id);
            if std::mem::take(&mut self.find.wants_focus) {
                field.request_focus();
            }
            let enter = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
            let forward = enter && ui.input(|input| input.modifiers.shift);
            let previous =
                crate::widgets::button(ui, tr("Previous")).clicked() || (enter && !forward);
            let next = crate::widgets::button(ui, tr("Next")).clicked() || forward;
            if previous || next {
                let found = self.output.find(&self.find.query, next);
                match found {
                    Some(row) => {
                        let text = self.output.text(row).trim().to_owned();
                        self.voice.say_now(&text);
                    }
                    None => self.voice.say_now(tr("Not found.")),
                }
                if enter {
                    field.request_focus();
                }
            }
            let escape = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
            if crate::widgets::button(ui, tr("Close")).clicked() || escape {
                self.find.open = false;
                self.output.end_find();
            }
        });
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
    let x_height = ui.fonts_mut(|fonts| fonts.row_height(&palette::font())) * 0.08;
    let (centre, arm) = (egui::pos2(mark.center().x, line + x_height), 4.0);
    let chevron = vec![
        centre + egui::vec2(-arm * 0.5, -arm),
        centre + egui::vec2(arm * 0.5, 0.0),
        centre + egui::vec2(-arm * 0.5, arm),
    ];
    let stroke = egui::Stroke::new(1.6, theme::current().accent);
    ui.painter().add(egui::Shape::line(chevron, stroke));
}

/// What the last command said, at the far end of the line, over whatever
/// of the typed text reaches that far.
fn said_at_the_end(ui: &egui::Ui, bar: egui::Rect, text: String, colour: egui::Color32) {
    let at = bar.right_center() - egui::vec2(14.0, 0.0);
    let font = theme::font(crate::theme::Weight::Regular, 13.0);
    let galley = ui.painter().layout_no_wrap(text, font, colour);
    let size = galley.size();
    let place = egui::Rect::from_min_size(egui::pos2(at.x - size.x, at.y - size.y / 2.0), size);
    ui.painter().rect_filled(
        place.expand2(egui::vec2(10.0, 2.0)),
        4.0,
        theme::current().background,
    );
    ui.painter().galley(place.min, galley, colour);
}

/// Searching the output: whether the bar is open, and what is searched for.
#[derive(Default)]
pub(super) struct Find {
    /// The find bar is showing, and what it searches for.
    pub(super) open: bool,
    pub(super) query: String,
    /// Give the find bar the keyboard on the next frame.
    pub(super) wants_focus: bool,
}
