//! Following Reinos de Leyenda with its plugin: its map, its lag, its gauges and its updating of the map.

use super::*;

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
    // The map is drawn all the same, about the rooms it may be: shown,
    // it can be explored with the keys.
    app.set_map_shown(true);
    harness.run_steps(3);
    assert!(map_widget(&harness).is_some());
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
        ["Going to Dunas: 1 move. The nearest of 3 rooms that match. Escape stops it."]
    );
    assert_eq!(sent_lines(&mut sent), ["sur"]);
    // A move that fails stops the walk.
    app.run_command("/map go 1");
    assert_eq!(sent_lines(&mut sent), ["sur"]);
    events.send(gmcp("Room.MovimientoErroneo", "{}")).unwrap();
    settle(&mut harness);
    assert!(harness.state().tab().walk.is_none());
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
    let said = harness.state().tab().messages().0.to_vec();
    let taking = (said.iter())
        .find(|message| message.text.starts_with("Importing the map from https://"))
        .unwrap();
    assert!(taking.corner);
    // Tests do not reach the web: it failed, and says so aside too, once
    // the thread that tried has said so, however busy the machine.
    let failed = |harness: &Harness<'_, App>| {
        (harness.state().tab().messages().0.iter())
            .find(|message| message.text.starts_with("Could not import the map"))
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
    assert!(failed.text.starts_with("Could not import the map"));
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
    assert!(app.map_source.importing.is_none());
    assert!(!notes(app).iter().any(|said| said.contains("map from")));
    // Offered in the palette, under the map, and in the map's menu.
    let sync = "Sync with the official map";
    assert_eq!(app.scripts.map_actions()[0].label, sync);
    let owner = app.scripts.map_actions()[0].owner.clone();
    assert_eq!(app.offered_by(&owner), "Reinos de Leyenda");
    // One of the player's, by its own name.
    assert_eq!(app.offered_by("scripts/Mapping/rooms.luau"), "rooms");
    app.palette.query = "official map".into();
    let offered = app.palette_items();
    assert_eq!(
        (offered[0].usage.as_str(), offered[0].group.as_str()),
        (sync, "Map")
    );
    app.act(commands::Act::MapAction(sync.into()), &ctx);
    assert!(app.map_source.importing.is_some());
    assert!(last_said(app).starts_with("Importing the map from https://"));
    // Taken away with the plugin.
    app.scripts.unload("Reinos de Leyenda/reinos.luau");
    app.run_actions();
    assert!(app.scripts.map_actions().is_empty());
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
    // Neither the shield, nor experience, nor the trade is shown.
    assert!(
        !summary.contains("Escudo") && !summary.contains("Experiencia") && !summary.contains("70"),
        "{summary}"
    );
    // States come and go, among the effects, by their short name; the
    // worst kinds first.
    let effects = |harness: &Harness<'_, App>| -> String {
        let summary = harness.state().tab().gauges.summary();
        let at = summary.find("Efectos").unwrap_or(summary.len());
        summary[at..]
            .split(", Vida")
            .next()
            .unwrap_or_default()
            .to_owned()
    };
    events
        .send(gmcp(
            "Char.Status.Estados.set",
            r#"{"debilidad":{"nombre":"debilidad","corto":"Debilidad","ayuda":"Vieja","tipo":"incapacidades"}}"#,
        ))
        .unwrap();
    events
        .send(gmcp(
            "Char.Status.Estados.add",
            r#"{"nombre":"dañadora","corto":"Enfermedad Dañadora","tipo":"enfermedades","ayuda":"Mortal"}"#,
        ))
        .unwrap();
    harness.run_steps(2);
    assert_eq!(effects(&harness), "Efectos: Enfermedad Dañadora, Debilidad");
    events
        .send(gmcp(
            "Char.Status.Estados.remove",
            r#"{"nombre":"dañadora","corto":"Enfermedad Dañadora","tipo":"enfermedades"}"#,
        ))
        .unwrap();
    harness.run_steps(2);
    assert_eq!(effects(&harness), "Efectos: Debilidad");
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
        (app.scripts.map_actions().into_iter())
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
    app.undo_change();
    assert_eq!(last_said(app), "Undone: Cueva is no longer on the map.");
    app.undo_change();
    assert_eq!(last_said(app), "Undone: “Dunas” has no name again.");
    // Switched off, nothing is learnt.
    app.act(commands::Act::MapAction(learning.into()), &ctx);
    assert_eq!(switched(app)[1], (learning.into(), Some(false)));
    assert!(app.map.borrow_mut().set_current(4));
    step("Pozo [s]", "norte");
    harness.run_steps(2);
    assert!(harness.state().tab().map.borrow().added().is_empty());
}

/// That nothing comes over GMCP is told in the game, not while the player is
/// yet to enter it, and not if it comes.
#[test]
fn that_nothing_comes_is_told_once_in_the_game() {
    let told = |app: &Tab| {
        let said = |note: &&String| note.contains("consentir gmcp on");
        notes(app).iter().filter(said).count()
    };
    for (line, gmcp, times) in [
        ("Introduce tu nombre:", false, 0),
        ("Novedades [ver]", false, 0),
        ("Anduar: Plaza Mayor [|so|,s,e,n,o]", false, 1),
        ("Anduar: Plaza Mayor [|so|,s,e,n,o]", true, 0),
    ] {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        let (_events, _sent) = fake_connection(app);
        desert(app);
        reinos(app);
        connected_now(app);
        for _ in 0..2 {
            app.scripts.on_line(line);
        }
        if gmcp {
            app.scripts.on_gmcp("Char.Status", "{}");
        }
        app.scripts.tick(Instant::now() + Duration::from_secs(100));
        app.run_actions();
        assert_eq!(told(app), times, "{line} {gmcp}");
        // Connected again before it was told, it is not told of that time.
        if times == 1 {
            connected_now(app);
            app.scripts.on_line(line);
            connected_now(app);
            app.scripts.tick(Instant::now() + Duration::from_secs(100));
            app.run_actions();
            assert_eq!(told(app), 1);
            app.scripts.on_line(line);
            app.scripts.tick(Instant::now() + Duration::from_secs(300));
            app.run_actions();
            assert_eq!(told(app), 2);
            continue;
        }
        // Once for each connection.
        app.scripts.on_line(line);
        app.scripts.tick(Instant::now() + Duration::from_secs(300));
        app.run_actions();
        assert_eq!(told(app), times, "{line} {gmcp}");
    }
}
