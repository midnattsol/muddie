//! The look of the interface: colours, type and spacing for each theme.
//!
//! The interface is set in Inter and the output in JetBrains Mono, both
//! built in so that it looks the same everywhere.

use std::sync::RwLock;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, TextStyle, Vec2,
};

use crate::settings::{Settings, ThemeName};

mod fonts;
mod palettes;

use fonts::install_fonts;
pub use fonts::{BUNDLED_FONTS, DEFAULT_FONT, bundled_font, installed_fonts};
use palettes::*;

/// The colours of one theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub dark: bool,
    /// Behind the output and the map.
    pub background: Color32,
    /// Bars, side panels and windows.
    pub surface: Color32,
    /// Where the interface itself takes text: fields and editors. Between
    /// `background` and `surface`.
    pub field: Color32,
    /// Controls, and anything raised from a surface.
    pub raised: Color32,
    /// A control under the pointer.
    pub hover: Color32,
    /// Hairlines between areas and around fields.
    pub border: Color32,
    pub text: Color32,
    /// Secondary text: labels, lines the client adds itself.
    pub text_dim: Color32,
    /// Captions and hints.
    pub text_faint: Color32,
    /// The one colour that marks what is selected, focused or primary.
    pub accent: Color32,
    /// Text on top of the accent.
    pub on_accent: Color32,
    pub good: Color32,
    pub danger: Color32,
    /// The 16 base terminal colours, tuned to read well on `background`.
    pub terminal: [Color32; 16],
}

/// The sizes things are drawn at, so that every screen has the same
/// rhythm: one height for a control, a few widths for a field, a few
/// for a window, and the gaps between.
pub mod size {
    /// A button, a field, a list to choose from.
    pub const CONTROL: f32 = 30.0;
    /// A row of settings, with its control.
    pub const ROW: f32 = 44.0;
    /// Controls at the right of a row of settings: a number; anything
    /// else that is typed in or chosen from, all as wide as each other.
    pub const FIELD_S: f32 = 96.0;
    pub const FIELD_L: f32 = 250.0;
    /// Windows: a question, a form, the settings, the profiles.
    pub const DIALOG_S: f32 = 460.0;
    pub const DIALOG_M: f32 = 620.0;
    pub const DIALOG_L: f32 = 720.0;
    pub const DIALOG_XL: f32 = 940.0;
    /// Gaps: inside a group, between groups, and between the sections
    /// of a page.
    pub const GAP_S: f32 = 6.0;
    pub const GAP_M: f32 = 12.0;
    pub const GAP_L: f32 = 28.0;
}

/// The contrast text must have with what is behind it to be read: the
/// least the accessibility guidelines ask for text of ordinary size.
pub const READABLE: f32 = 4.5;

impl Theme {
    pub fn named(name: ThemeName) -> Self {
        let mut theme = match name {
            ThemeName::Dark => DARK,
            ThemeName::Mocha => MOCHA,
            ThemeName::Dusk => DUSK,
            ThemeName::TokyoNight => TOKYO_NIGHT,
            ThemeName::Nord => NORD,
            ThemeName::Graphite => GRAPHITE,
            ThemeName::Light => LIGHT,
            ThemeName::Contrast => CONTRAST,
        };
        // Secondary text and hints are text too: each is moved only as far
        // as it takes to be read on every ground it is shown on.
        let grounds = [theme.background, theme.surface, theme.field, theme.raised];
        for colour in [&mut theme.text_dim, &mut theme.text_faint] {
            for ground in grounds {
                *colour = crate::palette::legible(*colour, ground, READABLE);
            }
        }
        theme
    }
}

struct Current {
    theme: Theme,
    /// The sixteen colours of the MUD's text.
    mud: [Color32; 16],
    output_size: f32,
    line_spacing: f32,
}

/// How many times the look has changed: what is worked out from it is
/// worked out again when this does.
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Which look is in use, to know when what was worked out from it is old.
pub fn generation() -> u64 {
    GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

static CURRENT: RwLock<Current> = RwLock::new(Current {
    theme: DARK,
    mud: VIVID,
    output_size: 15.0,
    line_spacing: 1.0,
});

/// The height of a line of the output, if it is not the font's own.
pub fn output_line_height() -> Option<f32> {
    let current = CURRENT.read().expect("theme lock");
    // A monospace line is about 1.32 times the size of its letters.
    (current.line_spacing != 1.0).then(|| current.output_size * 1.32 * current.line_spacing)
}

/// The theme in use.
pub fn current() -> Theme {
    CURRENT.read().expect("theme lock").theme
}

/// The sixteen base colours the MUD's text is drawn in. They are chosen
/// apart from the theme: the interface can be sober without the game
/// being so.
pub fn mud_colours() -> [Color32; 16] {
    CURRENT.read().expect("theme lock").mud
}

/// The font of the output and the input line.
pub fn output_font() -> FontId {
    FontId::monospace(CURRENT.read().expect("theme lock").output_size)
}

/// The weights the interface's typeface comes in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Medium,
    Semibold,
}

