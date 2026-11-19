//! The look of the interface: colours, spacing and type for each theme.

use std::sync::RwLock;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Vec2,
};

use crate::settings::{Settings, ThemeName};

/// The colours of one theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub dark: bool,
    /// Behind the output and the map.
    pub background: Color32,
    /// Bars and side panels.
    pub surface: Color32,
    /// Inputs, buttons and anything raised from a surface.
    pub raised: Color32,
    pub border: Color32,
    pub text: Color32,
    /// Secondary text: labels, hints, lines the client adds itself.
    pub text_dim: Color32,
    /// Selection, focus and the player's place on the map.
    pub accent: Color32,
    /// The 16 base terminal colours, tuned to read well on `background`.
    pub terminal: [Color32; 16],
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const DARK: Theme = Theme {
    dark: true,
    background: rgb(0x0f1116),
    surface: rgb(0x151821),
    raised: rgb(0x1e2230),
    border: rgb(0x272c3b),
    text: rgb(0xe2e5ee),
    text_dim: rgb(0x8a92a8),
    accent: rgb(0x8aa9ff),
    terminal: [
        rgb(0x2b2f3a),
        rgb(0xe8707a),
        rgb(0x9ccf7e),
        rgb(0xe9c582),
        rgb(0x6cb2f2),
        rgb(0xc98be6),
        rgb(0x5fc1cc),
        rgb(0xcbd0da),
        rgb(0x6c7385),
        rgb(0xff8791),
        rgb(0xb8ec95),
        rgb(0xffdc8f),
        rgb(0x8ccaff),
        rgb(0xe3a5f8),
        rgb(0x88e0ea),
        rgb(0xffffff),
    ],
};

const LIGHT: Theme = Theme {
    dark: false,
    background: rgb(0xfcfcfe),
    surface: rgb(0xf2f3f7),
    raised: rgb(0xffffff),
    border: rgb(0xd8dbe5),
    text: rgb(0x1c2030),
    text_dim: rgb(0x687089),
    accent: rgb(0x3d5bd9),
    terminal: [
        rgb(0x1c2030),
        rgb(0xc4323f),
        rgb(0x2f8a3b),
        rgb(0x9a6b00),
        rgb(0x1f66c9),
        rgb(0x9340b8),
        rgb(0x12808e),
        rgb(0x596079),
        rgb(0x7c8399),
        rgb(0xe0404e),
        rgb(0x3ba348),
        rgb(0xb88300),
        rgb(0x2f7ee6),
        rgb(0xad55d6),
        rgb(0x1a9aab),
        rgb(0x2b3046),
    ],
};

const CONTRAST: Theme = Theme {
    dark: true,
    background: rgb(0x000000),
    surface: rgb(0x000000),
    raised: rgb(0x1a1a1a),
    border: rgb(0xffffff),
    text: rgb(0xffffff),
    text_dim: rgb(0xe0e0e0),
    accent: rgb(0xffe600),
    terminal: [
        rgb(0x000000),
        rgb(0xff5555),
        rgb(0x55ff55),
        rgb(0xffff55),
        rgb(0x7aa7ff),
        rgb(0xff77ff),
        rgb(0x55ffff),
        rgb(0xffffff),
        rgb(0xaaaaaa),
        rgb(0xff8888),
        rgb(0x88ff88),
        rgb(0xffff88),
        rgb(0xa5c3ff),
        rgb(0xffaaff),
        rgb(0x88ffff),
        rgb(0xffffff),
    ],
};

impl Theme {
    pub fn named(name: ThemeName) -> Self {
        match name {
            ThemeName::Dark => DARK,
            ThemeName::Light => LIGHT,
            ThemeName::Contrast => CONTRAST,
        }
    }
}

struct Current {
    theme: Theme,
    output_size: f32,
}

static CURRENT: RwLock<Current> = RwLock::new(Current {
    theme: DARK,
    output_size: 15.0,
});

