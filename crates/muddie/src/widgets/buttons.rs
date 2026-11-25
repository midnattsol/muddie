//! Buttons, of every weight, and the choices made by pressing one.

use super::*;

/// The ordinary button, for something to do that is not the main thing:
/// outlined, filled only under the pointer. With [`primary_button`] and
/// [`quiet_button`], the three weights a button has.
pub fn button(ui: &mut Ui, text: impl Into<egui::WidgetText>) -> Response {
    let theme = theme::current();
    let kept = ui.visuals().widgets.clone();
    theme::button_look(&mut ui.visuals_mut().widgets, &theme);
    let widgets = &mut ui.visuals_mut().widgets;
    let edge = Stroke::new(1.0, theme::mix(theme.border, theme.text, 0.14));
    widgets.inactive.bg_fill = Color32::TRANSPARENT;
    widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    widgets.inactive.bg_stroke = edge;
    widgets.hovered.bg_stroke = edge;
    let response = ui.add(egui::Button::new(text).min_size(vec2(0.0, theme::size::CONTROL)));
    ui.visuals_mut().widgets = kept;
    response
}

/// A button with a picture before its words.
pub fn button_with(ui: &mut Ui, glyph: Glyph, text: &str) -> Response {
    button_with_tint(ui, glyph, text, None)
}

/// A [`button_with`] whose picture is in a colour of its own, if given.
pub(super) fn button_with_tint(
    ui: &mut Ui,
    glyph: Glyph,
    text: &str,
    tint: Option<Color32>,
) -> Response {
    let theme = theme::current();
    let response = button(ui, format!("     {text}"));
    let at = response.rect.left_center() + vec2(14.0, 0.0);
    let colour = tint.unwrap_or(if response.hovered() {
        theme.text
    } else {
        theme.text_dim
    });
    paint_glyph(
        ui.painter(),
        at,
        egui::Align2::LEFT_CENTER,
        glyph,
        12.5,
        colour,
    );
    // What it says is its words: the room for the picture is not read.
    (ui.ctx()).accesskit_node_builder(response.id, |node| node.set_label(text));
    response
}

/// A button that adds one more to a list: a dashed outline round a plus
/// and what it adds, so that it reads as a place for something new.
pub fn add_button(ui: &mut Ui, text: &str) -> Response {
    let theme = theme::current();
    let font = theme::font(Weight::Medium, 13.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, theme.text_dim);
    let size = vec2(galley.size().x + 44.0, theme::size::CONTROL);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), text));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = response.hovered() || response.has_focus();
        if hovered {
            painter.rect_filled(rect, CornerRadius::same(8), theme.hover);
        }
        let colour = if hovered {
            theme.accent
        } else {
            theme::mix(theme.border, theme.text, 0.3)
        };
        dashed_round(painter, rect.shrink(0.5), 8.0, Stroke::new(1.0, colour));
        let text_colour = if hovered { theme.text } else { theme.text_dim };
        paint_glyph(
            painter,
            rect.left_center() + vec2(14.0, 0.0),
            egui::Align2::LEFT_CENTER,
            Glyph::Plus,
            11.0,
            text_colour,
        );
        painter.galley(
            egui::pos2(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0),
            galley,
            text_colour,
        );
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(2.0),
                CornerRadius::same(10),
                ui.visuals().selection.stroke,
                egui::StrokeKind::Outside,
            );
        }
    }
    response
}

