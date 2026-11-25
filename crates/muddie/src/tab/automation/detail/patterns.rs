//! Trying a pattern on a sample: the pattern's field, what it would do, and the
//! groups it holds, shown as chips.

use super::*;

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
            node.set_description(trf("The pattern is not valid: {0}", &[reason]));
        }
    });
    if let Some(reason) = invalid {
        ui.horizontal(|ui| {
            widgets::glyph(ui, widgets::Glyph::Warning, 11.0, theme.danger);
            let text = trf("The pattern is not valid: {0}", &[reason]);
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
