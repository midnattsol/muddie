//! Tests of a session, driven as a user would: through the window, with
//! the connection stood in for.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use muddie_mapper::Pos;

use super::input::PARTIAL_SETTLE;
use super::*;
use crate::app::App;

/// A window that is not drawn, using `profiles` as its configuration.
fn window<'a>(profiles: Option<Profiles>) -> Harness<'a, App> {
    // Tests read the interface's text, so it must not follow the
    // language of whatever machine they run on.
    let english = |cc: &eframe::CreationContext<'_>, startup| {
        let app = App::new(cc, startup, Default::default());
        crate::i18n::set_language("en");
        app
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 700.0))
        .build_eframe(|cc| {
            let startup = Startup {
                profiles,
                profile: None,
                target: None,
                speech: false,
                scripts: Vec::new(),
            };
            english(cc, startup)
        });
    // A new session opens on the welcome screen; most tests are about
    // a session in use.
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(2);
    harness
}

fn output_lines(app: &Tab) -> Vec<String> {
    (0..app.output.len())
        .map(|row| app.output.text(row).to_owned())
        .collect()
}

/// Puts the window in the state of having a live connection, and
/// returns the two ends a real session would hold.
fn fake_connection(
    app: &mut Tab,
) -> (
    std::sync::mpsc::Sender<Event>,
    tokio::sync::mpsc::UnboundedReceiver<Command>,
) {
    let (events, receiver) = channel();
    let (commands, sent) = tokio::sync::mpsc::unbounded_channel();
    app.window_untold = true;
    app.connection = Some(Connection {
        commands,
        events: receiver,
    });
    (events, sent)
}

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
fn lines_are_announced_through_a_live_region_when_asked() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.set_announce(true);
    app.voice.set_enabled(true);
    let (events, _sent) = fake_connection(app);
    // Let the notice that speech is on be announced first.
    harness.run_steps(2);
    assert_eq!(harness.state().tab().announcement, "Speech on.");
    events
        .send(Event::Line(
            panels::styled_lines("An orc arrives.").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    // The line is in the scrollback too; the announcement is the node
    // marked as a live region.
    let announced = (harness.query_all_by_label("An orc arrives."))
        .filter(|node| node.accesskit_node().live() == egui::accesskit::Live::Polite)
        .count();
    assert_eq!(announced, 1);
    // The same line again still counts as a change to announce.
    events
        .send(Event::Line(
            panels::styled_lines("An orc arrives.").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    assert_eq!(
        harness.state().tab().announcement,
        "An orc arrives.\u{200b}"
    );
    // Speaking directly instead leaves nothing to announce.
    harness.state_mut().tab_mut().voice.set_announce(false);
    harness.run_steps(2);
    assert!(!harness.state().tab().voice.announcing());
}

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
fn map_drawn_by_the_server_is_asked_for_at_the_panels_size_and_shown() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let example = include_str!("../../../../examples/scripts/drawn-map.luau");
    app.scripts.exec("drawn-map", example);
    app.run_actions();
    assert_eq!(app.supports, ["Map.Render 1"]);
    let (events, mut sent) = fake_connection(app);
    events.send(Event::Connected).unwrap();
    // The size goes out once the panel has kept it for a moment.
    harness.run_steps(2);
    assert!(sent.try_recv().is_err());
    // However busy the machine: it is waited for, not timed.
    let mut answer = None;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        harness.run_steps(2);
        answer = sent.try_recv().ok();
        if answer.is_some() {
            break;
        }
    }
    let Some(Command::Gmcp { package, data }) = answer else {
        panic!("the panel's size was not sent");
    };
    assert_eq!(package, "Map.Render.Size");
    let size: serde_json::Value = serde_json::from_str(&data).unwrap();
    let (width, height) = (
        size["width"].as_u64().unwrap(),
        size["height"].as_u64().unwrap(),
    );
    assert!(
        (20..80).contains(&width) && (5..=60).contains(&height),
        "{data}"
    );
    assert!(sent.try_recv().is_err(), "sent once");

    let drawing = r##"{"width": 5, "height": 3, "lines": ["  #  ", "#-@-#", "  #  "]}"##;
    let render = Event::Gmcp {
        package: "Map.Render".into(),
        data: drawing.into(),
    };
    events.send(render).unwrap();
    harness.run_steps(3);
    let app = harness.state().tab();
    // It is the map's own panel that shows it, not one more.
    assert_eq!(app.layout.panels.len(), 2);
    let map = app
        .layout
        .panels
        .iter()
        .find(|panel| panel.kind == layout::Kind::Map)
        .unwrap();
    let drawn = app.panel_text.iter().find(|(id, _)| *id == map.id).unwrap();
    assert_eq!(drawn.1.fixed.len(), 3);
    assert!(harness.query_by_label("#-@-#").is_some());
}

#[test]
fn tab_completes_with_words_the_server_gives() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let example = include_str!("../../../../examples/scripts/completions.luau");
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

#[test]
fn client_commands_are_grouped() {
    let read = |line| client_command(line).map(|(command, rest)| format!("{command} {rest}"));
    assert_eq!(
        read("/map walk Temple of Mota").unwrap(),
        "/go Temple of Mota"
    );
    assert_eq!(
        read("/plugin install https://example.org/x.git mine").unwrap(),
        "/install https://example.org/x.git mine"
    );
    assert_eq!(read("/map").unwrap(), "/map ");
    // The names of before are gone, and what is not ours is the MUD's.
    assert_eq!(read("/where"), None);
    assert_eq!(read("/say hello"), None);
    assert_eq!(read("north"), None);

    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    assert!(app.map_command("/map hide") && !app.map_shown());
    assert!(app.map_command("/map show") && app.map_shown());
    // The palette is the list of the commands: asking for help opens it.
    assert!(app.map_command("/help") && app.palette_open);
    app.toggle_palette();
    // Groups: listed, and switched for the session by name.
    app.scripts
        .exec("t", r#"bind("f9", "kick", {group = "Combat"})"#);
    assert!(app.map_command("/group combat off") && !app.scripts.group_enabled("Combat"));
    assert!(app.map_command("/group Combat") && app.scripts.group_enabled("Combat"));
    assert!(app.map_command("/group"));
    assert!(output_lines(app).iter().any(|line| line == "Combat: on"));
    // An incomplete group opens the palette at what it has, instead
    // of going to the MUD.
    assert!(app.map_command("/map dance"));
    assert!(app.palette_open && app.palette_query == "map ");
}

#[test]
fn commands_are_chosen_from_a_palette_and_answer_at_the_line() {
    let mut harness = window(None);
    harness.run_steps(2);
    let before = output_lines(harness.state().tab()).len();
    // Typing a slash in the command line offers nothing over it.
    harness.state_mut().tab_mut().input = "/pl".into();
    harness.run_steps(3);
    assert!(harness.query_by_label_contains("plugin install").is_none());
    harness.state_mut().tab_mut().input.clear();
    // The palette goes to places, and lists the commands, each said for
    // what it is.
    harness.state_mut().tab_mut().toggle_palette();
    harness.run_steps(3);
    harness.get_by_label("Incoming data. Go to");
    harness.state_mut().tab_mut().palette_query = "map".into();
    harness.run_steps(3);
    harness.get_by_label_contains("map where");
    // What it goes to is done there and then: the editor, at a section.
    harness.state_mut().tab_mut().palette_query = "timers".into();
    harness.run_steps(3);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert_eq!((app.view(), app.rules_section), (1, automation::TIMERS));
    app.set_view(0);
    // And what the window does is asked of it.
    app.toggle_palette();
    app.palette_query = "about".into();
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
    harness.state_mut().tab_mut().toggle_palette();
    harness.run_steps(2);
    // A slash puts the commands first, to run with Enter.
    harness.state_mut().tab_mut().palette_query = "/pl".into();
    harness.run_steps(3);
    harness.get_by_label_contains("plugin install");
    assert!(harness.query_by_label_contains("map where").is_none());
    // The arrows choose; Enter on one that takes something more
    // completes it, to type that after.
    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert!(app.palette_open);
    assert_eq!(app.palette_query, "plugin install ");
    // Escape closes it.
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().tab().palette_open);
    assert!(harness.query_by_label_contains("plugin install").is_none());
    // One Enter runs what is chosen, typed in full or not, and closes
    // the palette. A line of answer is said in the command line, and
    // the lines of the game are as they were.
    let app = harness.state_mut().tab_mut();
    app.scripts
        .exec("t", r#"bind("f9", "kick", {group = "Combat"})"#);
    app.toggle_palette();
    app.palette_query = "map hi".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(!app.map_shown() && !app.palette_open);
    assert_eq!(output_lines(app).len(), before);
    let app = harness.state_mut().tab_mut();
    app.toggle_palette();
    app.palette_query = "group Com".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(!app.scripts.group_enabled("Combat"));
    let note = app.bar_note.clone().expect("a line of answer");
    assert_eq!(
        (note.text.as_str(), note.problem),
        ("Group Combat off.", false)
    );
    assert_eq!(output_lines(app).len(), before);
    // Left alone, it goes by itself after a moment.
    harness.run_steps(40);
    assert!(harness.state().tab().bar_note.is_none());
    // A line of Lua is run from the palette as from the command line:
    // what it prints is its answer, and so is that it was run, or
    // that it could not be.
    let run = |harness: &mut Harness<'_, App>, code: &str| {
        let app = harness.state_mut().tab_mut();
        app.toggle_palette();
        app.palette_query = format!("lua {code}");
        harness.run_steps(2);
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        let app = harness.state().tab();
        assert!(!app.palette_open);
        let note = app.bar_note.clone().expect("an answer");
        (note.text, note.problem)
    };
    assert_eq!(
        run(&mut harness, r#"supports("Comm 1")"#),
        ("Done.".to_owned(), false)
    );
    assert_eq!(harness.state().tab().supports, ["Comm 1"]);
    assert_eq!(
        run(&mut harness, r#"echo("two")"#),
        ("two".to_owned(), false)
    );
    let (said, problem) = run(&mut harness, "error('boom')");
    assert!(said.contains("boom") && problem, "{said}");
    assert_eq!(output_lines(harness.state().tab()).len(), before);
    // What a command lists stays in the palette, to choose from:
    // Enter on a group switches it.
    let app = harness.state_mut().tab_mut();
    app.toggle_palette();
    app.palette_query = "gr".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.palette_open && app.results.is_some());
    harness.get_by_label("Combat. off");
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.scripts.group_enabled("Combat") && !app.palette_open);
    // Typed in the command line after a slash, a command is carried
    // out the same, and is not kept there to be sent again. What
    // could not be done stays said, until something is typed.
    harness.state_mut().tab_mut().input = "/map find nowhere".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(40);
    let app = harness.state().tab();
    assert!(app.input.is_empty());
    let note = app.bar_note.clone().expect("a refusal");
    assert_eq!(
        (note.text.as_str(), note.problem),
        ("No room matches.", true)
    );
}

#[test]
fn panels_are_kept_fed_by_triggers_and_moved_about() {
    let dir = std::env::temp_dir().join(format!("muddie-app-panels-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    let app = harness.state_mut().tab_mut();
    // What a profile starts with.
    let kinds: Vec<layout::Kind> = app.layout.panels.iter().map(|panel| panel.kind).collect();
    assert_eq!(kinds, [layout::Kind::Map, layout::Kind::Gauges]);
    // A trigger shows its line in a panel, which is made if need be,
    // and with the line hidden it is only there.
    app.rules.triggers.push(Trigger {
        enabled: true,
        pattern: "tells you".into(),
        panel: "Tells".into(),
        gag: true,
        ..Trigger::default()
    });
    for (name, code) in app.rules.chunks() {
        app.scripts.exec(&name, &code);
    }
    let (events, _sent) = fake_connection(app);
    let line = |text: &str| Event::Line(panels::styled_lines(text).remove(0));
    events
        .send(line("\x1b[33mAna tells you: hola\x1b[0m"))
        .unwrap();
    events.send(line("A plain line")).unwrap();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let tells = app
        .layout
        .panels
        .iter()
        .find(|panel| panel.name == "Tells")
        .expect("made")
        .clone();
    let content = &app
        .panel_text
        .iter()
        .find(|(id, _)| *id == tells.id)
        .unwrap()
        .1;
    assert_eq!(
        (content.log.len(), content.log.text(0)),
        (1, "Ana tells you: hola")
    );
    assert_eq!(output_lines(app), ["A plain line"]);
    // The channels go to the panel told to show them.
    (app.layout.panels.iter_mut())
        .filter(|panel| panel.id == tells.id)
        .for_each(|panel| panel.channels = true);
    app.channel_line(&panels::styled_lines("[ooc] Luis: buenas").remove(0));
    assert_eq!(
        app.panel_text
            .iter()
            .find(|(id, _)| *id == tells.id)
            .unwrap()
            .1
            .log
            .len(),
        2
    );
    // Moved to another side and hidden, it is all there the next time.
    let index = app
        .layout
        .panels
        .iter()
        .position(|panel| panel.id == tells.id)
        .unwrap();
    app.layout.panels[index].side = layout::Side::Left;
    app.set_map_shown(false);
    harness.run_steps(3);
    assert!(
        harness.query_by_label("Where am I?").is_none(),
        "the map is hidden"
    );
    let kept = layout::Layout::load(&dir.join("layout.toml")).unwrap();
    assert_eq!(kept.panels[index].side, layout::Side::Left);
    assert!(
        !kept
            .panels
            .iter()
            .any(|panel| panel.kind == layout::Kind::Map && panel.visible)
    );
    // Two panels on one side share it, each with its part.
    let app = harness.state_mut().tab_mut();
    app.set_map_shown(true);
    let map = app
        .layout
        .panels
        .iter()
        .position(|panel| panel.kind == layout::Kind::Map)
        .unwrap();
    app.layout.panels[index].side = app.layout.panels[map].side;
    harness.run_steps(3);
    let app = harness.state().tab();
    let rect = |id| {
        app.panel_rects
            .iter()
            .find(|(known, _)| *known == id)
            .unwrap()
            .1
    };
    let (above, below) = (rect(app.layout.panels[map].id), rect(tells.id));
    assert!((above.width() - below.width()).abs() < 1.0 && above.bottom() < below.top());
    assert!(
        (above.height() - below.height()).abs() < 2.0,
        "equal weights, equal parts"
    );
    // Joined as tabs they share one place, and the one out of sight
    // says so when something comes for it.
    let app = harness.state_mut().tab_mut();
    app.layout.panels[index].stack = 1;
    app.layout.panels[map].stack = 1;
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let rect = |app: &Tab, id| {
        app.panel_rects
            .iter()
            .find(|(known, _)| *known == id)
            .unwrap()
            .1
    };
    assert_eq!(rect(app, app.layout.panels[map].id), rect(app, tells.id));
    let first = app.layout.panels[map.min(index)].id;
    let hidden = if first == tells.id {
        app.layout.panels[map].id
    } else {
        tells.id
    };
    assert!(app.shown_tabs.contains(&(1, first)));
    if hidden == tells.id {
        app.add_to_panel(
            "Tells",
            &panels::styled_lines("Luis tells you: eh").remove(0),
        );
        assert_eq!(app.unread_tabs, [tells.id]);
        harness.get_by_label("Tells").click();
        harness.run_steps(3);
        let app = harness.state().tab();
        assert!(app.shown_tabs.contains(&(1, tells.id)) && app.unread_tabs.is_empty());
    }
    let app = harness.state_mut().tab_mut();
    app.layout.panels[index].stack = 0;
    app.layout.panels[map].stack = 0;
    harness.run_steps(3);
    // Its size is as it was drawn, not a little more each time.
    let before = harness.state().tab().layout.panels[index].size;
    harness.run_steps(5);
    assert_eq!(harness.state().tab().layout.panels[index].size, before);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn variables_are_the_sessions_and_kept_only_with_a_saved_profile() {
    let dir = std::env::temp_dir().join(format!("muddie-app-vars-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    let app = harness.state_mut().tab_mut();
    // No profile: they last through the scripts starting again, and
    // nothing is written anywhere.
    app.scripts.exec("t", r#"vars.target = "orc""#);
    app.load_scripts();
    assert_eq!(
        app.scripts.variables(),
        [("target".to_owned(), serde_json::json!("orc"))]
    );
    assert!(!dir.join("data.json").exists());
    // Another session does not see them.
    let mut other = window(Some(Profiles::at(dir.clone())));
    assert!(other.state_mut().tab_mut().scripts.variables().is_empty());
    // Saved as a profile, they go with it, and are there the next time.
    let app = harness.state_mut().tab_mut();
    app.host = "mud.example.org".into();
    app.port = "4000".into();
    app.profile_name = "Reinos".into();
    app.save_profile();
    assert_eq!(app.profile.as_deref(), Some("Reinos"));
    assert_eq!(app.scripts.variables().len(), 1);
    let mut later = window(Some(Profiles::at(dir.clone())));
    let app = later.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    assert_eq!(
        app.scripts.variables(),
        [("target".to_owned(), serde_json::json!("orc"))]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn what_the_mud_sends_is_kept_to_look_at_and_made_into_gauges() {
    let mut harness = window(None);
    let (events, _sent) = fake_connection(harness.state_mut().tab_mut());
    // Kept from the start, as when it is asked to go on outside the
    // debugging view.
    harness.state_mut().tab_mut().data_keep = true;
    let vitals = |hp: u32| Event::Gmcp {
        package: "Char.Vitals".into(),
        data: format!(r#"{{"health": {{"current": {hp}, "max": 100}}}}"#),
    };
    events.send(vitals(40)).unwrap();
    events.send(vitals(62)).unwrap();
    let hunger = Event::Msdp {
        variable: "HUNGER".into(),
        data: "\"7\"".into(),
    };
    events.send(hunger).unwrap();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let seen: Vec<(&str, bool, u32)> = app
        .data_seen
        .iter()
        .map(|seen| (seen.name.as_str(), seen.msdp, seen.count))
        .collect();
    assert_eq!(seen, [("Char.Vitals", false, 2), ("HUNGER", true, 1)]);
    assert!(app.data_seen[0].json.contains("62") && app.data_seen[0].earlier[0].1.contains("40"));
    // The editor shows it, and a number there becomes a gauge with
    // its maximum found, in a panel of gauges.
    app.set_view(2);
    harness.run_steps(3);
    assert!(
        harness
            .query_by_label("Char.Vitals.health.current")
            .is_some()
    );
    harness
        .get_all_by_label("Make a gauge")
        .next()
        .unwrap()
        .click();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let made = app
        .rules_draft
        .gauges
        .last()
        .expect("a gauge was made")
        .clone();
    assert_eq!(
        (made.name.as_str(), made.value.as_str(), made.max.as_str()),
        (
            "health",
            "Char.Vitals.health.current",
            "Char.Vitals.health.max"
        )
    );
    assert_eq!(app.view(), 1, "it is shown in the editor to be finished");
    // Saved, it follows the value, in the panel it was put in.
    app.close_automation();
    events.send(vitals(55)).unwrap();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.gauge_is_in("health", made.panel));
    assert!(app.gauges.summary().contains("health 55 of 100"));
}

#[test]
fn long_scrollback_is_not_laid_out_all_at_once_when_the_text_changes() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    (0..6000).for_each(|n| {
        app.show_local(&format!(
            "line {n} of a long session, long enough to matter"
        ))
    });
    harness.run_steps(3);
    // A change of the text's size: every row may be another height.
    let mut settings = harness.state().tab().settings.clone();
    settings.font_size = 19.0;
    let ctx = harness.ctx.clone();
    crate::theme::apply(&ctx, &settings).unwrap();
    harness.state_mut().tab_mut().settings_changed(&settings);
    harness.step();
    // One frame does not pay for six thousand rows: it lays out a
    // share, and what is on the screen.
    let measured = harness.state().tab().output.measured();
    assert!(measured < 3000, "{measured} rows were laid out in a frame");
    // And the end stays in view while the rest catches up.
    harness.run_steps(40);
    assert!(harness.query_by_label("Latest lines").is_none());
}

#[test]
fn a_variable_is_edited_as_its_type() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.scripts
        .set_variable("bag", Some(serde_json::json!({"gold": 5})))
        .unwrap();
    app.open_automation(5);
    harness.run_steps(3);
    // A table: a key and a value a row, added to and typed over.
    harness.get_by_label("Add a field").click();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    app.variable_items[1] = ("keys".into(), "3".into());
    harness.run_steps(2);
    let kept = |app: &Tab| app.scripts.variables()[0].1.clone();
    assert_eq!(
        kept(harness.state().tab()),
        serde_json::json!({"gold": 5, "keys": 3})
    );
    // Made an array, it keeps its values and loses their keys.
    harness.get_by_label("array").click();
    harness.run_steps(3);
    assert_eq!(kept(harness.state().tab()), serde_json::json!([5, 3]));
    // A boolean is a switch.
    harness.get_by_label("boolean").click();
    harness.run_steps(3);
    assert_eq!(kept(harness.state().tab()), serde_json::json!(true));
    harness.get_by_label("Value").click();
    harness.run_steps(3);
    assert_eq!(kept(harness.state().tab()), serde_json::json!(false));
}

/// Says what a frame costs in the views that could be slow, to look
/// for what to make faster:
/// `cargo test -p muddie frame_costs -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check"]
fn frame_costs() {
    let cost = |harness: &mut Harness<'_, App>, what: &str| {
        // Once what a burst of text left to be measured has been: that is
        // done a share a frame, and is not what a frame costs after.
        harness.run_steps(90);
        let started = Instant::now();
        for _ in 0..30 {
            harness.step();
        }
        println!(
            "{what}: {:.2} ms a frame",
            started.elapsed().as_secs_f64() * 1000.0 / 30.0
        );
    };
    let mut harness = window(None);
    harness.set_size(egui::vec2(1600.0, 1000.0));
    cost(&mut harness, "empty session");
    let app = harness.state_mut().tab_mut();
    for n in 0..20_000 {
        app.show_local(&format!(
            "\x1b[32mline {n}\x1b[0m of a long session, with some text to wrap now and then"
        ));
    }
    cost(&mut harness, "20000 lines of scrollback");
    let app = harness.state_mut().tab_mut();
    for n in 0..2000 {
        let line = panels::styled_lines(&format!("[ooc] someone says thing {n}")).remove(0);
        app.channel_line(&line);
        app.add_to_panel("Combat", &line);
    }
    app.gauges.set("HP", Some((80.0, 100.0)));
    cost(&mut harness, "with panels of 2000 lines and gauges");
    let app = harness.state_mut().tab_mut();
    for n in 0..300 {
        app.rules.triggers.push(Trigger {
            name: format!("Trigger {n}"),
            enabled: true,
            pattern: format!("pattern {n}"),
            send: "say hi".into(),
            group: format!("Group {}", n % 6),
            ..Trigger::default()
        });
    }
    app.open_automation(0);
    cost(&mut harness, "editor with 300 triggers");
    let app = harness.state_mut().tab_mut();
    for n in 0..120 {
        app.saw_data(
            &format!("Pkg.Number{n}"),
            false,
            r#"{"a": 1, "b": {"c": [1, 2, 3], "d": "text"}}"#,
        );
    }
    app.set_view(2);
    cost(&mut harness, "debug with 120 packages");
}

#[test]
fn a_burst_of_lines_is_measured_a_share_a_frame() {
    let mut harness = window(None);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    (0..6000).for_each(|n| app.show_local(&format!("line {n} of a burst of text")));
    // Laid out all at once, each row would hold its laid out text for
    // the frame; a share is, the newest, which are the ones in view.
    harness.step();
    let share = crate::output::REMEASURED_PER_FRAME;
    let measured = harness.state().tab().output.measured();
    assert!(
        measured > 0 && measured <= share + 100,
        "{measured} rows measured in a frame"
    );
    // The view follows to the end, where the newest already are; a
    // frame or two later, as the size of what it holds settles.
    // However many frames that takes on a busy machine.
    for _ in 0..12 {
        harness.step();
        if harness
            .query_by_label("line 5999 of a burst of text")
            .is_some()
        {
            break;
        }
    }
    assert!(harness.query_by_label("Latest lines").is_none());
    harness.get_by_label("line 5999 of a burst of text");
    // The rest follow, and the end stays in view meanwhile.
    harness.run_steps(6000 / share + 2);
    assert_eq!(
        harness.state().tab().output.measured(),
        harness.state().tab().output.len()
    );
    assert!(harness.query_by_label("Latest lines").is_none());
}

#[test]
fn gauges_are_shown_in_the_order_of_the_editor() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    let gauge = |name: &str| crate::rules::Gauge {
        name: name.into(),
        value: format!("Char.{name}"),
        ..crate::rules::Gauge::starter()
    };
    app.rules.gauges = vec![gauge("HP"), gauge("MP"), gauge("Gold")];
    // The MUD reports them in an order of its own.
    for name in ["Gold", "HP", "MP"] {
        app.gauges.set(name, Some((1.0, 2.0)));
    }
    harness.run_steps(3);
    assert_eq!(
        harness.state().tab().gauges.summary(),
        "HP 1 of 2, MP 1 of 2, Gold 1 of 2"
    );
    // Moved in the editor with Alt and an arrow, and saved, the panel
    // follows.
    let app = harness.state_mut().tab_mut();
    app.open_automation(automation::GAUGES);
    app.select_rule(automation::GAUGES, 2);
    harness.run_steps(3);
    harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowUp);
    harness.run_steps(3);
    harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowUp);
    harness.run_steps(3);
    let names = |app: &Tab| -> Vec<String> {
        (app.rules_draft.gauges.iter())
            .map(|gauge| gauge.name.clone())
            .collect()
    };
    assert_eq!(names(harness.state().tab()), ["Gold", "HP", "MP"]);
    // At the top it stays; down it goes, and no further than the end.
    harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowUp);
    harness.run_steps(3);
    assert_eq!(names(harness.state().tab()), ["Gold", "HP", "MP"]);
    harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowDown);
    harness.run_steps(3);
    assert_eq!(names(harness.state().tab()), ["HP", "Gold", "MP"]);
    // Dropped before another, as dragging does.
    let app = harness.state_mut().tab_mut();
    app.move_gauge(2, 0);
    assert_eq!(names(app), ["MP", "HP", "Gold"]);
    app.close_automation();
    harness.run_steps(3);
    assert_eq!(
        harness.state().tab().gauges.summary(),
        "MP 1 of 2, HP 1 of 2, Gold 1 of 2"
    );
}

#[test]
fn what_arrives_is_kept_only_while_it_is_looked_at() {
    let mut harness = window(None);
    let (events, mut sent) = fake_connection(harness.state_mut().tab_mut());
    let vitals = || Event::Gmcp {
        package: "Char.Vitals".into(),
        data: r#"{"hp": 5}"#.into(),
    };
    // With the debugging view closed, nothing is kept.
    events.send(vitals()).unwrap();
    harness.run_steps(3);
    assert!(harness.state().tab().data_seen.is_empty());
    // Open, it is: by the protocol each thing came by. The connection
    // is told to say what it otherwise passes over.
    harness.state_mut().tab_mut().set_view(2);
    harness.run_steps(2);
    let told: Vec<Command> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
    assert!(matches!(told.last(), Some(Command::Trace(true))));
    events.send(vitals()).unwrap();
    let tag = Event::Other {
        protocol: "MXP".into(),
        name: "send".into(),
        data: "<send href=\"look\">".into(),
    };
    events.send(tag).unwrap();
    events
        .send(Event::Protocol {
            name: "GMCP".into(),
            on: true,
        })
        .unwrap();
    harness.run_steps(3);
    let kept = |app: &Tab| -> Vec<String> {
        (app.data_seen.iter())
            .map(|seen| format!("{} {}", seen.protocol, seen.name))
            .collect()
    };
    assert_eq!(
        kept(harness.state().tab()),
        ["GMCP Char.Vitals", "MXP send"]
    );
    harness.get_by_label("GMCP: In use");
    // Choosing a protocol lists what came by it alone.
    harness.get_by_label("MXP: Not offered by this MUD").click();
    harness.run_steps(3);
    assert!(harness.query_by_label("Char.Vitals").is_none());
    // In the list, and laid out beside it.
    assert!(harness.query_all_by_label("send").count() >= 2);
    // Closed again, it stops, unless asked to go on.
    harness.state_mut().tab_mut().set_view(0);
    harness.run_steps(2);
    events.send(vitals()).unwrap();
    harness.run_steps(3);
    assert_eq!(harness.state().tab().data_seen[0].count, 1);
    harness.state_mut().tab_mut().data_keep = true;
    events.send(vitals()).unwrap();
    harness.run_steps(3);
    assert_eq!(harness.state().tab().data_seen[0].count, 2);
}

#[test]
fn the_server_is_told_how_much_text_fits() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1100.0, 700.0));
    let (_events, mut sent) = fake_connection(harness.state_mut().tab_mut());
    harness.state_mut().tab_mut().window_untold = false;
    // Once the size has been the same for a moment, it is told.
    let mut told = |harness: &mut Harness<'_, App>| {
        for _ in 0..40 {
            harness.run_steps(2);
            while let Ok(command) = sent.try_recv() {
                if let Command::Resize(wide, tall) = command {
                    return (wide, tall);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the size was not told");
    };
    let first = told(&mut harness);
    assert!(
        first.0 > 40 && first.1 > 10,
        "{first:?} is not the size of the text"
    );
    // A wider and shorter window fits more across and less down.
    harness.set_size(egui::vec2(1500.0, 500.0));
    let second = told(&mut harness);
    assert!(
        second.0 > first.0 && second.1 < first.1,
        "{first:?} then {second:?}"
    );
}

#[test]
fn sounds_a_mud_asks_for_are_played_from_the_players_own_or_fetched() {
    let dir = std::env::temp_dir().join(format!("muddie-media-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    let sounds = profiles.sound_dirs(None).remove(0);
    std::fs::create_dir_all(&sounds).unwrap();
    std::fs::write(sounds.join("own.wav"), b"RIFF").unwrap();
    let mut harness = window(Some(profiles));
    let app = harness.state_mut().tab_mut();
    // One the player has is found there, whatever address is given.
    let found = app.media_file("own.wav", Some("https://mud.example.org/s"));
    assert_eq!(found, Some((String::new(), Some(sounds.join("own.wav")))));
    // One they do not have is on the web, at the address the message
    // gives, or the one given for all of them.
    assert_eq!(app.media_file("far.wav", None), None);
    app.media(crate::media::Media::Default(
        "https://mud.example.org/sounds/".into(),
    ));
    let (address, here) = app.media_file("far.wav", None).unwrap();
    assert_eq!(address, "https://mud.example.org/sounds/far.wav");
    assert_eq!(here, None);
    // Nothing is looked for outside its folder, nor that is no sound.
    assert_eq!(app.media_file("../own.wav", None), None);
    assert_eq!(app.media_file("/etc/passwd", None), None);
    assert_eq!(app.media_file("script.sh", None), None);
    // With fetching switched off, nothing is asked of the web; nor is
    // what could not be had before.
    app.prefs.media_downloads = false;
    app.media_fetch(address.clone(), None);
    assert!(app.media_jobs.is_empty());
    app.prefs.media_downloads = true;
    app.media_missing.push(address.clone());
    app.media_fetch(address, None);
    assert!(app.media_jobs.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_sound_of_the_sound_protocol_is_found_by_kind_and_by_pattern() {
    let dir = std::env::temp_dir().join(format!("muddie-msp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    let sounds = profiles.sound_dirs(None).remove(0);
    std::fs::create_dir_all(sounds.join("combat")).unwrap();
    for name in [
        "combat/hit1.wav",
        "combat/hit2.wav",
        "combat/miss.wav",
        "door.wav",
    ] {
        std::fs::write(sounds.join(name), b"RIFF").unwrap();
    }
    let mut harness = window(Some(profiles));
    let app = harness.state_mut().tab_mut();
    // A name with a star is one of the files it stands for.
    let found = app.sound_among("combat/hit*.wav").unwrap();
    assert!(
        ["combat/hit1.wav", "combat/hit2.wav"].contains(&found.as_str()),
        "{found}"
    );
    assert_eq!(app.sound_among("d??r.wav").as_deref(), Some("door.wav"));
    assert_eq!(app.sound_among("combat/parry*.wav"), None);
    // One without is looked for as it is, and none out of the folder.
    assert_eq!(app.sound_among("door.wav"), None);
    assert_eq!(app.sound_among("../*.wav"), None);
    // Stopping may say where files are fetched from when none is given.
    app.msp(Msp::Stop {
        music: true,
        url: Some("http://mud.example/m/".into()),
    });
    assert_eq!(app.media_base.as_deref(), Some("http://mud.example/m/"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn what_mxp_asks_for_reaches_the_panels_the_gauges_and_the_command_line() {
    use muddie_core::Mxp;
    let mut harness = window(None);
    let (events, _sent) = fake_connection(harness.state_mut().tab_mut());
    let send = |event: Event| events.send(event).unwrap();
    let value = |name: &str, value: &str| {
        Event::Mxp(Mxp::Value {
            name: name.into(),
            value: value.into(),
        })
    };
    // A gauge follows values by name, once they are known.
    send(Event::Mxp(Mxp::Gauge {
        entity: "hp".into(),
        max: "maxhp".into(),
        caption: "Health".into(),
        colour: "red".into(),
        bar: true,
    }));
    send(value("hp", "62"));
    send(value("maxhp", "100"));
    // A frame is a panel by its name, and what is sent to it goes there.
    send(Event::Mxp(Mxp::Frame {
        name: "Scores".into(),
        open: true,
        title: String::new(),
        align: "left".into(),
    }));
    let mut line = panels::styled_lines("Ana 12").remove(0);
    line.dest = Some("Scores".into());
    send(Event::Line(line.clone()));
    send(Event::Line(panels::styled_lines("in the game").remove(0)));
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.gauges.summary(), "Health 62 of 100");
    let panel = (app.layout.panels.iter())
        .find(|panel| panel.name == "Scores")
        .cloned()
        .unwrap();
    assert!(panel.visible && panel.side == layout::Side::Left);
    assert_eq!(app.text_of(panel.id).log.text(0), "Ana 12");
    assert_eq!(output_lines(app), ["in the game"]);
    // Emptied, and closed, when asked.
    send(Event::Mxp(Mxp::Clear("Scores".into())));
    send(Event::Mxp(Mxp::Frame {
        name: "scores".into(),
        open: false,
        title: String::new(),
        align: String::new(),
    }));
    // Where to go on is said, and left to the player.
    send(Event::Mxp(Mxp::Relocate {
        host: "mud.example".into(),
        port: "4001".into(),
    }));
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let panel = (app.layout.panels.iter())
        .find(|panel| panel.name == "Scores")
        .cloned()
        .unwrap();
    assert!(!panel.visible && app.text_of(panel.id).log.len() == 0);
    assert!(
        output_lines(app)
            .iter()
            .any(|line| line.contains("mud.example, port 4001"))
    );
    assert!(app.connection.is_some());
    // A link that prompts puts its command in the command line.
    app.link_clicked(crate::output::Clicked::Prompt("tell guard".into()));
    assert_eq!(app.input, "tell guard ");
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
fn a_script_is_run_from_its_button_and_says_how_it_went() {
    let dir = std::env::temp_dir().join(format!("muddie-run-{}", std::process::id()));
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
    // Typed, and run at once from its button: it takes effect...
    harness.state_mut().tab_mut().script_text = r#"bind("f9", "kick")"#.into();
    harness.run_steps(2);
    harness.get_by_label("Run").click();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(
        matches!(&app.script_ran, Some((_, None))),
        "{:?}",
        app.script_ran
    );
    assert_eq!(app.script_saved, r#"bind("f9", "kick")"#);
    harness.get_by_label_contains("Last run: ");
    // ...and one that fails says why, there, not among the lines of
    // the game.
    let before = output_lines(harness.state().tab()).len();
    harness.state_mut().tab_mut().script_text = "error('boom')".into();
    harness.run_steps(2);
    harness.get_by_label("Run").click();
    harness.run_steps(3);
    let app = harness.state().tab();
    let Some((_, Some(error))) = &app.script_ran else {
        panic!("the error was not kept: {:?}", app.script_ran);
    };
    assert!(error.contains("boom"), "{error}");
    assert_eq!(output_lines(app).len(), before);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn channels_have_a_tab_each_in_the_panel_of_channels() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    // A script says what was said on which channel; and a channel
    // can be made known before anyone speaks on it.
    app.scripts.exec(
        "t",
        r#"channel("chat", "Ana: hola") channel("mercenario") channel("chat", "Luis: buenas")"#,
    );
    app.run_actions();
    // The usual message of GMCP names its channel too.
    let said = r#"{"channel": "bando", "talker": "Eva", "text": "Eva: al norte"}"#;
    let (events, _sent) = fake_connection(app);
    events
        .send(Event::Gmcp {
            package: "Comm.Channel.Text".into(),
            data: said.into(),
        })
        .unwrap();
    harness.run_steps(3);
    let app = harness.state().tab();
    let names: Vec<&str> = app
        .channels
        .iter()
        .map(|channel| channel.name.as_str())
        .collect();
    assert_eq!(names, ["chat", "mercenario", "bando"]);
    assert_eq!(app.channels[0].log.len(), 2);
    // One panel, not one for each: the panel of channels shows all
    // of it, and a tab for each channel.
    assert_eq!(
        app.layout
            .panels
            .iter()
            .filter(|panel| panel.kind == layout::Kind::Text)
            .count(),
        1
    );
    harness.get_by_label("Luis: buenas");
    harness.get_by_label("Eva: al norte");
    // A channel's tab shows what was said on it alone.
    harness.get_by_label("mercenario");
    harness.get_by_label("chat").click();
    harness.run_steps(3);
    harness.get_by_label("Ana: hola");
    assert!(harness.query_by_label("Eva: al norte").is_none());
    // What is said on another meanwhile marks its tab, until looked at.
    let app = harness.state_mut().tab_mut();
    app.scripts.exec("t", r#"channel("bando", "Eva: esperad")"#);
    app.run_actions();
    harness.run_steps(2);
    let unread = |app: &Tab, name: &str| app.channels.iter().any(|c| c.name == name && c.unread);
    assert!(unread(harness.state().tab(), "bando") && !unread(harness.state().tab(), "chat"));
    harness.get_by_label("bando, new").click();
    harness.run_steps(3);
    assert!(!unread(harness.state().tab(), "bando"));
    harness.get_by_label("Eva: esperad");
}

#[test]
fn a_script_switched_off_is_kept_and_not_run() {
    let dir = std::env::temp_dir().join(format!("muddie-scriptoff-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Test", &crate::profile::Profile::default())
        .unwrap();
    let scripts = profiles.scripts_dir("Test");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::write(
        scripts.join("keys.luau"),
        r#"bind("f9", "kick", {group = "Combat"})"#,
    )
    .unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Test".into())));
    assert_eq!(app.scripts.groups(), ["Combat"]);
    // Switched off beside its name, it stops being run once saved...
    app.open_automation(automation::SCRIPTS);
    harness.run_steps(3);
    let index = (harness.state().tab().script_files.iter())
        .position(|file| file.ends_with("keys.luau"))
        .unwrap();
    harness
        .state_mut()
        .tab_mut()
        .select_rule(automation::SCRIPTS, index);
    harness.run_steps(3);
    harness
        .get_by_role_and_label(egui::accesskit::Role::Switch, "Enabled")
        .click();
    harness.run_steps(2);
    std::thread::sleep(Duration::from_millis(1000));
    harness.run_steps(3);
    let app = harness.state().tab();
    assert_eq!(app.rules.scripts_off, ["keys.luau"]);
    assert!(app.scripts.groups().is_empty());
    // ...and is still there, to be switched on again.
    assert!(scripts.join("keys.luau").is_file());
    harness
        .get_by_role_and_label(egui::accesskit::Role::Switch, "Enabled")
        .click();
    harness.run_steps(2);
    std::thread::sleep(Duration::from_millis(1000));
    harness.run_steps(3);
    assert_eq!(harness.state().tab().scripts.groups(), ["Combat"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn channels_are_given_a_panel_where_there_is_none_for_them() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    // A layout with a map and gauges, and nowhere for text.
    app.layout
        .panels
        .retain(|panel| panel.kind != layout::Kind::Text);
    assert!(!app.layout.panels.iter().any(|panel| panel.channels));
    app.scripts
        .exec("t", r#"channel("chat") channel("chat", "Ana: hola")"#);
    app.run_actions();
    harness.run_steps(3);
    // A panel was made for them, once, and what was said is in it.
    let app = harness.state_mut().tab_mut();
    let made: Vec<_> = app
        .layout
        .panels
        .iter()
        .filter(|panel| panel.channels)
        .cloned()
        .collect();
    assert_eq!(made.len(), 1);
    assert!(made[0].visible && made[0].kind == layout::Kind::Text);
    // Called Channels, whatever the language, under the map.
    assert_eq!(
        (made[0].name.as_str(), made[0].side),
        ("Channels", layout::Side::Right)
    );
    let app = harness.state().tab();
    let right: Vec<&str> = (app.layout.panels.iter())
        .filter(|panel| panel.side == layout::Side::Right)
        .map(|panel| panel.name.as_str())
        .collect();
    assert_eq!(right, ["Map", "Channels"]);
    harness.get_by_label("Ana: hola");
    harness.get_by_label("chat");
    // Hidden, it is not made again.
    let app = harness.state_mut().tab_mut();
    app.layout
        .panels
        .iter_mut()
        .for_each(|panel| panel.visible &= !panel.channels);
    app.scripts.exec("t", r#"channel("chat", "Luis: buenas")"#);
    app.run_actions();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert_eq!(
        app.layout
            .panels
            .iter()
            .filter(|panel| panel.channels)
            .count(),
        1
    );
}

#[test]
fn a_script_turns_a_muds_own_channel_messages_into_tabs() {
    // As a player of a MUD with channels of its own would have it: a
    // layout with no panel of text, and a script for its messages.
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.layout
        .panels
        .retain(|panel| panel.kind != layout::Kind::Text);
    let script = r#"supports("Comm 1")

on_gmcp("Comm.MensajeCanal", function(data)
    channel(data.canal, data.mensaje)
end)

on_gmcp("Comm.Canales", function(data)
    for nombre in pairs(data) do
        channel(nombre)
    end
end)"#;
    app.scripts.exec("Channels", script);
    app.run_actions();
    let (events, _sent) = fake_connection(app);
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    events
        .send(gmcp(
            "Comm.Canales",
            r#"{"chat": "chat", "mercenario": "mercenario"}"#,
        ))
        .unwrap();
    let said = "{\"canal\": \"chat\", \"mensaje\": \"[Chat] Naylhena \\u001b[1m\\u001b[30mSacerdotisa \\u001b[0mde la \\u001b[36mcalumnia\\u001b[0m: \\u001b[1m\\u001b[37mre\"}";
    events.send(gmcp("Comm.MensajeCanal", said)).unwrap();
    harness.run_steps(4);
    let app = harness.state().tab();
    let mut names: Vec<&str> = app
        .channels
        .iter()
        .map(|channel| channel.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["chat", "mercenario"]);
    assert_eq!(
        app.layout
            .panels
            .iter()
            .filter(|panel| panel.channels && panel.visible)
            .count(),
        1
    );
    // The message, with its colours taken out of the way, in view,
    // and a tab for each channel.
    harness.get_by_label("[Chat] Naylhena Sacerdotisa de la calumnia: re");
    harness.get_by_label("chat");
    harness.get_by_label("mercenario");
}

#[test]
fn a_channel_goes_to_the_panel_a_script_names() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    // A panel the player made by hand, and put on the left.
    let id = app.panel_called("Conversaciones");
    let made = app.layout.panel_mut(id).unwrap();
    (made.side, made.size) = (layout::Side::Left, 300.0);
    let panels = app.layout.panels.len();
    // Named by a script, it is the one used, as it is; and one that
    // is not there is made.
    app.scripts.exec(
        "t",
        r#"channel("chat", "Ana: hola", {panel = "conversaciones"})
            channel("bando", "Eva: al norte", {panel = "Conversaciones"})
            channel("susurro", "Luis te susurra: ven", {panel = "Privado"})"#,
    );
    app.run_actions();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.layout.panels.len(), panels + 1);
    let kept = app.layout.panel(id).cloned().unwrap();
    assert!(kept.side == layout::Side::Left && kept.size == 300.0);
    let private = (app.layout.panels.iter())
        .find(|panel| panel.name == "Privado")
        .cloned()
        .unwrap();
    // Each panel has what was said on its channels, and their tabs.
    assert_eq!(app.text_of(id).log.len(), 2);
    assert_eq!(app.text_of(private.id).log.text(0), "Luis te susurra: ven");
    let tabs = |app: &Tab, panel: &str| -> Vec<String> {
        (app.channels.iter())
            .filter(|channel| channel.panel.eq_ignore_ascii_case(panel))
            .map(|channel| channel.name.clone())
            .collect()
    };
    assert_eq!(tabs(app, "Conversaciones"), ["chat", "bando"]);
    assert_eq!(tabs(app, "Privado"), ["susurro"]);
    // The panel of channels, which none of them was sent to, has none.
    harness.get_by_label("bando").click();
    harness.run_steps(3);
    harness.get_by_label("Eva: al norte");
    assert!(harness.query_by_label("Ana: hola").is_none());
}

#[test]
fn saving_a_rule_or_a_script_leaves_what_the_other_scripts_keep() {
    let dir = std::env::temp_dir().join(format!("muddie-partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Test", &crate::profile::Profile::default())
        .unwrap();
    let scripts = profiles.scripts_dir("Test");
    std::fs::create_dir_all(&scripts).unwrap();
    // One script keeps what it is told, as one drawing a map would;
    // another is the one being worked on.
    std::fs::write(
        scripts.join("canvas.luau"),
        r#"cells = cells or 0
            on_gmcp("Map.Canvas", function() cells += 1 end)
            alias("^cells$", function() echo("cells " .. cells) end)"#,
    )
    .unwrap();
    std::fs::write(
        scripts.join("other.luau"),
        r#"alias("^v$", function() echo("v1") end)"#,
    )
    .unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Test".into())));
    let said = |app: &mut Tab, typed: &str| {
        app.scripts.on_input(typed);
        app.run_actions();
        output_lines(app).last().cloned().unwrap_or_default()
    };
    app.scripts.on_gmcp("Map.Canvas.Full", "{}");
    app.scripts.on_gmcp("Map.Canvas.Patch", "{}");
    assert_eq!(said(app, "cells"), "cells 2");
    // A rule is made and saved: it takes effect, and the scripts are
    // not run again for it.
    app.rules_draft = app.rules.clone();
    app.rules_draft.aliases.push(Alias {
        name: "hi".into(),
        enabled: true,
        pattern: "hi".into(),
        send: "say hello".into(),
        ..Alias::default()
    });
    app.save_rules();
    assert_eq!(said(app, "cells"), "cells 2");
    let (_events, mut sent) = fake_connection(app);
    app.scripts.on_input("hi");
    app.run_actions();
    assert_eq!(
        sent.try_recv().ok(),
        Some(Command::Send("say hello".into()))
    );
    // Saved again, the rule is there once, not twice.
    app.save_rules();
    app.scripts.on_input("hi");
    app.run_actions();
    assert_eq!(
        sent.try_recv().ok(),
        Some(Command::Send("say hello".into()))
    );
    assert!(sent.try_recv().is_err());
    // The other script is changed and run again from the editor: it
    // is the new one, and the first still has what it kept.
    app.open_automation(automation::SCRIPTS);
    harness.run_steps(3);
    let index = (harness.state().tab().script_files.iter())
        .position(|file| file.ends_with("other.luau"))
        .unwrap();
    harness
        .state_mut()
        .tab_mut()
        .select_rule(automation::SCRIPTS, index);
    harness.run_steps(3);
    harness.state_mut().tab_mut().script_text = r#"alias("^v$", function() echo("v2") end)"#.into();
    harness.run_steps(2);
    harness.get_by_label("Run").click();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert_eq!(said(app, "v"), "v2");
    assert_eq!(said(app, "cells"), "cells 2");
    // Switched off, a script is stopped; it can still be run by hand,
    // and running everything again starts it only if it is on.
    app.rules_draft = app.rules.clone();
    app.rules_draft.scripts_off.push("other.luau".into());
    app.save_rules();
    let before = output_lines(app).len();
    app.scripts.on_input("v");
    app.run_actions();
    assert_eq!(output_lines(app).len(), before, "it was not stopped");
    assert_eq!(said(app, "cells"), "cells 2");
    app.run_scripts();
    assert_eq!(said(app, "v"), "v2");
    app.load_scripts();
    let before = output_lines(app).len();
    app.scripts.on_input("v");
    app.run_actions();
    assert_eq!(
        output_lines(app).len(),
        before,
        "one switched off was started with the rest"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_plugin_for_the_mud_is_offered_and_switched_on_without_the_rest_starting_over() {
    let dir = std::env::temp_dir().join(format!("muddie-included-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Reinos", &crate::profile::Profile::default())
        .unwrap();
    let mut harness = window(Some(profiles.clone()));
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    app.host = "rlmud.org".into();
    let reinos = crate::bundled::find("reinos-de-leyenda").unwrap();
    let common = crate::bundled::find(crate::bundled::COMMON).unwrap();
    // What many MUDs send the same way is on to begin with; the plugin
    // of one MUD is not.
    assert!(app.included(common) && !app.included(reinos));
    app.scripts.exec(
        "keeps",
        "kept = 7 alias(\"^kept$\", function() echo(\"kept \" .. kept) end)",
    );
    // Connected to that MUD, its plugin is offered over the command line.
    let (events, _sent) = fake_connection(app);
    events.send(Event::Connected).unwrap();
    harness.run_steps(3);
    harness.get_by_label_contains("There is a plugin for Reinos de Leyenda");
    // Switched on there, it works at once: what the MUD says on its
    // channels has a tab, and nothing else was run again.
    harness.get_by_label("Enable").click();
    harness.run_steps(3);
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    events
        .send(gmcp(
            "Comm.MensajeCanal",
            r#"{"canal": "chat", "mensaje": "[Chat] Ana: hola"}"#,
        ))
        .unwrap();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert!(app.included(reinos) && app.plugin_offer.is_none());
    assert_eq!(app.channels[0].name, "chat");
    app.scripts.on_input("kept");
    app.run_actions();
    assert_eq!(output_lines(app).last().map(String::as_str), Some("kept 7"));
    // It is kept with the profile, and not offered again.
    let saved = profiles.load("Reinos").unwrap();
    assert_eq!(saved.included_on, ["reinos-de-leyenda"]);
    events.send(Event::Connected).unwrap();
    harness.run_steps(3);
    assert!(harness.state().tab().plugin_offer.is_none());
    // Switched off, what it made is gone.
    let app = harness.state_mut().tab_mut();
    app.set_included(reinos, false);
    let before = app.channels[0].log.len();
    app.scripts.on_gmcp(
        "Comm.MensajeCanal",
        r#"{"canal": "chat", "mensaje": "otra"}"#,
    );
    app.run_actions();
    assert_eq!(app.channels[0].log.len(), before);
    // Declined instead, it is not offered again in this profile either.
    app.offer_plugin_for("rlmud.org", "");
    assert!(app.plugin_offer.is_some());
    app.decline_plugin();
    app.offer_plugin_for("", "Reinos de Leyenda");
    assert!(app.plugin_offer.is_none());
    assert_eq!(
        profiles.load("Reinos").unwrap().included_declined,
        ["reinos-de-leyenda"]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn what_many_muds_send_alike_is_a_plugin_that_can_be_switched_off() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let common = crate::bundled::find(crate::bundled::COMMON).unwrap();
    let (events, _sent) = fake_connection(app);
    let vitals = || Event::Gmcp {
        package: "Char.Vitals".into(),
        data: r#"{"hp": 80, "maxhp": 100}"#.into(),
    };
    let said = || Event::Gmcp {
        package: "Comm.Channel.Text".into(),
        data: r#"{"channel": "ooc", "text": "Ana: hola"}"#.into(),
    };
    // On, as it is to begin with: gauges and channels by themselves.
    events.send(vitals()).unwrap();
    events.send(said()).unwrap();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert!(app.gauges.summary().contains("80 of 100"));
    assert_eq!(app.channels.len(), 1);
    // Off, the client makes nothing of them by itself; scripts still
    // get them.
    app.set_included(common, false);
    app.gauges.clear();
    app.channels.clear();
    app.scripts.exec(
        "t",
        r#"on_gmcp("Char.Vitals", function(data) echo("hp " .. data.hp) end)"#,
    );
    events.send(vitals()).unwrap();
    events.send(said()).unwrap();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.gauges.summary().is_empty() && app.channels.is_empty());
    assert_eq!(output_lines(app).last().map(String::as_str), Some("hp 80"));
}

#[test]
fn what_the_client_says_can_be_read_again() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.note("Connected.");
    // An answer said in the command line for a moment is kept too.
    app.scripts
        .exec("t", r#"bind("f9", "kick", {group = "Combat"})"#);
    app.run_command("/group Combat off");
    app.run_command("/map find nowhere");
    harness.run_steps(3);
    let (said, unread) = harness.state().tab().notices();
    let texts: Vec<(&str, bool)> = said
        .iter()
        .map(|notice| (notice.text.as_str(), notice.problem))
        .collect();
    assert_eq!(
        texts,
        [
            ("Connected.", false),
            ("Group Combat off.", false),
            ("No room matches.", true)
        ]
    );
    assert_eq!(unread, 3);
    // The bell says how many are new, and opening it shows them.
    harness.get_by_label("Messages, 3 new").click();
    harness.run_steps(3);
    harness.get_by_label("No room matches.");
    assert_eq!(harness.state().tab().notices().1, 0);
}

#[test]
fn what_is_deleted_can_be_had_back() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.rules.triggers.push(Trigger {
        name: "Fight back".into(),
        enabled: true,
        pattern: "attacks you".into(),
        send: "kill".into(),
        ..Trigger::default()
    });
    app.scripts
        .set_variable("target", Some(serde_json::json!("orc")))
        .unwrap();
    app.open_automation(automation::TRIGGERS);
    harness.run_steps(3);
    // Deleted, it is gone, and the editor says so with a way back.
    harness.get_by_label("Delete").click();
    harness.run_steps(3);
    assert!(harness.state().tab().rules_draft.triggers.is_empty());
    harness.get_by_label_contains("Deleted Fight back.");
    harness.get_by_label("Undo").click();
    harness.run_steps(3);
    assert_eq!(
        harness.state().tab().rules_draft.triggers[0].name,
        "Fight back"
    );
    // A variable too, with the keys that undo anywhere.
    harness
        .state_mut()
        .tab_mut()
        .open_automation(automation::VARIABLES);
    harness.run_steps(3);
    harness.get_by_label("Delete").click();
    harness.run_steps(3);
    assert!(harness.state().tab().scripts.variables().is_empty());
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::Z);
    harness.run_steps(3);
    let kept = harness.state().tab().scripts.variables();
    assert_eq!(kept, [("target".to_owned(), serde_json::json!("orc"))]);
}

#[test]
fn a_panel_is_arranged_from_its_own_menu_with_no_mode_to_enter() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.started = true;
    harness.run_steps(3);
    // Every panel has its handle and its menu, all the time.
    harness.get_by_label("Move Gauges");
    harness.get_by_label("Gauges options").click();
    harness.run_steps(2);
    // From there it goes to another side...
    harness.get_by_label("At the left").click();
    harness.run_steps(3);
    let side = |app: &Tab, name: &str| {
        app.layout
            .panels
            .iter()
            .find(|panel| panel.name == name)
            .map(|panel| panel.side)
    };
    assert_eq!(
        side(harness.state().tab(), "Gauges"),
        Some(layout::Side::Left)
    );
    // ...and is hidden, to be shown again from the menu or the palette.
    harness.get_by_label("Gauges options").click();
    harness.run_steps(2);
    harness.get_by_label("Hide Gauges").click();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert!(
        !app.layout
            .panels
            .iter()
            .find(|panel| panel.name == "Gauges")
            .unwrap()
            .visible
    );
    // A panel is added without a mode, from the palette.
    let before = app.layout.panels.len();
    app.toggle_palette();
    app.palette_query = "add a text panel".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(harness.state().tab().layout.panels.len(), before + 1);
}

#[test]
fn the_one_menu_opens_and_does_what_it_says() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    harness.get_by_label("Menu").click();
    harness.run_steps(3);
    harness.get_by_label("Keyboard shortcuts");
    harness.get_by_label("Settings").click();
    harness.run_steps(3);
    harness.get_by_label("Language");
}

#[test]
fn a_panel_is_dragged_by_its_handle_to_another_place() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(4);
    // The chat, above, taken by its dots and let go on the upper part of
    // the map, at the right.
    let from = harness.get_by_label("Move Gauges").rect().center();
    let map = harness.get_by_label("Move Map").rect().center() + egui::vec2(60.0, 60.0);
    harness.hover_at(from);
    harness.run_steps(1);
    harness.drag_at(from);
    harness.run_steps(1);
    for step in 1..=10 {
        let at = from + (map - from) * (step as f32 / 10.0);
        harness.hover_at(at);
        harness.run_steps(1);
    }
    harness.drop_at(map);
    harness.run_steps(4);
    let app = harness.state().tab();
    let side = |name: &str| {
        (app.layout.panels.iter())
            .find(|panel| panel.name == name)
            .map(|panel| panel.side)
    };
    assert_eq!(side("Gauges"), Some(layout::Side::Right));
    assert!(app.dragged_panel.is_none());
}

#[test]
fn a_panels_menu_is_reached_from_the_keyboard() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    harness.get_by_label("Gauges options").focus();
    harness.run_steps(2);
    assert!(harness.get_by_label("Gauges options").is_focused());
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    harness.get_by_label("At the left");
}

#[test]
fn a_gauge_is_changed_where_it_is_shown() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.started = true;
    app.rules.gauges.push(crate::rules::Gauge {
        name: "Salud".into(),
        value: "Char.Vitals.hp".into(),
        max: "100".into(),
        ..crate::rules::Gauge::starter()
    });
    app.gauges.set("Salud", Some((40.0, 100.0)));
    // One the MUD made by itself is shown, and not to be changed here.
    app.gauges.set("MV", Some((10.0, 20.0)));
    harness.run_steps(3);
    assert!(harness.query_by_label("Edit MV").is_none());
    // One made in the editor opens its details over the game.
    harness.get_by_label("Edit Salud").click();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert_eq!((app.gauge_here, app.view()), (Some(0), 0));
    app.rules_draft.gauges[0].colour = "red".into();
    harness.run_steps(2);
    // Closed, what was changed is saved, as in the editor.
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.gauge_here.is_none());
    assert_eq!(app.rules.gauges[0].colour, "red");
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
            harness.state().tab().palette_open,
            "closed again at frame {frame}"
        );
    }
    harness.get_by_label("Incoming data. Go to");
    // A press anywhere else does close it.
    harness.get_by_label("Menu").click();
    harness.run_steps(3);
    assert!(!harness.state().tab().palette_open);
}

#[test]
fn what_a_click_opens_stays_open() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.started = true;
    app.rules.gauges.push(crate::rules::Gauge {
        name: "Salud".into(),
        value: "Char.Vitals.hp".into(),
        max: "100".into(),
        ..crate::rules::Gauge::starter()
    });
    app.gauges.set("Salud", Some((40.0, 100.0)));
    app.note("Connected.");
    harness.run_steps(3);
    let steady = |harness: &mut Harness<'_, crate::app::App>, what: &str, shown: &str| {
        // It may be laid out a few frames late; once there, it stays.
        harness.run_steps(4);
        for frame in 0..6 {
            assert!(
                harness.query_by_label(shown).is_some(),
                "{what}: gone at frame {frame}"
            );
            harness.step();
        }
        harness.key_press(Key::Escape);
        harness.run_steps(3);
    };
    let session = harness
        .get_all_by_role(egui::accesskit::Role::Tab)
        .find(|tab| {
            tab.accesskit_node()
                .label()
                .is_some_and(|label| label.contains("Not connected"))
        })
        .expect("the session's tab");
    session.click_secondary();
    steady(&mut harness, "session menu", "Save as profile");
    harness.get_by_label("Menu").click();
    steady(&mut harness, "menu", "Keyboard shortcuts");
    harness.get_by_label("Messages, 1 new").click();
    steady(&mut harness, "messages", "Clear");
    harness.get_by_label("Gauges options").click();
    steady(&mut harness, "panel menu", "At the left");
    harness.get_by_label("Edit Salud").click();
    steady(&mut harness, "gauge", "Open in the editor");
    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.get_by_label("Settings").click();
    steady(&mut harness, "settings", "Language");
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
    // What depends on the MUD is the profile's, not every session's.
    (harness.get_by_role_and_label(egui::accesskit::Role::Switch, "Keep the connection alive"))
        .click();
    harness.run_steps(2);
    let character = || (egui::accesskit::Role::TextInput, "Character");
    harness
        .get_by_role_and_label(character().0, character().1)
        .focus();
    harness.run_steps(2);
    (harness.get_by_role_and_label(character().0, character().1)).type_text("Naylhena");
    harness.run_steps(2);
    // Closing the settings keeps what was changed, without a button, and
    // the session playing with the profile takes it.
    harness.get_by_label("Close").click();
    harness.run_steps(3);
    assert_eq!(harness.state().tab().character, "Naylhena");
    let saved = Profiles::at(dir.clone()).load("Reinos").unwrap();
    assert_eq!(saved.character, "Naylhena");
    // It was on, as it is to begin with: off in this profile only.
    assert_eq!(
        saved.session.map(|session| session.keepalive_seconds),
        Some(0)
    );
    assert_eq!(harness.state().app_settings().keepalive_seconds, 60);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_plugin_that_comes_with_muddie_is_made_ones_own_to_change() {
    let dir = std::env::temp_dir().join(format!("muddie-customize-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles.save("Reinos", &Profile::default()).unwrap();
    let mut harness = window(Some(profiles.clone()));
    harness.set_size(egui::vec2(1300.0, 860.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    let plugin = crate::bundled::find("reinos-de-leyenda").unwrap();
    app.set_included(plugin, true);
    harness.state_mut().edit_profile("Reinos");
    harness.run_steps(3);
    harness
        .get_by_label("Customize Reinos de Leyenda")
        .click_accesskit();
    harness.run_steps(4);
    // Its code is a script of the profile's, open in the editor, and the
    // one that came is off so as not to run twice.
    let path = profiles
        .scripts_dir("Reinos")
        .join("reinos-de-leyenda.luau");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        plugin.code.unwrap()
    );
    let app = harness.state().tab();
    assert!(!app.included(plugin));
    assert_eq!((app.view(), app.rules_section), (1, automation::SCRIPTS));
    assert_eq!(
        app.script_files
            .get(app.rules_selected[automation::SCRIPTS]),
        Some(&path)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn session_is_logged_when_asked() {
    let dir = std::env::temp_dir().join(format!("muddie-app-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    let app = harness.state_mut().tab_mut();
    app.host = "mud.example.org".into();
    assert!(app.map_command("/log"));
    let (events, _sent) = fake_connection(app);
    events
        .send(Event::Line(
            panels::styled_lines("\x1b[31mAn orc\x1b[0m arrives").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    let path = app.log.as_ref().unwrap().path().to_owned();
    assert!(app.map_command("/log"));
    assert!(app.log.is_none());
    let log = std::fs::read_to_string(path).unwrap();
    assert!(log.contains("An orc arrives"), "{log}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rules_made_in_the_window_are_saved_and_take_effect() {
    let dir = std::env::temp_dir().join(format!("muddie-app-rules-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    harness.state_mut().tab_mut().open_automation(0);
    harness.run_steps(4);
    harness.get_by_label("Add").click();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    app.rules_draft.triggers[0].pattern = "you are hungry".into();
    app.rules_draft.triggers[0].send = "eat bread".into();
    // Edits are saved once they stop for a moment, with no button.
    harness.run_steps(2);
    assert!(!dir.join("rules.toml").exists(), "not while still editing");
    std::thread::sleep(Duration::from_millis(1000));
    harness.run_steps(2);
    assert_eq!(
        Rules::load(&dir.join("rules.toml")).unwrap().triggers.len(),
        1
    );
    // Closing the window saves at once.
    let app = harness.state_mut().tab_mut();
    app.rules_draft.triggers[0].name = "Hunger".into();
    app.close_automation();
    assert_eq!(
        Rules::load(&dir.join("rules.toml")).unwrap().triggers[0].name,
        "Hunger"
    );

    let app = harness.state_mut().tab_mut();
    let (events, mut sent) = fake_connection(app);
    events
        .send(Event::Line(
            panels::styled_lines("Suddenly you are hungry.").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    assert_eq!(sent.try_recv().unwrap(), Command::Send("eat bread".into()));
    // Reloading scripts keeps the rules working.
    assert!(harness.state_mut().tab_mut().map_command("/reload"));
    events
        .send(Event::Line(
            panels::styled_lines("Again you are hungry.").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    assert_eq!(sent.try_recv().unwrap(), Command::Send("eat bread".into()));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn macro_key_is_recorded_from_the_next_key_pressed() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.open_automation(2);
    app.rules_draft.keys.push(KeyBinding {
        enabled: true,
        send: "look".into(),
        ..KeyBinding::default()
    });
    app.recording_key = true;
    harness.run_steps(3);
    // A plain letter cannot be bound, so it is not taken.
    harness.key_press(Key::A);
    harness.run_steps(2);
    assert!(harness.state().tab().recording_key);
    harness.key_press_modifiers(Modifiers::CTRL | Modifiers::SHIFT, Key::K);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.rules_draft.keys[0].key, "ctrl+shift+k");
    assert!(!app.recording_key);
    // The keypad is recorded by its own names.
    app.recording_key = true;
    app.keypad_pressed = vec!["numpad8"];
    harness.run_steps(2);
    assert_eq!(harness.state().tab().rules_draft.keys[0].key, "numpad8");
    // Escape leaves the key as it was.
    harness.state_mut().tab_mut().recording_key = true;
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert_eq!(
        (app.rules_draft.keys[0].key.as_str(), app.recording_key),
        ("numpad8", false)
    );
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

#[test]
fn arriving_lines_do_not_flash_the_view_of_the_latest_lines() {
    let mut harness = window(None);
    for burst in 0..12 {
        let app = harness.state_mut().tab_mut();
        (0..9).for_each(|n| app.show_local(&format!("burst {burst} line {n}")));
        // Frame by frame: the view must not come up even for one.
        for _ in 0..3 {
            harness.step();
            assert!(
                harness.query_by_label("Latest lines").is_none(),
                "shown during burst {burst}"
            );
        }
    }
}

#[test]
fn sound_requests_from_the_server_are_not_shown() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    for text in [
        "before",
        "!!SOUND(missing.wav V=50)",
        "!!MUSIC(Off)",
        "after",
    ] {
        events
            .send(Event::Line(panels::styled_lines(text).remove(0)))
            .unwrap();
    }
    harness.run_steps(2);
    assert_eq!(output_lines(harness.state().tab()), ["before", "after"]);
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

/// A window with every panel in use, as after some minutes of play.
fn demo_window<'a>() -> Harness<'a, App> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 700.0))
        .wgpu()
        .build_eframe(|cc| {
            let startup = Startup {
                profiles: None,
                profile: None,
                target: None,
                speech: false,
                scripts: Vec::new(),
            };
            let app = App::new(cc, startup, Default::default());
            crate::i18n::set_language("en");
            app
        });
    let app = harness.state_mut().tab_mut();
    app.started = true;
    app.host = "mud.example.org".into();
    app.port = "4000".into();
    app.tls = true;
    app.profile_name = "Reinos".into();
    app.note("Connected.");
    let text = "\x1b[1;32mPlaza Mayor\x1b[0m\n\
            Una plaza amplia con una fuente en el centro. Al norte se alza la \x1b[33mtorre\x1b[0m \
            del reloj y al este se abre un callejón estrecho que huele a pan recién hecho.\n\
            \x1b[36mSalidas: norte, este, sur\x1b[0m\n\
            \x1b[31mUn orco te ataca\x1b[0m con su \x1b[1;35mhacha rúnica\x1b[0m.\n\
            \x1b[38;5;208mAna te dice: ¿vienes a la torre?\x1b[0m";
    for line in panels::styled_lines(text) {
        app.output.push(&line);
    }
    app.output.push_local("> matar orco");
    app.output
        .set_partial(&panels::styled_lines("HP:80 MP:20> ")[0]);
    app.gauges.set("HP", Some((80.0, 100.0)));
    app.gauges.set("MP", Some((20.0, 50.0)));
    app.gauges.set("MV", Some((140.0, 150.0)));
    for line in panels::styled_lines("\x1b[33m[ooc] Ana: hola a todos\x1b[0m\n[ooc] Luis: buenas") {
        app.channel_line(&line);
    }
    app.show_in_panel("Minimapa", Some("  #\n#-\x1b[1;33m@\x1b[0m-#\n  #"));
    app.input = "matar orco".into();
    {
        let mut map = app.map.borrow_mut();
        let room = |id, name: &str, area: &str, exits: &[(&str, RoomId)]| RoomInfo {
            id,
            name: name.into(),
            area: area.into(),
            exits: (exits.iter())
                .map(|(direction, to)| ((*direction).to_owned(), *to))
                .collect(),
            pos: None,
        };
        for info in [
            room(1, "Puerta", "ciudad", &[("n", 2)]),
            room(
                2,
                "Plaza Mayor",
                "ciudad",
                &[("s", 1), ("n", 3), ("e", 4), ("w", 9)],
            ),
            room(3, "Torre", "ciudad", &[("s", 2), ("u", 5), ("portal", 20)]),
            room(4, "Callejón", "ciudad", &[("w", 2), ("e", 6), ("ne", 7)]),
            room(6, "Panadería", "ciudad", &[("w", 4)]),
            room(7, "Patio", "ciudad", &[("sw", 4), ("n", 8)]),
            room(8, "Camino", "afueras", &[("s", 7)]),
            room(4, "Callejón", "ciudad", &[("w", 2), ("e", 6), ("ne", 7)]),
            room(
                2,
                "Plaza Mayor",
                "ciudad",
                &[("s", 1), ("n", 3), ("e", 4), ("w", 9)],
            ),
        ] {
            map.visit(info).unwrap();
        }
        map.set_avoid(6, true).unwrap();
        map.edit_exit(4, "ne", |exit| exit.locked = true).unwrap();
    }
    harness.run_steps(3);
    harness
}

/// The same window as it looks during play, with the connection open.
fn connected_demo_window<'a>() -> Harness<'a, App> {
    let mut harness = demo_window();
    // Dropping the far ends is fine: nothing is sent or received here.
    fake_connection(harness.state_mut().tab_mut());
    harness.run_steps(3);
    harness
}

/// Renders the demo window to the PNG named by `MUDDIE_SCREENSHOT`, to
/// look at the result. Needs a GPU or software Vulkan, so it is not
/// part of the normal test run:
/// `MUDDIE_SCREENSHOT=/tmp/muddie.png cargo test -p muddie -- --ignored`
#[test]
#[ignore = "needs a graphics adapter; writes an image for visual review"]
fn screenshot() {
    let path = std::env::var_os("MUDDIE_SCREENSHOT").map(PathBuf::from);
    let save = |harness: &mut Harness<'_, App>, name: &str| {
        let image = harness.render().expect("rendering failed");
        if let Some(path) = &path {
            let path = path.with_extension(format!("{name}.png"));
            image.save(path).expect("could not write the screenshot");
        }
    };
    // What a new session opens on.
    let dir = std::env::temp_dir().join(format!("muddie-shot-welcome-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    for (name, host, port) in [
        ("Reinos de Leyenda", "rlmud.org", 23),
        ("Aardwolf", "aardmud.org", 4000),
        ("Discworld", "discworld.starturtle.net", 4242),
    ] {
        let profile = Profile {
            host: host.into(),
            port,
            ..Profile::default()
        };
        profiles.save(name, &profile).unwrap();
    }
    let mut welcome = window(Some(profiles));
    welcome.state_mut().tab_mut().started = false;
    welcome.run_steps(3);
    save(&mut welcome, "welcome");
    welcome
        .state_mut()
        .open_settings(crate::app::Section::Profiles);
    welcome.run_steps(4);
    save(&mut welcome, "profiles");
    welcome.state_mut().edit_profile("Aardwolf");
    welcome.run_steps(4);
    save(&mut welcome, "profile");
    std::fs::remove_dir_all(&dir).unwrap();

    // A plugin for the MUD connected to, offered over the command line.
    let mut offer = connected_demo_window();
    let app = offer.state_mut().tab_mut();
    app.offer_plugin_for("rlmud.org", "");
    offer.run_steps(3);
    save(&mut offer, "plugin-offer");

    // The channels of the MUD, each with its tab in one panel.
    let mut channels = connected_demo_window();
    let app = channels.state_mut().tab_mut();
    app.scripts.exec(
            "t",
            "channel(\"chat\", \"[Chat] Naylhena: \\27[1mre\\27[0m\") channel(\"mercenario\") channel(\"bando\", \"[Bando] Eva: al norte\") channel(\"chat\", \"[Chat] Luis: buenas\")",
        );
    app.run_actions();
    channels.run_steps(3);
    channels.get_by_label("chat").click();
    channels.run_steps(3);
    let app = channels.state_mut().tab_mut();
    app.scripts
        .exec("t", "channel(\"bando\", \"[Bando] Eva: esperad\")");
    app.run_actions();
    channels.run_steps(3);
    save(&mut channels, "channels");

    // A script, run from its button, and one that fails.
    let dir = std::env::temp_dir().join(format!("muddie-shot-script-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Reinos", &crate::profile::Profile::default())
        .unwrap();
    let mut script = window(Some(profiles));
    script.set_size(egui::vec2(1500.0, 900.0));
    let app = script.state_mut().tab_mut();
    app.select_profile(Some("Reinos".into()));
    app.open_automation(automation::SCRIPTS);
    app.add_script(None);
    script.run_steps(3);
    script.state_mut().tab_mut().script_text = "supports(\"Comm 1\")\n\non_gmcp(\"Comm.MensajeCanal\", function(data)\n\tpanel_add(data.canal, data.mensaje)\nend)\n".into();
    script.run_steps(2);
    script.get_by_label("Run").click();
    script.run_steps(4);
    save(&mut script, "script");
    script.state_mut().tab_mut().script_text = "supports(\"Comm 1\")\nerror(\"boom\")\n".into();
    script.run_steps(2);
    script.get_by_label("Run").click();
    script.run_steps(4);
    save(&mut script, "script-error");
    std::fs::remove_dir_all(&dir).unwrap();

    let mut settings = demo_window();
    settings
        .state_mut()
        .open_settings(crate::app::Section::Appearance);
    settings.run_steps(4);
    save(&mut settings, "settings");
    settings
        .state_mut()
        .open_settings(crate::app::Section::General);
    settings.run_steps(4);
    save(&mut settings, "settings-general");
    // The same text in each of the fonts that come with the program.
    for (name, _) in crate::theme::BUNDLED_FONTS {
        let mut fonts = connected_demo_window();
        let ctx = fonts.ctx.clone();
        fonts
            .state_mut()
            .use_font(&ctx, &crate::theme::bundled_font(name));
        fonts.run_steps(4);
        save(&mut fonts, &format!("font-{}", name.replace(' ', "")));
    }
    // The plugins, with one installed and enabled and one not.
    let dir = std::env::temp_dir().join(format!("muddie-shot-plugins-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for plugin in ["aardwolf-mapper", "combat-gauges"] {
        std::fs::create_dir_all(dir.join("packages").join(plugin)).unwrap();
        std::fs::write(dir.join("packages").join(plugin).join("init.luau"), "").unwrap();
    }
    let profiles = Profiles::at(dir.clone());
    let profile = Profile {
        packages: vec!["aardwolf-mapper".into()],
        ..Profile::default()
    };
    profiles.save("Aardwolf", &profile).unwrap();
    let mut plugins = window(Some(profiles));
    plugins.state_mut().edit_profile("Aardwolf");
    plugins.run_steps(4);
    plugins.get_by_label("Update all").scroll_to_me();
    plugins.run_steps(3);
    save(&mut plugins, "plugins");
    std::fs::remove_dir_all(&dir).unwrap();
    // The automation window, with rules of each kind to look at.
    let mut rules = demo_window();
    rules.set_size(egui::vec2(1500.0, 900.0));
    let app = rules.state_mut().tab_mut();
    app.rules.triggers.push(Trigger {
        name: "Tells".into(),
        group: "Social".into(),
        enabled: true,
        pattern: "tells you".into(),
        speak: "a tell".into(),
        highlight: "yellow".into(),
        ..Trigger::default()
    });
    app.rules.triggers.push(Trigger {
            name: "Fight back".into(),
            group: "Combat".into(),
            enabled: true,
            pattern: r"^(\w+) attacks you".into(),
            regex: true,
            send: "kill %1\nshield".into(),
            sound: "alert.ogg".into(),
            lua: true,
            script: "-- Only the dangerous ones\nsend(\"kill \" .. m[1])\nlocal feared = {Orc = true, Troll = true}\nif feared[m[1]] and gmcp(\"Char.Vitals\").hp < 40 then\n\tsend(\"flee\")\n\techo(\"Too weak for \" .. m[1])\nend".into(),
            ..Trigger::default()
        });
    app.rules.triggers.push(Trigger {
        enabled: false,
        pattern: "The wind howls".into(),
        gag: true,
        ..Trigger::default()
    });
    app.rules.keys.push(KeyBinding {
        name: "North".into(),
        enabled: true,
        key: "numpad8".into(),
        send: "north".into(),
        ..KeyBinding::default()
    });
    app.open_automation(0);
    app.rules_selected[0] = 1;
    app.rules_sample = "Orc attacks you with a rusty axe!".into();
    rules.run_steps(4);
    save(&mut rules, "automation");
    // The same with a mistake in the script, and then as commands.
    let app = rules.state_mut().tab_mut();
    let good = app.rules_draft.triggers[1].script.clone();
    app.rules_draft.triggers[1].script =
        good.replace("then\n\tsend(\"flee\")", "\n\tsend(\"flee\"");
    rules.run_steps(3);
    save(&mut rules, "mistake");
    let app = rules.state_mut().tab_mut();
    app.rules_draft.triggers[1].script = good;
    app.rules_draft.triggers[1].lua = false;
    rules.run_steps(3);
    save(&mut rules, "commands");
    rules.state_mut().tab_mut().rules_draft.triggers[1].lua = true;
    let app = rules.state_mut().tab_mut();
    app.rules_section = 2;
    rules.run_steps(3);
    save(&mut rules, "macros");
    let app = rules.state_mut().tab_mut();
    app.rules_draft.timers.push(crate::rules::Timer {
        name: "Guardar".into(),
        tenths: 3000,
        send: "save".into(),
        ..crate::rules::Timer::default()
    });
    app.rules_section = 3;
    rules.run_steps(3);
    save(&mut rules, "timers");
    let app = rules.state_mut().tab_mut();
    app.scripts
        .set_variable("objetivo", Some(serde_json::json!("orco jefe")))
        .unwrap();
    app.scripts
        .set_variable("muertes", Some(serde_json::json!(12)))
        .unwrap();
    app.scripts
        .set_variable(
            "enemigo",
            Some(serde_json::json!({"nombre": "troll", "vida": 340, "jefe": true})),
        )
        .unwrap();
    app.scripts
        .set_variable("ruta", Some(serde_json::json!(["norte", "norte", "este"])))
        .unwrap();
    app.rules_section = 5;
    app.rules_selected[5] = 0;
    rules.run_steps(3);
    save(&mut rules, "variables");
    rules.state_mut().tab_mut().rules_selected[5] = 3;
    rules.run_steps(3);
    save(&mut rules, "variable-array");
    let app = rules.state_mut().tab_mut();
    app.saw_data(
        "Char.Vitals",
        false,
        r#"{"health": {"current": 40, "max": 100}}"#,
    );
    app.saw_data("Char.Vitals", false, r#"{"health": {"current": 62, "max": 100}, "stamina": {"current": 80, "max": 90}, "focus": {"current": 12, "max": 30}}"#);
    app.saw_data(
        "Map.Render",
        false,
        r##"{"width": 5, "height": 3, "lines": ["  #  ", "#-@-#", "  #  "]}"##,
    );
    app.saw_data(
        "Room.Info",
        false,
        r#"{"num": 2, "name": "Plaza Mayor", "exits": {"n": 5, "e": 3}}"#,
    );
    app.saw("MXP", "send", r#""<send href=\"mirar\">""#);
    app.protocols = vec![
        ("GMCP".into(), true),
        ("MXP".into(), true),
        ("MSSP".into(), true),
        ("ATCP".into(), false),
    ];
    app.set_view(2);
    rules.run_steps(3);
    save(&mut rules, "data");
    // The menus, as someone using Spanish sees them.
    {
        let mut menus = window(None);
        menus.state_mut().tab_mut().started = true;
        menus.run_steps(2);
        menus.get_by_label("Menu").click();
        menus.run_steps(4);
        save(&mut menus, "menu");
    }
    // A session whose MUD has reported no room: the map says so.
    let mut unmapped = window(None);
    fake_connection(unmapped.state_mut().tab_mut());
    unmapped
        .state_mut()
        .tab_mut()
        .show_local("Bienvenido a Fimbulvetr.");
    unmapped.run_steps(3);
    save(&mut unmapped, "unmapped");
    // The layout mode, and the gauges made from it.
    let mut layout = connected_demo_window();
    layout.set_size(egui::vec2(1500.0, 900.0));
    let app = layout.state_mut().tab_mut();
    // The chat at the right, under the map, sharing its side.
    let mut chat = app.layout.panels.remove(0);
    chat.side = layout::Side::Right;
    app.layout.panels.push(chat);
    // And two panels of text under it in one place, as tabs.
    app.add_to_panel(
        "Combate",
        &panels::styled_lines("\x1b[31mUn orco te ataca\x1b[0m con su hacha.").remove(0),
    );
    app.add_to_panel(
        "Tells",
        &panels::styled_lines("\x1b[33mAna te dice: ¿vienes?\x1b[0m").remove(0),
    );
    for panel in &mut app.layout.panels {
        if panel.kind == layout::Kind::Text {
            panel.side = layout::Side::Right;
            panel.stack = 1;
        }
    }
    app.layout.panels.retain(|panel| panel.name != "Minimapa");
    layout.run_steps(4);
    save(&mut layout, "layout");
    let app = layout.state_mut().tab_mut();
    let gauges = app
        .layout
        .panels
        .iter()
        .find(|panel| panel.kind == layout::Kind::Gauges)
        .unwrap()
        .id;
    app.open_gauges(gauges);
    app.rules_draft.gauges.push(crate::rules::Gauge {
        name: "Salud".into(),
        value: "Char.Vitals.health.current".into(),
        max: "Char.Vitals.health.max".into(),
        colour: "green".into(),
        steps: vec![
            crate::rules::Step {
                under: 60,
                colour: "yellow".into(),
            },
            crate::rules::Step {
                under: 30,
                colour: "red".into(),
            },
        ],
        ..crate::rules::Gauge::starter()
    });
    app.rules_draft.gauges.push(crate::rules::Gauge {
        name: "Envenenado".into(),
        value: "Char.Status.poisoned".into(),
        kind: crate::rules::GaugeKind::Light,
        colour: "red".into(),
        off_colour: "green".into(),
        ..crate::rules::Gauge::starter()
    });
    app.rules_selected[4] = 0;
    layout.run_steps(4);
    save(&mut layout, "gauges");
    layout.state_mut().tab_mut().rules_selected[4] = 1;
    layout.run_steps(4);
    save(&mut layout, "gauge-light");
    // The palette of commands over the command line, and an answer.
    let mut palette = connected_demo_window();
    let app = palette.state_mut().tab_mut();
    app.scripts.exec(
        "t",
        r#"bind("f9", "kick", {group = "Combate"}) bind("f8", "hi", {group = "Social"})"#,
    );
    app.toggle_palette();
    palette.run_steps(8);
    save(&mut palette, "palette");
    palette.state_mut().tab_mut().palette_query = "map".into();
    palette.run_steps(4);
    save(&mut palette, "palette-typed");
    palette.state_mut().tab_mut().palette_query = "gr".into();
    palette.run_steps(3);
    palette.key_press(Key::Enter);
    palette.run_steps(4);
    save(&mut palette, "answer");
    palette.key_press(Key::Enter);
    palette.run_steps(3);
    save(&mut palette, "bar-note");
    // The command line with nothing typed in it.
    let mut empty = connected_demo_window();
    empty.state_mut().tab_mut().input.clear();
    empty.run_steps(3);
    save(&mut empty, "empty");
    // The find bar and the view of the latest lines while scrolled back.
    let mut find = connected_demo_window();
    let app = find.state_mut().tab_mut();
    (0..60).for_each(|n| app.show_local(&format!("line {n} of filler to scroll through")));
    app.find_open = true;
    app.find_query = "orco".into();
    find.run_steps(3);
    find.get_by_label("Previous").click();
    find.run_steps(4);
    save(&mut find, "find");
    // The other themes.
    for (theme, name) in [
        (crate::settings::ThemeName::Light, "light"),
        (crate::settings::ThemeName::Contrast, "contrast"),
    ] {
        let mut themed = connected_demo_window();
        let ctx = themed.ctx.clone();
        themed.state_mut().use_theme(&ctx, theme);
        themed.run_steps(3);
        save(&mut themed, name);
    }
    let ctx = find.ctx.clone();
    find.state_mut()
        .use_theme(&ctx, crate::settings::ThemeName::Dark);
    let mut harness = connected_demo_window();
    assert!(harness.query_by_label("Connection").is_none());
    let image = harness.render().expect("rendering failed");
    assert_eq!((image.width(), image.height()), (1100, 700));
    if let Some(path) = &path {
        image.save(path).expect("could not write the screenshot");
    }
    // A second view: every area on the level.
    harness.state_mut().tab_mut().map_view.all_areas = true;
    harness.run_steps(3);
    let image = harness.render().expect("rendering failed");
    if let Some(path) = &path {
        (image.save(path.with_extension("all-areas.png"))).expect("could not write the screenshot");
    }
    // A third: the menu of the room under the pointer. The map is
    // centred on the player's room, so that is the room it opens on.
    harness.get_by_label("Map view").click_secondary();
    harness.run_steps(3);
    assert!(harness.query_by_label("Avoid in routes").is_some());
    let image = harness.render().expect("rendering failed");
    if let Some(path) = &path {
        let path = path.with_extension("menu.png");
        image.save(path).expect("could not write the screenshot");
    }
    // Choosing to avoid the room marks it on the map.
    harness.get_by_label("Avoid in routes").click();
    harness.run_steps(3);
    assert!(harness.state().tab().map.borrow().room(2).unwrap().avoid);
    // The map placed the rooms as walked: the patio is north-east of
    // the alley, one cell east of the square.
    let map = harness.state().tab().map.borrow();
    assert_eq!(map.room(7).unwrap().pos, Pos { x: 2, y: 2, z: 0 });
}
