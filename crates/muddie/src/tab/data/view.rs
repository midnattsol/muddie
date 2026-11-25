//! The view of what the MUD sends: its protocols, its list, and one thing
//! received laid out.

use super::*;

/// Small text of a colour.
pub(in crate::tab) fn small(text: &str, colour: egui::Color32) -> RichText {
    RichText::new(text)
        .font(theme::font(Weight::Medium, 11.5))
        .color(colour)
}

/// The protocols to show: those that carry something to see, then
/// whatever else the MUD offered but the plainest.
pub(in crate::tab) fn protocol_names(protocols: &[(String, bool)]) -> Vec<&str> {
    let main = ["GMCP", "MSDP", "MSSP", "MXP", "MSP", "MCCP2", "CHARSET"];
    let others = (protocols.iter())
        .map(|(name, _)| name.as_str())
        .filter(|name| !main.contains(name) && !["ECHO", "SGA", "EOR"].contains(name));
    main.iter().copied().chain(others).collect()
}

/// A protocol as a chip: filled when in use, an outline when offered and
/// not used, only its name, faint, when not offered; and the accent when
/// the list is filtered by it. Drawn to its own size, so that a row of
/// them wraps. Returns whether it was clicked.
pub(in crate::tab) fn protocol_chip(
    ui: &mut egui::Ui,
    name: &str,
    state: Option<bool>,
    chosen: bool,
) -> bool {
    let theme = theme::current();
    let clear = egui::Color32::TRANSPARENT;
    let (colour, fill, edge, about) = match state {
        _ if chosen => (
            theme.on_accent,
            theme.accent,
            theme.accent,
            tr("Filtering by this protocol"),
        ),
        Some(true) => (theme.text, theme.hover, theme.hover, tr("In use")),
        Some(false) => (
            theme.text_dim,
            clear,
            theme.text_faint,
            tr("Offered by the MUD, not used by Muddie"),
        ),
        None => (
            theme.text_faint.gamma_multiply(0.55),
            clear,
            clear,
            tr("Not offered by this MUD"),
        ),
    };
    let galley =
        ui.painter()
            .layout_no_wrap(name.to_owned(), theme::font(Weight::Medium, 11.5), colour);
    let size = galley.size() + egui::vec2(14.0, 8.0);
    let (rect, shown) = ui.allocate_exact_size(size, egui::Sense::click());
    let corner = egui::CornerRadius::same(6);
    ui.painter().rect_filled(rect, corner, fill);
    ui.painter().rect_stroke(
        rect,
        corner,
        egui::Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, colour);
    shown.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, true, chosen, format!("{name}: {about}"))
    });
    widgets::name_tip(shown, about).clicked()
}

/// The name of what was received, and what it came by, how many times,
/// and when last.
pub(in crate::tab) fn seen_heading(ui: &mut egui::Ui, seen: &Seen) {
    let theme = theme::current();
    ui.horizontal(|ui| {
        let title = RichText::new(&seen.name)
            .font(theme::font(Weight::Semibold, 20.0))
            .color(theme.text);
        let shown = ui.add(Label::new(title).selectable(true));
        widgets::heading_role(ui, &shown, &seen.name);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let times = crate::i18n::trn(
                "received {0} time",
                "received {0} times",
                u64::from(seen.count),
            );
            let said = format!("{} · {times} · {}", seen.protocol, seen.at);
            ui.label(
                RichText::new(said)
                    .font(theme::font(Weight::Regular, 12.0))
                    .color(theme.text_faint),
            );
        });
    });
    ui.add(egui::Separator::default().spacing(2.0));
}

/// Its values one by one, each with its path to copy and, if it is a
/// number, a gauge to make of it. Returns the path a gauge was asked for.
pub(in crate::tab) fn values_group(ui: &mut egui::Ui, seen: &Seen, all: &[Leaf]) -> Option<String> {
    let theme = theme::current();
    let mut gauge = None;
    widgets::card(
        ui,
        Some(widgets::Glyph::Data),
        tr("Values"),
        |_| {},
        |ui| {
            if seen.json.is_empty() {
                ui.label(RichText::new(tr("Empty message.")).color(theme.text_faint));
            }
            ui.spacing_mut().item_spacing.y = 0.0;
            for leaf in all {
                widgets::setting_row(ui, &leaf.path, None, &mut |ui| {
                    let copy = widgets::copy_button(ui, tr("Copy the path"), &leaf.path);
                    if leaf.gauge
                        && widgets::glyph_button(ui, widgets::Glyph::Gauge, tr("Make a gauge"))
                            .clicked()
                    {
                        gauge = Some(leaf.path.clone());
                    }
                    ui.add(Label::new(value_text(&leaf.shown)).truncate());
                    copy
                });
            }
            if !all.is_empty() {
                ui.add_space(6.0);
                let how = tr(
                    "A path is what a gauge follows, and what a script reads after gmcp.get(\"…\").",
                );
                ui.label(
                    RichText::new(how)
                        .font(theme::font(Weight::Regular, 11.5))
                        .color(theme.text_faint),
                );
            }
        },
    );
    gauge
}

