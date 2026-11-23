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
        note: String::new(),
        renamed: false,
        added: false,
        shape: Default::default(),
        symbol: String::new(),
        styled: false,
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
        answer(app, "/map go pozo"),
        ["Going to Pozo: 1 move. Escape stops it."]
    );
    assert_eq!(sent_lines(&mut sent), ["norte"]);
    step("Pozo [s]", "norte");
    settle(&mut harness);
    let app = harness.state_mut().tab_mut();
    assert!(app.walk.is_none());
    assert_eq!(
        answer(app, "/map go dunas"),
        ["Going to Dunas: 1 move. The nearest of 3 rooms of that name. Escape stops it."]
    );
    assert_eq!(sent_lines(&mut sent), ["sur"]);
    // A move that fails stops the walk.
    app.run_command("/map go 1");
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
        ["Dunas (#3). In Desierto, at 6, 0, 0. Exits: norte, oeste. Marked casa."]
    );
    assert_eq!(said(app, "/map areas"), ["Desierto: 4 rooms"]);
    assert!(said(app, "/map unmark casa")[0].contains("No room is marked casa now"));
    // A note, said with the room and found by its words.
    assert_eq!(said(app, "/map note  Cactus seco "), ["Noted: Cactus seco"]);
    assert_eq!(
        said(app, "/map info"),
        ["Dunas (#3). In Desierto, at 6, 0, 0. Exits: norte, oeste. Note: Cactus seco."]
    );
    let found = app.palette_places("cactus");
    assert_eq!(
        (found[0].usage.as_str(), found[0].about.as_str()),
        ("Cactus seco", "note, Dunas · you are here")
    );
    assert_eq!(found[0].act, Some(commands::Act::Place("3".into())));
    assert_eq!(said(app, "/map unnote"), ["The note was taken off."]);
    assert!(app.palette_places("cactus").is_empty());
}