/// The dashed outline of a rounded rectangle.
fn dashed_round(painter: &egui::Painter, rect: egui::Rect, radius: f32, stroke: Stroke) {
    let (dash, gap) = (4.0, 3.0);
    let r = radius;
    let lines = [
        [
            egui::pos2(rect.left() + r, rect.top()),
            egui::pos2(rect.right() - r, rect.top()),
        ],
        [
            egui::pos2(rect.left() + r, rect.bottom()),
            egui::pos2(rect.right() - r, rect.bottom()),
        ],
        [
            egui::pos2(rect.left(), rect.top() + r),
            egui::pos2(rect.left(), rect.bottom() - r),
        ],
        [
            egui::pos2(rect.right(), rect.top() + r),
            egui::pos2(rect.right(), rect.bottom() - r),
        ],
    ];
    for line in lines {
        painter.extend(egui::Shape::dashed_line(&line, stroke, dash, gap));
    }
    // The corners, as quarter circles.
    for (centre, from) in [
        (
            egui::pos2(rect.left() + r, rect.top() + r),
            std::f32::consts::PI,
        ),
        (
            egui::pos2(rect.right() - r, rect.top() + r),
            1.5 * std::f32::consts::PI,
        ),
        (egui::pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (
            egui::pos2(rect.left() + r, rect.bottom() - r),
            0.5 * std::f32::consts::PI,
        ),
    ] {
        let points: Vec<egui::Pos2> = (0..=6)
            .map(|step| {
                let angle = from + step as f32 / 6.0 * std::f32::consts::FRAC_PI_2;
                centre + vec2(angle.cos(), angle.sin()) * r
            })
            .collect();
        painter.add(egui::Shape::line(points, stroke));
    }
}

/// The button for the main thing to do in a view, filled with the accent.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let theme = theme::current();
    let label = RichText::new(text)
        .font(theme::font(Weight::Medium, 13.5))
        .color(theme.on_accent);
    raised(
        ui,
        egui::Button::new(label)
            .fill(theme.accent)
            .stroke(Stroke::NONE)
            .min_size(vec2(0.0, theme::size::CONTROL)),
    )
}

/// A button with no fill until the pointer is over it, for secondary
/// actions and toolbars.
pub fn quiet_button(ui: &mut Ui, text: &str) -> Response {
    let theme = theme::current();
    let label = RichText::new(text).color(theme.text_dim);
    raised(
        ui,
        egui::Button::new(label)
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE),
    )
}

/// A small button that shows a sign, such as `+`: `label` says what it
/// does, to a screen reader and when pointed at. Without a `frame` it is
/// only the sign until the pointer is over it.
pub fn sign_button(ui: &mut Ui, sign: &str, label: &str, frame: bool) -> Response {
    let theme = theme::current();
    let text = RichText::new(sign)
        .font(theme::font(Weight::Regular, 16.0))
        .color(if frame { theme.text } else { theme.text_dim });
    let response = raised(
        ui,
        egui::Button::new(text)
            .frame(frame)
            .min_size(vec2(26.0, 26.0)),
    );
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_label(label);
    });
    name_tip(response, label)
}

/// The cross that closes something. `label` says what, for screen readers.
pub fn close_button(ui: &mut Ui, label: &str, size: f32) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = response.hovered() || response.has_focus();
        if hovered {
            painter.rect_filled(rect, CornerRadius::same(6), theme.hover);
        }
        let colour = if hovered {
            theme.text
        } else {
            theme.text_faint
        };
        let arm = size * 0.2;
        let stroke = Stroke::new(1.3, colour);
        let centre = rect.center();
        painter.line_segment(
            [centre - Vec2::splat(arm), centre + Vec2::splat(arm)],
            stroke,
        );
        painter.line_segment([centre + vec2(arm, -arm), centre + vec2(-arm, arm)], stroke);
    }
    name_tip(response, label)
}

/// A round button in the accent with an arrow: go. `label` says where,
/// for screen readers.
pub fn go_button(ui: &mut Ui, label: &str, size: f32) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = response.hovered() || response.has_focus();
        let fill = if hovered {
            theme::mix(theme.accent, Color32::WHITE, 0.14)
        } else {
            theme.accent
        };
        painter.circle_filled(rect.center(), size / 2.0, fill);
        let stroke = Stroke::new(1.8, theme.on_accent);
        let (centre, arm) = (rect.center(), size * 0.17);
        painter.line_segment(
            [centre - vec2(arm * 1.2, 0.0), centre + vec2(arm * 1.2, 0.0)],
            stroke,
        );
        painter.add(egui::Shape::line(
            vec![
                centre + vec2(arm * 0.2, -arm),
                centre + vec2(arm * 1.2, 0.0),
                centre + vec2(arm * 0.2, arm),
            ],
            stroke,
        ));
    }
    name_tip(response, label)
}

