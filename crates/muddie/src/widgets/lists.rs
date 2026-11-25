//! Rows to choose among: in a sidebar, a list, a group, or as cards.

use super::*;

/// An entry in a sidebar: its name, and a count at the right if given.
pub fn nav_item(ui: &mut Ui, selected: bool, text: &str, count: Option<usize>) -> Response {
    let theme = theme::current();
    let height = 36.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Tab, ui.is_enabled(), selected, text));
    // How many there are, as it is seen at its right.
    if let Some(count) = count {
        let many = crate::i18n::trn("{0} item", "{0} items", count as u64);
        (ui.ctx()).accesskit_node_builder(response.id, |node| node.set_description(many));
    }
    if ui.is_rect_visible(rect) {
        let fill = match (selected, response.hovered()) {
            (true, _) => theme.raised,
            (false, true) => theme.raised.gamma_multiply(0.6),
            (false, false) => Color32::TRANSPARENT,
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(7), fill);
        let colour = if selected { theme.text } else { theme.text_dim };
        let weight = if selected {
            Weight::Medium
        } else {
            Weight::Regular
        };
        painter.text(
            rect.left_center() + vec2(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            text,
            theme::font(weight, 13.5),
            colour,
        );
        if let Some(count) = count.filter(|count| *count > 0) {
            painter.text(
                rect.right_center() - vec2(12.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                count.to_string(),
                theme::font(Weight::Regular, 12.0),
                theme.text_faint,
            );
        }
    }
    response
}

/// A row of a list: a title with a line of detail under it. A row that is
/// not `active` (a rule switched off, say) is shown faded.
pub fn list_row(ui: &mut Ui, selected: bool, title: &str, detail: &str, active: bool) -> Response {
    let row = ListRow {
        title,
        detail,
        selected,
        active,
        ..ListRow::default()
    };
    row.show(ui)
}

/// How tall a row of a list is, with no detail and with one.
pub const ROW_SHORT: f32 = 34.0;
pub const ROW_TALL: f32 = 48.0;
/// How tall a folder's line is.
pub const FOLDER_ROW: f32 = 32.0;

/// How far in each level of folders is.
pub const INDENT: f32 = 18.0;

/// A row of a list, with all it can have: a title with a line of detail
/// under it and its tags beside it, faded unless `active`, and `depth`
/// folders in, with a line down for each.
#[derive(Default)]
pub struct ListRow<'a> {
    pub title: &'a str,
    pub detail: &'a str,
    /// The detail part by part, each with its picture: shown in place of
    /// `detail`, which is what a screen reader is told.
    pub parts: &'a [(Glyph, String)],
    pub tags: &'a [String],
    pub selected: bool,
    pub active: bool,
    pub depth: usize,
    /// It failed: its title is in the colour of what could not be done.
    pub problem: bool,
    /// Its id, the same every frame, for the keyboard to be put on it.
    pub key: Option<egui::Id>,
    /// It is the one of its list Tab stops at; the others are gone to
    /// with the arrows.
    pub current: bool,
    /// In a tree: its place among those beside it, from 1, and how many
    /// they are. Its level is `depth`.
    pub place: Option<(usize, usize)>,
}

/// Makes a line of a list, `height` tall: clicked, and stopped at by Tab
/// only if it is `current`, as in a tree, where the arrows go through the
/// rest; with `key` as its id, if given.
pub(super) fn list_line(
    ui: &mut Ui,
    height: f32,
    key: Option<egui::Id>,
    current: bool,
) -> Response {
    let size = vec2(ui.available_width(), height);
    let Some(key) = key else {
        return ui.allocate_exact_size(size, Sense::click()).1;
    };
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let sense = match current {
        true => Sense::click(),
        false => Sense::CLICK,
    };
    ui.interact(rect, key, sense)
}

/// While the keyboard is on a line of a list, the arrows are the list's,
/// not for going to what is beside it.
pub fn hold_arrows(ui: &Ui, line: &Response) {
    if line.has_focus() {
        let filter = egui::EventFilter {
            horizontal_arrows: true,
            vertical_arrows: true,
            ..Default::default()
        };
        ui.memory_mut(|memory| memory.set_focus_lock_filter(line.id, filter));
    }
}

/// The room at the right of what scrolls for the bar it scrolls by, which
/// is drawn over what is under it.
const BAR_ROOM: f32 = 14.0;

