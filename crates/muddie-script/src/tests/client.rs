//! What scripts are told of the client, and ask of it: the room the player
//! moves to, a list to choose from, the clipboard, and whether the game is
//! read aloud.

use super::*;

fn echoes(actions: Vec<Action>) -> Vec<String> {
    (actions.into_iter())
        .filter_map(|action| match action {
            Action::Echo(said) => Some(said.text),
            _ => None,
        })
        .collect()
}

#[test]
fn a_script_is_told_when_the_player_is_in_another_room() {
    let mut engine =
        engine(r#"on("room", function(room, from) echo(`{room.name} {room.id} from {from}`) end)"#);
    // Nothing while the map does not know where the player is.
    engine.tick(Instant::now());
    assert_eq!(engine.take_actions(), []);
    engine.exec(
        "map",
        r#"map.visit({id = 1, name = "Gate"}) map.visit({id = 1, name = "Gate"})"#,
    );
    engine.tick(Instant::now());
    engine.exec("map", r#"map.visit({id = 2, name = "Square"})"#);
    engine.tick(Instant::now());
    // The same room again is no move.
    engine.tick(Instant::now());
    let said = echoes(engine.take_actions());
    assert_eq!(said, ["Gate 1 from nil", "Square 2 from 1"]);
    // Waited for, as anything listened to.
    engine.exec(
        "walker",
        r#"coroutine.wrap(function() local room = wait("room") echo("now in " .. room.name) end)()"#,
    );
    engine.exec("map", r#"map.visit({id = 3, name = "Tower"})"#);
    engine.tick(Instant::now());
    let said = echoes(engine.take_actions());
    assert!(said.contains(&"now in Tower".to_owned()), "{said:?}");
}

#[test]
fn a_script_offers_a_list_and_is_told_what_was_chosen() {
    let mut engine = engine("");
    engine.exec(
        "tells",
        r#"choose({"Ana", {text = "Bob", about = {en = "2 tells", es = "2 telepatías"}}},
            function(index, text) echo(`{index} {text}`) end,
            {title = "Reply to"})"#,
    );
    let actions = engine.take_actions();
    let [Action::Choose { id, title, items }] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    assert_eq!(title, "Reply to");
    assert_eq!(
        items,
        &[
            ("Ana".to_owned(), String::new()),
            ("Bob".to_owned(), "2 tells".to_owned())
        ]
    );
    engine.chosen(*id, Some((1, "Bob")));
    assert_eq!(echoes(engine.take_actions()), ["2 Bob"]);
    // Told once: an answer after that is nothing.
    engine.chosen(*id, Some((0, "Ana")));
    assert_eq!(engine.take_actions(), []);
    // Closed without choosing, it is told nothing was chosen.
    engine.exec(
        "tells",
        r#"choose({"Ana"}, function(index) echo(tostring(index)) end)"#,
    );
    let Some(Action::Choose { id, title, .. }) = engine.take_actions().pop() else {
        panic!("nothing offered");
    };
    assert_eq!(title, "");
    engine.chosen(id, None);
    assert_eq!(echoes(engine.take_actions()), ["nil"]);
    for (code, said) in [
        (
            "choose({}, function() end)",
            "choose is given nothing to choose from",
        ),
        (
            "choose({true}, function() end)",
            "an item to choose is a text",
        ),
        ("choose({'a'}, function() end, {tittle = 'x'})", "tittle"),
    ] {
        engine.exec("t", code);
        let actions = engine.take_actions();
        let error = error_text(&actions);
        assert!(error.contains(said), "{code}: {error}");
    }
}

#[test]
fn a_script_copies_and_reads_back_what_muddie_copied() {
    let mut engine = engine("");
    engine.exec("t", "echo(tostring(clipboard.get()))");
    assert_eq!(echoes(engine.take_actions()), ["nil"]);
    engine.exec("t", r#"clipboard.set("2235(3221)") echo(clipboard.get())"#);
    assert_eq!(
        engine.take_actions(),
        [
            Action::Copy("2235(3221)".into()),
            Action::Echo("2235(3221)".into())
        ]
    );
    // What the player copied with the review keys, too.
    engine.copied("Ana tells you: hola");
    engine.exec("t", "echo(clipboard.get())");
    assert_eq!(echoes(engine.take_actions()), ["Ana tells you: hola"]);
}

#[test]
fn a_script_has_the_game_read_aloud_or_not_until_it_goes() {
    let mut engine = engine("");
    engine.exec("t", "echo(tostring(session.reading))");
    assert_eq!(echoes(engine.take_actions()), ["true"]);
    engine.exec(
        "modes.luau",
        "session.reading = false echo(tostring(session.reading))",
    );
    assert_eq!(
        engine.take_actions(),
        [Action::Reading(false), Action::Echo("false".into())]
    );
    // Gone with the script that silenced it, the game is read again.
    engine.unload("modes.luau");
    assert_eq!(engine.take_actions(), [Action::Reading(true)]);
    // The player's own choice is told, and not undone with a script.
    engine.set_reading(false);
    engine.exec("other.luau", "echo(tostring(session.reading))");
    engine.unload("other.luau");
    assert_eq!(echoes(engine.take_actions()), ["false"]);
    engine.exec("t", "session.reading = 1");
    let actions = engine.take_actions();
    assert!(error_text(&actions).contains("session.reading is true or false"));
}
