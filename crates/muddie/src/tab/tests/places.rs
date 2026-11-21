//! The map in words, and following a MUD that shows rooms by name only,
//! as Reinos de Leyenda does, with its plugin.

use super::*;
use muddie_mapper::{Exit, Room};

/// A desert of dunes alike, 1 - 2 - 3 from west to east, with a well
/// north of the last, written as a map taken from Mudlet would be.
fn desert(app: &mut Tab) {
    let room = |id: RoomId, name: &str, exits: &[(&str, RoomId)]| Room {
        id,
        name: name.to_owned(),
        area: "Desierto".to_owned(),
        pos: Pos {
            x: id as i32 * 2,
            y: 0,
            z: 0,
        },
        exits: (exits.iter())
            .map(|(way, to)| {
                (
                    (*way).to_owned(),
                    Exit {
                        to: *to,
                        ..Exit::default()
                    },
                )
            })
            .collect(),
        weight: 1,
        avoid: false,
        colour: None,
    };
    let rooms = vec![
        room(1, "Dunas [e]", &[("e", 2)]),
        room(2, "Dunas", &[("w", 1), ("e", 3)]),
        room(3, "Dunas", &[("w", 2), ("n", 4)]),
        room(4, "Pozo", &[("s", 3)]),
    ];
    app.map.borrow_mut().replace_all(rooms, None).unwrap();
}

/// What a command answers: each line, with what it is about if anything.
fn answer(app: &mut Tab, command: &str) -> Vec<String> {
    let before = app.notices().0.len();
    app.run_command(command);
    // A line, over the corner; a list, in the palette.
    if app.palette.results.is_none() {
        return (app.notices().0[before..].iter())
            .filter(|notice| notice.corner)
            .map(|notice| notice.text.clone())
            .collect();
    }
    (app.palette.results.iter().flatten())
        .map(|item| match item.about.as_str() {
            "" => item.usage.clone(),
            about => format!("{}: {about}", item.usage),
        })
        .collect()
}

/// The last thing the client said of itself.
fn last_said(app: &Tab) -> String {
    (app.notices().0.last()).map_or_else(String::new, |notice| notice.text.clone())
}

/// Steps the window once what the plugin of Reinos waits for, to take the
/// messages of one step together, has passed.
fn settle(harness: &mut Harness<'_, App>) {
    harness.run_steps(1);
    std::thread::sleep(Duration::from_millis(80));
    harness.run_steps(2);
}

fn sent_lines(sent: &mut tokio::sync::mpsc::UnboundedReceiver<Command>) -> Vec<String> {
    std::iter::from_fn(|| sent.try_recv().ok())
        .filter_map(|command| match command {
            Command::Send(line) => Some(line),
            _ => None,
        })
        .collect()
}

/// Reinos de Leyenda's plugin, as installed: its script running, and its
/// rules among the session's.
fn reinos(app: &mut Tab) {
    let plugin = crate::bundled::find("reinos-de-leyenda").unwrap();
    for (name, code) in plugin.scripts {
        app.scripts.exec(name, code);
    }
    app.rules.gauges.extend(plugin.read_rules().gauges);
    app.run_rules();
    app.run_actions();
}

#[test]
fn reinos_de_leyenda_is_followed_by_the_rooms_it_shows_and_walked_in_its_words() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, mut sent) = fake_connection(app);
    desert(app);
    reinos(app);
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    // A step as Reinos takes one: the room in its text, then the move.
    let step = |title: &str, way: &str| {
        let line = panels::styled_lines(title).remove(0);
        events.send(Event::Line(line)).unwrap();
        events
            .send(gmcp("Room.Movimiento", &format!("{way:?}")))
            .unwrap();
    };
    // Three dunes, one written with its exits: the map is unsure.
    events.send(gmcp("Room.Actual", r#""Dunas""#)).unwrap();
    settle(&mut harness);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.map.borrow().candidates(), [1, 2, 3]);
    assert!(
        notes(app)
            .iter()
            .any(|said| said.contains("in one of 3 rooms alike"))
    );
    // East, to a dune with a door west and an exit east: of the two
    // east of a dune, the middle one.
    step("Dunas [|o|,e]", "este");
    settle(&mut harness);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.map.borrow().current().unwrap().id, 2);
    assert!(
        notes(app)
            .iter()
            .any(|said| said.contains("knows where you are again: Dunas"))
    );
    step("Dunas [o,n]", "este");
    settle(&mut harness);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.map.borrow().current().unwrap().id, 3);
    // To the well and back by the nearest of the dunes: sent as the MUD says.
    assert_eq!(
        answer(app, "/map travel pozo"),
        ["Going to Pozo: 1 move. Escape stops it."]
    );
    assert_eq!(sent_lines(&mut sent), ["norte"]);
    step("Pozo [s]", "norte");
    settle(&mut harness);
    let app = harness.state_mut().tab_mut();
    assert!(app.walk.is_none());
    assert_eq!(
        answer(app, "/map travel dunas"),
        ["Going to Dunas: 1 move. The nearest of 3 rooms of that name. Escape stops it."]
    );
    assert_eq!(sent_lines(&mut sent), ["sur"]);
    // A move that fails stops the walk.
    app.run_command("/map travel 1");
    assert_eq!(sent_lines(&mut sent), ["sur"]);
    events.send(gmcp("Room.MovimientoErroneo", "{}")).unwrap();
    settle(&mut harness);
    assert!(harness.state().tab().walk.is_none());
}

