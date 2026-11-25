//! Images of every screen, for looking at by eye.

use super::*;

mod map;
mod panel_views;
mod rules_window;

use map::{shows_map, shows_map_menu};
use panel_views::{shows_layout_view, shows_panels};
use rules_window::shows_editor;

/// Renders every screen of the demo to a PNG beside the one named by
/// `MUDDIE_SCREENSHOT`, to look at the result. Needs a GPU or software
/// Vulkan, so it is not part of the normal test run:
/// `MUDDIE_SCREENSHOT=/tmp/muddie.png cargo test -p muddie -- --ignored`
#[test]
#[ignore = "needs a graphics adapter; writes an image for visual review"]
fn screenshot() {
    shows_welcome();
    shows_plugin_offer();
    shows_channels();
    shows_script();
    shows_settings_and_fonts();
    shows_every_settings_page();
    shows_plugins();
    shows_editor();
    shows_menu();
    shows_unmapped();
    shows_panels();
    shows_layout_view();
    shows_palette();
    shows_empty_line();
    shows_find_and_themes();
    shows_map();
    shows_map_menu();
}

/// Writes what the window shows to the image of this name, if asked to.
fn save(harness: &mut Harness<'_, App>, name: &str) {
    let image = harness.render().expect("rendering failed");
    if let Some(path) = std::env::var_os("MUDDIE_SCREENSHOT").map(PathBuf::from) {
        let path = path.with_extension(format!("{name}.png"));
        image.save(path).expect("could not write the screenshot");
    }
}