/// The interface's typeface at a weight and size.
pub fn font(weight: Weight, size: f32) -> FontId {
    let family = match weight {
        Weight::Regular => FontFamily::Proportional,
        Weight::Medium => FontFamily::Name("medium".into()),
        Weight::Semibold => FontFamily::Name("semibold".into()),
    };
    FontId::new(size, family)
}

/// Makes the interface look as the settings say. Returns an error if the
/// chosen font file cannot be used; the built-in font is kept then.
pub fn apply(ctx: &egui::Context, settings: &Settings) -> Result<(), String> {
    let theme = Theme::named(settings.theme);
    *CURRENT.write().expect("theme lock") = Current {
        theme,
        mud: match settings.mud_palette {
            crate::settings::MudPalette::Vivid => VIVID,
            crate::settings::MudPalette::Soft => theme.terminal,
            crate::settings::MudPalette::Classic => CLASSIC,
        },
        output_size: settings.font_size.clamp(8.0, 40.0),
        line_spacing: settings.line_spacing.clamp(0.8, 2.0),
    };
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ctx.set_zoom_factor(settings.ui_scale.clamp(0.75, 2.0));
    ctx.set_theme(if theme.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.all_styles_mut(|style| {
        surfaces(&mut style.visuals, &theme);
        controls(&mut style.visuals, &theme);
        spacing(&mut style.spacing);
        style.text_styles = text_styles();
    });
    fonts_if_changed(ctx, settings)
}

/// The surfaces, their edges and shadows, what is selected, and links.
fn surfaces(visuals: &mut egui::Visuals, theme: &Theme) {
    visuals.panel_fill = theme.surface;
    visuals.window_fill = theme.surface;
    visuals.extreme_bg_color = theme.field;
    visuals.faint_bg_color = theme.raised;
    visuals.window_stroke = Stroke::new(1.0, theme.border);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(9);
    let shadow = |offset, blur, dark, light| Shadow {
        offset: [0, offset],
        blur,
        spread: 0,
        color: Color32::from_black_alpha(if theme.dark { dark } else { light }),
    };
    visuals.window_shadow = shadow(12, 36, 130, 50);
    visuals.popup_shadow = shadow(6, 18, 110, 40);
    visuals.selection.bg_fill = theme.accent.gamma_multiply(0.3);
    visuals.selection.stroke = Stroke::new(1.0, theme.accent);
    visuals.hyperlink_color = theme.accent;
    visuals.override_text_color = None;
    visuals.weak_text_color = Some(theme.text_faint);
    visuals.button_frame = true;
    visuals.slider_trailing_fill = true;
    visuals.indent_has_left_vline = false;
}

/// What is not interactive has the surface's edge. Buttons, by default,
/// look like what is typed in or chosen: see [`field_look`]; each button
/// takes [`button_look`] for itself.
fn controls(visuals: &mut egui::Visuals, theme: &Theme) {
    let widgets = &mut visuals.widgets;
    let radius = CornerRadius::same(8);
    widgets.noninteractive.bg_fill = theme.surface;
    widgets.noninteractive.weak_bg_fill = theme.surface;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme.border);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.text_dim);
    widgets.noninteractive.corner_radius = radius;
    button_look(widgets, theme);
    for widget in [
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.fg_stroke = Stroke::new(1.0, theme.text);
        widget.corner_radius = radius;
        widget.expansion = 0.0;
    }
    field_look(visuals, theme.field, theme);
}

/// A button stands out from what it is on by its fill, which lightens
/// under the pointer and as it is pressed.
pub fn button_look(widgets: &mut egui::style::Widgets, theme: &Theme) {
    for (widget, fill) in [
        (&mut widgets.inactive, mix(theme.raised, theme.text, 0.035)),
        (&mut widgets.hovered, mix(theme.raised, theme.text, 0.1)),
        (&mut widgets.active, mix(theme.raised, theme.text, 0.16)),
        (&mut widgets.open, theme.hover),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, mix(fill, theme.text, 0.06));
    }
    widgets.active.bg_stroke = Stroke::new(1.0, theme.accent);
}

