//! The fonts: those that come with Muddie, the system's monospaced ones,
//! and putting the one chosen in place.

use super::*;

/// The fonts for the output that come with the program besides the
/// default one, by name. The settings name one as `muddie:<name>`, so
/// that it is never taken for a font of the system called the same.
pub const BUNDLED_FONTS: [(&str, &[u8]); 1] = [(
    "JetBrains Mono",
    include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf"),
)];

/// The name of the font the output is in unless another is chosen.
pub const DEFAULT_FONT: &str = "MesloLGS NF";

/// What the settings call a bundled font.
pub fn bundled_font(name: &str) -> String {
    format!("muddie:{name}")
}

/// An installed font that suits the output: its family, where its
/// regular face is, and which face of that file it is.
pub struct InstalledFont {
    pub family: String,
    file: std::path::PathBuf,
    index: u32,
}

impl InstalledFont {
    /// Where its regular face is: the file, and which face of it.
    pub fn place(&self) -> (String, u32) {
        (self.file.display().to_string(), self.index)
    }
}

/// The monospaced fonts installed on the system, by family, or nothing
/// while they are still being looked for. Going through every font of the
/// system takes a moment, so it is done once, apart, the first time this
/// is asked.
pub fn installed_fonts() -> Option<&'static [InstalledFont]> {
    static FONTS: std::sync::OnceLock<Vec<InstalledFont>> = std::sync::OnceLock::new();
    static LOOKING: std::sync::Once = std::sync::Once::new();
    LOOKING.call_once(|| {
        std::thread::spawn(|| {
            let mut database = fontdb::Database::new();
            database.load_system_fonts();
            let mut fonts: Vec<InstalledFont> = Vec::new();
            for face in database.faces() {
                let upright =
                    face.style == fontdb::Style::Normal && face.weight == fontdb::Weight::NORMAL;
                let file = match &face.source {
                    fontdb::Source::File(file) | fontdb::Source::SharedFile(file, _) => {
                        file.clone()
                    }
                    fontdb::Source::Binary(_) => continue,
                };
                let Some((family, _)) = face.families.first() else {
                    continue;
                };
                if face.monospaced && upright && !fonts.iter().any(|font| font.family == *family) {
                    fonts.push(InstalledFont {
                        family: family.clone(),
                        file,
                        index: face.index,
                    });
                }
            }
            fonts.sort_by_key(|font| font.family.to_lowercase());
            let _ = FONTS.set(fonts);
        });
    });
    FONTS.get().map(Vec::as_slice)
}

/// Installs the built-in typefaces, and the output font the settings ask
/// for, if they ask for one.
pub(super) fn install_fonts(ctx: &egui::Context, settings: &Settings) -> Result<(), String> {
    let mut fonts = built_in_fonts();
    let result = add_output_font(&mut fonts, settings);
    finish(ctx, fonts, result)
}

/// The definitions with the built-in typefaces in their families.
fn built_in_fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let built_in: [(&str, &'static [u8]); 4] = [
        (
            "inter",
            include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
        ),
        (
            "inter-medium",
            include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
        ),
        (
            "inter-semibold",
            include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
        ),
        (
            "meslo",
            include_bytes!("../../assets/fonts/MesloLGS-NF-Regular.ttf"),
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
    // The pictures of the interface come from its icons.
    (fonts.families).insert(FontFamily::Name("icons".into()), vec!["meslo".to_owned()]);
    let monospace = fonts.families.entry(FontFamily::Monospace).or_default();
    monospace.insert(0, "meslo".to_owned());
    // The symbol and emoji fonts too: MUDs send them, and code can hold them.
    for name in &fallbacks {
        if !monospace.contains(name) {
            monospace.push(name.clone());
        }
    }
    fonts
}

/// Puts the output font the settings ask for first among the monospaced
/// ones. It gives the error when a font file cannot be read.
fn add_output_font(fonts: &mut FontDefinitions, settings: &Settings) -> Result<(), String> {
    // One that comes with the program is simply put first.
    let bundled = settings.font_family.strip_prefix("muddie:");
    if let Some((_, bytes)) =
        bundled.and_then(|name| BUNDLED_FONTS.iter().find(|(known, _)| *known == name))
    {
        fonts
            .font_data
            .insert("output".to_owned(), FontData::from_static(bytes).into());
        let monospace = fonts.families.entry(FontFamily::Monospace).or_default();
        monospace.insert(0, "output".to_owned());
        return Ok(());
    }
    // One of ours that is no longer there: the default serves.
    if bundled.is_some() {
        return Ok(());
    }
    // A file: the one an installed font was found in when it was chosen,
    // or one given by hand.
    let chosen =
        (!settings.font_file.is_empty()).then(|| (settings.font_file.clone(), settings.font_index));
    let mut result = Ok(());
    if let Some((file, index)) = chosen {
        match std::fs::read(&file) {
            Ok(bytes) => {
                let mut data = FontData::from_owned(bytes);
                data.index = index;
                fonts.font_data.insert("output".to_owned(), data.into());
                let monospace = fonts.families.entry(FontFamily::Monospace).or_default();
                monospace.insert(0, "output".to_owned());
            }
            Err(error) => result = Err(format!("{file}: {error}")),
        }
    }
    result
}

pub(super) fn finish(
    ctx: &egui::Context,
    fonts: FontDefinitions,
    result: Result<(), String>,
) -> Result<(), String> {
    ctx.set_fonts(fonts);
    result
}
