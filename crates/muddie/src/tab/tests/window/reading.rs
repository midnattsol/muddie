//! What a screen reader is told: names, headings, what the game sends, the keys to review it.

use super::*;

/// Every control has a name a screen reader says, in every view: the game,
/// each section of the editor (with folders, rules, scripts and a folder
/// chosen), what the MUD sends, the layout, the palette and each page of
/// the settings.
#[test]
fn every_control_has_a_name() {
    use egui::accesskit::Role;
    let controls = [
        Role::Button,
        Role::CheckBox,
        Role::Switch,
        Role::TextInput,
        Role::MultilineTextInput,
        Role::SearchInput,
        Role::ComboBox,
        Role::Slider,
        Role::SpinButton,
        Role::Tab,
        Role::TreeItem,
        Role::ListItem,
        Role::MenuItem,
        Role::Splitter,
        Role::Link,
    ];
    let seen = std::cell::Cell::new(0);
    let unnamed = |harness: &Harness<'_, App>, view: &str| -> Vec<String> {
        (harness.query_all(egui_kittest::kittest::by()))
            .inspect(|_| seen.set(seen.get() + 1))
            .filter(|node| controls.contains(&node.accesskit_node().role()))
            .filter(|node| {
                let node = node.accesskit_node();
                let label = node.label().unwrap_or_default();
                label.trim().is_empty() && !node.is_hidden()
            })
            .map(|node| {
                format!(
                    "{view}: {:?} {:?}",
                    node.accesskit_node().role(),
                    node.accesskit_node().value()
                )
            })
            .collect()
    };
    let mut harness = connected_demo_window();
    harness.set_size(egui::vec2(1400.0, 900.0));
    let app = harness.state_mut().tab_mut();
    app.rules
        .triggers
        .push(trigger("tells you", |rule| rule.folder = "Social".into()));
    app.rules
        .aliases
        .push(alias("k", |rule| rule.folder = "Combat".into()));
    app.saw_data("Char.Vitals", false, r#"{"hp": 80, "maxhp": 100}"#);
    harness.run_steps(3);
    let mut missing = unnamed(&harness, "game");
    for section in automation::Section::ALL {
        harness.state_mut().tab_mut().open_automation(section);
        harness.run_steps(3);
        missing.extend(unnamed(&harness, &format!("editor section {section:?}")));
    }
    harness
        .state_mut()
        .tab_mut()
        .open_automation(automation::TRIGGERS);
    harness.state_mut().tab_mut().choose_folder("Social");
    harness.run_steps(3);
    missing.extend(unnamed(&harness, "a folder"));
    harness.state_mut().tab_mut().set_view(View::Layout);
    harness.run_steps(3);
    missing.extend(unnamed(&harness, "layout"));
    harness.state_mut().tab_mut().set_view(View::Game);
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run_steps(3);
    missing.extend(unnamed(&harness, "palette"));
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    for section in crate::app::Section::ALL {
        harness.state_mut().open_settings(section);
        harness.run_steps(3);
        missing.extend(unnamed(&harness, section.name()));
    }
    assert!(
        seen.get() > 1000,
        "only {} nodes were looked at",
        seen.get()
    );
    assert!(
        missing.is_empty(),
        "controls with no name:\n{}",
        missing.join("\n")
    );
}

/// Titles and captions are headings to a screen reader, in the case they
/// are written in, though captions are drawn in capitals.
#[test]
fn titles_and_captions_are_headings() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.open_automation(automation::TRIGGERS);
    app.editor.draft.triggers.push(Default::default());
    harness.run_steps(3);
    harness.get_by_role_and_label(egui::accesskit::Role::Heading, "Editor");
    harness.get_by_role_and_label(egui::accesskit::Role::Heading, "When");
}

