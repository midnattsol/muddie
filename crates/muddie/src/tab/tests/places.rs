//! The map in words, and following a MUD that shows rooms by name only,
//! as Reinos de Leyenda does, with its plugin.

use super::*;
use muddie_mapper::{Exit, Room};

mod drawing;
mod exploring;
mod prompts;
mod reading;
mod reinos;

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
        cost: 0,
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
    let before = app.messages().0.len();
    app.run_command(command);
    // A line, over the corner; a list, in the palette.
    if app.palette.results.is_none() {
        return (app.messages().0[before..].iter())
            .filter(|message| message.corner)
            .map(|message| message.text.clone())
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
    (app.messages().0.last()).map_or_else(String::new, |message| message.text.clone())
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
    let plugin = &crate::plugins::fixture::reinos();
    for (name, code) in &plugin.scripts {
        (app.scripts).offer_module(&format!("{}/{name}", plugin.folder()), code);
    }
    for (name, code) in &plugin.scripts {
        app.scripts
            .exec_owned(&format!("{}/{name}", plugin.folder()), name, code);
    }
    app.rules.gauges.extend(plugin.read_rules().unwrap().gauges);
    app.run_rules();
    app.run_actions();
}

#[test]
fn the_map_is_said_in_words() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    desert(app);
    app.scripts.exec(
        "t",
        r#"map.directions = {n = {"norte"}, s = {"sur"}, e = {"este"}, w = {"oeste", "o"}}"#,
    );
    let said = answer;
    let unknown = said(app, "/map exits");
    assert!(
        unknown[0].contains("does not know where you are"),
        "{unknown:?}"
    );
    assert_eq!(said(app, "/map here 3"), ["Dunas. Exits: norte, oeste."]);
    assert_eq!(said(app, "/map exits"), ["norte: Pozo", "oeste: Dunas"]);
    assert_eq!(said(app, "/map way 1"), ["Dunas [e], 2 moves: 2 oeste."]);
    assert_eq!(
        said(app, "/map near"),
        ["Pozo: 1 move: norte", "Dunas [e]: 2 moves: 2 oeste"]
    );
    said(app, "/map mark casa");
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
    // An exit listed is gone through when chosen, as one move.
    let (_events, mut sent) = fake_connection(app);
    assert!(app.do_client_command("/map through oeste"));
    assert_eq!(sent_lines(&mut sent), ["oeste"]);
    assert!(app.walk.is_some());
    assert!(said(app, "/map through este")[0].contains("no such exit"));
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
        if harness.state().tab().map_source.importing.is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let said = last_said(harness.state().tab());
    assert!(said.starts_with("Could not import the map"), "{said}");
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
        let taking = app.map_source.importing.as_ref();
        taking.map(|(_, _, why)| *why)
    };
    // Taken before readers were noted, or by an older one: whole.
    assert!(why_taken(None) == Some(crate::tab::places::ImportWhy::Script));
    let older = muddie_mapper::MUDLET_READER - 1;
    assert!(why_taken(Some(older)) == Some(crate::tab::places::ImportWhy::Script));
    // By this one: only if newer than the version it has.
    let current = Some(muddie_mapper::MUDLET_READER);
    assert!(why_taken(current) == Some(crate::tab::places::ImportWhy::Update));
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
    app.scripts.exec("plugin", r#"map.language = "es""#);
    app.run_actions();
    assert_eq!(spoken(app).1, "nw");
    // Automatic, the plugin's is taken over the MUD's.
    app.language.chosen.clear();
    app.language.announced = Some("English".into());
    app.speak_the_language();
    assert_eq!(spoken(app).1, "noroeste");
}
