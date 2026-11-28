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
            trigger("^Spam> $", function(m, line) line:hide() end)"#,
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

/// A prompt with no end comes again with each packet of data that arrives
/// while it waits: kept from being read once, it is not read then, and of
/// what comes after it on its line only that is read.
#[test]
fn a_silenced_prompt_is_not_read_when_data_comes_after_it() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    (app.scripts).exec(
        "test",
        r#"trigger("^HP .*> $", function(m, line) line:mute() end)"#,
    );
    app.voice.away = false;
    let (events, _sent) = fake_connection(app);
    let partial = |text: &str| Event::Partial(panels::styled_lines(text).remove(0));
    let settle = |harness: &mut Harness<'_, App>| {
        harness.run_steps(2);
        std::thread::sleep(PARTIAL_SETTLE + Duration::from_millis(50));
        harness.run_steps(2);
    };
    harness.state_mut().tab_mut().voice.said.clear();
    for _ in 0..3 {
        events.send(partial("HP 10> ")).unwrap();
        settle(&mut harness);
    }
    assert_eq!(harness.state().tab().voice.said, [] as [String; 0]);
    let line = panels::styled_lines("HP 10> A goblin arrives.").remove(0);
    events.send(Event::Line(line)).unwrap();
    harness.run_steps(2);
    assert_eq!(harness.state().tab().voice.said, ["A goblin arrives."]);
    // Read as any other once it says something else.
    events.send(partial("Name: ")).unwrap();
    settle(&mut harness);
    assert_eq!(harness.state().tab().voice.said[1..], ["Name:"]);
}

/// A script reads something else in place of a line or of a prompt: once,
/// as the line would be read, or not.
#[test]
fn a_line_is_read_as_a_script_says() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    (app.scripts).exec(
        "test",
        r#"trigger("^HP (\\d+)> (.*)$", function(m, line)
            line:read(m[2] ~= "" and m[2] or `life {m[1]}`)
        end)"#,
    );
    let (events, _sent) = fake_connection(app);
    app.voice.said.clear();
    let line = |text: &str| panels::styled_lines(text).remove(0);
    let settle = |harness: &mut Harness<'_, App>| {
        harness.run_steps(2);
        std::thread::sleep(PARTIAL_SETTLE + Duration::from_millis(50));
        harness.run_steps(2);
        std::mem::take(&mut harness.state_mut().tab_mut().voice.said)
    };
    events.send(Event::Line(line("HP 10> A goblin."))).unwrap();
    assert_eq!(settle(&mut harness), ["A goblin."]);
    // A prompt not ended, as it settles; ended as it was, no more; and with
    // something after it, that.
    events.send(Event::Partial(line("HP 9> "))).unwrap();
    assert_eq!(settle(&mut harness), ["life 9"]);
    events.send(Event::Partial(line("HP 9> "))).unwrap();
    assert_eq!(settle(&mut harness), [] as [String; 0]);
    events.send(Event::Line(line("HP 9> "))).unwrap();
    assert_eq!(settle(&mut harness), [] as [String; 0]);
    events.send(Event::Partial(line("HP 8> "))).unwrap();
    assert_eq!(settle(&mut harness), ["life 8"]);
    events.send(Event::Line(line("HP 8> An orc."))).unwrap();
    assert_eq!(settle(&mut harness), ["An orc."]);
    // One the MUD marks.
    events.send(Event::Prompt(line("HP 7> "))).unwrap();
    assert_eq!(settle(&mut harness), ["life 7"]);
    // Prompts not read, what is read in their place is not either.
    let mut settings = harness.state().tab().settings.clone();
    settings.speech_prompts = false;
    harness.state_mut().tab_mut().voice.configure(&settings);
    events.send(Event::Prompt(line("HP 6> "))).unwrap();
    events.send(Event::Partial(line("HP 5> "))).unwrap();
    assert_eq!(settle(&mut harness), [] as [String; 0]);
    events.send(Event::Line(line("HP 5> A wolf."))).unwrap();
    assert_eq!(settle(&mut harness), ["A wolf."]);
    // The game not read, nothing is.
    harness.state_mut().tab_mut().voice.set_muted(true);
    events.send(Event::Line(line("HP 4> A bear."))).unwrap();
    assert_eq!(settle(&mut harness), [] as [String; 0]);
}

/// What a script says is of the game: not said while the game is not read.
#[test]
fn a_script_says_nothing_while_the_game_is_not_read() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.said.clear();
    let say = |app: &mut Tab| {
        app.scripts
            .exec("test", r#"speak("one") speak("two", {aloud = "cut"})"#);
        app.run_actions();
    };
    say(app);
    assert_eq!(app.voice.said, ["one", "two"]);
    app.voice.set_muted(true);
    say(app);
    assert_eq!(app.voice.said, ["one", "two"]);
}

