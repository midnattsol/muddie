//! What the details of every kind of rule have in common: their heading,
//! what they do, and trying them.

use super::*;

/// A line of small print under a control.
pub(in crate::tab) fn note(ui: &mut egui::Ui, text: &str) {
    let small = RichText::new(text)
        .font(theme::font(Weight::Regular, 11.5))
        .color(theme::current().text_faint);
    ui.label(small);
}

/// Beside a switch that is off, that it is: what is not running should
/// not look as if it were.
pub(in crate::tab) fn off_badge(ui: &mut egui::Ui, on: bool) {
    if !on {
        widgets::badge(ui, tr("Disabled"), false);
    }
}

/// What a rule would send of a match, or that it runs its Luau, in a line,
/// as it would: the groups and the variables put in.
pub(in crate::tab) fn sends_said(
    send: &str,
    luau: bool,
    captures: &regex::Captures,
    scripts: &muddie_script::Engine,
) -> Option<String> {
    if luau {
        return Some(tr("runs the script").to_owned());
    }
    let sent: Vec<String> = (crate::rules::commands(send).iter())
        .map(|command| scripts.expand(&crate::rules::fill(command, captures)))
        .collect();
    (!sent.is_empty()).then(|| trf("sends {0}", &[&sent.join("; ")]))
}

/// What a rule does: either the commands it sends or the script it runs.
pub(in crate::tab) struct ActionEdit<'a> {
    pub(in crate::tab) luau: &'a mut bool,
    pub(in crate::tab) commands: &'a mut String,
    pub(in crate::tab) code: &'a mut String,
    /// The rule matches something, so there are groups to refer to.
    pub(in crate::tab) matched: bool,
    pub(in crate::tab) commands_hint: &'a str,
    pub(in crate::tab) script_hint: &'a str,
    pub(in crate::tab) about_commands: &'a str,
    pub(in crate::tab) about_script: &'a str,
    /// How tall its editor was made, and whether it is open larger.
    pub(in crate::tab) sizes: &'a mut widgets::CodeSizes,
    /// The rule's name, for the heading of its editor open larger.
    pub(in crate::tab) name: String,
}

/// The part of a rule that says what it does, the same for triggers,
/// aliases, macros and timers: a card with the choice between commands
/// and a Luau script, and the editor of the one chosen, which fits what it
/// holds until it is made taller or shorter by its grip, and which opens
/// larger, over the window, from the card's heading. Returns what to say
/// when the editor's height was changed with the keyboard.
pub(in crate::tab) fn action_block(
    ui: &mut egui::Ui,
    action: ActionEdit<'_>,
    mistake: Option<&(usize, String)>,
) -> Option<String> {
    let ActionEdit {
        luau: lua,
        commands,
        code: script,
        matched,
        commands_hint,
        script_hint,
        about_commands,
        about_script,
        sizes,
        name,
    } = action;
    let mut expand = None;
    let said = widgets::card(
        ui,
        Some(widgets::Glyph::Send),
        tr("Do"),
        |ui| {
            expand = Some(widgets::glyph_button(
                ui,
                widgets::Glyph::Expand,
                tr("Expand editor"),
            ))
        },
        |ui| {
            let current = usize::from(*lua);
            match widgets::segmented(ui, &[tr("Commands"), "Luau"], current) {
                Some(0) => *lua = false,
                Some(_) => {
                    *lua = true;
                    // Start the script from the commands there were.
                    if script.trim().is_empty() {
                        *script = crate::rules::commands_as_luau(commands, matched);
                    }
                }
                None => {}
            }
            ui.add_space(4.0);
            let (label, value, hint, language, about, mistake) = match *lua {
                true => (
                    "Luau",
                    &mut *script,
                    script_hint,
                    widgets::Language::Luau,
                    about_script,
                    mistake,
                ),
                false => (
                    tr("Commands"),
                    &mut *commands,
                    commands_hint,
                    widgets::Language::Commands,
                    about_commands,
                    None,
                ),
            };
            let height = &mut sizes.heights[language.slot()];
            let spare = sizes.spare;
            let (_, said) = widgets::sizable_code_editor(
                ui, label, value, hint, language, height, spare, mistake,
            );
            note(ui, about);
            said
        },
    );
    let expand = expand?;
    let focus = egui::Id::unique("focus the expanded editor");
    if expand.clicked() {
        sizes.expanded = true;
        ui.data_mut(|data| data.insert_temp(focus, true));
    }
    let (label, value, hint, language) = match *lua {
        true => ("Luau", script, script_hint, widgets::Language::Luau),
        false => (
            tr("Commands"),
            commands,
            commands_hint,
            widgets::Language::Commands,
        ),
    };
    // As large as the window lets it be, less the dialog's heading and
    // margins.
    let title = match name.trim() {
        "" => label.to_owned(),
        name => format!("{label} · {name}"),
    };
    let screen = ui.ctx().content_rect();
    let tall = (screen.height() - 190.0).max(160.0);
    widgets::dialog(
        ui.ctx(),
        "expanded editor",
        &title,
        &mut sizes.expanded,
        screen.width() - 120.0,
        |ui| {
            let height = widgets::CodeHeight::Fixed(tall);
            let editor = widgets::code_editor(
                ui,
                label,
                value,
                hint,
                language,
                height,
                mistake.filter(|_| *lua),
            );
            if ui
                .data_mut(|data| data.remove_temp::<bool>(focus))
                .is_some()
            {
                editor.request_focus();
            }
        },
    );
    // Closed, the keyboard goes back to where it opened it: the frame after,
    // as nothing under a dialog takes it while the dialog is drawn.
    let back = focus.with("back");
    if ui.data_mut(|data| data.remove_temp::<bool>(back)).is_some() {
        expand.request_focus();
    }
    let was_open = ui.data(|data| data.get_temp::<bool>(focus.with("open")));
    if !sizes.expanded && was_open == Some(true) {
        ui.data_mut(|data| data.insert_temp(back, true));
        ui.ctx().request_repaint();
    }
    let open = sizes.expanded;
    ui.data_mut(|data| data.insert_temp(focus.with("open"), open));
    said
}