#[test]
fn the_map_is_said_in_words() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    desert(app);
    app.scripts.exec(
        "t",
        r#"map.directions({n = {"norte"}, s = {"sur"}, e = {"este"}, w = {"oeste", "o"}})"#,
    );
    let said = answer;
    let unknown = said(app, "/map exits");
    assert!(
        unknown[0].contains("does not know where you are"),
        "{unknown:?}"
    );
    assert_eq!(said(app, "/map here 3"), ["Dunas. Exits: norte, oeste."]);
    assert_eq!(said(app, "/map exits"), ["norte: Pozo", "oeste: Dunas"]);
    assert_eq!(said(app, "/map route 1"), ["Dunas [e], 2 moves: 2 oeste."]);
    assert_eq!(
        said(app, "/map near"),
        ["Pozo: 1 move: norte", "Dunas [e]: 2 moves: 2 oeste"]
    );
    said(app, "/map mark casa");
    assert_eq!(said(app, "/map mark"), ["casa: Dunas"]);
    assert_eq!(
        said(app, "/map info casa"),
        ["Dunas (#3). In Desierto, at 6, 0, 0. Exits: norte, oeste. Called casa."]
    );
    assert_eq!(said(app, "/map areas"), ["Desierto: 4 rooms"]);
    assert!(said(app, "/map unmark casa")[0].contains("No room is called casa now"));
}

#[test]
fn a_map_of_mudlet_is_imported_from_a_file() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let refused = answer(app, "/map import");
    assert!(
        refused[0].contains("Give a map made with Mudlet"),
        "{refused:?}"
    );
    app.run_command("/map import /no/such/map.dat");
    for _ in 0..50 {
        harness.run_steps(1);
        if harness.state().tab().map_source.taking.is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let said = last_said(harness.state().tab());
    assert!(said.starts_with("Could not take the map"), "{said}");
}

#[test]
fn the_plugin_takes_the_servers_map_at_once_and_it_is_said_aside() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (_events, _sent) = fake_connection(app);
    app.set_map_shown(false);
    reinos(app);
    harness.run_steps(2);
    let said = harness.state().tab().notices().0.to_vec();
    let taking = (said.iter())
        .find(|notice| notice.text.starts_with("Taking the map from https://"))
        .unwrap();
    assert!(taking.corner);
    // Tests do not reach the web: it failed, and says so aside too, once
    // the thread that tried has said so, however busy the machine.
    let failed = |harness: &Harness<'_, App>| {
        (harness.state().tab().notices().0.iter())
            .find(|notice| notice.text.starts_with("Could not take the map"))
            .cloned()
    };
    for _ in 0..100 {
        if failed(&harness).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        harness.run_steps(1);
    }
    let failed = failed(&harness).expect("the failure said");
    assert!(failed.corner && failed.problem, "{failed:?}");
    assert!(failed.text.starts_with("Could not take the map"));
    // Over the corner, though the game is in sight.
    harness.get_by_label(&failed.text);
    assert!(
        !output_lines(harness.state().tab())
            .iter()
            .any(|line| line.contains("map from"))
    );
}