/// A value as it is shown: in the colours of the game, if it carries the
/// codes of colours a MUD sends, rather than the codes themselves; a screen
/// reader is told its words alone. A value of several lines is shown on
/// one.
fn value_text(value: &str) -> egui::WidgetText {
    let font = egui::FontId::monospace(13.0);
    if !value.contains('\x1b') {
        let plain = RichText::new(value).font(font).color(theme::current().text);
        return plain.into();
    }
    let mut job = egui::text::LayoutJob::default();
    for (index, line) in crate::panels::styled_lines(value).iter().enumerate() {
        if index > 0 {
            job.append(
                " ⏎ ",
                0.0,
                egui::TextFormat::simple(font.clone(), theme::current().text_faint),
            );
        }
        let coloured = crate::output::server_job(line);
        for section in &coloured.sections {
            let mut format = section.format.clone();
            format.font_id = font.clone();
            let range = section.byte_range.start.0..section.byte_range.end.0;
            job.append(&coloured.text[range], 0.0, format);
        }
    }
    job.into()
}

/// The message as it came, laid out to be read, and copied.
pub(in crate::tab) fn raw_message(ui: &mut egui::Ui, seen: &Seen, value: &Value) {
    let theme = theme::current();
    let pretty = serde_json::to_string_pretty(value).unwrap_or_default();
    let mut raw = if seen.json.is_empty() {
        ""
    } else {
        pretty.as_str()
    };
    let rows = pretty.lines().count().clamp(1, 16);
    let font = egui::FontId::monospace(13.0);
    let mut coloured = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
        let tokens = crate::syntax::json_tokens(text.as_str());
        let mut job = crate::syntax::layout(&tokens, &font, &theme);
        job.wrap.max_width = wrap_width;
        ui.fonts_mut(|fonts| fonts.layout_job(job))
    };
    widgets::card(
        ui,
        Some(widgets::Glyph::Code),
        tr("Raw message"),
        |_| {},
        |ui| {
            let shown = ui.add(
                TextEdit::multiline(&mut raw)
                    .font(egui::FontId::monospace(13.0))
                    .margin(egui::Margin::symmetric(12, 10))
                    .desired_rows(rows)
                    .desired_width(f32::INFINITY)
                    .layouter(&mut coloured),
            );
            (ui.ctx()).accesskit_node_builder(shown.id, |node| node.set_label(tr("Raw message")));
        },
    );
}

/// Those that came before it, newest first.
pub(in crate::tab) fn history(ui: &mut egui::Ui, seen: &Seen) {
    if seen.earlier.is_empty() {
        return;
    }
    widgets::card(
        ui,
        Some(widgets::Glyph::Timer),
        tr("History"),
        |_| {},
        |ui| {
            for (at, json) in &seen.earlier {
                let line = RichText::new(format!("{at}  {json}"))
                    .font(egui::FontId::monospace(12.0))
                    .color(theme::current().text_dim);
                ui.add(Label::new(line).truncate());
            }
        },
    );
}