/// What looks like a field to search in and opens the palette: where
/// anything can be found, with the keys that open it from anywhere.
pub fn palette_button(ui: &mut Ui, width: f32, keys: &str) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click());
    // Named as it is everywhere; what it is for written in it, as a
    // field's hint, and said after its name.
    let (name, hint) = (
        crate::i18n::tr("Palette"),
        crate::i18n::tr("Search or run a command"),
    );
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), name));
    (ui.ctx()).accesskit_node_builder(response.id, |node| node.set_description(hint));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if response.hovered() {
            theme.hover
        } else {
            theme.field
        };
        painter.rect_filled(rect, CornerRadius::same(8), fill);
        painter.rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(1.0, theme.border),
            egui::StrokeKind::Inside,
        );
        // A magnifying glass, the words, and the keys at the far end.
        let glass = rect.left_center() + vec2(14.0, -1.0);
        let stroke = Stroke::new(1.3, theme.text_faint);
        painter.circle_stroke(glass, 4.0, stroke);
        painter.line_segment([glass + vec2(3.0, 3.0), glass + vec2(6.0, 6.0)], stroke);
        let small = theme::font(Weight::Regular, 12.5);
        let clip = painter.with_clip_rect(rect.shrink(4.0));
        clip.text(
            rect.left_center() + vec2(28.0, 0.0),
            egui::Align2::LEFT_CENTER,
            hint,
            small.clone(),
            theme.text_faint,
        );
        clip.text(
            rect.right_center() - vec2(10.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            keys,
            small,
            theme.text_faint,
        );
    }
    response
}

/// A choice between a few views, as a row of words with the current one
/// raised. Returns the one clicked.
pub fn segmented(ui: &mut Ui, choices: &[&str], current: usize) -> Option<usize> {
    let theme = theme::current();
    let mut chosen = None;
    Frame::new()
        .fill(theme.field)
        .corner_radius(CornerRadius::same(9))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            // In their order, whichever way the row they are in runs.
            let widths = choices.iter().map(|name| {
                let font = theme::font(Weight::Medium, 12.5);
                let text = ui
                    .painter()
                    .layout_no_wrap((*name).to_owned(), font, theme.text);
                text.size().x + 24.0
            });
            let total = widths.sum::<f32>() + 2.0 * choices.len().saturating_sub(1) as f32;
            let row = Layout::left_to_right(Align::Center);
            let reached = current.min(choices.len().saturating_sub(1));
            let list = egui::UiBuilder::new()
                .layout(row)
                .accessibility_role(egui::accesskit::Role::TabList);
            ui.allocate_ui_with_layout(vec2(total, 24.0), row, |ui| {
                ui.scope_builder(list, |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let mut tabs = Vec::with_capacity(choices.len());
                    for (index, name) in choices.iter().enumerate() {
                        let selected = index == current;
                        let font = theme::font(Weight::Medium, 12.5);
                        let width = ui
                            .painter()
                            .layout_no_wrap((*name).to_owned(), font.clone(), theme.text)
                            .size()
                            .x;
                        let (rect, response) = ui.allocate_exact_size(
                            vec2(width + 24.0, 24.0),
                            tab_sense(index == reached),
                        );
                        response.widget_info(|| {
                            WidgetInfo::selected(Role::Tab, ui.is_enabled(), selected, *name)
                        });
                        let hovered = response.hovered() || response.has_focus();
                        if selected {
                            ui.painter()
                                .rect_filled(rect, CornerRadius::same(7), theme.raised);
                        }
                        let colour = if selected || hovered {
                            theme.text
                        } else {
                            theme.text_dim
                        };
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            *name,
                            font,
                            colour,
                        );
                        if response.clicked() && !selected {
                            chosen = Some(index);
                        }
                        tabs.push(response);
                    }
                    if let Some(index) = tab_keys(ui, &tabs).filter(|index| *index != current) {
                        chosen = Some(index);
                    }
                });
            });
        });
    chosen
}

