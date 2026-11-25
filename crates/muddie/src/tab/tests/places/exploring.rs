//! Finding, choosing and exploring places on the map with the keys alone, for whoever does not see it.

use super::*;

/// For whoever does not see the map: the places of it found from the
/// palette, nearest first, with how far; Enter goes, Shift and Enter says
/// the way; Escape in the command line stops a walk.
#[test]
fn places_are_found_in_the_palette_and_a_walk_stops_with_escape() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (_events, mut sent) = fake_connection(app);
    desert(app);
    app.scripts.exec("t", r#"map.names({exits = "[]"}) map.directions({n = {"norte"}, s = {"sur"}, e = {"este"}, w = {"oeste"}})"#);
    assert!(app.map.borrow_mut().set_current(4));
    app.map.borrow_mut().mark("casa", 1).unwrap();
    app.toggle_palette();
    app.palette.query = "dun".into();
    harness.run_steps(2);
    let items = harness.state_mut().tab_mut().palette_items();
    let places: Vec<(String, String)> = (items.iter())
        .filter(|item| matches!(item.act, Some(super::super::super::commands::Act::Place(_))))
        .map(|item| (item.usage.clone(), item.about.clone()))
        .collect();
    assert_eq!(
        places,
        // Which way, on the map as drawn: the dunes are in a row.
        [(
            "Dunas".to_owned(),
            "1 move to the oeste · nearest of 3".to_owned()
        )]
    );
    // Shift and Enter says the way, in the palette.
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert_eq!(
        app.palette.message.clone(),
        Some(("Dunas, 1 move: sur.".to_owned(), false))
    );
    // A mark is a place too; Enter goes there.
    app.palette.query = "cas".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert!(app.walk.is_some() && !app.palette.open);
    let lines: Vec<Command> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
    assert_eq!(lines.first(), Some(&Command::Send("sur".into())));
    // Escape in the empty command line stops it.
    harness.get_by_label("Command").click();
    harness.run_steps(1);
    eprintln!(
        "DBG input {:?} walk {} popup {} focused {:?}",
        harness.state().tab().input,
        harness.state().tab().walk.is_some(),
        egui::Popup::is_any_open(&harness.ctx),
        harness.ctx.memory(|m| m.focused())
    );
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert!(app.walk.is_none());
    assert!(notes(app).iter().any(|said| said == "Stopped."));
}

/// A room of the map is chosen with a click, to be told under it and to
/// whoever reads the screen; the keys move it along the exits, Enter walks
/// there, and Escape chooses none.
#[test]
fn a_room_is_chosen_on_the_map_to_see_it_and_moved_with_the_keys() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 860.0));
    let app = harness.state_mut().tab_mut();
    let (_events, mut sent) = fake_connection(app);
    desert(app);
    app.scripts.exec(
        "t",
        r#"map.directions({e = {"este"}, w = {"oeste"}, n = {"norte"}, s = {"sur"}}) map.names({exits = "[]"})"#,
    );
    app.run_actions();
    assert!(app.map.borrow_mut().set_current(1));
    harness.run_steps(3);
    // The player's room is in the middle of the map.
    let middle = map_widget(&harness).unwrap().rect().center();
    harness.hover_at(middle);
    harness.run_steps(1);
    harness.drag_at(middle);
    harness.run_steps(1);
    harness.drop_at(middle);
    harness.run_steps(2);
    let chosen = |harness: &Harness<'_, App>| harness.state().tab().map_view.chosen_room();
    assert_eq!(chosen(&harness), Some(1));
    // East along the exits, twice.
    harness.key_press(egui::Key::ArrowRight);
    harness.key_press(egui::Key::ArrowRight);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), Some(3));
    // No exit south: said so, and it stays.
    harness.key_press(egui::Key::ArrowDown);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), Some(3));
    // Enter walks there.
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert_eq!(sent_lines(&mut sent), ["este"]);
    // Escape stops the walk first, then leaves: none chosen.
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), Some(3));
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), None);
}

