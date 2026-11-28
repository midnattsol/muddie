//! Journeys a blind player makes with the keyboard alone.

use super::*;
use egui::accesskit::Role;

fn focused(harness: &Harness<'_, App>) -> Option<(Role, String)> {
    (harness.query_all(egui_kittest::kittest::by()))
        .find(|node| node.is_focused())
        .map(|node| {
            let node = node.accesskit_node();
            (node.role(), node.label().unwrap_or_default().to_string())
        })
}

/// First launch: the welcome says to write New profile in the palette,
/// and that is what the player does.
#[test]
fn new_profile_is_in_the_palette_as_the_welcome_says() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/journey-welcome-profiles");
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    {
        let tab = harness.state_mut().tab_mut();
        tab.started = false;
        tab.voice.force_voice();
        tab.voice.pretend_reader = true;
    }
    harness.run_steps(4);
    let welcome = harness.state().tab().voice.said.join(" ");
    assert!(welcome.contains("New profile, in the palette"), "{welcome}");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run_steps(3);
    harness.event(egui::Event::Text("new profile".into()));
    harness.run_steps(3);
    harness.key_press(Key::Enter);
    harness.run_steps(4);
    let said = harness.state().tab().voice.said.clone();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        harness
            .query_by_role_and_label(Role::Dialog, "New profile")
            .is_some(),
        "no New profile dialog; focused {:?}; said {said:?}",
        focused(&harness)
    );
}

/// Escape in the editor goes back to the game, as its page says, from a
/// control that is not a field too.
#[test]
fn escape_on_a_control_of_the_editor_goes_back_to_the_game() {
    let mut harness = connected_demo_window();
    harness.state_mut().tab_mut().keys_to_the_line();
    harness.run_steps(3);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::E);
    harness.run_steps(4);
    assert_eq!(focused(&harness), Some((Role::Tab, "Triggers".into())));
    harness.key_press(Key::Escape);
    harness.run_steps(6);
    assert_eq!(
        (harness.state().tab().view(), focused(&harness)),
        (View::Game, Some((Role::TextInput, "Command".into())))
    );
}

/// Ctrl+W closes the session on screen; whoever cannot see the bar of
/// sessions hears which one is on screen now, as with Ctrl+Tab.
#[test]
fn closing_a_session_says_which_is_on_screen_now() {
    let mut harness = window(None);
    harness.state_mut().tab_mut().host = "first.example.org".into();
    harness.state_mut().tab_mut().voice.force_voice();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::T);
    harness.run_steps(3);
    {
        let tab = harness.state_mut().tab_mut();
        tab.started = true;
        tab.host = "second.example.org".into();
        tab.voice.force_voice();
    }
    harness.run_steps(3);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::W);
    harness.run_steps(4);
    let tab = harness.state().tab();
    assert_eq!(tab.title(), "first.example.org");
    assert!(
        tab.voice
            .said
            .iter()
            .any(|said| said.contains("first.example.org")),
        "said {:?}",
        tab.voice.said
    );
}

/// A session out of sight that loses its connection says which it is:
/// "Disconnected" alone is taken for the session being played.
#[test]
fn a_session_out_of_sight_says_which_was_disconnected() {
    let mut harness = window(None);
    let (events, _sent) = {
        let app = harness.state_mut().tab_mut();
        app.host = "first.example.org".into();
        app.voice.force_voice();
        fake_connection(app)
    };
    events.send(Event::Connected).unwrap();
    harness.run_steps(3);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::T);
    harness.run_steps(3);
    harness.state_mut().tab_mut().started = true;
    harness.state_mut().tab_mut().host = "second.example.org".into();
    harness.run_steps(3);
    events
        .send(Event::Disconnected(Some("Connection reset".into())))
        .unwrap();
    harness.run_steps(3);
    let said = harness.state().tabs()[0].voice.said.clone();
    let told = said.iter().find(|said| said.contains("Disconnected"));
    assert!(
        told.is_some_and(|told| told.starts_with("first.example.org: "))
            && said
                .iter()
                .any(|said| said.ends_with("Reconnecting in 1 second.")),
        "said {said:?}"
    );
}

/// What was typed in the script editor just before the session is closed
/// (Ctrl+W) must reach the file, as closing the editor itself does.
#[test]
fn closing_the_session_keeps_what_was_just_typed_in_a_script() {
    let dir = std::env::temp_dir().join(format!("muddie-hunt-close-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Test", &crate::profile::Profile::default())
        .unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Test".into())));
    app.open_automation(automation::SCRIPTS);
    app.add_script(None);
    harness.run_steps(3);
    harness.state_mut().tab_mut().editor.script_text = r#"macro("f9", "kick")"#.into();
    harness.run_steps(2);
    let path = harness.state().tab().editor.script_loaded.clone().unwrap();
    let (held, key) = crate::shortcuts::CLOSE_SESSION.first();
    harness.key_press_modifiers(held, key);
    harness.run_steps(2);
    // The session was closed: the one there now has no profile.
    assert_eq!(harness.state().tab().profile_name(), None);
    let written = std::fs::read_to_string(&path).unwrap_or_default();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(written, r#"macro("f9", "kick")"#);
}
