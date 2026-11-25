//! How the palette's list is drawn: the group headings, the answer, the rows
//! with their glyphs, and the line of keys at the foot.

use super::*;

/// The name of a group of the list, or of what the list answers, over it.
pub(super) fn group_heading(ui: &mut egui::Ui, text: &str, asked: bool) {
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
pub(super) fn answered(ui: &mut egui::Ui, text: &str, problem: bool) {
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
pub(super) fn keys_at_the_foot(ui: &mut egui::Ui, chosen: Option<&Item>, asking: bool) {
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
pub(super) fn palette_row(ui: &mut egui::Ui, item: &Item, chosen: bool) -> egui::Response {
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
    let end = paint_keys(&painter, inside, keys, &theme);
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
    let (start, width) = paint_usage(ui, item, inside, end, &theme);
    // What it is for, in what room is left, cut short with an ellipsis.
    let room = end - (start + width + 14.0);
    if !about.is_empty() && room > 40.0 {
        paint_about(ui, about, inside, (start + width + 14.0, room), &theme);
    }
    row
}

/// The keys that do the same, in a chip at the right of `inside`. Returns
/// where the rest of the row must end.
fn paint_keys(
    painter: &egui::Painter,
    inside: egui::Rect,
    keys: &str,
    theme: &theme::Theme,
) -> f32 {
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
    end
}

/// What the row is, whole, after its picture, up to `end`. Returns where
/// it starts and how wide it is.
fn paint_usage(
    ui: &mut egui::Ui,
    item: &Item,
    inside: egui::Rect,
    end: f32,
    theme: &theme::Theme,
) -> (f32, f32) {
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
    (start, width)
}

/// What the row is for, from `x` in `room`, cut short with an ellipsis.
fn paint_about(
    ui: &mut egui::Ui,
    about: &str,
    inside: egui::Rect,
    place: (f32, f32),
    theme: &theme::Theme,
) {
    let (x, room) = place;
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
    let at = egui::pos2(x, inside.center().y - about.size().y / 2.0);
    ui.painter().galley(at, about, theme.text_faint);
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
        Act::View(View::Editor) => (Glyph::Code, crate::shortcuts::SWITCH_VIEW.written()),
        Act::View(View::Layout) => (Glyph::Panel, ""),
        Act::View(View::Data) => (Glyph::Data, crate::shortcuts::DATA_VIEW.written()),
        Act::Section(_) => (Glyph::Code, ""),
        Act::App(request) => match request {
            AppRequest::NewSession => (Glyph::Plus, crate::shortcuts::NEW_SESSION.written()),
            AppRequest::CloseSession => (Glyph::Close, crate::shortcuts::CLOSE_SESSION.written()),
            AppRequest::Settings(_) => (Glyph::Settings, crate::shortcuts::SETTINGS.written()),
            AppRequest::Shortcuts => (Glyph::Key, ""),
            AppRequest::Profiles
            | AppRequest::Profile(..)
            | AppRequest::SaveAsProfile
            | AppRequest::Play(_) => (Glyph::Profile, ""),
            _ => (Glyph::Check, ""),
        },
        Act::Reconnect => (Glyph::Restart, crate::shortcuts::RECONNECT.written()),
        Act::Disconnect => (Glyph::Close, ""),
        Act::Find => (Glyph::Search, crate::shortcuts::FIND.written()),
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