/// The colour of a tab with something in it not looked at yet.
pub fn fresh_colour() -> egui::Color32 {
    crate::panels::colour_named("yellow").unwrap_or(theme::current().accent)
}

/// A tab's name as a screen reader says it: and that it has something
/// new, if it has.
pub fn fresh_said(text: &str, fresh: bool) -> String {
    match fresh {
        true => format!("{text}, {}", crate::i18n::tr("new")),
        false => text.to_owned(),
    }
}

/// A tab of a row of them: the one shown is filled, and one with
/// something in it that has not been looked at stands out, with a dot,
/// and says so to a screen reader.
pub fn tab(ui: &mut Ui, text: &str, on: bool, fresh: bool) -> Response {
    let theme = theme::current();
    let weight = if on || fresh {
        Weight::Medium
    } else {
        Weight::Regular
    };
    let marked = fresh_colour();
    let colour = match (on, fresh) {
        (true, _) => theme.text,
        (false, true) => marked,
        (false, false) => theme.text_dim,
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), theme::font(weight, 12.5), colour);
    // Room for the dot beside the name.
    let dot = if fresh { 10.0 } else { 0.0 };
    let size = vec2(galley.size().x + 18.0 + dot, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, tab_sense(on));
    response.widget_info(|| {
        WidgetInfo::selected(Role::Tab, ui.is_enabled(), on, fresh_said(text, fresh))
    });
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if on {
            painter.rect_filled(rect, CornerRadius::same(7), theme.raised);
        } else if response.hovered() || response.has_focus() {
            painter.rect_filled(
                rect,
                CornerRadius::same(7),
                theme.raised.gamma_multiply(0.6),
            );
        }
        let at = egui::pos2(rect.left() + 9.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(at, galley, colour);
        if fresh {
            painter.circle_filled(rect.right_center() - vec2(9.0, 0.0), 3.0, marked);
        }
    }
    response
}

/// How a tab of a list of them is pressed: any with the pointer, and only
/// the one shown reached with Tab, the others with the arrows: see
/// [`tab_keys`].
fn tab_sense(shown: bool) -> Sense {
    match shown {
        true => Sense::click(),
        false => Sense::CLICK,
    }
}

/// The keys of a list of tabs, as in any other: on one, the arrows show
/// the one beside it, round from the last to the first, and Home and End
/// the first and the last, with the keyboard going to it. Returns the one
/// to show.
pub fn tab_keys(ui: &Ui, tabs: &[Response]) -> Option<usize> {
    let at = tabs.iter().position(Response::has_focus)?;
    let count = tabs.len();
    let to = ui.input_mut(|input| {
        let mut pressed = |key| input.consume_key(egui::Modifiers::NONE, key);
        if pressed(egui::Key::ArrowRight) || pressed(egui::Key::ArrowDown) {
            Some((at + 1) % count)
        } else if pressed(egui::Key::ArrowLeft) || pressed(egui::Key::ArrowUp) {
            Some((at + count - 1) % count)
        } else if pressed(egui::Key::Home) {
            Some(0)
        } else if pressed(egui::Key::End) {
            Some(count - 1)
        } else {
            None
        }
    })?;
    ui.memory_mut(|memory| {
        // Not to whatever egui finds that way: the tab beside.
        memory.move_focus(egui::FocusDirection::None);
        memory.request_focus(tabs[to].id);
    });
    Some(to)
}

/// Adds a button in the look of a button, not of a field: see
/// [`theme::button_look`].
fn raised(ui: &mut Ui, button: egui::Button<'_>) -> Response {
    let kept = ui.visuals().widgets.clone();
    theme::button_look(&mut ui.visuals_mut().widgets, &theme::current());
    let response = ui.add(button);
    ui.visuals_mut().widgets = kept;
    response
}