/// For whoever does not see the map: Alt+E explores it from where they
/// are, without moving, all with the keys. Each room is said short: its
/// name, how far and which way, its exits, and only what is worth knowing
/// besides. Home comes back, Enter goes, and Escape or anything typed
/// leaves it for the command line, with what was typed.
#[test]
fn the_map_is_explored_with_the_keys_alone() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 860.0));
    let app = harness.state_mut().tab_mut();
    let (_events, mut sent) = fake_connection(app);
    desert(app);
    app.scripts.exec(
        "t",
        r#"map.directions({e = {"este"}, w = {"oeste"}, n = {"norte"}, s = {"sur"}}) map.names({exits = "[]"})"#,
    );
    app.run_actions();
    assert!(app.map.borrow_mut().set_current(1));
    app.voice.force_voice();
    harness.run_steps(3);
    let said = |harness: &mut Harness<'_, App>| {
        let app = harness.state_mut().tab_mut();
        let said = app.voice.said.join(" | ");
        app.voice.said.clear();
        said
    };
    said(&mut harness);
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::E);
    harness.run_steps(3);
    assert_eq!(
        said(&mut harness),
        "Exploring the map from Dunas. Exits: este. Arrows follow the exits, Enter goes, Escape goes back. F1 says all the keys."
    );
    let focused = |harness: &Harness<'_, App>, label: &str| {
        harness
            .get_by_label_contains(label)
            .accesskit_node()
            .is_focused()
    };
    assert!(map_widget(&harness).unwrap().is_focused());
    harness.key_press(Key::ArrowRight);
    harness.run_steps(2);
    assert_eq!(
        said(&mut harness),
        "Dunas. 1 move to the este. Exits: este, oeste."
    );
    harness.key_press(Key::ArrowRight);
    harness.key_press(Key::ArrowUp);
    harness.run_steps(2);
    assert_eq!(
        said(&mut harness),
        "Dunas. 2 moves to the este. Exits: norte, oeste. | Pozo. 3 moves to the este. Exits: sur."
    );
    // No way on: said, and it stays.
    harness.key_press(Key::ArrowUp);
    harness.run_steps(2);
    assert_eq!(said(&mut harness), "No exit norte.");
    // Home: back to the player.
    harness.key_press(Key::Home);
    harness.run_steps(2);
    assert_eq!(said(&mut harness), "Dunas. you are here. Exits: este.");
    // The room chosen is kept in the middle, as the player's is.
    harness.key_press(Key::ArrowRight);
    harness.key_press(Key::ArrowRight);
    harness.run_steps(3);
    let cells = |harness: &Harness<'_, App>| harness.state().tab().map_view.pan().x.round() as i32;
    let cell = cells(&harness) / -4;
    assert!(cell > 0, "two rooms east, four cells, centred");
    // Leaving, the view follows the player again.
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert_eq!(cells(&harness), 0);
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::E);
    harness.run_steps(3);
    said(&mut harness);
    // To the well, and Enter goes there.
    for key in [Key::ArrowRight, Key::ArrowRight, Key::ArrowUp, Key::Enter] {
        harness.key_press(key);
    }
    harness.run_steps(2);
    assert_eq!(sent_lines(&mut sent), ["este"]);
    // A letter typed: back to the command line, with it.
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::E);
    harness.run_steps(3);
    assert!(map_widget(&harness).unwrap().is_focused());
    harness.event(egui::Event::Text("m".into()));
    harness.run_steps(3);
    assert!(focused(&harness, "Command"));
    assert_eq!(harness.state().tab().input, "m");
    // Escape: back too, as it was; the first stops the walk still on.
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::E);
    harness.run_steps(3);
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(harness.state().tab().walk.is_none());
    assert!(map_widget(&harness).unwrap().is_focused());
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(focused(&harness, "Command"));
    assert_eq!(harness.state().tab().input, "m");
}

