//! Connecting, sending, logging in, keeping and logging a session.

use super::*;

#[test]
fn typed_commands_are_split_and_reach_the_server() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (_events, mut sent) = fake_connection(app);
    app.input = "n;n;look".into();
    app.submit();
    let commands: Vec<Command> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
    let expected = ["n", "n", "look"].map(|command| Command::Send(command.into()));
    assert_eq!(commands, expected);
}

#[test]
fn lost_connection_is_retried_but_a_closed_one_is_not() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.host = "127.0.0.1".into();
    let (events, _sent) = fake_connection(app);
    events
        .send(Event::Disconnected(Some("connection reset".into())))
        .unwrap();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert!(app.reconnect_at.is_some());
    assert!(
        notes(app)
            .iter()
            .any(|line| line.starts_with("Reconnecting in 1 "))
    );

    // Closed by the server without an error: nothing to retry.
    app.reconnect_at = None;
    let (events, _sent) = fake_connection(app);
    events.send(Event::Disconnected(None)).unwrap();
    harness.run_steps(2);
    assert!(harness.state().tab().reconnect_at.is_none());

    // Asked for by the user: not retried even if it ends in an error.
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    app.disconnect();
    events
        .send(Event::Disconnected(Some("broken pipe".into())))
        .unwrap();
    harness.run_steps(2);
    assert!(harness.state().tab().reconnect_at.is_none());
}

#[test]
fn idle_connection_gets_a_keep_alive() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.prefs.keepalive_seconds = 1;
    let (_events, mut sent) = fake_connection(app);
    app.last_activity = Instant::now() - Duration::from_secs(2);
    harness.run_steps(2);
    assert_eq!(sent.try_recv().unwrap(), Command::KeepAlive);
    assert!(sent.try_recv().is_err(), "only one until idle again");
}

#[test]
fn reconnecting_closes_the_connection_and_opens_it_again() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.host = "127.0.0.1".into();
    app.port = "1".into();
    let (events, mut sent) = fake_connection(app);
    // Alt+R asks the connection to close...
    let (held, key) = crate::shortcuts::RECONNECT.first();
    harness.key_press_modifiers(held, key);
    harness.run_steps(2);
    assert_eq!(sent.try_recv().ok(), Some(Command::Disconnect));
    assert!(harness.state().tab().reconnect_wanted);
    // ...and once it has, it is to connect again at once, though it
    // was the user who closed it.
    events.send(Event::Disconnected(None)).unwrap();
    let app = harness.state_mut().tab_mut();
    app.handle_events();
    assert!(!app.reconnect_wanted && app.connection.is_none());
    assert!(app.reconnect_at.is_some_and(|at| at <= Instant::now()));
}

