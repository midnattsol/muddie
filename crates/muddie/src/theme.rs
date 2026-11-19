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

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The default: sober. Neutral charcoal with no tint, and no accent colour
/// either: what is selected or primary is marked in near-white. The only
/// colour on screen is the MUD's own.
const DARK: Theme = Theme {
    dark: true,
    background: rgb(0x141415),
    surface: rgb(0x1f1f21),
    field: rgb(0x19191b),
    raised: rgb(0x2d2d30),
    hover: rgb(0x3a3a3e),
    border: rgb(0x2f2f32),
    text: rgb(0xe8e8e8),
    text_dim: rgb(0xa1a1a4),
    text_faint: rgb(0x6c6c70),
    accent: rgb(0xe4e4e6),
    on_accent: rgb(0x131314),
    good: rgb(0x7cc68d),
    danger: rgb(0xe5737a),
    terminal: [
        rgb(0x3a3a3d),
        rgb(0xe07a7f),
        rgb(0x8cc98a),
        rgb(0xd9bd7c),
        rgb(0x7fa8d6),
        rgb(0xb596cf),
        rgb(0x6fbfc4),
        rgb(0xc8c8cb),
        rgb(0x6c6c70),
        rgb(0xf0949a),
        rgb(0xa6dca3),
        rgb(0xecd49a),
        rgb(0x9cc0ea),
        rgb(0xcbb0e2),
        rgb(0x8dd6da),
        rgb(0xffffff),
    ],
};

/// Mocha, after the Catppuccin palette: soft, warm darks and pastel
/// colours, with mauve as the accent.
const MOCHA: Theme = Theme {
    dark: true,
    background: rgb(0x11111b),
    surface: rgb(0x1e1e2e),
    field: rgb(0x181825),
    raised: rgb(0x313244),
    hover: rgb(0x45475a),
    border: rgb(0x2b2c3f),
    text: rgb(0xcdd6f4),
    text_dim: rgb(0xa6adc8),
    text_faint: rgb(0x6c7086),
    accent: rgb(0xcba6f7),
    on_accent: rgb(0x1e1e2e),
    good: rgb(0xa6e3a1),
    danger: rgb(0xf38ba8),
    terminal: [
        rgb(0x45475a),
        rgb(0xf38ba8),
        rgb(0xa6e3a1),
        rgb(0xf9e2af),
        rgb(0x89b4fa),
        rgb(0xf5c2e7),
        rgb(0x94e2d5),
        rgb(0xbac2de),
        rgb(0x585b70),
        rgb(0xf5a2ba),
        rgb(0xb9ecb5),
        rgb(0xfbebc6),
        rgb(0xa5c6fb),
        rgb(0xf8d4ee),
        rgb(0xaeeadf),
        rgb(0xffffff),
    ],
};

/// Dusk: the blue-violet of a sky after sunset, with the amber of the
/// last light as the one accent. The chrome is the lighter tone, and what
/// is read or typed sits in darker wells.
const DUSK: Theme = Theme {
    dark: true,
    background: rgb(0x11121b),
    surface: rgb(0x222436),
    field: rgb(0x1a1c2a),
    raised: rgb(0x2c2f45),
    hover: rgb(0x383c57),
    border: rgb(0x33364e),
    text: rgb(0xe9eaf4),
    text_dim: rgb(0xa9acc6),
    text_faint: rgb(0x71759a),
    accent: rgb(0xf6ad63),
    on_accent: rgb(0x2a1a08),
    good: rgb(0x7ddba0),
    danger: rgb(0xff7d8a),
    terminal: [
        rgb(0x2c2f45),
        rgb(0xf0767e),
        rgb(0x8fd68a),
        rgb(0xebc983),
        rgb(0x6fb1f5),
        rgb(0xc792ea),
        rgb(0x63c7d1),
        rgb(0xcfd2d9),
        rgb(0x6d717b),
        rgb(0xff8b94),
        rgb(0xaeeb9c),
        rgb(0xffdd93),
        rgb(0x8fc8ff),
        rgb(0xe0a8fa),
        rgb(0x8ae3ec),
        rgb(0xffffff),
    ],
};