/// As a session with Reinos de Leyenda showed: with lag, steps come in
/// bursts, each the room written and then the move, and none is lost; a
/// map gone astray is put right by the room written; and looking, which
/// writes the room with no move, puts it right too.
#[test]
fn reinos_is_followed_through_lag_and_put_right_by_the_room_written() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    desert(app);
    reinos(app);
    let here = |harness: &Harness<'_, App>| {
        harness
            .state()
            .tab()
            .map
            .borrow()
            .current()
            .map(|room| room.id)
    };
    let line = |text: &str| Event::Line(panels::styled_lines(text).remove(0));
    let moved = |way: &str| Event::Gmcp {
        package: "Room.Movimiento".into(),
        data: format!("{way:?}"),
    };
    // Two steps in one burst: both are taken.
    assert!(harness.state().tab().map.borrow_mut().set_current(1));
    for event in [
        line("Dunas [o,e]"),
        moved("este"),
        // After the prompt, which ends in no new line.
        line("] Dunas [o,n]"),
        moved("este"),
    ] {
        events.send(event).unwrap();
    }
    harness.run_steps(2);
    assert_eq!(here(&harness), Some(3));
    // Astray, three rooms off: the room written puts it right.
    assert!(harness.state().tab().map.borrow_mut().set_current(1));
    events.send(line("Pozo [s]")).unwrap();
    events.send(moved("norte")).unwrap();
    harness.run_steps(2);
    assert_eq!(here(&harness), Some(4));
    // Looking: the room written, and no move after it.
    assert!(harness.state().tab().map.borrow_mut().set_current(2));
    events.send(line("Dunas [o,n]")).unwrap();
    harness.run_steps(1);
    std::thread::sleep(Duration::from_millis(300));
    harness.run_steps(2);
    assert_eq!(here(&harness), Some(3));
    // A line that ends in brackets but is no room is left alone.
    events.send(line("[Chat] Ana: hola [jaja]")).unwrap();
    harness.run_steps(1);
    std::thread::sleep(Duration::from_millis(300));
    harness.run_steps(2);
    assert_eq!(here(&harness), Some(3));
}

