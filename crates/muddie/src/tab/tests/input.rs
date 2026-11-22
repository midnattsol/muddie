//! The command line, the palette and the keys.

use super::*;

#[test]
fn tab_completes_from_recent_output() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.show_local("A goblin sharpens a golden axe.");
    app.input = "kill go".into();
    harness.run_steps(2);
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    assert_eq!(harness.state().tab().input, "kill goblin");
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    assert_eq!(harness.state().tab().input, "kill golden");
}

#[test]
fn tab_completes_with_words_the_server_gives() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let example = include_str!("../../../../../examples/scripts/completions.luau");
    app.scripts.exec("completions", example);
    app.run_actions();
    let (events, _sent) = fake_connection(app);
    let words = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    events
        .send(words(
            "Dev.Completions",
            r#"{"words": ["teleport", "tell"]}"#,
        ))
        .unwrap();
    events
        .send(words(
            "Dev.Completions.Ids",
            r#"{"words": ["temple_guard"]}"#,
        ))
        .unwrap();
    // A newer list replaces the one of the same package only.
    events
        .send(words("Dev.Completions", r#"{"words": ["teleport"]}"#))
        .unwrap();
    harness.run_steps(2);
    harness.state_mut().tab_mut().input = "te".into();
    harness.run_steps(2);
    for expected in ["teleport", "temple_guard", "te"] {
        harness.key_press(Key::Tab);
        harness.run_steps(2);
        assert_eq!(harness.state().tab().input, expected);
    }
}

#[test]
fn unmarked_prompt_fires_triggers_once_it_settles() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.scripts.exec(
        "test",
        r#"trigger("^Name: $", function() send("gandalf") end)
            trigger("^Spam> $", function() gag() end)"#,
    );
    let (events, mut sent) = fake_connection(app);
    let partial = |text: &str| Event::Partial(panels::styled_lines(text).remove(0));
    events.send(partial("Name: ")).unwrap();
    harness.run_steps(2);
    assert!(sent.try_recv().is_err(), "must wait for the text to settle");
    std::thread::sleep(PARTIAL_SETTLE + Duration::from_millis(50));
    harness.run_steps(2);
    assert_eq!(sent.try_recv().unwrap(), Command::Send("gandalf".into()));
    // The server then closes the same text as a line: no second firing.
    events
        .send(Event::Prompt(panels::styled_lines("Name: ").remove(0)))
        .unwrap();
    harness.run_steps(2);
    assert!(sent.try_recv().is_err());
    assert_eq!(output_lines(harness.state().tab()), ["Name: "]);

    // A gagged prompt disappears and stays hidden when completed.
    events.send(partial("Spam> ")).unwrap();
    harness.run_steps(2);
    std::thread::sleep(PARTIAL_SETTLE + Duration::from_millis(50));
    harness.run_steps(2);
    events
        .send(Event::Prompt(panels::styled_lines("Spam> ").remove(0)))
        .unwrap();
    harness.run_steps(2);
    assert_eq!(output_lines(harness.state().tab()), ["Name: "]);
}