/// Whatever is typed in or chosen from looks alike: a text field, a list
/// to choose from, a number. It has `fill`, a hairline round it that firms
/// up under the pointer, and the accent while it is in use.
pub fn field_look(visuals: &mut egui::Visuals, fill: Color32, theme: &Theme) {
    visuals.text_edit_bg_color = Some(fill);
    let edge = mix(theme.border, theme.text, 0.1);
    let widgets = &mut visuals.widgets;
    for (widget, stroke) in [
        (&mut widgets.inactive, edge),
        (&mut widgets.hovered, mix(edge, theme.text, 0.25)),
        (&mut widgets.active, theme.accent),
        (&mut widgets.open, theme.accent),
    ] {
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, stroke);
    }
}

/// The room between and around things.
fn spacing(spacing: &mut egui::style::Spacing) {
    spacing.item_spacing = Vec2::new(8.0, 8.0);
    spacing.button_padding = Vec2::new(13.0, 6.0);
    spacing.interact_size.y = 30.0;
    spacing.menu_margin = Margin::same(6);
    spacing.window_margin = Margin::same(16);
    spacing.combo_width = 120.0;
    spacing.icon_width = 16.0;
    spacing.scroll.bar_width = 8.0;
    spacing.scroll.floating = true;
}

/// The type of each kind of text.
fn text_styles() -> std::collections::BTreeMap<TextStyle, FontId> {
    [
        (TextStyle::Heading, font(Weight::Semibold, 20.0)),
        (TextStyle::Body, font(Weight::Regular, 13.5)),
        (TextStyle::Button, font(Weight::Medium, 13.0)),
        (TextStyle::Small, font(Weight::Regular, 11.5)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into()
}

/// Puts the fonts in place, which is costly, only when they changed.
fn fonts_if_changed(ctx: &egui::Context, settings: &Settings) -> Result<(), String> {
    let chosen = (
        settings.font_family.clone(),
        settings.font_file.clone(),
        settings.font_index,
    );
    let marker = egui::Id::unique("muddie fonts in place");
    if ctx
        .data(|data| data.get_temp::<(String, String, u32)>(marker))
        .as_ref()
        == Some(&chosen)
    {
        return Ok(());
    }
    ctx.data_mut(|data| data.insert_temp(marker, chosen));
    install_fonts(ctx, settings)
}

/// Blends `amount` of `other` into `base`, giving an opaque colour.
pub fn mix(base: Color32, other: Color32, amount: f32) -> Color32 {
    let channel =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    Color32::from_rgb(
        channel(base.r(), other.r()),
        channel(base.g(), other.g()),
        channel(base.b(), other.b()),
    )
}

#[cfg(test)]
mod readable_tests {
    use super::*;
    use crate::palette::contrast;

    #[test]
    fn every_text_can_be_read_on_every_ground_of_every_theme() {
        let names = [
            ThemeName::Dark,
            ThemeName::Mocha,
            ThemeName::Dusk,
            ThemeName::TokyoNight,
            ThemeName::Nord,
            ThemeName::Graphite,
            ThemeName::Light,
            ThemeName::Contrast,
        ];
        for name in names {
            let theme = Theme::named(name);
            let grounds = [theme.background, theme.surface, theme.field, theme.raised];
            for (what, colour) in [
                ("text", theme.text),
                ("text_dim", theme.text_dim),
                ("text_faint", theme.text_faint),
            ] {
                for ground in grounds {
                    let measured = contrast(colour, ground);
                    assert!(
                        measured >= READABLE - 0.01,
                        "{name:?} {what}: {measured:.2} on {ground:?}"
                    );
                }
            }
            // The accent marks what is on or chosen: it stands out from
            // the grounds, and what is written on it can be read.
            for ground in grounds {
                let measured = contrast(theme.accent, ground);
                assert!(
                    measured >= 3.0,
                    "{name:?} accent: {measured:.2} on {ground:?}"
                );
            }
            let on = contrast(theme.on_accent, theme.accent);
            assert!(on >= READABLE, "{name:?} on_accent: {on:.2}");
            // And the hints are still fainter than the secondary text.
            assert!(
                contrast(theme.text_faint, theme.surface)
                    <= contrast(theme.text_dim, theme.surface)
            );
        }
    }
}
