//! Muddie's own keys chosen by a script: the map explored, the lines read
//! and speech silenced with the keys the script gives, and with its own
//! again once the script is gone.

use super::*;

fn said(harness: &mut Harness<'_, App>) -> String {
    let app = harness.state_mut().tab_mut();
    let said = app.voice.said.join(" | ");
    app.voice.said.clear();
    said
}

#[test]
fn the_map_is_explored_with_the_keys_a_script_gives() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (_events, _sent) = fake_connection(app);
    desert(app);
    app.scripts.exec(
        "keys.luau",
        r#"map.directions = {e = {"este"}, w = {"oeste"}, n = {"norte"}, s = {"sur"}}
        shortcuts.explore = "f9"
        shortcuts.map_east = {"l", "numpad6"}
        shortcuts.map_west = "h""#,
    );
    app.run_actions();
    assert!(app.map.borrow_mut().set_current(1));
    app.voice.force_voice();
    harness.run_steps(3);
    // Its own keys no longer explore it; those of the script do.
    let (held, key) = crate::shortcuts::EXPLORE.first_own();
    harness.key_press_modifiers(held, key);
    harness.run_steps(3);
    assert!(!harness.state().tab().map_view.exploring());
    harness.key_press(Key::F9);
    harness.run_steps(3);
    assert!(harness.state().tab().map_view.exploring());
    said(&mut harness);
    // A letter follows the exit, and types nothing.
    harness.key_press(Key::L);
    harness.event(egui::Event::Text("l".into()));
    harness.run_steps(2);
    assert_eq!(
        said(&mut harness),
        "Dunas. 1 move to the este. Exits: este, oeste."
    );
    assert!(harness.state().tab().map_view.exploring());
    // The arrow is no longer east; the keypad is.
    harness.key_press(Key::ArrowRight);
    harness.run_steps(2);
    assert_eq!(said(&mut harness), "");
    harness.state_mut().tab_mut().keypad_pressed = vec!["numpad6"];
    harness.event(egui::Event::Text("6".into()));
    harness.run_steps(2);
    assert!(said(&mut harness).starts_with("Dunas. 2 moves to the este."));
    harness.key_press(Key::H);
    harness.run_steps(2);
    assert!(said(&mut harness).starts_with("Dunas. 1 move to the este."));
    // F1 says the keys as they are now.
    harness.key_press(Key::F1);
    harness.run_steps(2);
    assert!(said(&mut harness).contains("L, Keypad 6: Follow the exit east."));
    // Gone with the script, its own keys are back.
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    app.scripts.unload("keys.luau");
    harness.run_steps(2);
    assert_eq!(
        crate::shortcuts::EXPLORE.keys(),
        crate::shortcuts::EXPLORE.defaults()
    );
    harness.key_press_modifiers(held, key);
    harness.run_steps(3);
    assert!(harness.state().tab().map_view.exploring());
}

#[test]
fn the_lines_are_read_and_speech_silenced_with_the_keys_a_script_gives() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    let (events, _sent) = fake_connection(app);
    for text in ["First.", "Second.", "Third."] {
        let line = panels::styled_lines(text).remove(0);
        events.send(Event::Line(line)).unwrap();
    }
    let app = harness.state_mut().tab_mut();
    app.scripts.exec(
        "keys.luau",
        r#"shortcuts.read_previous = "numpad8"
        shortcuts.read_recent_1 = "f8"
        shortcuts.hush = "f12""#,
    );
    harness.run_steps(3);
    said(&mut harness);
    harness.key_press(Key::F8);
    harness.run_steps(2);
    assert_eq!(said(&mut harness), "Third.");
    // The first press reads the last line, and each after it the one before.
    for line in ["Third.", "Second."] {
        harness.state_mut().tab_mut().keypad_pressed = vec!["numpad8"];
        harness.event(egui::Event::Text("8".into()));
        harness.run_steps(2);
        assert_eq!(said(&mut harness), line);
    }
    // Read with the keypad, it typed nothing.
    assert_eq!(harness.state().tab().input, "");
    // Ctrl alone no longer silences speech; the key the script gave does.
    let stopped = harness.state().tab().voice.stopped;
    harness.key_press_modifiers(Modifiers::CTRL, Key::A);
    harness.run_steps(2);
    assert_eq!(harness.state().tab().voice.stopped, stopped);
    harness.key_press(Key::F12);
    harness.run_steps(2);
    assert_eq!(harness.state().tab().voice.stopped, stopped + 1);
}

#[test]
fn the_keys_that_walk_in_the_game_walk_on_the_map() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (_events, mut sent) = fake_connection(app);
    desert(app);
    // As ScriptsRL walks: Alt and the letters around the right hand.
    app.scripts.exec(
        "walking.luau",
        r#"map.directions = {e = {"este"}, w = {"oeste"}, n = {"norte"}, s = {"sur"}}
        macro("alt+o", "este")
        macro("alt+u", "w")
        macro("alt+b", "buscar")
        macro("alt+f", function() send("matar x") end)"#,
    );
    app.run_actions();
    assert!(app.map.borrow_mut().set_current(1));
    app.voice.force_voice();
    harness.run_steps(3);
    let (held, key) = crate::shortcuts::EXPLORE.first();
    harness.key_press_modifiers(held, key);
    harness.run_steps(3);
    said(&mut harness);
    harness.key_press_modifiers(Modifiers::ALT, Key::O);
    harness.run_steps(2);
    assert_eq!(
        said(&mut harness),
        "Dunas. 1 move to the este. Exits: este, oeste."
    );
    harness.key_press_modifiers(Modifiers::ALT, Key::O);
    harness.run_steps(2);
    assert!(said(&mut harness).starts_with("Dunas. 2 moves to the este."));
    // In the MUD's words or in English, short or long.
    harness.key_press_modifiers(Modifiers::ALT, Key::U);
    harness.run_steps(2);
    assert!(said(&mut harness).starts_with("Dunas. 1 move to the este."));
    // Nothing went to the MUD; what is not a way goes as always.
    harness.key_press_modifiers(Modifiers::ALT, Key::B);
    harness.run_steps(2);
    let sent: Vec<String> = std::iter::from_fn(|| sent.try_recv().ok())
        .filter_map(|command| match command {
            Command::Send(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(sent, ["buscar"]);
    assert!(harness.state().tab().map_view.exploring());
}
