//! Profiles and plugins.

use super::*;

#[test]
fn a_session_is_saved_as_a_profile_that_brings_its_scripts() {
    let dir = std::env::temp_dir().join(format!("muddie-app-profile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    let app = harness.state_mut().tab_mut();
    app.host = "mud.example.org".into();
    app.port = "4000".into();
    app.tls = true;
    app.verify_certificate = false;
    // Asked for a name, with one made from the MUD's address.
    app.app_request = Some(AppRequest::SaveAsProfile);
    harness.run_steps(3);
    harness.get_by_label("Save").click();
    harness.run_steps(3);
    let saved = Profiles::at(dir.clone()).load("Mud").unwrap();
    assert_eq!((saved.host.as_str(), saved.port), ("mud.example.org", 4000));
    assert!(saved.tls && !saved.verify_certificate);
    assert_eq!(harness.state().tab().profile.as_deref(), Some("Mud"));

    // Scripts dropped in the profile's folder run when it is selected.
    std::fs::write(
        dir.join("profiles/Mud/scripts/hello.luau"),
        "echo('hola desde el perfil')",
    )
    .unwrap();
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(None));
    app.host.clear();
    assert!(app.select_profile(Some("Mud".into())));
    assert_eq!(app.host, "mud.example.org");
    assert!(output_lines(app).contains(&"hola desde el perfil".to_owned()));
    assert!(!app.select_profile(Some("Missing".into())));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_plugin_for_the_mud_is_offered_and_switched_on_without_the_rest_starting_over() {
    let dir = std::env::temp_dir().join(format!("muddie-included-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Reinos", &crate::profile::Profile::default())
        .unwrap();
    let mut harness = window(Some(profiles.clone()));
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    app.host = "rlmud.org".into();
    let reinos = crate::bundled::find("reinos-de-leyenda").unwrap();
    let common = crate::bundled::find(crate::bundled::COMMON).unwrap();
    // What many MUDs send the same way is on to begin with; the plugin
    // of one MUD is not.
    assert!(app.included(common) && !app.included(reinos));
    app.scripts.exec(
        "keeps",
        "kept = 7 alias(\"^kept$\", function() echo(\"kept \" .. kept) end)",
    );
    // Connected to that MUD, its plugin is offered over the command line.
    let (events, _sent) = fake_connection(app);
    events.send(Event::Connected).unwrap();
    harness.run_steps(3);
    harness.get_by_label_contains("There is a plugin for Reinos de Leyenda");
    // Switched on there, it works at once: what the MUD says on its
    // channels has a tab, and nothing else was run again.
    harness.get_by_label("Enable").click();
    harness.run_steps(3);
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    events
        .send(gmcp(
            "Comm.MensajeCanal",
            r#"{"canal": "chat", "mensaje": "[Chat] Ana: hola"}"#,
        ))
        .unwrap();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert!(app.included(reinos) && app.plugin_offer.is_none());
    assert_eq!(app.panels.channels[0].name, "chat");
    app.scripts.on_input("kept");
    app.run_actions();
    assert_eq!(output_lines(app).last().map(String::as_str), Some("kept 7"));
    // It is kept with the profile, and not offered again.
    let saved = profiles.load("Reinos").unwrap();
    assert_eq!(saved.included_on, ["reinos-de-leyenda"]);
    events.send(Event::Connected).unwrap();
    harness.run_steps(3);
    assert!(harness.state().tab().plugin_offer.is_none());
    // Switched off, what it made is gone.
    let app = harness.state_mut().tab_mut();
    app.set_included(reinos, false);
    let before = app.panels.channels[0].log.len();
    app.scripts.on_gmcp(
        "Comm.MensajeCanal",
        r#"{"canal": "chat", "mensaje": "otra"}"#,
    );
    app.run_actions();
    assert_eq!(app.panels.channels[0].log.len(), before);
    // Declined instead, it is not offered again in this profile either.
    app.offer_plugin_for("rlmud.org", "");
    assert!(app.plugin_offer.is_some());
    app.decline_plugin();
    app.offer_plugin_for("", "Reinos de Leyenda");
    assert!(app.plugin_offer.is_none());
    assert_eq!(
        profiles.load("Reinos").unwrap().included_declined,
        ["reinos-de-leyenda"]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn what_many_muds_send_alike_is_a_plugin_that_can_be_switched_off() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let common = crate::bundled::find(crate::bundled::COMMON).unwrap();
    let (events, _sent) = fake_connection(app);
    let vitals = || Event::Gmcp {
        package: "Char.Vitals".into(),
        data: r#"{"hp": 80, "maxhp": 100}"#.into(),
    };
    let said = || Event::Gmcp {
        package: "Comm.Channel.Text".into(),
        data: r#"{"channel": "ooc", "text": "Ana: hola"}"#.into(),
    };
    // On, as it is to begin with: gauges and channels by themselves.
    events.send(vitals()).unwrap();
    events.send(said()).unwrap();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert!(app.gauges.summary().contains("80 of 100"));
    assert_eq!(app.panels.channels.len(), 1);
    // Off, the client makes nothing of them by itself; scripts still
    // get them.
    app.set_included(common, false);
    app.gauges.clear();
    app.panels.channels.clear();
    app.scripts.exec(
        "t",
        r#"on_gmcp("Char.Vitals", function(data) echo("hp " .. data.hp) end)"#,
    );
    events.send(vitals()).unwrap();
    events.send(said()).unwrap();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.gauges.summary().is_empty() && app.panels.channels.is_empty());
    assert_eq!(output_lines(app).last().map(String::as_str), Some("hp 80"));
}

#[test]
fn a_profile_keeps_what_is_changed_in_the_settings_as_it_is_changed() {
    let dir = std::env::temp_dir().join(format!("muddie-autosave-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save(
            "Reinos",
            &Profile {
                host: "rlmud.org".into(),
                port: 5001,
                ..Profile::default()
            },
        )
        .unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1300.0, 860.0));
    assert!(
        harness
            .state_mut()
            .tab_mut()
            .select_profile(Some("Reinos".into()))
    );
    harness.state_mut().edit_profile("Reinos");
    harness.run_steps(3);
    let character = || (egui::accesskit::Role::TextInput, "Character");
    harness
        .get_by_role_and_label(character().0, character().1)
        .focus();
    harness.run_steps(2);
    (harness.get_by_role_and_label(character().0, character().1)).type_text("Naylhena");
    harness.run_steps(2);
    // What depends on the MUD is the profile's, not every session's.
    (harness.get_by_role_and_label(egui::accesskit::Role::Switch, "Keep the connection alive"))
        .click_accesskit();
    harness.run_steps(2);
    // Closing the settings keeps what was changed, without a button, and
    // the session playing with the profile takes it.
    harness.get_by_label("Close").click();
    harness.run_steps(3);
    assert_eq!(harness.state().tab().character, "Naylhena");
    let saved = Profiles::at(dir.clone()).load("Reinos").unwrap();
    assert_eq!(saved.character, "Naylhena");
    // It was on, as it is to begin with: off in this profile only.
    assert_eq!(
        saved.session.map(|session| session.keepalive_seconds),
        Some(0)
    );
    assert_eq!(harness.state().app_settings().keepalive_seconds, 60);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_plugin_that_comes_with_muddie_is_made_ones_own_to_change() {
    let dir = std::env::temp_dir().join(format!("muddie-customize-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles.save("Reinos", &Profile::default()).unwrap();
    let mut harness = window(Some(profiles.clone()));
    harness.set_size(egui::vec2(1300.0, 860.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    let plugin = crate::bundled::find("reinos-de-leyenda").unwrap();
    app.set_included(plugin, true);
    harness.state_mut().edit_profile("Reinos");
    harness.run_steps(3);
    // In its menu.
    harness
        .get_by_label("Reinos de Leyenda options")
        .click_accesskit();
    harness.run_steps(2);
    harness.get_by_label("Customize").click_accesskit();
    harness.run_steps(4);
    // Its code is a script of the profile's, open in the editor, and the
    // one that came is off so as not to run twice.
    let path = profiles
        .scripts_dir("Reinos")
        .join("reinos-de-leyenda.luau");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        plugin.code.unwrap()
    );
    let app = harness.state().tab();
    assert!(!app.included(plugin));
    assert_eq!((app.view(), app.editor.section), (1, automation::SCRIPTS));
    assert_eq!(
        app.editor
            .script_files
            .get(app.editor.selected[automation::SCRIPTS]),
        Some(&path)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn welcome_screen_lists_the_profiles_and_gives_way_once_connecting() {
    let dir = std::env::temp_dir().join(format!("muddie-app-welcome-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    let profile = Profile {
        host: "reinos.example.org".into(),
        port: 4000,
        ..Profile::default()
    };
    profiles.save("Reinos", &profile).unwrap();
    let mut harness = window(Some(profiles));
    harness.state_mut().tab_mut().started = false;
    harness.run_steps(3);
    assert!(harness.query_by_label("Reinos").is_some());
    assert!(
        harness
            .query_by_label("Where do you want to play?")
            .is_some()
    );
    // The session's own controls are not there yet.
    assert!(harness.query_by_label("Command").is_none());
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    assert!(harness.query_by_label("Command").is_some());
    assert!(
        harness
            .query_by_label("Where do you want to play?")
            .is_none()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