#[test]
fn client_commands_are_carried_out_and_the_rest_is_the_mud_s() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    // What is not one of the client's is the MUD's.
    for line in ["/map", "/map walk 3", "/where", "/say hello", "north"] {
        assert!(!app.do_client_command(line), "{line}");
    }
    assert!(app.do_client_command("/map stop"));
    // Removing a plugin is asked once more, however it is asked for.
    assert!(app.do_client_command("/plugin remove extra"));
    assert_eq!(
        app.take_app_request(),
        Some(AppRequest::RemovePlugin {
            name: "extra".to_owned(),
            profile: None
        })
    );
    // Tags, switched for the session by name.
    app.scripts
        .exec("t", r#"macro("f9", "kick", {tags = {"Combat"}})"#);
    let tag_on = |app: &Tab| app.scripts.switches().tag_on("combat");
    assert!(app.do_client_command("/tag Combat off") && !tag_on(app));
    assert!(app.do_client_command("/tag combat on") && tag_on(app));
    assert!(app.do_client_command("/tag Nadie on"));
    assert_eq!(
        notes(app).last().unwrap(),
        "There is no tag called Nadie on."
    );
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
    assert_eq!(
        (app.view(), app.editor.section),
        (View::Editor, automation::TIMERS)
    );
    app.set_view(View::Game);
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
        .exec("t", r#"macro("f9", "kick", {tags = {"Combat"}})"#);
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
    assert!(!app.messages().0.last().unwrap().corner);
    assert_eq!(output_lines(app).len(), before);
    // Run Luau… asks for it in the field, named as what it asks.
    let run = |harness: &mut Harness<'_, App>, code: &str| {
        let app = harness.state_mut().tab_mut();
        app.palette.open = true;
        app.palette.query = "run luau".into();
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(2);
        // The field is named as what it asks, for a screen reader.
        harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "Luau to run");
        harness.state_mut().tab_mut().palette.query = code.into();
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        let app = harness.state().tab();
        assert!(app.palette.open && app.palette.asking.is_none());
        app.palette.message.clone().expect("an answer")
    };
    assert_eq!(
        run(&mut harness, r#"gmcp.support("Comm 1")"#),
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
    app.palette.query = "run luau".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let said = harness.state().tab().voice.said.clone();
    assert!(
        said.iter()
            .any(|line| line == "Luau to run: write it, then Enter."),
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
    harness.get_by_label("Palette").click();
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
        .exec("test", r#"macro("f5", "look") macro("ctrl+k", "kick")"#);
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
    app.scripts.exec("test", r#"macro("numpad8", "north")"#);
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
    app.open_automation(automation::MACROS);
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

/// Every key there is can be bound, held with Ctrl: the engine knows them
/// all by the names they have here.
#[test]
fn every_key_is_known_to_scripts() {
    for key in Key::ALL {
        let name = format!("ctrl+{}", key.name().to_lowercase());
        assert_eq!(muddie_script::bindable(&name), Ok(()));
    }
    for name in crate::keypad::names() {
        assert_eq!(muddie_script::bindable(name), Ok(()));
    }
}

/// What is sent is stopped or changed by who listens to it, wherever it
/// comes from; not what is told of no one.
#[test]
fn what_is_sent_goes_as_who_listens_leaves_it() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (_events, mut sent) = fake_connection(app);
    app.scripts.exec(
        "t",
        r#"on("send", function(text)
            if text == "quit" then return false end
            send("told " .. text)
            return text:upper()
        end)
        alias("^go$", function() send("north") send("quit") end)"#,
    );
    app.input = "go".into();
    app.submit();
    app.input = "look".into();
    app.submit();
    app.character = "ana".into();
    app.login = Login::Name(Instant::now());
    app.settled_partial = Some(("Name: ".into(), None));
    app.login_name(false);
    let mut lines = Vec::new();
    while let Ok(command) = sent.try_recv() {
        if let Command::Send(line) = command {
            lines.push(line);
        }
    }
    assert_eq!(lines, ["NORTH", "told north", "LOOK", "told look", "ana"]);
}

/// What a script feeds is taken as what the MUD sends, and one that feeds
/// itself does not stop the rest.
#[test]
fn what_a_script_feeds_is_taken_as_the_mud_s() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.scripts.exec(
        "t",
        r#"trigger("^An (orc)", function(m, line) line:link("kill " .. m[1]) speak("seen") end)
        trigger("^again$", function() session.feed("again") end)"#,
    );
    app.voice.said.clear();
    app.scripts.exec(
        "t",
        r#"session.feed("\27[31mAn orc\27[0m attacks!\nIt hits you.") session.feed("again")"#,
    );
    app.run_actions();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let lines = output_lines(app);
    assert_eq!(lines[..2], ["An orc attacks!", "It hits you."]);
    assert!(lines.len() > 3 && lines.len() < 20, "{lines:?}");
    assert!(app.voice.said.contains(&"seen".to_owned()));
    app.scripts.unload("t");
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    // Told of what is typed, but not of a password.
    app.scripts.exec("t", r#"session.input = "look" hush()"#);
    let stopped = app.voice.stopped;
    app.run_actions();
    assert_eq!(
        (app.input.as_str(), app.voice.stopped),
        ("look", stopped + 1)
    );
    app.input_hidden = true;
    app.scripts.exec("t", r#"session.input = "x""#);
    app.run_actions();
    assert_eq!(app.input, "look");
    app.voice.away = true;
    app.scripts.exec("t", "hush()");
    app.run_actions();
    assert_eq!(app.voice.stopped, stopped + 1);
}
