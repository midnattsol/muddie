//! What tells Muddie apart on the desktop: its name there and its icon.

/// The name the desktop knows Muddie by: its window, its entry in the
/// menu of applications, and its icon are all called this.
pub const APP_ID: &str = "io.github.midnattsol.Muddie";

/// Muddie's icon, as the desktop shows it; also its mark in the window.
pub const ICON_PNG: &[u8] =
    include_bytes!("../../../assets/icons/hicolor/256x256/apps/io.github.midnattsol.Muddie.png");