/// Keeps what is laid out next in `ui`, inside something that scrolls,
/// clear of the bar it scrolls by: switches, counts and highlights are not
/// under it, nor taken by it.
pub fn keep_clear_of_bar(ui: &mut Ui) {
    ui.set_max_width(ui.available_width() - BAR_ROOM);
}

/// How far apart the lines of a list are: their highlights never touch.
pub const LINE_GAP: f32 = 4.0;

/// Under a line of a list, as it is chosen or pointed at: chosen, a fill in
/// the accent with a bar at its left; pointed at, a faint fill; the two
/// never alike, and nothing more where it is chosen.
pub(super) fn line_backdrop(ui: &Ui, body: egui::Rect, selected: bool, hovered: bool) {
    let theme = theme::current();
    let round = CornerRadius::same(8);
    if selected {
        ui.painter()
            .rect_filled(body, round, theme.accent.gamma_multiply(0.2));
        let bar = egui::Rect::from_min_size(
            body.left_top() + vec2(0.0, 7.0),
            vec2(3.0, (body.height() - 14.0).max(4.0)),
        );
        ui.painter()
            .rect_filled(bar, CornerRadius::same(2), theme.accent);
    } else if hovered {
        ui.painter()
            .rect_filled(body, round, theme.raised.gamma_multiply(0.45));
    }
}

/// Around a line of a list, what is about to act on it: where what is
/// dragged would go, or what a menu open acts on. Unlike the line chosen.
pub fn target_ring(ui: &Ui, rect: egui::Rect) {
    let ring = Stroke::new(1.5, theme::current().accent);
    let round = CornerRadius::same(8);
    ui.painter()
        .rect_stroke(rect.shrink(1.0), round, ring, egui::StrokeKind::Inside);
}

/// Tells a screen reader where a line is in a tree: how deep, and its
/// place among those beside it.
pub(super) fn tree_place(node: &mut egui::accesskit::Node, depth: usize, place: (usize, usize)) {
    node.set_level(depth + 1);
    node.set_position_in_set(place.0);
    node.set_size_of_set(place.1);
}

/// The screen readers of Linux are not told how deep a line of a tree is,
/// nor whether a folder is open: there, both are in its description. On
/// Windows and macOS they are told, and say it themselves, in their own
/// words; said again in ours, it would be heard twice.
const SAID_IN_WORDS: bool = cfg!(target_os = "linux");

/// How deep a line of a tree is, in words, when it is in a folder, where
/// the screen reader is not told otherwise.
pub(super) fn level_words(depth: usize) -> Option<String> {
    (SAID_IN_WORDS && depth > 0).then(|| crate::i18n::trf("level {0}", &[&(depth + 1).to_string()]))
}

/// Whether a folder is open, in words, where the screen reader is not
/// told otherwise.
pub(super) fn folded_words(open: bool) -> Option<&'static str> {
    SAID_IN_WORDS.then(|| match open {
        true => crate::i18n::tr("expanded"),
        false => crate::i18n::tr("collapsed"),
    })
}