/// After the Tokyo Night palette: deep blues with a blue accent.
const TOKYO_NIGHT: Theme = Theme {
    dark: true,
    background: rgb(0x16161e),
    surface: rgb(0x1f2335),
    field: rgb(0x1a1b26),
    raised: rgb(0x292e42),
    hover: rgb(0x343a55),
    border: rgb(0x2a2f45),
    text: rgb(0xc0caf5),
    text_dim: rgb(0xa9b1d6),
    text_faint: rgb(0x565f89),
    accent: rgb(0x7aa2f7),
    on_accent: rgb(0x16161e),
    good: rgb(0x9ece6a),
    danger: rgb(0xf7768e),
    terminal: [
        rgb(0x414868),
        rgb(0xf7768e),
        rgb(0x9ece6a),
        rgb(0xe0af68),
        rgb(0x7aa2f7),
        rgb(0xbb9af7),
        rgb(0x7dcfff),
        rgb(0xa9b1d6),
        rgb(0x565f89),
        rgb(0xff899d),
        rgb(0xb9f27c),
        rgb(0xfaba4a),
        rgb(0x8db0ff),
        rgb(0xc7a9ff),
        rgb(0xa4daff),
        rgb(0xffffff),
    ],
};

/// After the Nord palette: cool blue-greys with a frost accent.
const NORD: Theme = Theme {
    dark: true,
    background: rgb(0x242933),
    surface: rgb(0x2e3440),
    field: rgb(0x2a303b),
    raised: rgb(0x3b4252),
    hover: rgb(0x434c5e),
    border: rgb(0x394050),
    text: rgb(0xeceff4),
    text_dim: rgb(0xc2c9d6),
    text_faint: rgb(0x7b88a1),
    accent: rgb(0x88c0d0),
    on_accent: rgb(0x2e3440),
    good: rgb(0xa3be8c),
    danger: rgb(0xbf616a),
    terminal: [
        rgb(0x4c566a),
        rgb(0xbf616a),
        rgb(0xa3be8c),
        rgb(0xebcb8b),
        rgb(0x81a1c1),
        rgb(0xb48ead),
        rgb(0x88c0d0),
        rgb(0xe5e9f0),
        rgb(0x616e88),
        rgb(0xd0737d),
        rgb(0xb4cf9c),
        rgb(0xf0d8a4),
        rgb(0x93b4d4),
        rgb(0xc6a2bf),
        rgb(0x8fbcbb),
        rgb(0xffffff),
    ],
};

/// Neutral greys with a blue accent, for those who want no tint.
const GRAPHITE: Theme = Theme {
    dark: true,
    background: rgb(0x161618),
    surface: rgb(0x232326),
    field: rgb(0x1c1c1f),
    raised: rgb(0x2e2e33),
    hover: rgb(0x3a3a40),
    border: rgb(0x323237),
    text: rgb(0xededf0),
    text_dim: rgb(0xa5a5ad),
    text_faint: rgb(0x6e6e77),
    accent: rgb(0x5b9dff),
    on_accent: rgb(0xffffff),
    good: rgb(0x3ecf8e),
    danger: rgb(0xff6369),
    terminal: [
        rgb(0x3a3a40),
        rgb(0xf0767e),
        rgb(0x8fd68a),
        rgb(0xebc983),
        rgb(0x6fb1f5),
        rgb(0xc792ea),
        rgb(0x63c7d1),
        rgb(0xcfd2d9),
        rgb(0x6d717b),
        rgb(0xff8b94),
        rgb(0xaeeb9c),
        rgb(0xffdd93),
        rgb(0x8fc8ff),
        rgb(0xe0a8fa),
        rgb(0x8ae3ec),
        rgb(0xffffff),
    ],
};

