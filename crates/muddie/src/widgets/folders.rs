//! A folder in a list: its row, its arrow and the field its name is typed in.

use super::*;

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