impl ListRow<'_> {
    pub fn show(self, ui: &mut Ui) -> Response {
        let theme = theme::current();
        let height = if self.detail.is_empty() && self.parts.is_empty() {
            34.0
        } else {
            48.0
        };
        let response = list_line(ui, height, self.key, self.current);
        let rect = response.rect;
        // Chosen, not ticked; what it does, and whether it is off, as it is
        // seen under its title and faded.
        let role = match self.place {
            Some(_) => Role::TreeItem,
            None => Role::ListItem,
        };
        response.widget_info(|| WidgetInfo::labeled(role, ui.is_enabled(), self.title));
        (ui.ctx()).accesskit_node_builder(response.id, |node| {
            node.set_selected(self.selected);
            if let Some(place) = self.place {
                tree_place(node, self.depth, place);
            }
            let mut about = vec![self.detail.trim().to_owned()];
            about.extend(self.tags.iter().cloned());
            if !self.active {
                about.push(crate::i18n::tr("Disabled").to_owned());
            }
            about.extend(self.place.and_then(|_| level_words(self.depth)));
            about.retain(|part| !part.is_empty());
            if !about.is_empty() {
                node.set_description(about.join(", "));
            }
        });
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let body = indented(ui, rect, self.depth);
        line_backdrop(ui, body, self.selected, response.hovered());
        let fade = if self.active { 1.0 } else { 0.45 };
        let painter = ui.painter().with_clip_rect(body.shrink2(vec2(8.0, 0.0)));
        let left = body.left() + 12.0;
        let title_y = if self.detail.is_empty() && self.parts.is_empty() {
            body.center().y
        } else {
            body.top() + 16.0
        };
        let ink = if self.problem {
            theme.danger
        } else {
            theme.text
        };
        let title = painter.text(
            egui::pos2(left, title_y),
            egui::Align2::LEFT_CENTER,
            self.title,
            theme::font(Weight::Medium, 13.5),
            ink.gamma_multiply(fade),
        );
        let mut at = title.right() + 8.0;
        for tag in self.tags.iter().filter(|tag| !tag.trim().is_empty()) {
            at = tag_pill(&painter, egui::pos2(at, title_y), tag.trim(), fade).right() + 4.0;
        }
        // Off, it says so at the right, besides being faded.
        if !self.active {
            let galley = painter.layout_no_wrap(
                crate::i18n::tr("Off").to_owned(),
                theme::font(Weight::Medium, 10.5),
                theme.text_faint,
            );
            let pill = egui::Rect::from_min_size(
                egui::pos2(
                    body.right() - galley.size().x - 22.0,
                    title_y - galley.size().y / 2.0 - 2.0,
                ),
                galley.size() + vec2(12.0, 4.0),
            );
            painter.rect_stroke(
                pill,
                CornerRadius::same(255),
                Stroke::new(1.0, theme.border),
                egui::StrokeKind::Inside,
            );
            painter.galley(pill.min + vec2(6.0, 2.0), galley, theme.text_faint);
        }
        let line_y = body.top() + 33.0;
        let ink = theme.text_dim.gamma_multiply(fade);
        if !self.parts.is_empty() {
            let mut at = left;
            for (glyph, text) in self.parts {
                let mark = paint_glyph(
                    &painter,
                    egui::pos2(at, line_y),
                    egui::Align2::LEFT_CENTER,
                    *glyph,
                    10.5,
                    theme.text_faint.gamma_multiply(fade),
                );
                let words = painter.text(
                    egui::pos2(mark.right() + 5.0, line_y),
                    egui::Align2::LEFT_CENTER,
                    text,
                    theme::font(Weight::Regular, 12.0),
                    ink,
                );
                at = words.right() + 12.0;
            }
        } else if !self.detail.is_empty() {
            painter.text(
                egui::pos2(left, line_y),
                egui::Align2::LEFT_CENTER,
                self.detail,
                theme::font(Weight::Regular, 12.0),
                ink,
            );
        }
        response
    }
}

/// Draws a line down for each folder a row is in, and returns the part of
/// the row left for it.
pub(super) fn indented(ui: &Ui, rect: egui::Rect, depth: usize) -> egui::Rect {
    let guide = Stroke::new(1.0, theme::current().border);
    for level in 0..depth {
        let x = rect.left() + 14.0 + INDENT * level as f32;
        ui.painter().vline(x, rect.y_range(), guide);
    }
    rect.with_min_x(rect.left() + INDENT * depth as f32)
}

/// A tag, as a small pill starting at `left`, centred on its height.
/// The colour of a tag, the same wherever it is shown, made from its name
/// as it is compared: tags are told apart at a glance.
pub fn tag_colour(tag: &str) -> Color32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    crate::rules::tag_key(tag).hash(&mut hasher);
    let terminal = theme::current().terminal;
    [
        terminal[4],
        terminal[5],
        terminal[6],
        terminal[2],
        terminal[3],
        terminal[1],
    ][(hasher.finish() % 6) as usize]
}

pub fn tag_pill(
    painter: &egui::Painter,
    left_centre: egui::Pos2,
    tag: &str,
    fade: f32,
) -> egui::Rect {
    let colour = tag_colour(tag);
    let galley = painter.layout_no_wrap(
        tag.to_owned(),
        theme::font(Weight::Medium, 11.0),
        colour.gamma_multiply(fade),
    );
    let size = galley.size() + vec2(12.0, 4.0);
    let pill = egui::Rect::from_min_size(left_centre - vec2(0.0, size.y / 2.0), size);
    painter.rect_filled(
        pill,
        CornerRadius::same(255),
        colour.gamma_multiply(0.14 * fade),
    );
    painter.galley(pill.min + vec2(6.0, 2.0), galley, colour);
    pill
}