/// What the game sends is read as the player wants it: a line a trigger
/// silences is shown and not read, prompts can be left unread, what a
/// trigger moves to a panel is read with the panel's name, and the review
/// keys go through any panel of text, saying where they stop.
#[test]
fn the_game_is_read_as_wanted_and_any_panel_is_read_back() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    app.rules
        .triggers
        .push(trigger("spam", |rule| rule.when.silence = true));
    app.rules.triggers.push(trigger("tells you", |rule| {
        rule.when.panel = "Tells".into();
        rule.when.gag = true;
    }));
    for (name, code) in app.rules.chunks() {
        app.scripts.exec(&name, &code);
    }
    let mut settings = app.settings.clone();
    settings.speech_prompts = false;
    app.voice.configure(&settings);
    app.voice.force_voice();
    let (events, _sent) = fake_connection(app);
    let line = |text: &str| Event::Line(panels::styled_lines(text).remove(0));
    events.send(line("some spam here")).unwrap();
    events
        .send(Event::Prompt(panels::styled_lines("<100hp>").remove(0)))
        .unwrap();
    events.send(line("Ana tells you: hola")).unwrap();
    events.send(line("Bob tells you: adiós")).unwrap();
    harness.run_steps(3);
    let said = |harness: &Harness<'_, App>| harness.state().tab().voice.said.clone();
    assert_eq!(
        said(&harness),
        ["Tells: Ana tells you: hola", "Tells: Bob tells you: adiós"]
    );
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.output.text(0), "some spam here");
    app.voice.said.clear();
    // Alt+Shift reads the panel back, the newest first, and says so at
    // the start of it.
    let review = egui::Modifiers::ALT | egui::Modifiers::SHIFT;
    harness.key_press_modifiers(review, egui::Key::ArrowUp);
    harness.key_press_modifiers(review, egui::Key::ArrowUp);
    harness.key_press_modifiers(review, egui::Key::ArrowUp);
    harness.run_steps(1);
    assert_eq!(
        said(&harness),
        [
            "Bob tells you: adiós",
            "Ana tells you: hola",
            "First line: Ana tells you: hola"
        ]
    );
    // Alt alone reads the game output, not the panel.
    harness.state_mut().tab_mut().voice.said.clear();
    harness.key_press_modifiers(egui::Modifiers::ALT, egui::Key::ArrowUp);
    harness.run_steps(1);
    assert_eq!(said(&harness), ["<100hp>"]);
    // Alt+Shift+Right names the panel it goes to.
    harness.state_mut().tab_mut().voice.said.clear();
    harness.key_press_modifiers(review, egui::Key::ArrowRight);
    harness.run_steps(1);
    assert_eq!(said(&harness), ["Tells, 2 lines."]);
}

/// With new lines interrupting, a burst of them is read whole: only its
/// first line cuts short what was being said.
#[test]
fn a_burst_of_lines_interrupts_once() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let mut settings = app.settings.clone();
    settings.speech_interrupts = true;
    app.voice.configure(&settings);
    app.voice.force_voice();
    let (events, _sent) = fake_connection(app);
    for text in ["The Square", "A wide square.", "Exits: north."] {
        events
            .send(Event::Line(panels::styled_lines(text).remove(0)))
            .unwrap();
    }
    harness.run_steps(3);
    let voice = &harness.state().tab().voice;
    assert_eq!(
        voice.said,
        ["The Square", "A wide square.", "Exits: north."]
    );
    assert_eq!(voice.cut, ["The Square"]);
}

#[test]
fn ctrl_silences_the_sessions_out_of_sight_too() {
    let mut harness = window(None);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::T);
    harness.run_steps(3);
    let stopped = |harness: &Harness<App>| -> Vec<usize> {
        let tabs = harness.state().tabs();
        tabs.iter().map(|tab| tab.voice.stopped).collect()
    };
    assert_eq!(harness.state().tabs().len(), 2);
    let before = stopped(&harness);
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::CTRL));
    harness.run_steps(2);
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    harness.run_steps(2);
    let after = stopped(&harness);
    assert!(
        before
            .iter()
            .zip(&after)
            .all(|(before, after)| after > before),
        "{before:?} {after:?}"
    );
}