#[test]
fn the_map_explored_is_searched_named_noted_and_gone_back_along() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 860.0));
    let app = harness.state_mut().tab_mut();
    let (_events, mut sent) = fake_connection(app);
    desert(app);
    app.scripts.exec(
        "t",
        r#"map.directions({e = {"este"}, w = {"oeste"}, n = {"norte"}, s = {"sur"}}) map.names({exits = "[]"}) macro("numpad6", "este") macro("numpadhome", "noroeste")"#,
    );
    app.run_actions();
    assert!(app.map.borrow_mut().set_current(1));
    app.voice.force_voice();
    harness.run_steps(3);
    let said = |harness: &mut Harness<'_, App>| {
        let app = harness.state_mut().tab_mut();
        let said = app.voice.said.join(" | ");
        app.voice.said.clear();
        said
    };
    // The field and the words before it both have the label.
    let focused = |harness: &Harness<'_, App>, label: &str| {
        (harness.query_all_by_label_contains(label)).any(|node| node.accesskit_node().is_focused())
    };
    let typed = |harness: &mut Harness<'_, App>, text: &str| {
        harness.event(egui::Event::Text(text.into()));
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(3);
    };
    let command = |harness: &mut Harness<'_, App>, key| {
        harness.key_press_modifiers(Modifiers::COMMAND, key);
        harness.run_steps(3);
    };
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::E);
    harness.run_steps(3);
    said(&mut harness);
    // The keypad moves on the map: what it is bound to is not sent.
    harness.state_mut().tab_mut().keypad_pressed = vec!["numpad6"];
    harness.key_press(Key::Num6);
    harness.event(egui::Event::Text("6".into()));
    harness.run_steps(2);
    assert_eq!(
        said(&mut harness),
        "Dunas. 1 move to the este. Exits: este, oeste."
    );
    // Keypad 7 with Num Lock off is north-west, not the Home egui makes
    // of it, which would go back to the player.
    harness.state_mut().tab_mut().keypad_pressed = vec!["numpadhome"];
    harness.event(egui::Event::Key {
        key: Key::Home,
        physical_key: Some(Key::Num7),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    harness.run_steps(2);
    assert_eq!(said(&mut harness), "No exit northwest.");
    // Backspace goes back along the rooms chosen, to where it began.
    harness.key_press(Key::ArrowRight);
    harness.run_steps(2);
    said(&mut harness);
    for _ in 0..3 {
        harness.key_press(Key::Backspace);
        harness.run_steps(2);
    }
    assert_eq!(
        said(&mut harness),
        "Dunas. 1 move to the este. Exits: este, oeste. | Dunas. you are here. Exits: este. | This is where you began exploring."
    );
    // A search: what is found is said, Enter goes to it on the map.
    command(&mut harness, Key::F);
    assert!(focused(&harness, "Find on the map"));
    typed(&mut harness, "pozo");
    assert_eq!(
        said(&mut harness),
        "Find on the map: write it, then Enter. | 1 found. 1 of 1: Pozo, 3 moves to the este."
    );
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(said(&mut harness), "Pozo. 3 moves to the este. Exits: sur.");
    assert!(map_widget(&harness).unwrap().is_focused());
    // A note and a name, said with the room from then on.
    command(&mut harness, Key::N);
    assert!(focused(&harness, "Note on Pozo"));
    typed(&mut harness, "Agua fresca");
    assert_eq!(
        said(&mut harness),
        "Note on Pozo: write it, then Enter. | Noted: Agua fresca"
    );
    command(&mut harness, Key::B);
    typed(&mut harness, "agua");
    assert_eq!(
        said(&mut harness),
        "Mark for Pozo: write it, then Enter. | This room is now marked agua: typed in the palette, agua goes to it."
    );
    assert!(map_widget(&harness).unwrap().is_focused());
    harness.key_press(Key::Space);
    harness.run_steps(2);
    assert_eq!(
        said(&mut harness),
        "Pozo. 3 moves to the este. Exits: sur. Marked agua. Note: Agua fresca."
    );
    command(&mut harness, Key::I);
    let info = said(&mut harness);
    assert!(info.starts_with("Pozo (#4)."), "{info}");
    assert!(info.contains("Marked agua. Note: Agua fresca"), "{info}");
    // From where the player is, what is noted near.
    harness.key_press(Key::Home);
    harness.run_steps(2);
    said(&mut harness);
    command(&mut harness, Key::L);
    assert_eq!(said(&mut harness), "Near: agua, 3 moves to the este.");
    harness.key_press(Key::F1);
    harness.run_steps(2);
    assert!(said(&mut harness).starts_with("Arrows, or the keypad, follow the exits"));
    // Escape leaves a search for the map, and the map for the line.
    command(&mut harness, Key::F);
    harness.event(egui::Event::Text("agua".into()));
    harness.run_steps(2);
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(map_widget(&harness).unwrap().is_focused());
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(focused(&harness, "Command"));
    // All of it without a word to the MUD.
    assert!(sent_lines(&mut sent).is_empty());
    let map = harness.state().tab().map.borrow();
    assert_eq!(
        (map.room(4).unwrap().note.as_str(), map.marked("agua")),
        ("Agua fresca", Some(4))
    );
}

/// The rooms the map added of itself are listed, to go to, said apart,
/// and undone; and the rooms that look wrong are found.
#[test]
fn the_rooms_the_map_added_are_listed_said_apart_undone_and_checked() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    desert(app);
    assert_eq!(answer(app, "/map added"), ["The map has added no room."]);
    assert_eq!(
        answer(app, "/map check"),
        ["Nothing on the map looks wrong."]
    );
    let exits = ["s".to_owned(), "e".to_owned()];
    {
        let mut map = app.map.borrow_mut();
        map.set_current(4);
        let seen = muddie_mapper::Arrival {
            name: "Oasis",
            exits: &exits,
            came: "n",
        };
        map.add(seen).unwrap();
    }
    let id = muddie_mapper::ADDED_FROM;
    assert_eq!(
        answer(app, "/map added"),
        [format!("Oasis (#{id}): Desierto")]
    );
    let info = answer(app, "/map info");
    assert!(
        info[0].contains("Not explored: east, south. The map added it where you went"),
        "{info:?}"
    );
    // A room with no name, which nothing leads to.
    app.map
        .borrow_mut()
        .set_room(RoomInfo {
            id: 9,
            name: String::new(),
            area: "Desierto".into(),
            exits: Default::default(),
            pos: None,
            colour: None,
        })
        .unwrap();
    let checked = answer(app, "/map check");
    assert_eq!(checked.len(), 1);
    assert!(checked[0].ends_with(": no name"), "{checked:?}");
    app.undo_change();
    assert_eq!(last_said(app), "Undone: Oasis is no longer on the map.");
    assert!(app.map.borrow().room(id).is_none());
    app.undo_change();
    assert_eq!(
        last_said(app),
        "The map has changed nothing of itself to undo."
    );
    assert_eq!(
        answer(app, "/map remove 9"),
        ["(#9) is no longer on the map."]
    );
    assert_eq!(
        answer(app, "/map check"),
        ["Nothing on the map looks wrong."]
    );
}