/// What a new session opens on.
fn shows_welcome() {
    let dir = std::env::temp_dir().join(format!("muddie-shot-welcome-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    for (name, host, port) in [
        ("Reinos de Leyenda", "rlmud.org", 23),
        ("Aardwolf", "aardmud.org", 4000),
        ("Discworld", "discworld.starturtle.net", 4242),
    ] {
        let profile = Profile {
            host: host.into(),
            port,
            ..Profile::default()
        };
        profiles.save(name, &profile).unwrap();
    }
    let mut welcome = window(Some(profiles));
    welcome.state_mut().tab_mut().started = false;
    welcome.run_steps(3);
    save(&mut welcome, "welcome");
    // The layout of a session not yet anyone's.
    welcome.state_mut().tab_mut().set_view(View::Layout);
    welcome.run_steps(3);
    save(&mut welcome, "welcome-layout");
    welcome.state_mut().tab_mut().set_view(View::Game);
    welcome.run_steps(2);
    welcome.state_mut().open_profiles(Some("Discworld"));
    welcome.run_steps(4);
    save(&mut welcome, "profiles");
    // One in use, under the pointer.
    welcome.state_mut().tab_mut().profile = Some("Aardwolf".into());
    welcome.run_steps(2);
    welcome
        .get_by_label("Aardwolf, aardmud.org:4000, In use")
        .hover();
    welcome.run_steps(3);
    save(&mut welcome, "profiles-hover");
    welcome.state_mut().edit_profile("Aardwolf");
    welcome.run_steps(4);
    save(&mut welcome, "profile");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A plugin for the MUD connected to, offered over the command line.
fn shows_plugin_offer() {
    let mut offer = connected_drawn_demo_window();
    let app = offer.state_mut().tab_mut();
    app.offer_plugin_for("rlmud.org", "");
    offer.run_steps(3);
    save(&mut offer, "plugin-offer");
}

/// The channels of the MUD, each with its tab in one panel.
fn shows_channels() {
    let mut channels = connected_drawn_demo_window();
    let app = channels.state_mut().tab_mut();
    app.scripts.exec(
            "t",
            "channels.say(\"chat\", \"[Chat] Naylhena: \\27[1mre\\27[0m\") channels.open(\"mercenario\") channels.say(\"bando\", \"[Bando] Eva: al norte\") channels.say(\"chat\", \"[Chat] Luis: buenas\")",
        );
    app.run_actions();
    channels.run_steps(3);
    channels.get_by_label("chat").click();
    channels.run_steps(3);
    let app = channels.state_mut().tab_mut();
    app.scripts
        .exec("t", "channels.say(\"bando\", \"[Bando] Eva: esperad\")");
    app.run_actions();
    channels.run_steps(3);
    save(&mut channels, "channels");
}

/// A script, run from its button, and one that fails.
fn shows_script() {
    let dir = std::env::temp_dir().join(format!("muddie-shot-script-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Reinos", &crate::profile::Profile::default())
        .unwrap();
    let mut script = window(Some(profiles));
    script.set_size(egui::vec2(1500.0, 900.0));
    let app = script.state_mut().tab_mut();
    app.select_profile(Some("Reinos".into()));
    app.open_automation(automation::SCRIPTS);
    app.add_script(None);
    script.run_steps(3);
    script.state_mut().tab_mut().editor.script_text = "gmcp.support(\"Comm 1\")\n\non(\"gmcp\", \"Comm.MensajeCanal\", function(data)\n\tpanels.add(data.canal, data.mensaje)\nend)\n".into();
    script.run_steps(2);
    script.get_by_label("Run").click();
    script.run_steps(4);
    save(&mut script, "script");
    script.state_mut().tab_mut().editor.script_text =
        "gmcp.support(\"Comm 1\")\nerror(\"boom\")\n".into();
    script.run_steps(2);
    script.get_by_label("Run").click();
    script.run_steps(4);
    save(&mut script, "script-error");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Every page of the settings, and a profile's from top to bottom, in
/// English and in Spanish, whose words are longer.
fn shows_every_settings_page() {
    use crate::app::Section;
    let dir = std::env::temp_dir().join(format!("muddie-shot-pages-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    let profile = Profile {
        host: "rlmud.org".into(),
        port: 23,
        ..Profile::default()
    };
    profiles.save("Reinos de Leyenda", &profile).unwrap();
    for language in ["en", "es"] {
        let mut window = window(Some(profiles.clone()));
        let ctx = window.ctx.clone();
        window.state_mut().use_language(&ctx, language);
        for section in Section::ALL {
            window.state_mut().open_settings(section);
            window.run_steps(4);
            save(
                &mut window,
                &format!("page-{language}-{}", section.name().to_lowercase()),
            );
        }
        window.state_mut().edit_profile("Reinos de Leyenda");
        let tabs = crate::app::ProfileTab::ALL;
        for (number, tab) in tabs.into_iter().enumerate() {
            window.state_mut().show_profile_tab(tab);
            window.run_steps(4);
            save(
                &mut window,
                &format!("page-{language}-profile-{}", number + 1),
            );
        }
        // Installing a plugin asks where from.
        let install = crate::i18n::tr("Install from git…");
        window.get_by_label(install).click_accesskit();
        window.run_steps(4);
        save(&mut window, &format!("page-{language}-install"));
    }
    crate::i18n::set_language("en");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The settings, and the same text in each of the fonts that come with the
/// program.
fn shows_settings_and_fonts() {
    let mut settings = drawn_demo_window();
    settings
        .state_mut()
        .open_settings(crate::app::Section::Appearance);
    settings.run_steps(4);
    save(&mut settings, "settings");
    settings
        .state_mut()
        .open_settings(crate::app::Section::General);
    settings.run_steps(4);
    save(&mut settings, "settings-general");
    // The same text in each of the fonts that come with the program.
    for (name, _) in crate::theme::BUNDLED_FONTS {
        let mut fonts = connected_drawn_demo_window();
        let ctx = fonts.ctx.clone();
        fonts
            .state_mut()
            .use_font(&ctx, &crate::theme::bundled_font(name));
        fonts.run_steps(4);
        save(&mut fonts, &format!("font-{}", name.replace(' ', "")));
    }
}

/// The plugins, with one installed and enabled and one not.
fn shows_plugins() {
    let dir = std::env::temp_dir().join(format!("muddie-shot-plugins-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for plugin in ["aardwolf-mapper", "combat-gauges"] {
        std::fs::create_dir_all(dir.join("plugins").join(plugin)).unwrap();
        std::fs::write(dir.join("plugins").join(plugin).join("init.luau"), "").unwrap();
    }
    let profiles = Profiles::at(dir.clone());
    let profile = Profile {
        plugins_on: vec!["aardwolf-mapper".into()],
        ..Profile::default()
    };
    profiles.save("Aardwolf", &profile).unwrap();
    let mut plugins = window(Some(profiles));
    plugins.state_mut().edit_profile("Aardwolf");
    plugins
        .state_mut()
        .show_profile_tab(crate::app::ProfileTab::Plugins);
    plugins.run_steps(4);
    plugins.get_by_label("Update all").scroll_to_me();
    plugins.run_steps(3);
    save(&mut plugins, "plugins");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The menus, as someone using Spanish sees them.
fn shows_menu() {
    {
        let mut menus = window(None);
        menus.state_mut().tab_mut().started = true;
        menus.run_steps(2);
        menus.get_by_label("Menu").click();
        menus.run_steps(4);
        save(&mut menus, "menu");
    }
}

/// A session whose MUD has reported no room: the map says so.
fn shows_unmapped() {
    let mut unmapped = window(None);
    fake_connection(unmapped.state_mut().tab_mut());
    unmapped
        .state_mut()
        .tab_mut()
        .show_local("Bienvenido a Fimbulvetr.");
    unmapped.run_steps(3);
    save(&mut unmapped, "unmapped");
}

/// The palette of commands over the command line, and an answer.
fn shows_palette() {
    let mut palette = connected_drawn_demo_window();
    let app = palette.state_mut().tab_mut();
    app.scripts.exec(
        "t",
        r#"macro("f9", "kick", {tags = {"combate"}}) macro("f8", "hi", {tags = {"social"}})"#,
    );
    app.toggle_palette();
    palette.run_steps(8);
    save(&mut palette, "palette");
    palette.state_mut().tab_mut().palette.query = "map".into();
    palette.run_steps(4);
    save(&mut palette, "palette-typed");
    // What needs a word more asks for it in the field: a place to go to,
    // offered as it is typed.
    palette.state_mut().tab_mut().palette.query = "go to".into();
    palette.run_steps(3);
    palette.key_press(Key::Enter);
    palette.run_steps(3);
    palette.state_mut().tab_mut().palette.query = "to".into();
    palette.run_steps(4);
    save(&mut palette, "palette-asking");
    palette.key_press(Key::Escape);
    palette.run_steps(2);
    // A tag, switched, answers in the palette.
    palette.state_mut().tab_mut().palette.query = "tag combate".into();
    palette.run_steps(3);
    palette.key_press(Key::Enter);
    palette.run_steps(4);
    save(&mut palette, "answer");
    palette.key_press(Key::Escape);
    palette.run_steps(3);
    save(&mut palette, "bar-note");
}

/// The command line with nothing typed in it.
fn shows_empty_line() {
    let mut empty = connected_drawn_demo_window();
    empty.state_mut().tab_mut().input.clear();
    empty.run_steps(3);
    save(&mut empty, "empty");
}

/// The find bar and the view of the latest lines while scrolled back.
fn shows_find_and_themes() {
    let mut find = connected_drawn_demo_window();
    let app = find.state_mut().tab_mut();
    (0..60).for_each(|n| app.show_local(&format!("line {n} of filler to scroll through")));
    app.find.open = true;
    app.find.query = "orco".into();
    find.run_steps(3);
    find.get_by_label("Previous").click();
    find.run_steps(4);
    save(&mut find, "find");
    // The other themes.
    for (theme, name) in [
        (crate::settings::ThemeName::Light, "light"),
        (crate::settings::ThemeName::Contrast, "contrast"),
    ] {
        let mut themed = connected_drawn_demo_window();
        let ctx = themed.ctx.clone();
        themed.state_mut().use_theme(&ctx, theme);
        themed.run_steps(3);
        save(&mut themed, name);
    }
    let ctx = find.ctx.clone();
    find.state_mut()
        .use_theme(&ctx, crate::settings::ThemeName::Dark);
}