#[test]
fn a_shortcut_is_pressed_with_other_keys_chosen_in_the_settings() {
    let mut harness = window(None);
    harness.state_mut().tab_mut().voice.force_voice();
    harness.state_mut().open_settings(crate::app::Section::Keys);
    harness.run_steps(3);
    let own = crate::shortcuts::PALETTE.written();
    harness.get_by_label(&format!("Palette: {own}")).click();
    harness.run_steps(2);
    let said = |harness: &Harness<App>| harness.state().tab().voice.said.last().cloned();
    // Keys Muddie has for something else are not taken.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::E);
    harness.run_steps(2);
    let editor = crate::shortcuts::SWITCH_VIEW.all_written();
    assert_eq!(
        said(&harness).as_deref(),
        Some(format!("{editor} is already used: Between the game and the editor.").as_str())
    );
    // Keys that type are not either.
    harness.key_press(Key::P);
    harness.run_steps(2);
    assert_eq!(
        said(&harness).as_deref(),
        Some("Those keys type: hold Ctrl or Alt with them, or use a function key or the keypad.")
    );
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    harness.run_steps(2);
    let chosen = crate::shortcuts::PALETTE.written();
    assert_ne!(chosen, own);
    assert_eq!(
        said(&harness).as_deref(),
        Some(format!("Palette: {chosen}.").as_str())
    );
    assert!(harness.state().settings().shortcuts.contains_key("palette"));
    // The palette opens with them, and no more with its own.
    harness.state_mut().close_settings_for_tests();
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run_steps(2);
    assert!(!harness.state().tab().palette.open);
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    harness.run_steps(2);
    assert!(harness.state().tab().palette.open);
    // Its menu adds more keys, and gives it its own back.
    harness.key_press(Key::Escape);
    harness.state_mut().open_settings(crate::app::Section::Keys);
    harness.run_steps(3);
    let menu = |harness: &mut Harness<App>, item: &str| {
        let label = format!("Palette: {}", crate::shortcuts::PALETTE.all_written());
        harness.get_by_label(&label).focus();
        harness.run_steps(1);
        harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
        harness.run_steps(3);
        harness.get_by_label(item).click();
        harness.run_steps(2);
    };
    menu(&mut harness, "Add keys…");
    harness.key_press(Key::F9);
    harness.run_steps(2);
    assert_eq!(
        said(&harness).as_deref(),
        Some(format!("Palette: {chosen}, F9.").as_str())
    );
    // Cmd on a Mac, where Ctrl is not what the palette is held with.
    let kept = match cfg!(target_os = "macos") {
        true => "shift+cmd+p, f9",
        false => "ctrl+shift+p, f9",
    };
    assert_eq!(harness.state().settings().shortcuts["palette"], kept);
    menu(&mut harness, "Reset");
    assert!(harness.state().settings().shortcuts.is_empty());
    assert_eq!(crate::shortcuts::PALETTE.written(), own);
}

/// The messages open with their shortcut, with the keyboard on the newest,
/// read whole; Enter copies it, and the arrows go to the others.
#[test]
fn the_messages_are_read_and_copied_with_the_keyboard() {
    let mut harness = window(None);
    harness.state_mut().tab_mut().voice.force_voice();
    let app = harness.state_mut().tab_mut();
    app.note("First of all.");
    app.note("Then this.");
    harness.run_steps(2);
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::N);
    harness.run_steps(3);
    // Shown over the corner too: the one in the list is an item of it.
    let item = egui::accesskit::Role::ListItem;
    assert!(
        harness
            .get_by_role_and_label(item, "Then this.")
            .is_focused()
    );
    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(item, "First of all.")
            .is_focused()
    );
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let said = harness.state().tab().voice.said.last().cloned();
    assert_eq!(said.as_deref(), Some("Copied."));
}