const LIGHT: Theme = Theme {
    dark: false,
    background: rgb(0xffffff),
    surface: rgb(0xf5f5f7),
    field: rgb(0xffffff),
    raised: rgb(0xe9e9ed),
    hover: rgb(0xdddde3),
    border: rgb(0xdcdce1),
    text: rgb(0x1d1d1f),
    text_dim: rgb(0x62636b),
    text_faint: rgb(0x9a9ba3),
    accent: rgb(0x0a6cff),
    on_accent: rgb(0xffffff),
    good: rgb(0x1f9d63),
    danger: rgb(0xd92d36),
    terminal: [
        rgb(0x1d1d1f),
        rgb(0xc4323f),
        rgb(0x2f8a3b),
        rgb(0x946500),
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
    field: rgb(0x000000),
    raised: rgb(0x1c1c1c),
    hover: rgb(0x333333),
    border: rgb(0xffffff),
    text: rgb(0xffffff),
    text_dim: rgb(0xe6e6e6),
    text_faint: rgb(0xcccccc),
    accent: rgb(0xffe600),
    on_accent: rgb(0x000000),
    good: rgb(0x55ff55),
    danger: rgb(0xff5555),
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
            ThemeName::Mocha => MOCHA,
            ThemeName::Dusk => DUSK,
            ThemeName::TokyoNight => TOKYO_NIGHT,
            ThemeName::Nord => NORD,
            ThemeName::Graphite => GRAPHITE,
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
        visuals.extreme_bg_color = theme.field;
        visuals.text_edit_bg_color = Some(theme.field);
        visuals.faint_bg_color = theme.raised;
        visuals.window_stroke = Stroke::new(1.0, theme.border);
        visuals.window_corner_radius = CornerRadius::same(12);
        visuals.menu_corner_radius = CornerRadius::same(9);
        visuals.window_shadow = Shadow {
            offset: [0, 12],
            blur: 36,
            spread: 0,
            color: Color32::from_black_alpha(if theme.dark { 130 } else { 50 }),
        };
        visuals.popup_shadow = Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(if theme.dark { 110 } else { 40 }),
        };
        visuals.selection.bg_fill = theme.accent.gamma_multiply(0.3);
        visuals.selection.stroke = Stroke::new(1.0, theme.accent);
        visuals.hyperlink_color = theme.accent;
        visuals.override_text_color = None;
        visuals.weak_text_color = Some(theme.text_faint);
        visuals.button_frame = true;
        visuals.slider_trailing_fill = true;
        visuals.indent_has_left_vline = false;

        // Controls have no outline of their own: they are told apart from
        // the surface by their fill, and from each other by spacing.
        let radius = CornerRadius::same(8);
        let widgets = &mut visuals.widgets;
        widgets.noninteractive.bg_fill = theme.surface;
        widgets.noninteractive.weak_bg_fill = theme.surface;
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme.border);
        widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.text_dim);
        widgets.noninteractive.corner_radius = radius;
        for (widget, fill) in [
            (&mut widgets.inactive, mix(theme.raised, theme.text, 0.035)),
            (&mut widgets.hovered, mix(theme.raised, theme.text, 0.1)),
            (&mut widgets.active, mix(theme.raised, theme.text, 0.16)),
            (&mut widgets.open, theme.hover),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.bg_stroke = Stroke::new(1.0, mix(fill, theme.text, 0.06));
            widget.fg_stroke = Stroke::new(1.0, theme.text);
            widget.corner_radius = radius;
            widget.expansion = 0.0;
        }
        widgets.active.bg_stroke = Stroke::new(1.0, theme.accent);

        let spacing = &mut style.spacing;
        spacing.item_spacing = Vec2::new(8.0, 8.0);
        spacing.button_padding = Vec2::new(13.0, 6.0);
        spacing.interact_size.y = 30.0;
        spacing.menu_margin = Margin::same(6);
        spacing.window_margin = Margin::same(16);
        spacing.combo_width = 120.0;
        spacing.icon_width = 16.0;
        spacing.scroll.bar_width = 8.0;
        spacing.scroll.floating = true;

        style.text_styles = [
            (TextStyle::Heading, font(Weight::Semibold, 20.0)),
            (TextStyle::Body, font(Weight::Regular, 13.5)),
            (TextStyle::Button, font(Weight::Medium, 13.0)),
            (TextStyle::Small, font(Weight::Regular, 11.5)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
    });
    install_fonts(ctx, &settings.font_file)
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

/// Installs the built-in typefaces, and the user's output font if `file`
/// names one.
fn install_fonts(ctx: &egui::Context, file: &str) -> Result<(), String> {
    let mut fonts = FontDefinitions::default();
    let built_in: [(&str, &'static [u8]); 4] = [
        ("inter", include_bytes!("../assets/fonts/Inter-Regular.ttf")),
        (
            "inter-medium",
            include_bytes!("../assets/fonts/Inter-Medium.ttf"),
        ),
        (
            "inter-semibold",
            include_bytes!("../assets/fonts/Inter-SemiBold.ttf"),
        ),
        (
            "jetbrains-mono",
            include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
        ),
    ];
    for (name, bytes) in built_in {
        (fonts.font_data).insert(name.to_owned(), FontData::from_static(bytes).into());
    }
    // egui's own fonts stay behind ours, for symbols ours lack.
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (family, first) in [
        (FontFamily::Proportional, "inter"),
        (FontFamily::Name("medium".into()), "inter-medium"),
        (FontFamily::Name("semibold".into()), "inter-semibold"),
    ] {
        let mut names = vec![first.to_owned()];
        names.extend(fallbacks.iter().cloned());
        fonts.families.insert(family, names);
    }
    let monospace = fonts.families.entry(FontFamily::Monospace).or_default();
    monospace.insert(0, "jetbrains-mono".to_owned());
    // The symbol and emoji fonts too: MUDs send them, and code can hold them.
    for name in &fallbacks {
        if !monospace.contains(name) {
            monospace.push(name.clone());
        }
    }

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