#[test]
fn client_commands_are_grouped() {
    let read = |line| {
        client_commands::client_command(line).map(|(command, rest)| format!("{command} {rest}"))
    };
    assert_eq!(
        read("/map go Temple of Mota").unwrap(),
        "/go Temple of Mota"
    );
    // Its name before.
    assert_eq!(read("/map walk 3").unwrap(), "/go 3");
    assert_eq!(
        read("/plugin install https://example.org/x.git mine").unwrap(),
        "/install https://example.org/x.git mine"
    );
    // What is not one of the client's is the MUD's.
    assert_eq!(read("/map"), None);
    assert_eq!(read("/where"), None);
    assert_eq!(read("/say hello"), None);
    assert_eq!(read("north"), None);

    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    assert!(app.do_client_command("/map hide") && !app.map_shown());
    assert!(app.do_client_command("/map show") && app.map_shown());
    // Tags: listed, and switched for the session by name.
    app.scripts
        .exec("t", r#"bind("f9", "kick", {tags = {"Combat"}})"#);
    let tag_on = |app: &Tab| app.scripts.switches().tag_on("combat");
    assert!(app.do_client_command("/tag Combat off") && !tag_on(app));
    assert!(app.do_client_command("/tag combat") && tag_on(app));
    assert!(app.do_client_command("/tag"));
    assert!(notes(app).iter().any(|line| line == "combat: on"));
}

/// The palette is what can be done, each by what it does, in the
/// language of the interface: what needs a word more asks for it in the
/// palette's field, named so for a screen reader, and Escape goes back to
/// the search. The command line is the MUD's.
#[test]
fn the_palette_does_what_is_chosen_and_asks_in_its_field_for_what_it_needs() {
    let mut harness = window(None);
    harness.run_steps(2);
    let before = output_lines(harness.state().tab()).len();
    harness.state_mut().tab_mut().toggle_palette();
    harness.run_steps(3);
    harness.get_by_label("Where am I");
    // No command's words are shown, whatever is typed.
    harness.state_mut().tab_mut().palette.query = "map".into();
    harness.run_steps(3);
    assert!(harness.query_by_label_contains("map where").is_none());
    // What it goes to is done there and then: the editor, at a section.
    harness.state_mut().tab_mut().palette.query = "timers".into();
    harness.run_steps(3);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert_eq!((app.view(), app.editor.section), (1, automation::TIMERS));
    app.set_view(0);
    // And what the window does is asked of it.
    app.toggle_palette();
    app.palette.query = "about".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert!(
        harness
            .query_by_label_contains("A modern, accessible MUD client")
            .is_some()
    );
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    // A tag is a switch, said on or off, and what it answers stays in
    // the palette, not over the corner nor among the lines of the game.
    let app = harness.state_mut().tab_mut();
    app.scripts
        .exec("t", r#"bind("f9", "kick", {tags = {"Combat"}})"#);
    app.toggle_palette();
    app.palette.query = "tag combat".into();
    harness.run_steps(2);
    harness.get_by_label("Tag combat (on)");
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(!app.scripts.switches().tag_on("combat") && app.palette.open);
    assert_eq!(
        app.palette.message.clone(),
        Some(("Tag combat off.".to_owned(), false))
    );
    assert!(!app.notices().0.last().unwrap().corner);
    assert_eq!(output_lines(app).len(), before);
    // Run Lua… asks for it in the field, named as what it asks.
    let run = |harness: &mut Harness<'_, App>, code: &str| {
        let app = harness.state_mut().tab_mut();
        app.palette.open = true;
        app.palette.query = "run lua".into();
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(2);
        // The field is named as what it asks, for a screen reader.
        harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "Lua to run");
        harness.state_mut().tab_mut().palette.query = code.into();
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        let app = harness.state().tab();
        assert!(app.palette.open && app.palette.asking.is_none());
        app.palette.message.clone().expect("an answer")
    };
    assert_eq!(
        run(&mut harness, r#"support_gmcp("Comm 1")"#),
        ("Done.".to_owned(), false)
    );
    assert!(
        harness
            .state()
            .tab()
            .wanted_modules()
            .contains(&"Comm 1".to_owned())
    );
    assert_eq!(
        run(&mut harness, r#"echo("two")"#),
        ("two".to_owned(), false)
    );
    let (said, problem) = run(&mut harness, "error('boom')");
    assert!(said.contains("boom") && problem, "{said}");
    assert_eq!(output_lines(harness.state().tab()).len(), before);
    // Enter with nothing typed says what is asked; Escape goes back to
    // the search, and then closes the palette.
    let app = harness.state_mut().tab_mut();
    app.palette.query = "run lua".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let said = harness.state().tab().voice.said.clone();
    assert!(
        said.iter()
            .any(|line| line == "Lua to run: write it, then Enter."),
        "{said:?}"
    );
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert!(app.palette.open && app.palette.asking.is_none());
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().tab().palette.open);
    // The command line is the MUD's: what is typed there, a slash
    // before it or not, is sent.
    let (_events, mut sent) = fake_connection(harness.state_mut().tab_mut());
    harness.state_mut().tab_mut().input = "/map find nowhere".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let sent: Vec<Command> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
    assert!(
        sent.contains(&Command::Send("/map find nowhere".into())),
        "{sent:?}"
    );
}

#[test]
fn the_palette_opens_from_its_field_and_stays_open() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    // Pressed with the pointer, as the keys open it: it is there, and
    // stays there frame after frame.
    harness.get_by_label("Search or run a command").click();
    for frame in 0..6 {
        harness.step();
        assert!(
            harness.state().tab().palette.open,
            "closed again at frame {frame}"
        );
    }
    harness.get_by_label("Where am I");
    // A press anywhere else does close it.
    harness.get_by_label("Menu").click();
    harness.run_steps(3);
    assert!(!harness.state().tab().palette.open);
}

