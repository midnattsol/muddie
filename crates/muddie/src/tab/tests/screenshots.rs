//! Images of every screen, for looking at by eye.

use super::*;
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
    shows_plugins();
    shows_editor();
    shows_menu();
    shows_unmapped();
    shows_panels();
    shows_palette();
    shows_empty_line();
    shows_find_and_themes();
    shows_map();
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
    welcome
        .state_mut()
        .open_settings(crate::app::Section::Profiles);
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
    // Further down: how it is kept, its logs, and its plugins.
    welcome.get_by_label("Log every session").scroll_to_me();
    welcome.run_steps(3);
    save(&mut welcome, "profile-middle");
    welcome.get_by_label("Install").scroll_to_me();
    welcome.run_steps(3);
    save(&mut welcome, "profile-end");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A plugin for the MUD connected to, offered over the command line.
fn shows_plugin_offer() {
    let mut offer = connected_demo_window();
    let app = offer.state_mut().tab_mut();
    app.offer_plugin_for("rlmud.org", "");
    offer.run_steps(3);
    save(&mut offer, "plugin-offer");
}

/// The channels of the MUD, each with its tab in one panel.
fn shows_channels() {
    let mut channels = connected_demo_window();
    let app = channels.state_mut().tab_mut();
    app.scripts.exec(
            "t",
            "channel(\"chat\", \"[Chat] Naylhena: \\27[1mre\\27[0m\") channel(\"mercenario\") channel(\"bando\", \"[Bando] Eva: al norte\") channel(\"chat\", \"[Chat] Luis: buenas\")",
        );
    app.run_actions();
    channels.run_steps(3);
    channels.get_by_label("chat").click();
    channels.run_steps(3);
    let app = channels.state_mut().tab_mut();
    app.scripts
        .exec("t", "channel(\"bando\", \"[Bando] Eva: esperad\")");
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
    script.state_mut().tab_mut().editor.script_text = "supports(\"Comm 1\")\n\non_gmcp(\"Comm.MensajeCanal\", function(data)\n\tpanel_add(data.canal, data.mensaje)\nend)\n".into();
    script.run_steps(2);
    script.get_by_label("Run").click();
    script.run_steps(4);
    save(&mut script, "script");
    script.state_mut().tab_mut().editor.script_text =
        "supports(\"Comm 1\")\nerror(\"boom\")\n".into();
    script.run_steps(2);
    script.get_by_label("Run").click();
    script.run_steps(4);
    save(&mut script, "script-error");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The settings, and the same text in each of the fonts that come with the
/// program.
fn shows_settings_and_fonts() {
    let mut settings = demo_window();
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
        let mut fonts = connected_demo_window();
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
        std::fs::create_dir_all(dir.join("packages").join(plugin)).unwrap();
        std::fs::write(dir.join("packages").join(plugin).join("init.luau"), "").unwrap();
    }
    let profiles = Profiles::at(dir.clone());
    let profile = Profile {
        packages: vec!["aardwolf-mapper".into()],
        ..Profile::default()
    };
    profiles.save("Aardwolf", &profile).unwrap();
    let mut plugins = window(Some(profiles));
    plugins.state_mut().edit_profile("Aardwolf");
    plugins.run_steps(4);
    plugins.get_by_label("Update all").scroll_to_me();
    plugins.run_steps(3);
    save(&mut plugins, "plugins");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The automation window, with rules of each kind to look at.
fn shows_editor() {
    let mut rules = demo_window();
    rules.set_size(egui::vec2(1500.0, 900.0));
    let app = rules.state_mut().tab_mut();
    app.rules.triggers.push(Trigger {
        name: "Tells".into(),
        group: "Social".into(),
        enabled: true,
        pattern: "tells you".into(),
        speak: "a tell".into(),
        highlight: "yellow".into(),
        ..Trigger::default()
    });
    app.rules.triggers.push(Trigger {
            name: "Fight back".into(),
            group: "Combat".into(),
            enabled: true,
            pattern: r"^(\w+) attacks you".into(),
            regex: true,
            send: "kill %1\nshield".into(),
            sound: "alert.ogg".into(),
            lua: true,
            script: "-- Only the dangerous ones\nsend(\"kill \" .. m[1])\nlocal feared = {Orc = true, Troll = true}\nif feared[m[1]] and gmcp(\"Char.Vitals\").hp < 40 then\n\tsend(\"flee\")\n\techo(\"Too weak for \" .. m[1])\nend".into(),
            ..Trigger::default()
        });
    app.rules.triggers.push(Trigger {
        enabled: false,
        pattern: "The wind howls".into(),
        gag: true,
        ..Trigger::default()
    });
    app.rules.keys.push(KeyBinding {
        name: "North".into(),
        enabled: true,
        key: "numpad8".into(),
        send: "north".into(),
        ..KeyBinding::default()
    });
    app.open_automation(0);
    app.editor.selected[0] = 1;
    app.editor.sample = "Orc attacks you with a rusty axe!".into();
    rules.run_steps(4);
    save(&mut rules, "automation");
    // The same with a mistake in the script, and then as commands.
    let app = rules.state_mut().tab_mut();
    let good = app.editor.draft.triggers[1].script.clone();
    app.editor.draft.triggers[1].script =
        good.replace("then\n\tsend(\"flee\")", "\n\tsend(\"flee\"");
    rules.run_steps(3);
    save(&mut rules, "mistake");
    let app = rules.state_mut().tab_mut();
    app.editor.draft.triggers[1].script = good;
    app.editor.draft.triggers[1].lua = false;
    rules.run_steps(3);
    save(&mut rules, "commands");
    rules.state_mut().tab_mut().editor.draft.triggers[1].lua = true;
    let app = rules.state_mut().tab_mut();
    app.editor.section = 2;
    rules.run_steps(3);
    save(&mut rules, "macros");
    let app = rules.state_mut().tab_mut();
    app.editor.draft.timers.push(crate::rules::Timer {
        name: "Guardar".into(),
        tenths: 3000,
        send: "save".into(),
        ..crate::rules::Timer::default()
    });
    app.editor.section = 3;
    rules.run_steps(3);
    save(&mut rules, "timers");
    let app = rules.state_mut().tab_mut();
    app.scripts
        .set_variable("objetivo", Some(serde_json::json!("orco jefe")))
        .unwrap();
    app.scripts
        .set_variable("muertes", Some(serde_json::json!(12)))
        .unwrap();
    app.scripts
        .set_variable(
            "enemigo",
            Some(serde_json::json!({"nombre": "troll", "vida": 340, "jefe": true})),
        )
        .unwrap();
    app.scripts
        .set_variable("ruta", Some(serde_json::json!(["norte", "norte", "este"])))
        .unwrap();
    app.editor.section = 5;
    app.editor.selected[5] = 0;
    rules.run_steps(3);
    save(&mut rules, "variables");
    rules.state_mut().tab_mut().editor.selected[5] = 3;
    rules.run_steps(3);
    save(&mut rules, "variable-array");
    let app = rules.state_mut().tab_mut();
    app.saw_data(
        "Char.Vitals",
        false,
        r#"{"health": {"current": 40, "max": 100}}"#,
    );
    app.saw_data("Char.Vitals", false, r#"{"health": {"current": 62, "max": 100}, "stamina": {"current": 80, "max": 90}, "focus": {"current": 12, "max": 30}}"#);
    app.saw_data(
        "Map.Render",
        false,
        r##"{"width": 5, "height": 3, "lines": ["  #  ", "#-@-#", "  #  "]}"##,
    );
    app.saw_data(
        "Room.Info",
        false,
        r#"{"num": 2, "name": "Plaza Mayor", "exits": {"n": 5, "e": 3}}"#,
    );
    app.saw("MXP", "send", r#""<send href=\"mirar\">""#);
    app.protocols = vec![
        ("GMCP".into(), true),
        ("MXP".into(), true),
        ("MSSP".into(), true),
        ("ATCP".into(), false),
    ];
    app.set_view(2);
    rules.run_steps(3);
    save(&mut rules, "data");
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

/// The panels with their headers, sharing sides and places, and the
/// gauges made from them.
fn shows_panels() {
    let mut layout = connected_demo_window();
    layout.set_size(egui::vec2(1500.0, 900.0));
    let app = layout.state_mut().tab_mut();
    // The chat at the right, under the map, sharing its side.
    let mut chat = app.layout.panels.remove(0);
    chat.side = layout::Side::Right;
    app.layout.panels.push(chat);
    // And two panels of text under it in one place, as tabs.
    app.add_to_panel(
        "Combate",
        &panels::styled_lines("\x1b[31mUn orco te ataca\x1b[0m con su hacha.").remove(0),
    );
    app.add_to_panel(
        "Tells",
        &panels::styled_lines("\x1b[33mAna te dice: ¿vienes?\x1b[0m").remove(0),
    );
    for panel in &mut app.layout.panels {
        if panel.kind == layout::Kind::Text {
            panel.side = layout::Side::Right;
            panel.stack = 1;
        }
    }
    app.layout.panels.retain(|panel| panel.name != "Minimapa");
    layout.run_steps(4);
    save(&mut layout, "layout");
    let app = layout.state_mut().tab_mut();
    let gauges = app
        .layout
        .panels
        .iter()
        .find(|panel| panel.kind == layout::Kind::Gauges)
        .unwrap()
        .id;
    app.open_gauges(gauges);
    app.editor.draft.gauges.push(crate::rules::Gauge {
        name: "Salud".into(),
        value: "Char.Vitals.health.current".into(),
        max: "Char.Vitals.health.max".into(),
        colour: "green".into(),
        steps: vec![
            crate::rules::Step {
                under: 60,
                colour: "yellow".into(),
            },
            crate::rules::Step {
                under: 30,
                colour: "red".into(),
            },
        ],
        ..crate::rules::Gauge::starter()
    });
    app.editor.draft.gauges.push(crate::rules::Gauge {
        name: "Envenenado".into(),
        value: "Char.Status.poisoned".into(),
        kind: crate::rules::GaugeKind::Light,
        colour: "red".into(),
        off_colour: "green".into(),
        ..crate::rules::Gauge::starter()
    });
    app.editor.selected[4] = 0;
    layout.run_steps(4);
    save(&mut layout, "gauges");
    layout.state_mut().tab_mut().editor.selected[4] = 1;
    layout.run_steps(4);
    save(&mut layout, "gauge-light");
}

/// The palette of commands over the command line, and an answer.
fn shows_palette() {
    let mut palette = connected_demo_window();
    let app = palette.state_mut().tab_mut();
    app.scripts.exec(
        "t",
        r#"bind("f9", "kick", {group = "Combate"}) bind("f8", "hi", {group = "Social"})"#,
    );
    app.toggle_palette();
    palette.run_steps(8);
    save(&mut palette, "palette");
    palette.state_mut().tab_mut().palette.query = "map".into();
    palette.run_steps(4);
    save(&mut palette, "palette-typed");
    palette.state_mut().tab_mut().palette.query = "gr".into();
    palette.run_steps(3);
    palette.key_press(Key::Enter);
    palette.run_steps(4);
    save(&mut palette, "answer");
    palette.key_press(Key::Enter);
    palette.run_steps(3);
    save(&mut palette, "bar-note");
}

/// The command line with nothing typed in it.
fn shows_empty_line() {
    let mut empty = connected_demo_window();
    empty.state_mut().tab_mut().input.clear();
    empty.run_steps(3);
    save(&mut empty, "empty");
}

/// The find bar and the view of the latest lines while scrolled back.
fn shows_find_and_themes() {
    let mut find = connected_demo_window();
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
        let mut themed = connected_demo_window();
        let ctx = themed.ctx.clone();
        themed.state_mut().use_theme(&ctx, theme);
        themed.run_steps(3);
        save(&mut themed, name);
    }
    let ctx = find.ctx.clone();
    find.state_mut()
        .use_theme(&ctx, crate::settings::ThemeName::Dark);
}

/// The map: as walked, every area on a level, the menu of a room, a room
/// avoided.
fn shows_map() {
    // The first is the image named itself.
    let path = std::env::var_os("MUDDIE_SCREENSHOT").map(PathBuf::from);
    let mut harness = connected_demo_window();
    assert!(harness.query_by_label("Connection").is_none());
    let image = harness.render().expect("rendering failed");
    assert_eq!((image.width(), image.height()), (1100, 700));
    if let Some(path) = &path {
        image.save(path).expect("could not write the screenshot");
    }
    // A second view: every area on the level.
    harness.state_mut().tab_mut().map_view.all_areas = true;
    harness.run_steps(3);
    let image = harness.render().expect("rendering failed");
    if let Some(path) = &path {
        (image.save(path.with_extension("all-areas.png"))).expect("could not write the screenshot");
    }
    // A third: the menu of the room under the pointer. The map is
    // centred on the player's room, so that is the room it opens on.
    harness.get_by_label("Map view").click_secondary();
    harness.run_steps(3);
    assert!(harness.query_by_label("Avoid in routes").is_some());
    let image = harness.render().expect("rendering failed");
    if let Some(path) = &path {
        let path = path.with_extension("map-menu.png");
        image.save(path).expect("could not write the screenshot");
    }
    // Choosing to avoid the room marks it on the map.
    harness.get_by_label("Avoid in routes").click();
    harness.run_steps(3);
    assert!(harness.state().tab().map.borrow().room(2).unwrap().avoid);
    // The map placed the rooms as walked: the patio is north-east of
    // the alley, one cell east of the square.
    let map = harness.state().tab().map.borrow();
    assert_eq!(map.room(7).unwrap().pos, Pos { x: 2, y: 2, z: 0 });
}
