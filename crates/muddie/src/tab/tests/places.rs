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

#[test]
fn reinos_de_leyenda_is_followed_by_the_rooms_it_shows_and_walked_in_its_words() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, mut sent) = fake_connection(app);
    desert(app);
    app.scripts.exec(
        "reinos",
        include_str!("../../../plugins/reinos-de-leyenda.luau"),
    );
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
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
    events.send(gmcp("Room.Movimiento", r#""este""#)).unwrap();
    events
        .send(gmcp("Room.Actual", r#""Dunas [|o|, e]""#))
        .unwrap();
    settle(&mut harness);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.map.borrow().current().unwrap().id, 2);
    assert!(
        notes(app)
            .iter()
            .any(|said| said.contains("knows where you are again: Dunas"))
    );
    events.send(gmcp("Room.Movimiento", r#""este""#)).unwrap();
    events
        .send(gmcp("Room.Actual", r#""Dunas [o, n]""#))
        .unwrap();
    settle(&mut harness);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.map.borrow().current().unwrap().id, 3);
    // To the well and back by the nearest of the dunes: sent as the MUD says.
    assert_eq!(
        answer(app, "/map travel pozo"),
        ["Going to Pozo, in Desierto: 1 move."]
    );
    assert_eq!(sent_lines(&mut sent), ["norte"]);
    events.send(gmcp("Room.Movimiento", r#""norte""#)).unwrap();
    events.send(gmcp("Room.Actual", r#""Pozo [s]""#)).unwrap();
    settle(&mut harness);
    let app = harness.state_mut().tab_mut();
    assert!(app.walk.is_none());
    assert_eq!(
        answer(app, "/map travel dunas"),
        ["Going to Dunas, in Desierto: 1 move. The nearest of 3 rooms of that name."]
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
    assert_eq!(
        said(app, "/map here 3"),
        ["Dunas (#3) in Desierto. Exits: norte, oeste."]
    );
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
    app.scripts.exec(
        "reinos",
        include_str!("../../../plugins/reinos-de-leyenda.luau"),
    );
    harness.run_steps(2);
    let said = harness.state().tab().notices().0.to_vec();
    let taking = (said.iter())
        .find(|notice| notice.text.starts_with("Taking the map from https://"))
        .unwrap();
    assert!(taking.corner);
    // Tests do not reach the web: it failed, and says so aside too.
    let failed = harness.state().tab().notices().0.last().cloned().unwrap();
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

#[test]
fn reinos_is_followed_whichever_comes_first_or_by_the_move_sent() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    desert(app);
    app.scripts.exec(
        "reinos",
        include_str!("../../../plugins/reinos-de-leyenda.luau"),
    );
    assert!(app.map.borrow_mut().set_current(1));
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    // The room before the move: one step all the same.
    events
        .send(gmcp("Room.Actual", r#""Dunas [o, e]""#))
        .unwrap();
    events.send(gmcp("Room.Movimiento", r#""este""#)).unwrap();
    settle(&mut harness);
    assert_eq!(harness.state().tab().map.borrow().current().unwrap().id, 2);
    // No word of the move: the one sent, by a key bound or typed, is it.
    harness.state_mut().tab_mut().send("e".into());
    events
        .send(gmcp("Room.Actual", r#""Dunas [o, n]""#))
        .unwrap();
    settle(&mut harness);
    assert_eq!(harness.state().tab().map.borrow().current().unwrap().id, 3);
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
    app.scripts.exec(
        "reinos",
        include_str!("../../../plugins/reinos-de-leyenda.luau"),
    );
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
    app.scripts.exec(
        "reinos",
        include_str!("../../../plugins/reinos-de-leyenda.luau"),
    );
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
    for shown in [
        "Vida 3900 of 4141",
        "Energía 1300 of 1300",
        "Fe 220 of 220",
        "Experiencia",
    ] {
        assert!(summary.contains(shown), "{shown} in {summary}");
    }
    // No shield now, and the trade is not shown.
    assert!(
        !summary.contains("Escudo") && !summary.contains("70"),
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
    app.scripts.exec(
        "reinos",
        include_str!("../../../plugins/reinos-de-leyenda.luau"),
    );
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