/// Regular expressions as they were last compiled, so that one is not
/// compiled again every frame while it is shown.
#[derive(Default)]
pub(in crate::tab) struct Patterns(Option<(String, Result<regex::Regex, String>)>);

impl Patterns {
    /// The expression `source` compiles to, or why it does not.
    pub(in crate::tab) fn of(&mut self, source: &str) -> Result<&regex::Regex, &str> {
        if self.0.as_ref().is_none_or(|(known, _)| known != source) {
            let compiled = regex::Regex::new(source).map_err(|error| {
                // The last line of the error says what is wrong.
                let reason = error.to_string();
                reason.lines().last().unwrap_or_default().trim().to_owned()
            });
            self.0 = Some((source.to_owned(), compiled));
        }
        match &self.0 {
            Some((_, Ok(regex))) => Ok(regex),
            Some((_, Err(reason))) => Err(reason),
            None => Err(""),
        }
    }
}

/// What trying a rule on a sample gave.
pub(in crate::tab) enum Outcome {
    /// Nothing to try it on yet.
    Idle,
    /// The pattern is not a valid regular expression: the field says why.
    Invalid,
    NoMatch,
    Match(Matched),
}

/// A match of a sample: where it is, where each group is, and what the
/// rule would do with it.
pub(in crate::tab) struct Matched {
    /// The whole match, as a range of the sample.
    pub(in crate::tab) whole: std::ops::Range<usize>,
    /// Each group that took part: its number, and its range.
    pub(in crate::tab) groups: Vec<(usize, std::ops::Range<usize>)>,
    /// What the rule would do, said in a line; empty for nothing.
    pub(in crate::tab) effect: String,
}

pub(in crate::tab) fn try_pattern(
    patterns: &mut Patterns,
    source: &str,
    sample: &str,
    effect: impl FnOnce(&regex::Captures) -> String,
) -> Outcome {
    let regex = match patterns.of(source) {
        Ok(regex) => regex,
        Err(_) => return Outcome::Invalid,
    };
    if sample.is_empty() {
        return Outcome::Idle;
    }
    match regex.captures(sample) {
        Some(captures) => {
            let whole = captures.get(0).map_or(0..0, |found| found.range());
            let groups = (1..captures.len())
                .filter_map(|number| Some((number, captures.get(number)?.range())))
                .collect();
            Outcome::Match(Matched {
                whole,
                groups,
                effect: effect(&captures),
            })
        }
        None => Outcome::NoMatch,
    }
}