#[test]
fn bound_function_key_sends_its_command() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.scripts
        .exec("test", r#"bind("f5", "look") bind("ctrl+k", "kick")"#);
    let (_events, mut sent) = fake_connection(app);
    harness.key_press(Key::F5);
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::CTRL, Key::K);
    harness.run_steps(2);
    let commands: Vec<Command> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
    assert_eq!(
        commands,
        [Command::Send("look".into()), Command::Send("kick".into())]
    );
}

#[test]
fn bound_keypad_key_sends_its_command_and_types_nothing() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.scripts.exec("test", r#"bind("numpad8", "north")"#);
    let (_events, mut sent) = fake_connection(app);
    // What the window layer and egui each make of one press of keypad 8.
    app.keypad_pressed = vec!["numpad8", "numpad2"];
    harness.event(egui::Event::Text("8".into()));
    harness.event(egui::Event::Text("2".into()));
    harness.run_steps(2);
    assert_eq!(sent.try_recv().unwrap(), Command::Send("north".into()));
    // The bound key typed nothing; the unbound one typed its digit.
    assert_eq!(harness.state().tab().input, "2");
}

#[test]
fn macro_key_is_recorded_from_the_next_key_pressed() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.open_automation(2);
    app.editor.draft.macros.push(Macro {
        send: "look".into(),
        ..Macro::default()
    });
    app.editor.recording_key = true;
    harness.run_steps(3);
    // A plain letter cannot be bound, so it is not taken.
    harness.key_press(Key::A);
    harness.run_steps(2);
    assert!(harness.state().tab().editor.recording_key);
    harness.key_press_modifiers(Modifiers::CTRL | Modifiers::SHIFT, Key::K);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.editor.draft.macros[0].when.key, "ctrl+shift+k");
    assert!(!app.editor.recording_key);
    // The keypad is recorded by its own names.
    app.editor.recording_key = true;
    app.keypad_pressed = vec!["numpad8"];
    harness.run_steps(2);
    assert_eq!(
        harness.state().tab().editor.draft.macros[0].when.key,
        "numpad8"
    );
    // Escape leaves the key as it was.
    harness.state_mut().tab_mut().editor.recording_key = true;
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert_eq!(
        (
            app.editor.draft.macros[0].when.key.as_str(),
            app.editor.recording_key
        ),
        ("numpad8", false)
    );
}

#[test]
fn the_palette_is_a_list_to_page_through_that_says_how_long_it_is() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.state_mut().tab_mut().toggle_palette();
    // Opened empty, it has but the few most used; the map's are many.
    harness.state_mut().tab_mut().palette.query = "map".into();
    harness.run_steps(3);
    let count = harness.state_mut().tab_mut().palette_items().len();
    assert!(count > 12, "too few to page through: {count}");
    harness.get_by_label_contains(&format!("{count} results"));
    let chosen = |harness: &Harness<'_, App>| harness.state().tab().palette.chosen;
    // A page down, then to the end, then round to the first.
    harness.key_press(Key::PageDown);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), 11);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::End);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), count - 1);
    harness.key_press(Key::PageDown);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), count - 1, "a page stops at the end");
    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), 0, "one goes round");
    harness.key_press(Key::PageUp);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), 0);
}
