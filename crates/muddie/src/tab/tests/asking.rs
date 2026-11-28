//! What scripts ask of the window: a list for the player to choose from,
//! the clipboard, the game read aloud or not, and the room the player is in.

use super::*;

/// The last line of the output.
fn last_line(harness: &Harness<'_, App>) -> String {
    let output = &harness.state().tab().output;
    output.text(output.len() - 1).to_string()
}

#[test]
fn a_list_a_script_offers_is_read_and_chosen_with_the_keys() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    let offer = r#"choose({"Ana", {text = "Bob", about = "2 tells"}},
        function(index, name) echo(`chose {index} {name}`) end, {title = "Reply to"})"#;
    app.scripts.exec("tells", offer);
    app.run_actions();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert!(app.palette.open);
    assert!(
        app.voice.said.iter().any(|said| said == "Reply to"),
        "{:?}",
        app.voice.said
    );
    app.voice.said.clear();
    // The arrows go through it, each said; Enter chooses.
    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    let said = harness.state().tab().voice.said.join(" | ");
    assert!(said.contains("Bob"), "{said}");
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert!(!harness.state().tab().palette.open);
    assert_eq!(last_line(&harness), "chose 2 Bob");
    // Typed, it narrows down; closed, nothing was chosen.
    let app = harness.state_mut().tab_mut();
    app.scripts.exec("tells", offer);
    app.run_actions();
    harness.run_steps(3);
    harness.event(egui::Event::Text("an".into()));
    harness.run_steps(2);
    let items = harness.state_mut().tab_mut().palette_items();
    let names: Vec<&str> = items.iter().map(|item| item.usage.as_str()).collect();
    assert_eq!(names, ["Ana"]);
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert_eq!(last_line(&harness), "chose nil nil");
}

#[test]
fn a_script_silences_the_game_copies_and_hears_of_rooms() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    app.scripts.exec(
        "modes.luau",
        r#"session.reading = false
        on("room", function(room, from) echo(`in {room.name} from {from}`) end)"#,
    );
    app.run_actions();
    assert!(!app.voice.reads_game());
    // The player's own switch is told back.
    app.scripts.unload("modes.luau");
    app.run_actions();
    assert!(app.voice.reads_game());
    app.scripts.exec(
        "rooms.luau",
        r#"on("room", function(room, from) echo(`in {room.name} from {from}`) end)"#,
    );
    // However the map comes to know where the player is.
    app.scripts.exec(
        "rooms.luau",
        r#"map.set_room({id = 1, name = "Gate"}) map.set_room({id = 2, name = "Square"})"#,
    );
    app.run_actions();
    assert!(app.map.borrow_mut().set_current(1));
    harness.run_steps(2);
    assert_eq!(last_line(&harness), "in Gate from nil");
    assert!(
        harness
            .state_mut()
            .tab_mut()
            .map
            .borrow_mut()
            .set_current(2)
    );
    harness.run_steps(2);
    assert_eq!(last_line(&harness), "in Square from 1");
    // What the player copied with the review keys, a script reads back.
    let (held, key) = crate::shortcuts::COPY_READ.first();
    harness.key_press_modifiers(held, key);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    app.scripts
        .exec("t", r#"echo("copied: " .. clipboard.get())"#);
    app.run_actions();
    assert_eq!(last_line(&harness), "copied: in Square from 1");
}

#[test]
fn enter_on_nothing_found_does_not_end_the_choice() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let offer = r#"choose({"Ana", "Bob"}, function(index, name) echo(`chose {index} {name}`) end)"#;
    app.scripts.exec("tells", offer);
    app.run_actions();
    harness.run_steps(3);
    harness.event(egui::Event::Text("zz".into()));
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(
        app.palette.open && app.palette.asking.is_some(),
        "Enter on nothing found ended the choice: open {}, last line {:?}",
        app.palette.open,
        last_line(&harness)
    );
}