#[test]
fn character_name_answers_the_first_prompt() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.character = "Gandalf".into();
    let (events, mut sent) = fake_connection(app);
    events.send(Event::Connected).unwrap();
    events
        .send(Event::Line(
            panels::styled_lines("Welcome to the realm").remove(0),
        ))
        .unwrap();
    events
        .send(Event::Partial(
            panels::styled_lines("By what name are you known? ").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    assert!(sent.try_recv().is_err(), "a banner line is not a prompt");
    std::thread::sleep(PARTIAL_SETTLE + Duration::from_millis(50));
    harness.run_steps(2);
    assert_eq!(sent.try_recv().unwrap(), Command::Send("Gandalf".into()));
    // Without a profile there is no password to send.
    events.send(Event::InputHidden(true)).unwrap();
    harness.run_steps(2);
    assert!(sent.try_recv().is_err());
    assert_eq!(harness.state().tab().login, Login::Idle);
}

/// On a Mac, Option types characters (@ on a Spanish keyboard), and what it
/// types is left alone.
#[test]
#[cfg(not(target_os = "macos"))]
fn alt_r_reconnects_without_typing_an_r() {
    let mut harness = window(None);
    harness.state_mut().tab_mut().started = true;
    harness.state_mut().tab_mut().input = "kill".into();
    harness.run_steps(3);
    // What the window gives for Alt+R: the key, then the letter.
    harness.event(egui::Event::Key {
        key: Key::R,
        physical_key: Some(Key::R),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::ALT,
    });
    harness.event(egui::Event::Text("r".into()));
    harness.run_steps(3);
    assert_eq!(harness.state().tab().input, "kill");
}

#[test]
fn session_is_logged_when_asked() {
    let dir = std::env::temp_dir().join(format!("muddie-app-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    let app = harness.state_mut().tab_mut();
    app.host = "mud.example.org".into();
    app.toggle_log();
    let (events, _sent) = fake_connection(app);
    events
        .send(Event::Line(
            panels::styled_lines("\x1b[31mAn orc\x1b[0m arrives").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    let path = app.log.as_ref().unwrap().path().to_owned();
    app.toggle_log();
    assert!(app.log.is_none());
    let log = std::fs::read_to_string(path).unwrap();
    assert!(log.contains("An orc arrives"), "{log}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn sent_command_stays_selected_to_repeat_or_type_over() {
    // Only when asked for: it is off to begin with.
    let mut harness = window(None);
    harness.state_mut().tab_mut().settings.keep_command = true;
    let (_events, mut sent) = fake_connection(harness.state_mut().tab_mut());
    let send = |text: &str| Command::Send(text.into());
    harness.state_mut().tab_mut().input = "north".into();
    harness.run_steps(2);
    // Enter sends it and leaves it there; Enter again sends it again.
    for _ in 0..3 {
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        assert_eq!(sent.try_recv().unwrap(), send("north"));
        assert_eq!(harness.state().tab().input, "north");
    }
    // It is selected: what is typed takes its place.
    harness.get_by_label("Command").type_text("look");
    harness.run_steps(3);
    assert_eq!(harness.state().tab().input, "look");
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(sent.try_recv().unwrap(), send("look"));
    // Gone to read the output and back, it is still all chosen: what is
    // typed takes its place.
    harness.ctx.memory_mut(|memory| {
        if let Some(focused) = memory.focused() {
            memory.surrender_focus(focused);
        }
    });
    harness.run_steps(3);
    harness.get_by_label("Command").type_text("west");
    harness.run_steps(3);
    assert_eq!(harness.state().tab().input, "west");
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(sent.try_recv().unwrap(), send("west"));
    harness.state_mut().tab_mut().input = "look".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(sent.try_recv().unwrap(), send("look"));
    // The arrows browse from it as the newest command, not as a search.
    harness.key_press(Key::ArrowUp);
    harness.run_steps(3);
    assert_eq!(harness.state().tab().input, "west");
    // With the setting off, the line empties as it used to.
    let app = harness.state_mut().tab_mut();
    app.settings.keep_command = false;
    app.input = "south".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(sent.try_recv().unwrap(), send("south"));
    assert_eq!(harness.state().tab().input, "");
}

/// A click on the output, to choose text in it, leaves the keyboard in the
/// command line, in every frame: it does not go and come back, which drew
/// the line a frame without it, and made the command kept there, chosen
/// whole, blink.
#[test]
fn a_click_on_the_output_leaves_the_keyboard_in_the_command_line() {
    let mut harness = window(None);
    let (_events, _sent) = fake_connection(harness.state_mut().tab_mut());
    let app = harness.state_mut().tab_mut();
    app.settings.keep_command = true;
    app.show_local("An orc arrives.");
    app.input = "north".into();
    harness.run_steps(3);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    // Drawn with the keyboard in it, its text is chosen as it is drawn.
    let chosen = |harness: &Harness<'_, App>| {
        let command = harness.get_by_label("Command");
        let node = command.accesskit_node();
        node.is_focused() && node.data().text_selection().is_some()
    };
    assert!(chosen(&harness));
    harness.get_by_label("An orc arrives.").click();
    for _ in 0..4 {
        harness.step();
        assert!(
            chosen(&harness),
            "the command line was drawn without the keyboard"
        );
    }
}

/// The output's menu, by the other button, copies the line it is opened
/// on, or all the output; and the palette copies all of it too.
#[test]
fn the_outputs_menu_copies_a_line_or_all_of_it() {
    let mut harness = window(None);
    let (_events, _sent) = fake_connection(harness.state_mut().tab_mut());
    let app = harness.state_mut().tab_mut();
    app.show_local("An orc arrives.");
    app.show_local("The orc leaves.");
    harness.run_steps(3);
    let copied = |harness: &Harness<'_, App>| {
        (harness.output().platform_output.commands.iter()).find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
    };
    harness.get_by_label("An orc arrives.").click_secondary();
    harness.run_steps(2);
    harness.get_by_label("Copy line").click();
    harness.step();
    assert_eq!(copied(&harness).as_deref(), Some("An orc arrives."));
    harness.run_steps(2);
    harness.get_by_label("The orc leaves.").click_secondary();
    harness.run_steps(2);
    harness.get_by_label("Copy all output").click();
    harness.step();
    let all = copied(&harness).unwrap_or_default();
    assert!(all.contains("An orc arrives.\nThe orc leaves.\n"), "{all}");
}

/// A line of the output keeps who it is as old lines go and new ones
/// come: text chosen in it stays chosen in it, not in the line that took
/// its place.
#[test]
fn a_line_of_the_output_stays_itself_as_old_lines_go() {
    let mut harness = window(None);
    let (_events, _sent) = fake_connection(harness.state_mut().tab_mut());
    let app = harness.state_mut().tab_mut();
    app.output.restyle(100, false);
    for n in 0..100 {
        app.show_local(&format!("line {n}"));
    }
    harness.run_steps(3);
    let id = |harness: &Harness<'_, App>| harness.get_by_label("line 99").accesskit_node().id();
    let before = id(&harness);
    harness.state_mut().tab_mut().show_local("line 100");
    harness.run_steps(3);
    assert!(
        harness.query_by_label("line 0").is_none(),
        "the oldest went"
    );
    assert_eq!(id(&harness), before);
}

/// Text chosen in the output by dragging over it is copied from its menu,
/// as Ctrl+C copies it.
#[test]
fn text_chosen_in_the_output_is_copied_from_its_menu() {
    let mut harness = window(None);
    let (_events, _sent) = fake_connection(harness.state_mut().tab_mut());
    harness.state_mut().tab_mut().show_local("An orc arrives.");
    harness.run_steps(3);
    let line = harness.get_by_label("An orc arrives.").rect();
    let (from, to) = (
        line.left_center() + egui::vec2(1.0, 0.0),
        line.right_center() - egui::vec2(1.0, 0.0),
    );
    let press = |harness: &mut Harness<'_, App>, pos, pressed| {
        harness.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        harness.step();
    };
    harness.hover_at(from);
    harness.step();
    press(&mut harness, from, true);
    for step in 1..=5 {
        harness.hover_at(from + (to - from) * (step as f32 / 5.0));
        harness.step();
    }
    press(&mut harness, to, false);
    harness.run_steps(2);
    harness.get_by_label("An orc arrives.").click_secondary();
    harness.run_steps(2);
    harness.get_by_label("Copy").click();
    harness.step();
    let copied =
        (harness.output().platform_output.commands.iter()).find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        });
    assert_eq!(copied.as_deref(), Some("An orc arrives."));
}

#[test]
fn what_is_typed_when_a_password_is_asked_for_is_kept_secret() {
    use super::super::events::asks_for_password;
    assert!(asks_for_password("Introduce tu contraseña: "));
    assert!(asks_for_password("Password:"));
    assert!(!asks_for_password("Tu contraseña ha sido cambiada."));
    assert!(!asks_for_password("Nombre:"));
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, mut sent) = fake_connection(app);
    // The MUD asks without hiding what is typed: it is sent as a secret,
    // shown as dots, and kept out of the history and the scripts.
    events
        .send(Event::Partial(
            panels::styled_lines("Contraseña: ").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    assert!(harness.state().tab().typing_secret());
    let app = harness.state_mut().tab_mut();
    app.input = "hunter2".into();
    app.submit();
    assert_eq!(
        sent.try_recv().unwrap(),
        Command::SendSecret("hunter2".into())
    );
    assert!(!app.typing_secret());
    assert!(app.history.entries().iter().all(|entry| entry != "hunter2"));
    // What is typed after is as ever.
    app.input = "look".into();
    app.submit();
    assert_eq!(sent.try_recv().unwrap(), Command::Send("look".into()));
}
