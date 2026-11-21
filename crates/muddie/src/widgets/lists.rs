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
fn list_line(ui: &mut Ui, height: f32, key: Option<egui::Id>, current: bool) -> Response {
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
fn line_backdrop(ui: &Ui, body: egui::Rect, selected: bool, hovered: bool) {
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
fn tree_place(node: &mut egui::accesskit::Node, depth: usize, place: (usize, usize)) {
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
fn level_words(depth: usize) -> Option<String> {
    (SAID_IN_WORDS && depth > 0).then(|| crate::i18n::trf("level {0}", &[&(depth + 1).to_string()]))
}

/// Whether a folder is open, in words, where the screen reader is not
/// told otherwise.
fn folded_words(open: bool) -> Option<&'static str> {
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
fn indented(ui: &Ui, rect: egui::Rect, depth: usize) -> egui::Rect {
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

/// Where the arrow of a folder's line ends, given the line and how many
/// folders in it is: a click before it opens or closes the folder.
pub fn folder_arrow_end(line: egui::Rect, depth: usize) -> f32 {
    line.left() + INDENT * depth as f32 + 22.0
}

/// The field a folder's name is typed in, over its line, where the name
/// is: in the name's own letters, centred on the line, framed as the other
/// fields are, in the colour of a mistake if the name cannot be.
pub fn folder_name_field(
    ui: &mut Ui,
    line: egui::Rect,
    depth: usize,
    name: &mut String,
    id: egui::Id,
    wrong: bool,
) -> Response {
    let theme = theme::current();
    // Clear of the folder drawn before it, with room inside, and short of
    // the count and the switch.
    let left = line.left() + INDENT * depth as f32 + 40.0;
    let place = egui::Rect::from_min_max(
        egui::pos2(left, line.center().y - 12.0),
        egui::pos2(line.right() - 70.0, line.center().y + 12.0),
    );
    ui.painter()
        .rect_filled(place, CornerRadius::same(6), theme.surface);
    // As much room above the text as under it: the text on the line's
    // middle, where the name was.
    let font = theme::font(Weight::Medium, 13.0);
    let tall = ui.fonts_mut(|fonts| fonts.row_height(&font));
    // Inside the frame: a field without one keeps no margin of its own.
    let text = egui::Rect::from_center_size(place.center(), vec2(place.width() - 14.0, tall));
    let mut over = ui.new_child(egui::UiBuilder::new().max_rect(text));
    over.spacing_mut().interact_size.y = 0.0;
    let field = over.add(
        TextEdit::singleline(name)
            .id(id)
            .frame(Frame::NONE)
            .margin(Margin::ZERO)
            .font(font)
            .desired_width(text.width()),
    );
    let edge = if wrong { theme.danger } else { theme.accent };
    ui.painter().rect_stroke(
        place,
        CornerRadius::same(6),
        Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
    field
}

/// A folder in a list: an arrow that points down when it is open, a folder,
/// its name and how many it holds, `depth` folders in. Faded unless
/// `active`; marked as a thing of the list is when `selected`. Room is left
/// at the right for its switch.
pub struct FolderRow<'a> {
    pub name: &'a str,
    pub depth: usize,
    pub open: bool,
    pub active: bool,
    pub count: usize,
    pub selected: bool,
    /// As for a [`ListRow`].
    pub key: Option<egui::Id>,
    pub current: bool,
    pub place: Option<(usize, usize)>,
}

impl FolderRow<'_> {
    pub fn show(self, ui: &mut Ui) -> Response {
        let theme = theme::current();
        let response = list_line(ui, FOLDER_ROW, self.key, self.current);
        let rect = response.rect;
        // Open or closed, not ticked; chosen; how many it holds, and
        // whether it is off, as it is seen.
        response.widget_info(|| WidgetInfo::labeled(Role::TreeItem, ui.is_enabled(), self.name));
        (ui.ctx()).accesskit_node_builder(response.id, |node| {
            node.set_expanded(self.open);
            node.set_selected(self.selected);
            if let Some(place) = self.place {
                tree_place(node, self.depth, place);
            }
            let mut about: Vec<String> = folded_words(self.open)
                .map(str::to_owned)
                .into_iter()
                .collect();
            about.push(crate::i18n::trn("{0} item", "{0} items", self.count as u64));
            if !self.active {
                about.push(crate::i18n::tr("Disabled").to_owned());
            }
            about.extend(level_words(self.depth));
            node.set_description(about.join(", "));
        });
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let body = indented(ui, rect, self.depth);
        line_backdrop(ui, body, self.selected, response.hovered());
        let fade = if self.active { 1.0 } else { 0.45 };
        let ink = theme.text_dim.gamma_multiply(fade);
        let stroke = Stroke::new(1.4, ink);
        let tip = body.left_center() + vec2(12.0, 0.0);
        let arrow = match self.open {
            true => vec![
                tip + vec2(-4.0, -2.0),
                tip + vec2(0.0, 2.0),
                tip + vec2(4.0, -2.0),
            ],
            false => vec![
                tip + vec2(-2.0, -4.0),
                tip + vec2(2.0, 0.0),
                tip + vec2(-2.0, 4.0),
            ],
        };
        ui.painter().add(egui::Shape::line(arrow, stroke));
        draw_folder(ui.painter(), body.left_center() + vec2(30.0, 0.0), stroke);
        let painter = ui
            .painter()
            .with_clip_rect(body.with_max_x(body.right() - 80.0));
        painter.text(
            body.left_center() + vec2(42.0, 0.0),
            egui::Align2::LEFT_CENTER,
            self.name,
            theme::font(Weight::Medium, 13.0),
            theme.text.gamma_multiply(fade),
        );
        ui.painter().text(
            body.right_center() - vec2(58.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            self.count.to_string(),
            theme::font(Weight::Regular, 11.5),
            theme.text_faint,
        );
        response
    }
}

/// A saved profile to pick, as a card: a coloured initial, its name,
/// where it leads.
pub fn profile_card(ui: &mut Ui, width: f32, name: &str, place: &str) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 68.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), name));
    // Where it leads, as it is seen under its name.
    (ui.ctx()).accesskit_node_builder(response.id, |node| node.set_description(place));
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() || response.has_focus();
        let lift = ui.ctx().animate_bool_with_time(response.id, hovered, 0.12);
        let hue = profile_hue(name);
        let painter = ui.painter();
        let fill = theme::mix(theme.raised, theme.hover, lift);
        painter.rect_filled(rect, CornerRadius::same(14), fill);
        let edge = theme::mix(theme.border, hue, lift * 0.7);
        painter.rect_stroke(
            rect,
            CornerRadius::same(14),
            Stroke::new(1.0, edge),
            egui::StrokeKind::Inside,
        );
        let tile =
            egui::Rect::from_center_size(rect.left_center() + vec2(35.0, 0.0), Vec2::splat(40.0));
        initial_tile(painter, tile, name, fill);
        let painter = painter.with_clip_rect(rect.shrink2(vec2(12.0, 0.0)));
        two_lines(
            &painter,
            rect.left() + 68.0,
            rect.center().y,
            (name, 14.5),
            (place, 12.5),
        );
    }
    response
}