/// The theme in use.
pub fn current() -> Theme {
    CURRENT.read().expect("theme lock").theme
}

/// The font of the output and the input line.
pub fn output_font() -> FontId {
    FontId::monospace(CURRENT.read().expect("theme lock").output_size)
}

/// Makes the interface look as the settings say. Returns an error if the
/// chosen font file cannot be used; the built-in font is kept then.
pub fn apply(ctx: &egui::Context, settings: &Settings) -> Result<(), String> {
    let theme = Theme::named(settings.theme);
    *CURRENT.write().expect("theme lock") = Current {
        theme,
        output_size: settings.font_size.clamp(8.0, 40.0),
    };
    ctx.set_theme(if theme.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.all_styles_mut(|style| {
        let visuals = &mut style.visuals;
        visuals.panel_fill = theme.surface;
        visuals.window_fill = theme.surface;
        visuals.extreme_bg_color = theme.background;
        visuals.faint_bg_color = theme.raised;
        visuals.window_stroke = Stroke::new(1.0, theme.border);
        visuals.window_corner_radius = CornerRadius::same(10);
        visuals.menu_corner_radius = CornerRadius::same(8);
        visuals.selection.bg_fill = theme.accent.gamma_multiply(0.35);
        visuals.selection.stroke = Stroke::new(1.0, theme.accent);
        visuals.hyperlink_color = theme.accent;
        visuals.override_text_color = None;

        let radius = CornerRadius::same(6);
        let widgets = &mut visuals.widgets;
        widgets.noninteractive.bg_fill = theme.surface;
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme.border);
        widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.text_dim);
        widgets.noninteractive.corner_radius = radius;
        for (widget, lift) in [
            (&mut widgets.inactive, 0.0),
            (&mut widgets.hovered, 0.08),
            (&mut widgets.active, 0.16),
            (&mut widgets.open, 0.08),
        ] {
            let fill = mix(theme.raised, theme.text, lift);
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.bg_stroke = Stroke::new(1.0, mix(theme.border, theme.text, lift));
            widget.fg_stroke = Stroke::new(1.0, theme.text);
            widget.corner_radius = radius;
            widget.expansion = 0.0;
        }
        widgets.hovered.bg_stroke = Stroke::new(1.0, theme.accent.gamma_multiply(0.7));
        widgets.active.bg_stroke = Stroke::new(1.0, theme.accent);

        let spacing = &mut style.spacing;
        spacing.item_spacing = Vec2::new(8.0, 6.0);
        spacing.button_padding = Vec2::new(10.0, 4.0);
        spacing.interact_size.y = 26.0;
        spacing.menu_margin = egui::Margin::same(6);

        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(19.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Monospace, FontId::monospace(13.5)),
        ]
        .into();
    });
    set_output_font(ctx, &settings.font_file)
}

/// Blends `amount` of `other` into `base`.
fn mix(base: Color32, other: Color32, amount: f32) -> Color32 {
    let channel =
        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * amount).round() as u8;
    Color32::from_rgb(
        channel(base.r(), other.r()),
        channel(base.g(), other.g()),
        channel(base.b(), other.b()),
    )
}

/// Uses the font in `file` for monospace text, or the built-in one if
/// `file` is empty.
fn set_output_font(ctx: &egui::Context, file: &str) -> Result<(), String> {
    let mut fonts = FontDefinitions::default();
    let mut result = Ok(());
    if !file.is_empty() {
        match std::fs::read(file) {
            Ok(bytes) => {
                (fonts.font_data).insert("output".to_owned(), FontData::from_owned(bytes).into());
                let monospace = fonts.families.entry(FontFamily::Monospace).or_default();
                monospace.insert(0, "output".to_owned());
            }
            Err(error) => result = Err(format!("{file}: {error}")),
        }
    }
    ctx.set_fonts(fonts);
    result
}
