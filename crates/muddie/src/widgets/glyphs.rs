//! Small pictures drawn from the icons of the font that comes with the
//! program, to mark what something is at a glance. They are only seen: a
//! screen reader is told what they mean by what they are next to.

use super::*;

/// What a picture shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// A bolt: when it goes off.
    When,
    /// An arrow: what it sends.
    Send,
    /// A wand: what else it does.
    Also,
    Sound,
    Say,
    Colour,
    Hide,
    Panel,
    /// A flask: trying it out.
    Try,
    Restart,
    Copy,
    Folder,
    Key,
    Timer,
    Gauge,
    Data,
    Tag,
    Script,
    Code,
    Type,
    Map,
    Channels,
    Lock,
    Search,
    Up,
    Down,
    Close,
    Warning,
    Check,
    Plus,
    Settings,
    Profile,
    Text,
}

impl Glyph {
    fn char(self) -> char {
        let code = match self {
            Self::When => 0xf0e7,
            Self::Send => 0xf061,
            Self::Also => 0xf0d0,
            Self::Sound => 0xf001,
            Self::Say => 0xf075,
            Self::Colour => 0xf1fc,
            Self::Hide => 0xf070,
            Self::Panel => 0xf0db,
            Self::Try => 0xf0c3,
            Self::Restart => 0xf021,
            Self::Copy => 0xf0c5,
            Self::Folder => 0xf07c,
            Self::Key => 0xf11c,
            Self::Timer => 0xf017,
            Self::Gauge => 0xf0e4,
            Self::Data => 0xf1c0,
            Self::Tag => 0xf02b,
            Self::Script => 0xf120,
            Self::Code => 0xf121,
            Self::Type => 0xf1b2,
            Self::Map => 0xf279,
            Self::Channels => 0xf086,
            Self::Lock => 0xf023,
            Self::Search => 0xf002,
            Self::Up => 0xf062,
            Self::Down => 0xf063,
            Self::Close => 0xf00d,
            Self::Warning => 0xf071,
            Self::Check => 0xf00c,
            Self::Plus => 0xf067,
            Self::Settings => 0xf013,
            Self::Profile => 0xf007,
            Self::Text => 0xf03a,
        };
        char::from_u32(code).unwrap_or('?')
    }
}

/// The font the pictures are drawn in, at `size`.
pub fn glyph_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("icons".into()))
}

/// Draws a picture at `at`, placed by `align`.
pub fn paint_glyph(
    painter: &egui::Painter,
    at: egui::Pos2,
    align: egui::Align2,
    glyph: Glyph,
    size: f32,
    colour: Color32,
) -> egui::Rect {
    painter.text(at, align, glyph.char(), glyph_font(size), colour)
}

/// A small button that is only a picture, quiet until the pointer or the
/// keyboard is on it. `label` says what it does, to a screen reader and
/// when pointed at.
pub fn glyph_button(ui: &mut Ui, glyph: Glyph, label: &str) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let lit = response.hovered() || response.has_focus();
        if lit {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(7), theme.hover);
        }
        let colour = if lit { theme.text } else { theme.text_faint };
        paint_glyph(
            ui.painter(),
            rect.center(),
            egui::Align2::CENTER_CENTER,
            glyph,
            13.0,
            colour,
        );
    }
    response.on_hover_text(label)
}

/// Opens a file or a folder as the system does, in its own application.
pub fn open_path(path: &std::path::Path) {
    let opener = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener).arg(path).spawn();
}

/// A picture in the flow of `ui`, as wide as it is tall: seen only.
pub fn glyph(ui: &mut Ui, glyph: Glyph, size: f32, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size + 2.0), Sense::hover());
    paint_glyph(
        ui.painter(),
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        size,
        colour,
    );
}