/// The colours the groups of a match are shown in, in turn.
fn group_colour(number: usize) -> egui::Color32 {
    let terminal = theme::current().terminal;
    [
        terminal[4],
        terminal[5],
        terminal[2],
        terminal[3],
        terminal[6],
    ][(number - 1) % 5]
}

/// A pattern to type: coloured by what each part does when it is a regular
/// expression, and marked, with why, when it is not a valid one. A screen
/// reader is told why too, as the field's description.
pub(in crate::tab) fn pattern_field(
    ui: &mut egui::Ui,
    label: &str,
    pattern: &mut String,
    regex: bool,
    hint: &str,
    invalid: Option<&str>,
) {
    let theme = theme::current();
    let font = egui::FontId::monospace(13.5);
    widgets::sub_label(ui, label);
    let mut coloured = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
        let tokens = match regex {
            true => crate::syntax::regex_tokens(text.as_str()),
            false => vec![(crate::syntax::Kind::Plain, text.as_str())],
        };
        let mut job = crate::syntax::layout(&tokens, &font, &theme);
        job.wrap.max_width = wrap_width;
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    };
    let field = ui
        .scope(|ui| {
            if invalid.is_some() {
                let widgets = &mut ui.visuals_mut().widgets;
                for state in [
                    &mut widgets.inactive,
                    &mut widgets.hovered,
                    &mut widgets.active,
                ] {
                    state.bg_stroke = egui::Stroke::new(1.0, theme.danger);
                }
                ui.visuals_mut().selection.stroke = egui::Stroke::new(1.0, theme.danger);
            }
            ui.add(
                TextEdit::singleline(pattern)
                    .font(font.clone())
                    .hint_text(
                        RichText::new(hint)
                            .font(font.clone())
                            .color(theme.text_faint),
                    )
                    .margin(egui::Margin::symmetric(10, 7))
                    .desired_width(f32::INFINITY)
                    .layouter(&mut coloured),
            )
        })
        .inner;
    ui.ctx().accesskit_node_builder(field.id, |node| {
        node.set_label(label);
        if let Some(reason) = invalid {
            node.set_invalid(egui::accesskit::Invalid::True);
            node.set_description(trf("The pattern is not valid: {0}", &[&reason]));
        }
    });
    if let Some(reason) = invalid {
        ui.horizontal(|ui| {
            widgets::glyph(ui, widgets::Glyph::Warning, 11.0, theme.danger);
            let text = trf("The pattern is not valid: {0}", &[&reason]);
            ui.label(
                RichText::new(text)
                    .font(theme::font(Weight::Regular, 12.0))
                    .color(theme.danger),
            );
        });
    }
}

/// Trying a rule on a sample: the field to type it in, set apart from the
/// pattern, and what came of it, shown as it is: the part that matched,
/// each group in its colour, what each group holds, and what the rule
/// would do. Returns whether the pattern is invalid, and why.
pub(in crate::tab) fn try_block(
    ui: &mut egui::Ui,
    sample: &mut String,
    hint: &str,
    outcome: impl FnOnce(&str) -> Outcome,
) {
    let theme = theme::current();
    crate::widgets::inset(theme.surface).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            widgets::glyph(ui, widgets::Glyph::Try, 12.0, theme.text_dim);
            let title = RichText::new(tr("Try it"))
                .font(theme::font(Weight::Medium, 12.5))
                .color(theme.text_dim);
            ui.add(egui::Label::new(title).selectable(false));
        });
        theme::field_look(ui.visuals_mut(), theme.field, &theme);
        widgets::text_box(ui, sample, hint);
        match outcome(sample) {
            Outcome::Idle | Outcome::Invalid => {}
            Outcome::NoMatch => {
                ui.horizontal(|ui| {
                    widgets::glyph(ui, widgets::Glyph::Close, 11.0, theme.text_faint);
                    ui.label(RichText::new(tr("Does not match.")).color(theme.text_dim));
                });
            }
            Outcome::Match(matched) => show_match(ui, sample, &matched),
        }
    });
}