/// A saved profile in the list of the Profiles window: its coloured
/// initial, its name and where it leads, lit up when chosen or under the
/// pointer, and a dot when a session plays with it.
pub fn profile_row(ui: &mut Ui, selected: bool, name: &str, place: &str, in_use: bool) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 52.0), Sense::click());
    let said = match in_use {
        true => format!("{name}, {place}, {}", crate::i18n::tr("In use")),
        false => format!("{name}, {place}"),
    };
    response.widget_info(|| WidgetInfo::selected(Role::Tab, ui.is_enabled(), selected, &said));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hovered = response.hovered() || response.has_focus();
    let fill = match (selected, hovered) {
        (true, _) => theme.raised,
        (false, true) => theme.raised.gamma_multiply(0.6),
        (false, false) => Color32::TRANSPARENT,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), fill);
    let tile =
        egui::Rect::from_center_size(rect.left_center() + vec2(24.0, 0.0), Vec2::splat(32.0));
    initial_tile(
        painter,
        tile,
        name,
        theme::mix(theme.surface, fill, fill.a() as f32 / 255.0),
    );
    let clip = rect.with_max_x(rect.right() - if in_use { 24.0 } else { 10.0 });
    two_lines(
        &painter.with_clip_rect(clip),
        rect.left() + 50.0,
        rect.center().y,
        (name, 13.5),
        (place, 12.0),
    );
    if in_use {
        painter.circle_filled(rect.right_center() - vec2(14.0, 0.0), 3.5, theme.accent);
    }
    response
}

/// A profile's initial on a tile of its colour, over `ground`.
pub fn initial_tile(painter: &egui::Painter, tile: egui::Rect, name: &str, ground: Color32) {
    let hue = profile_hue(name);
    let radius = CornerRadius::same((tile.height() * 0.28) as u8);
    painter.rect_filled(tile, radius, theme::mix(ground, hue, 0.22));
    let initial: String = name.chars().take(1).flat_map(char::to_uppercase).collect();
    let size = tile.height() * 0.4;
    painter.text(
        tile.center(),
        egui::Align2::CENTER_CENTER,
        initial,
        theme::font(Weight::Semibold, size),
        hue,
    );
}

/// The colour of a profile: the same name always gets the same one.
fn profile_hue(name: &str) -> Color32 {
    let hues = [12, 10, 11, 13, 14, 9];
    let sum: usize = name.bytes().map(usize::from).sum();
    theme::current().terminal[hues[sum % hues.len()]]
}

/// A name, and a line about it under it, starting at `left` and centred
/// on `middle`; the sizes of their type are given with them.
fn two_lines(
    painter: &egui::Painter,
    left: f32,
    middle: f32,
    (name, big): (&str, f32),
    (under, small): (&str, f32),
) {
    let theme = theme::current();
    let at = |dy: f32| egui::pos2(left, middle + dy);
    painter.text(
        at(-9.0),
        egui::Align2::LEFT_CENTER,
        name,
        theme::font(Weight::Medium, big),
        theme.text,
    );
    painter.text(
        at(10.0),
        egui::Align2::LEFT_CENTER,
        under,
        theme::font(Weight::Regular, small),
        theme.text_dim,
    );
}

/// A dot that says at a glance how something is doing.
pub fn status_dot(ui: &mut Ui, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.5, colour);
}
