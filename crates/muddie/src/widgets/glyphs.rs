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
    /// Four arrows outward: shown larger.
    Expand,
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
            Self::Expand => 0xf065,
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
    glyph_button_as(ui, (glyph, None, label))
}

/// A [`glyph_button`] with its picture in a colour of its own, if given.
fn glyph_button_as(ui: &mut Ui, (glyph, tint, label): (Glyph, Option<Color32>, &str)) -> Response {
    let theme = theme::current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let lit = response.hovered() || response.has_focus();
        if lit {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(7), theme.hover);
        }
        let colour = tint.unwrap_or(if lit { theme.text } else { theme.text_faint });
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

/// How long a button that copied says so.
const COPIED_FOR: f64 = 1.5;

/// Where a button that copies `text` in `ui` keeps when it last did.
fn copied_key(ui: &Ui, text: &str) -> egui::Id {
    ui.scope_id().with(("copied at", text))
}

/// Whether the button of `key` copied a moment ago; while it did, the
/// screen is drawn again when the moment is over, to show it as it was.
fn copied_lately(ctx: &egui::Context, key: egui::Id) -> bool {
    let Some(at) = ctx.data(|data| data.get_temp::<f64>(key)) else {
        return false;
    };
    let left = at + COPIED_FOR - ctx.input(|input| input.time);
    if left > 0.0 {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(left));
    }
    left > 0.0
}

/// Copies `text` when `button` was clicked, and notes when under `key`,
/// for the button to say so.
fn copy_on_click(ctx: &egui::Context, button: &Response, key: egui::Id, text: &str) {
    if button.clicked() {
        ctx.copy_text(text.to_owned());
        let now = ctx.input(|input| input.time);
        ctx.data_mut(|data| data.insert_temp(key, now));
    }
}

/// A button with the picture of copying, named `label`, that copies
/// `text`; for a moment after, a tick in the colour of what went well,
/// named "Copied", to see and to hear that it did.
pub fn copy_button(ui: &mut Ui, label: &str, text: &str) -> Response {
    let key = copied_key(ui, text);
    let look = match copied_lately(ui.ctx(), key) {
        true => (
            Glyph::Check,
            Some(theme::current().good),
            crate::i18n::tr("Copied"),
        ),
        false => (Glyph::Copy, None, label),
    };
    let button = glyph_button_as(ui, look);
    copy_on_click(ui.ctx(), &button, key, text);
    button
}

/// The same with words: "Copy", and "Copied" with a tick for a moment
/// after it copied `text`.
pub fn copy_button_with(ui: &mut Ui, text: &str) -> Response {
    let key = copied_key(ui, text);
    let button = match copied_lately(ui.ctx(), key) {
        true => super::button_with_tint(
            ui,
            Glyph::Check,
            crate::i18n::tr("Copied"),
            Some(theme::current().good),
        ),
        false => super::button_with(ui, Glyph::Copy, crate::i18n::tr("Copy")),
    };
    copy_on_click(ui.ctx(), &button, key, text);
    button
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn a_copy_button_shows_and_says_that_it_copied_for_a_moment() {
        // The pictures are a font of their own, given before anything is
        // drawn with it.
        let mut ready = false;
        let mut harness = Harness::new_ui(move |ui| {
            if !std::mem::replace(&mut ready, true) {
                crate::theme::apply(ui.ctx(), &crate::settings::Settings::default()).unwrap();
                return;
            }
            copy_button(ui, "Copy the path", "Char.Vitals.hp");
            copy_button_with(ui, "$target");
        });
        harness.step();
        harness.step();
        harness.get_by_label("Copy the path").click();
        harness.step();
        harness.step();
        // Its name is now that it copied; the other is as it was.
        harness.get_by_label("Copied");
        harness.get_by_label("Copy").click();
        harness.step();
        harness.step();
        assert_eq!(harness.query_all_by_label("Copied").count(), 2);
        // A moment later, each is as it was.
        // The test's clock goes a frame at a time, a sixtieth of a second.
        for _ in 0..100 {
            harness.step();
        }
        harness.get_by_label("Copy the path");
        harness.get_by_label("Copy");
    }
}