impl Tab {
    /// Which protocols the MUD offered in this connection, and which of
    /// them are in use, each to filter the list by; why there may be
    /// nothing to see; and recording in the background.
    pub(in crate::tab) fn protocols_shown(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        let protocols = self.protocols.clone();
        let mut filter = self.data_view.protocol.clone();
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            widgets::caption(ui, tr("Protocols"));
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for name in protocol_names(&protocols) {
                let chosen = filter.as_deref() == Some(name);
                let state = protocols
                    .iter()
                    .find(|(known, _)| known == name)
                    .map(|(_, on)| *on);
                // A click shows only what came by it; another, all again.
                if protocol_chip(ui, name, state, chosen) {
                    filter = (!chosen).then(|| name.to_owned());
                }
            }
        });
        self.data_view.protocol = filter;
        let key = tr(
            "Filled: in use. Outlined: offered by the MUD, not used by Muddie. Faint: not offered.",
        );
        ui.horizontal_wrapped(|ui| ui.add(Label::new(small(key, theme.text_faint)).wrap()));
        let in_use = |name: &str| protocols.iter().any(|(known, on)| known == name && *on);
        if self.connection.is_some() && !in_use("GMCP") && !in_use("MSDP") {
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_space(4.0);
                let none = tr("This MUD offers neither GMCP nor MSDP: all it sends is text, and what it says on its channels comes as lines of the game.");
                ui.add(Label::new(small(none, theme.text_dim)).wrap());
            });
        }
        self.recording_controls(ui);
    }

    /// It is kept only while it is looked at, unless asked otherwise; and
    /// what was kept can be cleared.
    pub(in crate::tab) fn recording_controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let keep = tr("Record in the background");
            widgets::switch(ui, &mut self.data_view.keep, keep);
            ui.add(Label::new(small(keep, theme::current().text_dim)).selectable(false));
        });
        if !self.data_view.seen.is_empty() && widgets::quiet_button(ui, tr("Clear")).clicked() {
            self.data_view.seen.clear();
            self.data_view.selected = 0;
        }
        ui.add_space(6.0);
    }

    /// The list of what the MUD has sent, in the editor: which protocols
    /// are in use, as a filter, then each thing by its name.
    pub(in crate::tab) fn data_list(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        self.protocols_shown(ui);
        widgets::text_box(ui, &mut self.data_view.filter, tr("Search"));
        ui.add_space(4.0);
        let filter = self.data_view.filter.trim().to_lowercase();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                widgets::keep_clear_of_bar(ui);
                ui.spacing_mut().item_spacing.y = widgets::LINE_GAP;
                if self.data_view.seen.is_empty() {
                    ui.add_space(24.0);
                    let none = RichText::new(tr("No data received since this view was opened."))
                        .color(theme.text_faint);
                    ui.vertical_centered(|ui| ui.label(none));
                }
                // Those that pass what is searched for and the protocol
                // chosen; the one laid out is among them.
                let listed: Vec<usize> = (0..self.data_view.seen.len())
                    .filter(|&index| {
                        let seen = &self.data_view.seen[index];
                        let found = filter.is_empty()
                            || seen.name.to_lowercase().contains(&filter)
                            || seen.json.to_lowercase().contains(&filter);
                        let by = (self.data_view.protocol.as_ref())
                            .is_none_or(|only| *only == seen.protocol);
                        found && by
                    })
                    .collect();
                if !listed.contains(&self.data_view.selected) {
                    self.data_view.selected = listed.first().copied().unwrap_or(usize::MAX);
                }
                for index in listed {
                    let seen = &self.data_view.seen[index];
                    let (name, summary) = (seen.name.clone(), seen.summary());
                    if widgets::list_row(
                        ui,
                        self.data_view.selected == index,
                        &name,
                        &summary,
                        true,
                    )
                    .clicked()
                    {
                        self.data_view.selected = index;
                    }
                }
            });
    }

    /// The thing chosen in the list, laid out; or what the list is for.
    pub(in crate::tab) fn data_view(&mut self, ui: &mut egui::Ui) {
        let theme = theme::current();
        ui.spacing_mut().item_spacing.y = 10.0;
        let index = self.data_view.selected;
        if index < self.data_view.seen.len() {
            return self.data_detail(ui, index);
        }
        ui.add_space(60.0);
        let about = tr(
            "What the MUD sends besides text, with GMCP or MSDP, appears here as it comes:
look at it to know what a gauge can follow or a script can use.",
        );
        ui.vertical_centered(|ui| ui.label(RichText::new(about).color(theme.text_faint)));
    }

    /// One thing the MUD sent, laid out: its values one by one, each with
    /// its path to copy and, if it is a number, a gauge to make of it;
    /// then the message as it came, and those before it.
    pub(in crate::tab) fn data_detail(&mut self, ui: &mut egui::Ui, index: usize) {
        let Some(seen) = self.data_view.seen.get(index).cloned() else {
            return;
        };
        seen_heading(ui, &seen);
        let value: Value = serde_json::from_str(&seen.json).unwrap_or(Value::Null);
        let mut all = Vec::new();
        leaves(
            &seen.name,
            &value,
            !seen.msdp || !value.is_object(),
            &mut all,
        );
        let gauge = values_group(ui, &seen, &all);
        ui.add_space(6.0);
        raw_message(ui, &seen, &value);
        history(ui, &seen);
        if let Some(path) = gauge {
            self.gauge_from(&path, &all);
        }
    }
}
