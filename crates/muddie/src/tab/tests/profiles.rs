//! Profiles and plugins.

use super::*;

mod plugins;

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
    harness
        .get_by_role_and_label(egui::accesskit::Role::Tab, "Session")
        .click();
    harness.run_steps(2);
    // What the session keeps in the profile meanwhile is not undone by
    // what the window saves next.
    harness.state_mut().tab_mut().set_channel_quiet("ooc", true);
    harness.run_steps(2);
    (harness.get_by_role_and_label(egui::accesskit::Role::Switch, "Auto-reconnect"))
        .click_accesskit();
    harness.run_steps(2);
    // Closing the window keeps what was changed, without a button, and
    // the session playing with the profile takes it.
    harness.get_by_label("Close").click();
    harness.run_steps(3);
    assert_eq!(harness.state().tab().character, "Naylhena");
    let saved = Profiles::at(dir.clone()).load("Reinos").unwrap();
    assert_eq!(saved.character, "Naylhena");
    // It was on, as it is to begin with: off in this profile only.
    let session = saved.session.unwrap();
    assert_eq!(
        (session.reconnect, session.quiet_channels),
        (false, vec!["ooc".to_owned()])
    );
    assert!(crate::profile::SessionPrefs::default().reconnect);
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

/// A new profile can be made for a game Muddie has a plugin for: with its
/// address and its plugin on, named after it.
#[test]
fn a_new_profile_is_made_for_a_game_with_its_address_and_plugin() {
    let dir = std::env::temp_dir().join(format!("muddie-game-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    crate::plugins::fixture::download_template(&profiles);
    let mut harness = window(Some(profiles.clone()));
    harness.set_size(egui::vec2(1300.0, 860.0));
    harness.state_mut().open_profiles(None);
    harness.run_steps(2);
    // In the list and on the page, with no profile yet: either.
    harness
        .get_all_by_label("New profile")
        .next()
        .unwrap()
        .click();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(egui::accesskit::Role::ComboBox, "Game")
        .click();
    harness.run_steps(2);
    harness.get_by_label("Reinos de Leyenda").click();
    harness.run_steps(2);
    harness.get_by_label_contains("Ready to play Reinos de Leyenda");
    harness.get_by_label("Create").click();
    harness.run_steps(3);
    let saved = profiles.load("Reinos de Leyenda").unwrap();
    assert_eq!(
        (saved.host.as_str(), saved.port, saved.tls),
        ("reinosdeleyenda.es", 5001, false)
    );
    assert_eq!(saved.plugins_on, ["reinos-de-leyenda"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Made with the keyboard alone, as with a screen reader: it starts on
/// the game, which fills in the rest, and Tab goes on to the name and to
/// Create, never held in a field.
#[test]
fn a_new_profile_is_made_with_the_keyboard_alone() {
    let dir = std::env::temp_dir().join(format!("muddie-keys-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    let mut harness = window(Some(profiles.clone()));
    harness.set_size(egui::vec2(1300.0, 860.0));
    harness.state_mut().open_profiles(None);
    harness.run_steps(2);
    harness
        .get_all_by_label("New profile")
        .next()
        .unwrap()
        .click();
    harness.run_steps(3);
    let game = || egui::accesskit::Role::ComboBox;
    assert!(harness.get_by_role_and_label(game(), "Game").is_focused());
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    let field = egui::accesskit::Role::TextInput;
    assert!(harness.get_by_role_and_label(field, "Name").is_focused());
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    assert!(harness.get_by_label("Create").is_focused());
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert!(profiles.load("New profile").is_ok());
    std::fs::remove_dir_all(&dir).unwrap();
}