/// A match shown: the sample with the match marked and each group in its
/// colour, a chip for what each group holds, and what the rule would do.
fn show_match(ui: &mut egui::Ui, sample: &str, matched: &Matched) {
    let theme = theme::current();
    let font = egui::FontId::monospace(13.0);
    // Where each byte is: in a group, in the match, or outside it.
    let mut job = egui::text::LayoutJob::default();
    let mut cuts: Vec<usize> = vec![0, sample.len(), matched.whole.start, matched.whole.end];
    for (_, range) in &matched.groups {
        cuts.extend([range.start, range.end]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    for pair in cuts.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if from >= to || !sample.is_char_boundary(from) || !sample.is_char_boundary(to) {
            continue;
        }
        // The innermost group, which is the last that holds it.
        let group = (matched.groups.iter())
            .rev()
            .find(|(_, range)| range.start <= from && to <= range.end);
        let inside = matched.whole.start <= from && to <= matched.whole.end;
        let mut format = egui::TextFormat::simple(font.clone(), theme.text);
        match (group, inside) {
            (Some((number, _)), _) => {
                let colour = group_colour(*number);
                format.background = colour.gamma_multiply(0.3);
                format.color = colour;
                format.underline = egui::Stroke::new(1.0, colour);
            }
            (None, true) => format.background = theme.text.gamma_multiply(0.08),
            (None, false) => format.color = theme.text_faint,
        }
        job.append(&sample[from..to], 0.0, format);
    }
    ui.horizontal(|ui| {
        widgets::glyph(ui, widgets::Glyph::Check, 11.0, theme.good);
        ui.add(egui::Label::new(job).wrap());
    });
    if !matched.groups.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (number, range) in &matched.groups {
                let held = sample.get(range.clone()).unwrap_or_default();
                group_chip(ui, *number, held);
            }
        });
    }
    if !matched.effect.is_empty() {
        ui.horizontal(|ui| {
            widgets::glyph(ui, widgets::Glyph::Send, 11.0, theme.accent);
            let text = format!("{} {}", tr("Matches:"), matched.effect);
            ui.label(RichText::new(text).color(theme.text));
        });
    }
}

/// What a group of a match holds, as `%1 = Orc`, in the group's colour.
fn group_chip(ui: &mut egui::Ui, number: usize, held: &str) {
    let colour = group_colour(number);
    let text = format!("%{number} = {held}");
    egui::Frame::new()
        .fill(colour.gamma_multiply(0.16))
        .stroke(egui::Stroke::new(1.0, colour.gamma_multiply(0.7)))
        .corner_radius(egui::CornerRadius::same(u8::MAX))
        .inner_margin(egui::Margin::symmetric(9, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .font(egui::FontId::monospace(12.0))
                    .color(colour),
            );
        });
}

pub(in crate::tab) fn rule_header(
    ui: &mut egui::Ui,
    name: &mut String,
    enabled: &mut bool,
    hint: &str,
    rename: &mut bool,
) -> bool {
    let mut delete = false;
    // The name, then what is at the right of it, in the order they are
    // read.
    let id = ui.make_persistent_id("rule header");
    let font = theme::font(Weight::Semibold, 20.0);
    let state = *enabled;
    widgets::right_last(
        ui,
        id,
        &mut (),
        |_, ui| {
            name_field(ui, name, font.clone(), hint, rename, true);
        },
        |_, ui| {
            off_badge(ui, state);
            widgets::switch(ui, enabled, tr("Enabled")).on_hover_text(tr("Enabled"));
            delete = widgets::icon_button(ui, widgets::Icon::Trash, tr("Delete"), false).clicked();
        },
    );
    ui.add(egui::Separator::default().spacing(2.0));
    delete
}

/// The name at the head of the details, whatever they are of, edited where
/// it is: in its own font, with no frame. When renaming is asked for (F2,
/// or Rename in a row's menu) it is given the keys with its text chosen,
/// to be typed over.
pub(in crate::tab) fn name_field(
    ui: &mut egui::Ui,
    name: &mut String,
    font: egui::FontId,
    hint: &str,
    rename: &mut bool,
    editable: bool,
) -> egui::Response {
    let id = ui.make_persistent_id("detail name");
    let field = ui.add(
        TextEdit::singleline(name)
            .id(id)
            .font(font)
            .hint_text(RichText::new(hint).color(theme::current().text_faint))
            .frame(egui::Frame::NONE)
            .interactive(editable)
            .desired_width(f32::INFINITY),
    );
    ui.ctx()
        .accesskit_node_builder(field.id, |node| node.set_label(tr("Name")));
    if std::mem::take(rename) && editable {
        field.request_focus();
        if let Some(mut state) = TextEdit::load_state(ui.ctx(), id) {
            let all = egui::text::CCursorRange::select_all_str(name);
            state.cursor.set_char_range(Some(all));
            state.store(ui.ctx(), id);
        }
    }
    field
}