/// The output as a text a screen reader reads as it reads a document:
/// Tab from the empty command line goes to it, with the keyboard on the
/// newest line; what comes is added to it; typing goes back to the line.
#[test]
fn the_output_is_a_text_to_read_reached_with_tab() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    for line in ["The Temple.", "An orc is here."] {
        events
            .send(Event::Line(panels::styled_lines(line).remove(0)))
            .unwrap();
    }
    harness.run_steps(3);
    let text = egui::accesskit::Role::MultilineTextInput;
    harness.key_press(Key::Tab);
    harness.run_steps(3);
    let output = harness.get_by_role_and_label(text, "Output");
    assert!(output.is_focused());
    assert!(output.accesskit_node().is_read_only());
    assert_eq!(
        harness.state().tab().reader.text(),
        "The Temple.\nAn orc is here."
    );
    // What comes is added, and the place read stays.
    events
        .send(Event::Line(
            panels::styled_lines("The orc leaves.").remove(0),
        ))
        .unwrap();
    harness.run_steps(3);
    assert!(
        harness
            .state()
            .tab()
            .reader
            .text()
            .ends_with("\nThe orc leaves.")
    );
    assert!(harness.get_by_role_and_label(text, "Output").is_focused());
    // Typing goes back to the command line, with what was typed.
    harness.event(egui::Event::Text("n".into()));
    harness.run_steps(3);
    let line = harness.get_by_label("Command");
    assert!(line.is_focused());
    assert_eq!(harness.state().tab().input, "n");
    assert!(harness.query_by_role_and_label(text, "Output").is_none());
}

/// With a screen reader, the welcome screen says what to do first, once;
/// and while playing, F1 says the keys to know.
#[test]
fn what_to_do_is_said_on_welcome_and_with_f1() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    app.voice.pretend_reader = true;
    // On the welcome screen, as a new session opens.
    app.started = false;
    harness.run_steps(3);
    harness.run_steps(3);
    let said = |harness: &Harness<App>| harness.state().tab().voice.said.clone();
    let welcome = said(&harness)
        .iter()
        .filter(|said| said.starts_with("Welcome to Muddie."))
        .count();
    assert_eq!(welcome, 1, "{:?}", said(&harness));
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    harness.key_press(Key::F1);
    harness.run_steps(2);
    let last = said(&harness).last().cloned().unwrap_or_default();
    assert!(
        last.starts_with("Tab, with nothing typed, reads the output"),
        "{last}"
    );
}

#[test]
fn ctrl_tab_goes_from_session_to_session_saying_which() {
    let mut harness = window(None);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::T);
    harness.run_steps(3);
    harness.state_mut().tab_mut().host = "mud.example.org".into();
    harness.state_mut().tab_mut().voice.force_voice();
    harness.key_press_modifiers(Modifiers::CTRL, Key::Tab);
    harness.run_steps(3);
    harness.state_mut().tab_mut().voice.force_voice();
    let said = |harness: &Harness<App>| harness.state().tab().voice.said.last().cloned();
    harness.key_press_modifiers(Modifiers::CTRL, Key::Tab);
    harness.run_steps(3);
    assert_eq!(
        said(&harness).as_deref(),
        Some("Session 2 of 2: mud.example.org")
    );
    harness.key_press_modifiers(Modifiers::CTRL | Modifiers::SHIFT, Key::Tab);
    harness.run_steps(3);
    assert_eq!(
        said(&harness).as_deref(),
        Some("Session 1 of 2: New session")
    );
    // And the keyboard is in its command line.
    let focused = (harness.query_all(egui_kittest::kittest::by()))
        .find(|node| node.is_focused())
        .map(|node| node.accesskit_node().role());
    assert_eq!(focused, Some(egui::accesskit::Role::TextInput));
}
