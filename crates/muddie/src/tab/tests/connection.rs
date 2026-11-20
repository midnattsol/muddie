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
        output_lines(app)
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
    harness.key_press_modifiers(egui::Modifiers::ALT, Key::R);
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

#[test]
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
    assert!(app.do_client_command("/log"));
    let (events, _sent) = fake_connection(app);
    events
        .send(Event::Line(
            panels::styled_lines("\x1b[31mAn orc\x1b[0m arrives").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    let path = app.log.as_ref().unwrap().path().to_owned();
    assert!(app.do_client_command("/log"));
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
    // The arrows browse from it as the newest command, not as a search.
    harness.key_press(Key::ArrowUp);
    harness.run_steps(3);
    assert_eq!(harness.state().tab().input, "north");
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