/// As Reinos de Leyenda does, in a record of a session: the room shown once,
/// on logging in, and after that the moves alone. The plugin is loaded
/// before connecting, which opens the session's map in place of the one
/// there was: what the plugin taught that one goes with it.
#[test]
fn reinos_is_followed_by_its_moves_after_connecting_opened_the_map() {
    let mut harness = window(None);
    let ctx = harness.ctx.clone();
    let app = harness.state_mut().tab_mut();
    reinos(app);
    app.run_actions();
    // Connecting opens the map of where it leads; nothing listens there.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _inside = runtime.enter();
    app.host = "127.0.0.1".into();
    app.port = "9".into();
    app.connect(&ctx);
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    desert(app);
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    events.send(gmcp("Room.Actual", r#""Dunas [e]""#)).unwrap();
    settle(&mut harness);
    assert_eq!(harness.state().tab().map.borrow().current().unwrap().id, 1);
    for to in [2, 3] {
        harness.state_mut().tab_mut().send("este".into());
        events.send(gmcp("Room.Movimiento", r#""este""#)).unwrap();
        settle(&mut harness);
        assert_eq!(harness.state().tab().map.borrow().current().unwrap().id, to);
    }
}

/// What Reinos de Leyenda sends of the character, as a session shows it:
/// the gauges, all at first and then what changed; the states; the
/// channels, opened and closed.
#[test]
fn reinos_gives_gauges_states_and_channels_by_themselves() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    desert(app);
    reinos(app);
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    events.send(gmcp("Char.Status", r#"{"xp_oficio":{"max":70,"min":23},"xp":{"max":5726304,"min":1964952},"pe":{"max":1300,"min":1300},"fe":{"max":220,"min":220},"escudo":{"max":0,"min":0},"pvs":{"max":4141,"min":4141}}"#)).unwrap();
    events
        .send(gmcp("Char.Status", r#"{"pvs":{"min":3900}}"#))
        .unwrap();
    harness.run_steps(2);
    let summary = harness.state().tab().gauges.summary();
    for shown in ["Vida 3900 of 4141", "Energía 1300 of 1300", "Fe 220 of 220"] {
        assert!(summary.contains(shown), "{shown} in {summary}");
    }
    // No shield now, and neither experience nor the trade is shown.
    assert!(
        !summary.contains("Escudo") && !summary.contains("Experiencia") && !summary.contains("70"),
        "{summary}"
    );
    // States come and go, among the gauges.
    events
        .send(gmcp(
            "Char.Status.Estados.add",
            r#"{"nombre":"Envenenado","corto":"ven","tipo":"veneno","ayuda":"Pierdes vida"}"#,
        ))
        .unwrap();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert!(app.gauges.summary().contains("Envenenado"));
    events
        .send(gmcp(
            "Char.Status.Estados.remove",
            r#"{"nombre":"Envenenado"}"#,
        ))
        .unwrap();
    harness.run_steps(2);
    assert!(
        !harness
            .state()
            .tab()
            .gauges
            .summary()
            .contains("Envenenado")
    );
    // Channels: each in its tab, and gone when left.
    events
        .send(gmcp("Comm.Canales", r#"{"chat":"chat","keel":"keel"}"#))
        .unwrap();
    events
        .send(gmcp("Comm.ApagaCanal", r#"{"canal":"keel"}"#))
        .unwrap();
    harness.run_steps(2);
    let channels: Vec<String> = (harness.state().tab().panels.channels.iter())
        .map(|channel| channel.name.clone())
        .collect();
    assert_eq!(channels, ["chat"]);
}

#[test]
fn a_move_by_an_exit_the_map_does_not_have_is_said() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    desert(app);
    reinos(app);
    assert!(app.map.borrow_mut().set_current(4));
    events
        .send(Event::Gmcp {
            package: "Room.Movimiento".into(),
            data: r#""norte""#.into(),
        })
        .unwrap();
    settle(&mut harness);
    let app = harness.state().tab();
    assert_eq!(app.map.borrow().current().unwrap().id, 4);
    let said = notes(app);
    assert!(
        (said.iter()).any(|said| said.starts_with("El mapa no tiene la salida norte desde Pozo")),
        "{said:?}"
    );
}

#[test]
fn a_map_read_by_an_older_reader_is_taken_whole_again() {
    let address = "http://127.0.0.1:9/map.dat";
    let why_taken = |reader: Option<u32>| {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        let (_events, _sent) = fake_connection(app);
        {
            let mut map = app.map.borrow_mut();
            map.set_source(Some((address, Some("v1")))).unwrap();
            map.set_source_reader(reader).unwrap();
        }
        app.import_for_script(address.to_owned());
        let taking = app.map_source.taking.as_ref();
        taking.map(|(_, _, why)| *why)
    };
    // Taken before readers were noted, or by an older one: whole.
    assert!(why_taken(None) == Some(crate::tab::places::Taking::Script));
    let older = muddie_mapper::MUDLET_READER - 1;
    assert!(why_taken(Some(older)) == Some(crate::tab::places::Taking::Script));
    // By this one: only if newer than the version it has.
    let current = Some(muddie_mapper::MUDLET_READER);
    assert!(why_taken(current) == Some(crate::tab::places::Taking::Update));
}

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
        .filter(|item| matches!(item.act, Some(super::super::commands::Act::Place(_))))
        .map(|item| (item.usage.clone(), item.about.clone()))
        .collect();
    assert_eq!(
        places,
        [("Dunas".to_owned(), "1 move · nearest of 3".to_owned())]
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
    assert!(notes(app).iter().any(|said| said == "Walk stopped."));
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
    let middle = harness.get_by_label("Map view").rect().center();
    harness.hover_at(middle);
    harness.run_steps(1);
    harness.drag_at(middle);
    harness.run_steps(1);
    harness.drop_at(middle);
    harness.run_steps(2);
    let label = |harness: &Harness<'_, App>| {
        harness
            .get_by_label_contains("Map view")
            .accesskit_node()
            .label()
            .unwrap_or_default()
    };
    assert!(
        label(&harness).contains("Dunas · #1 · you are here · exits: este"),
        "{}",
        label(&harness)
    );
    // East along the exits, twice: two moves away, said.
    harness.key_press(egui::Key::ArrowRight);
    harness.key_press(egui::Key::ArrowRight);
    harness.run_steps(2);
    assert!(
        label(&harness).contains("#3 · 2 moves"),
        "{}",
        label(&harness)
    );
    // No exit south: said so, and it stays.
    harness.key_press(egui::Key::ArrowDown);
    harness.run_steps(2);
    assert!(label(&harness).contains("#3"));
    // Enter walks there.
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert_eq!(sent_lines(&mut sent), ["este"]);
    // Escape: none chosen.
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert_eq!(label(&harness), "Map view");
}