#[test]
fn a_map_of_mudlet_is_imported_from_a_file() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let refused = answer(app, "/map import");
    assert!(
        refused[0].contains("Say which map made with Mudlet"),
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

/// The session is connected, as the scripts are told.
fn connected_now(app: &mut Tab) {
    app.scripts.on_event("connect", "");
    app.run_actions();
}

#[test]
fn the_plugin_takes_the_servers_map_when_there_is_none_and_it_is_said_aside() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (_events, _sent) = fake_connection(app);
    app.set_map_shown(false);
    reinos(app);
    connected_now(app);
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

/// With a map, connecting leaves it as it is: it is brought up to date
/// only when the user asks, with what the plugin offers on the map.
#[test]
fn the_plugin_leaves_a_map_there_is_and_brings_it_up_to_date_when_asked() {
    let mut harness = window(None);
    let ctx = harness.ctx.clone();
    let app = harness.state_mut().tab_mut();
    let (_events, _sent) = fake_connection(app);
    desert(app);
    reinos(app);
    connected_now(app);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert!(app.map_source.taking.is_none());
    assert!(!notes(app).iter().any(|said| said.contains("map from")));
    // Offered in the palette, under the map, and in the map's menu.
    let sync = "Sync with the official map";
    assert_eq!(app.scripts.map_tools()[0].label, sync);
    assert_eq!(app.scripts.map_tools()[0].by, "Reinos de Leyenda");
    app.palette.query = "official map".into();
    let offered = app.palette_items();
    assert_eq!(
        (offered[0].usage.as_str(), offered[0].group.as_str()),
        (sync, "Map")
    );
    app.act(commands::Act::MapTool(sync.into()), &ctx);
    assert!(app.map_source.taking.is_some());
    assert!(last_said(app).starts_with("Taking the map from https://"));
    // Taken away with the plugin.
    app.scripts.unload("Reinos de Leyenda.luau");
    app.run_actions();
    assert!(app.scripts.map_tools().is_empty());
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
/// there was: all the plugin set that one up with goes with it, its words
/// for directions and its keeping the map up to date.
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
    let written = |title: &str| Event::Line(panels::styled_lines(title).remove(0));
    events.send(written("Pozo [s]")).unwrap();
    events.send(gmcp("Room.Movimiento", r#""norte""#)).unwrap();
    events.send(written("Cueva [s]")).unwrap();
    events.send(gmcp("Room.Movimiento", r#""norte""#)).unwrap();
    settle(&mut harness);
    let map = harness.state().tab().map.borrow();
    assert_eq!(map.current().unwrap().name, "Cueva");
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
        (said.iter()).any(|said| said.starts_with("The map has no exit norte from Pozo")),
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
        label(&harness).contains("Dunas · #1 · you are here · Exits: este"),
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
    assert!(focused(&harness, "Map view"));
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
    assert!(focused(&harness, "Map view"));
    harness.event(egui::Event::Text("m".into()));
    harness.run_steps(3);
    assert!(focused(&harness, "Command"));
    assert_eq!(harness.state().tab().input, "m");
    // Escape: back too, as it was.
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::E);
    harness.run_steps(3);
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
        r#"map.directions({e = {"este"}, w = {"oeste"}, n = {"norte"}, s = {"sur"}}) map.names({exits = "[]"}) bind("numpad6", "este") bind("numpadhome", "noroeste")"#,
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
    assert!(focused(&harness, "Search the map"));
    typed(&mut harness, "pozo");
    assert_eq!(
        said(&mut harness),
        "1 found. 1 of 1: Pozo, 3 moves to the este"
    );
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(said(&mut harness), "Pozo. 3 moves to the este. Exits: sur.");
    assert!(focused(&harness, "Map view"));
    // A note and a name, said with the room from then on.
    command(&mut harness, Key::N);
    assert!(focused(&harness, "Note on Pozo"));
    typed(&mut harness, "Agua fresca");
    assert_eq!(said(&mut harness), "Noted: Agua fresca");
    command(&mut harness, Key::B);
    typed(&mut harness, "agua");
    assert_eq!(said(&mut harness), "Marked agua.");
    assert!(focused(&harness, "Map view"));
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
    assert!(focused(&harness, "Map view"));
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

#[test]
fn the_game_is_played_in_the_language_its_profile_plugin_or_mud_says() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let spoken = |app: &Tab| {
        let map = app.map.borrow();
        (map.direction("noroeste"), map.command_for("nw").to_owned())
    };
    // Nothing said: English.
    app.speak_the_language();
    assert_eq!(spoken(app), ("noroeste".to_owned(), "nw".to_owned()));
    // The MUD says it over MSSP, by a name of the language.
    app.announce_language(&[("LANGUAGE".into(), "Spanish".into())]);
    assert_eq!(spoken(app), ("nw".to_owned(), "noroeste".to_owned()));
    // The profile's choice comes before the MUD's, and the plugin's.
    app.language.chosen = "en".into();
    app.scripts.exec("plugin", r#"map.language("es")"#);
    app.run_actions();
    assert_eq!(spoken(app).1, "nw");
    // Automatic, the plugin's is taken over the MUD's.
    app.language.chosen.clear();
    app.language.announced = Some("English".into());
    app.speak_the_language();
    assert_eq!(spoken(app).1, "noroeste");
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
    assert_eq!(
        answer(app, "/map undo"),
        ["Undone: Oasis is no longer on the map."]
    );
    assert!(app.map.borrow().room(id).is_none());
    assert_eq!(
        answer(app, "/map undo"),
        ["The map has changed nothing of itself to undo."]
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

/// The plugin of Reinos de Leyenda has the map keep itself up to date as
/// the player goes, a switch in the map's menu and the palette, on unless
/// switched off: what the map learns is told together a moment after, and
/// undone the last first. (What it learns, and when, is the map's own:
/// see its tests.)
#[test]
fn reinos_has_the_map_keep_itself_up_to_date_unless_switched_off() {
    let mut harness = window(None);
    let ctx = harness.ctx.clone();
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    desert(app);
    let said_there = |id: RoomId, name: &str, exits: &[(&str, RoomId)]| {
        let mut map = app.map.borrow_mut();
        let room = map.room(id).unwrap().clone();
        let info = RoomInfo {
            id,
            name: name.into(),
            area: room.area,
            exits: (exits.iter())
                .map(|(way, to)| ((*way).to_owned(), *to))
                .collect(),
            pos: Some(room.pos),
            colour: None,
        };
        map.set_room(info).unwrap();
    };
    said_there(2, "Moony te dice: hola", &[("w", 1), ("e", 3)]);
    said_there(3, "", &[("w", 2), ("n", 4)]);
    reinos(app);
    let learning = "Keep the map up to date as you go";
    let switched = |app: &Tab| {
        (app.scripts.map_tools().into_iter())
            .map(|tool| (tool.label, tool.checked))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        switched(app),
        [
            ("Sync with the official map".into(), None),
            (learning.into(), Some(true))
        ]
    );
    app.palette.query = "up to date".into();
    assert_eq!(app.palette_items()[0].usage, format!("{learning} (on)"));
    app.palette.query.clear();
    let step = |title: &str, way: &str| {
        let line = panels::styled_lines(title).remove(0);
        events.send(Event::Line(line)).unwrap();
        let moved = Event::Gmcp {
            package: "Room.Movimiento".into(),
            data: format!("{way:?}"),
        };
        events.send(moved).unwrap();
    };
    let here = |harness: &Harness<'_, App>| {
        let map = harness.state().tab().map.borrow();
        map.current().map(|room| (room.id, room.name.clone()))
    };
    // Through the dunes named wrong to the well: both are named; north of
    // the well, where the map has nothing, a cave is added.
    assert!(harness.state().tab().map.borrow_mut().set_current(1));
    step("Dunas [o,e]", "este");
    step("Dunas [o,n]", "este");
    step("Pozo [s]", "norte");
    step("Cueva [s]", "norte");
    harness.run_steps(2);
    let cave = muddie_mapper::ADDED_FROM;
    assert_eq!(here(&harness), Some((cave, "Cueva".into())));
    let ctx2 = harness.ctx.clone();
    let app = harness.state_mut().tab_mut();
    app.tell_map_changes(&ctx2, Instant::now() + Duration::from_secs(5));
    assert_eq!(
        last_said(app),
        "Map: 1 room added, 2 names put right. To undo the last: Undo the map's last change, in the palette."
    );
    app.run_command("/map undo");
    assert_eq!(last_said(app), "Undone: Cueva is no longer on the map.");
    app.run_command("/map undo");
    assert_eq!(last_said(app), "Undone: “Dunas” has no name again.");
    // Switched off, nothing is learnt.
    app.act(commands::Act::MapTool(learning.into()), &ctx);
    assert_eq!(switched(app)[1], (learning.into(), Some(false)));
    assert!(app.map.borrow_mut().set_current(4));
    step("Pozo [s]", "norte");
    harness.run_steps(2);
    assert!(harness.state().tab().map.borrow().added().is_empty());
}

/// A map with all in one area, as Reinos de Leyenda's is: tidying it would
/// draw every room again, so it is done only when told "all".
#[test]
fn a_very_large_area_is_tidied_only_when_told_all() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let rooms: Vec<Room> = (1..=2001)
        .map(|id: RoomId| Room {
            id,
            name: "Camino".into(),
            area: "Mapa Completo".into(),
            pos: Pos {
                // Tangled: ten rooms on each cell.
                x: id as i32 * 3 % 30,
                y: 0,
                z: 0,
            },
            exits: [(
                "e".to_owned(),
                Exit {
                    to: id % 2001 + 1,
                    ..Exit::default()
                },
            )]
            .into(),
            weight: 1,
            avoid: false,
            colour: None,
            note: String::new(),
            renamed: false,
            added: false,
            shape: Default::default(),
            symbol: String::new(),
            styled: false,
        })
        .collect();
    app.map.borrow_mut().replace_all(rooms, None).unwrap();
    assert!(app.map.borrow_mut().set_current(1));
    let x_of_2 = |app: &Tab| app.map.borrow().room(2).unwrap().pos.x;
    assert_eq!(
        answer(app, "/map tidy"),
        [
            "Mapa Completo has 2001 rooms: tidying it would draw them all again, and what was drawn by hand would be lost."
        ]
    );
    assert_eq!(x_of_2(app), 6);
    answer(app, "/map tidy all");
    assert_eq!(x_of_2(app), 4);
    // Tidy, it is not drawn again: the exit that closes the ring points
    // the wrong way however it is drawn.
    assert_eq!(
        answer(app, "/map tidy all"),
        ["The map is as tidy as it gets."]
    );
}

/// The appearance of the map, from its menu, and of a room, from the
/// room's: for it, or for every room called as it is; a room of no colour
/// or shape of its own takes the map's.
#[test]
fn the_appearance_of_the_map_and_of_rooms_is_chosen() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.started = true;
    let (_events, _sent) = fake_connection(app);
    desert(app);
    let called = app.map.borrow().called("Dunas");
    assert!(called.len() > 1);
    let shape =
        |harness: &Harness<'_, App>, id| harness.state().tab().map.borrow().room(id).unwrap().shape;
    // For the room alone.
    harness.state_mut().tab_mut().open_look(called[0]);
    harness.run_steps(2);
    harness.get_by_label("Circle").click();
    harness.run_steps(1);
    harness.get_by_label("Apply").click();
    harness.run_steps(2);
    assert_eq!(
        shape(&harness, called[0]),
        Some(muddie_mapper::Shape::Circle)
    );
    assert_eq!(shape(&harness, called[1]), None);
    // For all called so, with a symbol.
    harness.state_mut().tab_mut().open_look(called[1]);
    harness.run_steps(2);
    harness.get_by_label("Diamond").click();
    harness.get_by_label("A character or two").click();
    harness.run_steps(1);
    harness.get_by_label("A character or two").type_text("~");
    harness
        .get_by_label(&format!("All {} rooms called Dunas", called.len()))
        .click();
    harness.run_steps(1);
    harness.get_by_label("Apply").click();
    harness.run_steps(2);
    let map = harness.state().tab().map.borrow();
    for id in &called {
        let room = map.room(*id).unwrap();
        assert_eq!(
            (room.shape, room.symbol.as_str(), room.styled),
            (Some(muddie_mapper::Shape::Diamond), "~", true)
        );
    }
    drop(map);
    assert!(harness.state().tab().look.is_none());
    // The map's own, for rooms of none of their own; one given back the
    // map's shape draws in it.
    harness.state_mut().tab_mut().open_map_appearance();
    harness.run_steps(2);
    harness.get_by_label("Hexagon").click();
    harness.get_by_label("blue").click();
    harness.run_steps(1);
    harness.get_by_label("Apply").click();
    harness.run_steps(2);
    let appearance = harness.state().tab().map.borrow().appearance();
    assert_eq!(appearance.shape, muddie_mapper::Shape::Hexagon);
    assert!(appearance.colour.is_some());
    harness.state_mut().tab_mut().open_look(called[0]);
    harness.run_steps(2);
    harness.get_by_label("The map's").click();
    harness.run_steps(1);
    harness.get_by_label("Apply").click();
    harness.run_steps(2);
    assert_eq!(shape(&harness, called[0]), None);
}
