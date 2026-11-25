//! The line that says where the player is, the chips for its exits and the level buttons.

use super::*;

/// Where the player is, as text: the room's name and its region, then
/// its exits; for a screen reader, all in one line.
pub(super) fn heading(ui: &mut Ui, map: &Map, current: &Room) {
    let palette = theme::current();
    let (name, region) = place_parts(map, current);
    let mut text = RichText::new(name)
        .font(theme::font(Weight::Medium, 14.0))
        .color(palette.text);
    let shown = ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let named = ui.add(eframe::egui::Label::new(std::mem::take(&mut text)).selectable(false));
        if let Some(region) = region {
            let region = RichText::new(region)
                .font(theme::font(Weight::Regular, 12.5))
                .color(palette.text_faint);
            ui.add(eframe::egui::Label::new(region).selectable(false));
        }
        named
    });
    let said = format!(
        "{}. {}",
        place_words(map, current),
        exits_words(map, current)
    );
    (ui.ctx()).accesskit_node_builder(shown.inner.id, |node| node.set_label(said));
    ui.add_space(2.0);
    let chips = exit_chips(ui, map, current, ui.available_width());
    show_chips(ui, chips);
    ui.add_space(4.0);
}

/// A room's exits laid out as chips, each the arrow of its way, where it
/// has one, and its word in the MUD's language: where each goes, and the
/// room they take.
pub(super) struct Chips {
    chips: Vec<(
        Rect,
        Option<&'static str>,
        std::sync::Arc<eframe::egui::Galley>,
    )>,
    pub(super) size: Vec2,
}

/// The arrow of a way, for those of the compass, up and down.
fn way_arrow(way: &str) -> Option<&'static str> {
    Some(match way {
        "n" => "↑",
        "ne" => "↗",
        "e" => "→",
        "se" => "↘",
        "s" => "↓",
        "sw" => "↙",
        "w" => "←",
        "nw" => "↖",
        "u" => "▲",
        "d" => "▼",
        _ => return None,
    })
}

/// Lays a room's exits out as chips, in the order of the compass, in as
/// many rows as `width` takes; "no exits" if it has none.
pub(super) fn exit_chips(ui: &Ui, map: &Map, room: &Room, width: f32) -> Chips {
    const HEIGHT: f32 = 20.0;
    const PAD: f32 = 9.0;
    const ARROW: f32 = 13.0;
    const GAP: f32 = 4.0;
    let palette = theme::current();
    let font = theme::font(Weight::Regular, 12.0);
    let ways = ordered_exits(room);
    let mut words: Vec<(Option<&'static str>, String)> = (ways.iter())
        .map(|way| (way_arrow(way), direction_words(map, way)))
        .collect();
    if words.is_empty() {
        words.push((None, tr("no exits").to_owned()));
    }
    let (mut x, mut y, mut widest) = (0.0_f32, 0.0_f32, 0.0_f32);
    let mut chips = Vec::new();
    for (arrow, word) in words {
        let galley = ui
            .painter()
            .layout_no_wrap(word, font.clone(), palette.text_dim);
        let wide = PAD * 2.0 + galley.size().x + if arrow.is_some() { ARROW } else { 0.0 };
        if x > 0.0 && x + wide > width {
            (x, y) = (0.0, y + HEIGHT + GAP);
        }
        chips.push((
            Rect::from_min_size(Pos2::new(x, y), vec2(wide, HEIGHT)),
            arrow,
            galley,
        ));
        x += wide + GAP;
        widest = widest.max(x - GAP);
    }
    Chips {
        chips,
        size: vec2(widest, y + HEIGHT),
    }
}

/// Draws the chips laid out by [`exit_chips`]. They are drawn, not
/// widgets: what they say is said with the room.
pub(super) fn show_chips(ui: &mut Ui, chips: Chips) {
    let palette = theme::current();
    let (rect, _) = ui.allocate_exact_size(chips.size, Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    for (place, arrow, galley) in chips.chips {
        let place = place.translate(rect.min.to_vec2());
        painter.rect_filled(
            place,
            eframe::egui::CornerRadius::same(u8::MAX),
            palette.raised,
        );
        let mut x = place.min.x + 9.0;
        if let Some(arrow) = arrow {
            painter.text(
                Pos2::new(x, place.center().y),
                eframe::egui::Align2::LEFT_CENTER,
                arrow,
                theme::font(Weight::Regular, 11.0),
                palette.text_faint,
            );
            x += 13.0;
        }
        let at = Pos2::new(x, place.center().y - galley.size().y / 2.0);
        painter.galley(at, galley, palette.text_dim);
    }
}

/// The level to show, as floors one above another: up, the level shown,
/// and down, each step to the next level that has rooms. Returns what to
/// say: the level gone to, or that there is none that way. The buttons
/// stay pressable at either end, only drawn fainter, so that the keyboard
/// stays on them, and say so.
pub(super) fn level_steps(ui: &mut Ui, levels: &[i32], level: &mut i32) -> Option<String> {
    let below = levels.iter().rev().find(|known| **known < *level).copied();
    let above = levels.iter().find(|known| **known > *level).copied();
    let step = |ui: &mut Ui, sign, label, to: Option<i32>| {
        ui.scope(|ui| {
            if to.is_none() {
                ui.multiply_opacity(0.35);
            }
            crate::widgets::sign_button(ui, sign, label, false).clicked()
        })
        .inner
    };
    let mut said = None;
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        if step(ui, "▲", tr("Up a level"), above) {
            said = Some(match above {
                Some(to) => {
                    *level = to;
                    crate::i18n::trf("Level {0}", &[&to.to_string()])
                }
                None => tr("No level above.").to_owned(),
            });
        }
        // Its number, said as the level it is.
        let level_said = crate::i18n::trf("Level {0}", &[&level.to_string()]);
        let shown = ui.label(
            RichText::new(level.to_string())
                .font(theme::font(Weight::Medium, 12.5))
                .color(theme::current().text),
        );
        let shown = crate::widgets::name_tip(shown, &level_said);
        shown.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &level_said));
        if step(ui, "▼", tr("Down a level"), below) {
            said = Some(match below {
                Some(to) => {
                    *level = to;
                    crate::i18n::trf("Level {0}", &[&to.to_string()])
                }
                None => tr("No level below.").to_owned(),
            });
        }
    });
    said
}