/// How many rows of code fit in a view `height` tall once `taken` of it
/// has gone to what surrounds the editor.
pub(in crate::tab) fn script_rows(height: f32, taken: f32) -> usize {
    (((height - taken) / 17.0) as usize).max(8)
}

/// The colours to choose among, as a row of swatches.
pub(in crate::tab) fn swatches(ui: &mut egui::Ui, colour: &mut String) {
    let theme = theme::current();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for name in crate::panels::COLOUR_CHOICES {
            let shown = crate::panels::colour_named(name).unwrap_or(theme.accent);
            if widgets::swatch(ui, shown, *colour == name, tr(name)).clicked() {
                *colour = name.to_owned();
            }
        }
    });
}

impl Tab {
    pub(in crate::tab) fn automation_detail(&mut self, ui: &mut egui::Ui) {
        if self.editor.section == DATA {
            return self.data_view(ui);
        }
        let section = self.editor.section;
        let index = self.editor.selected[section];
        if let (Some(path), Some(kind)) = (self.editor.folder.clone(), section.kind()) {
            return self.folder_detail(ui, kind, &path);
        }
        if self.chosen().len() > 1 {
            return self.bulk_detail(ui);
        }
        if index >= self.section_len() {
            ui.add_space(60.0);
            let hint = RichText::new(empty_hint(section)).color(theme::current().text_faint);
            ui.vertical_centered(|ui| ui.label(hint));
            return;
        }
        ui.spacing_mut().item_spacing.y = 10.0;
        let delete = match section {
            TRIGGERS => self.trigger_detail(ui, index),
            ALIASES => self.alias_detail(ui, index),
            MACROS => self.macro_detail(ui, index),
            TIMERS => self.timer_detail(ui, index),
            GAUGES => self.gauge_detail(ui, index),
            VARIABLES => self.variable_detail(ui, index),
            _ => self.script_detail(ui, index),
        };
        if delete {
            self.delete_chosen();
            self.list_scripts();
        }
    }
}

/// What a section is for, said while nothing in it is chosen.
fn empty_hint(section: Section) -> &'static str {
    match section {
        TRIGGERS => tr(
            "A trigger reacts to what the MUD sends:\nit can answer, speak, play a sound, colour or hide the line.",
        ),
        ALIASES => tr("An alias turns a short command you type\ninto the commands you mean."),
        MACROS => tr("A macro sends commands at the press of a key."),
        TIMERS => tr("A timer does something after a while,\nonce or again and again."),
        GAUGES => tr(
            "A gauge shows something the MUD reports: a bar for your health,\na light for whether you are poisoned, a number for your gold.",
        ),
        VARIABLES => tr(
            "A variable is a value with a name, kept from one session to the next:\nwho you are hunting, what you carry, how many you have killed.",
        ),
        _ => tr(
            "Scripts are Luau code that is always loaded: what no rule can do,\nsuch as building the map or acting on what your MUD reports.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(source: &str, sample: &str) -> String {
        match try_pattern(&mut Patterns::default(), source, sample, |captures| {
            fill("kill %1", captures)
        }) {
            Outcome::Idle => "idle".into(),
            Outcome::Invalid => "invalid".into(),
            Outcome::NoMatch => "no match".into(),
            Outcome::Match(matched) => format!("match: {}", matched.effect),
        }
    }

    #[test]
    fn trying_a_pattern_says_what_would_happen() {
        assert_eq!(
            outcome(r"^(\w+) attacks", "Orc attacks you"),
            "match: kill Orc"
        );
        assert_eq!(outcome(r"^(\w+) attacks", "You attack"), "no match");
        assert_eq!(outcome(r"^(\w+) attacks", ""), "idle");
        assert_eq!(outcome("(unclosed", "x"), "invalid");
        // An invalid pattern is reported even with nothing to try it on.
        assert_eq!(outcome("(unclosed", ""), "invalid");
    }
}
