//! What scripts listen to, and the tables of what there is: gauges, sounds,
//! the session and the rest.

use super::*;

/// One function listens to everything, switched as a trigger is, and says
/// what there is when it is asked for what there is not.
#[test]
fn everything_is_listened_to_with_one_function() {
    let mut engine = engine(
        r#"on("gmcp", "Char", function(data, name) send(name .. " " .. data.hp) end, {place = "Fight/Life"})
        on("msdp", "HEALTH", function(value, name) send(name .. " " .. value) end, {tags = "fight"})
        on("var", "hp", function(value, name) send(name .. " " .. value) end)
        on("connect", function() send("connected") end)"#,
    );
    let go = |engine: &mut Engine| {
        engine.on_gmcp("Char.Vitals", r#"{"hp": 3}"#);
        engine.on_msdp("HEALTH", r#""4""#);
        engine.exec("t", "vars.hp = (vars.hp or 4) + 1");
        engine.on_event("connect", "");
        engine.take_actions()
    };
    assert_eq!(
        go(&mut engine),
        ["Char.Vitals 3", "HEALTH 4", "hp 5", "connected"].map(send)
    );
    // Switched by its place, a folder above it, or a tag.
    engine.exec("t", r#"triggers.disable("Fight") tags.disable("fight")"#);
    assert_eq!(go(&mut engine), ["hp 6", "connected"].map(send));
    engine.exec(
        "t",
        r#"triggers.enable("Fight") echo(table.concat(triggers.list(), " "))"#,
    );
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("Fight Fight/Life".into())]
    );
    for (code, said) in [
        (
            r#"on("conect", function() end)"#,
            "there is no conect to listen to; there are gmcp, msdp, var, gauge, channel, send, connect, disconnect, reader, panel, mxp",
        ),
        (
            r#"on("gmcp", function() end)"#,
            r#"on("gmcp", ...) needs the name of a package before the function"#,
        ),
        (
            r#"on("connect", "x", function() end)"#,
            r#"on("connect", ...) takes the function next, with no name"#,
        ),
        (r#"on("gmcp", "Char")"#, "on needs a function"),
        (
            r#"on("connect", function() end, {once = true})"#,
            "there is no option once here; there are place, tags",
        ),
        (
            r#"on_gmcp("Char", function() end)"#,
            r#"on_gmcp is no longer: use on("gmcp", package, function)"#,
        ),
    ] {
        engine.exec("t", code);
        match engine.take_actions().as_slice() {
            [Action::Error(error)] => assert!(error.contains(said), "{error}"),
            other => panic!("{code}: {other:?}"),
        }
    }
}

/// A variable is read as it is now, each time: not as it was when the
/// script was loaded, as Luau reads what never changes.
#[test]
fn a_variable_is_read_as_it_is_now() {
    let mut engine = engine("vars.hits = 1");
    engine.exec(
        "t",
        r#"trigger("^x$", function() vars.hits = vars.hits + 1 send(`{vars.hits}`) end)"#,
    );
    for _ in 0..3 {
        engine.on_line("x");
    }
    assert_eq!(engine.take_actions(), ["2", "3", "4"].map(send));
}

/// What a script reads of the session is as it is now; what there is not
/// is an error.
#[test]
fn a_script_is_told_of_the_session() {
    let mut engine = engine(
        r#"alias("^ask$", function()
            send(`{session.connected} {session.host} {session.profile}`)
        end)"#,
    );
    let mut ask = |session: Session| {
        engine.know_session(session);
        engine.on_input("ask");
        engine.take_actions()
    };
    assert_eq!(ask(Session::default()), [send("false  nil")]);
    let now = Session {
        connected: true,
        host: "mud.example.org".into(),
        profile: Some("Ana".into()),
    };
    assert_eq!(ask(now), [send("true mud.example.org Ana")]);
    for (code, said) in [
        ("echo(session.hots)", "there is no session.hots; there are"),
        ("session.host = 'x'", "session.host is asked for, not set"),
        ("gmcp('Char')", "gmcp(...) is no longer: use gmcp.get(...)"),
        (
            "gauges.set('T', {kind = 'text'})",
            "a gauge of the kind text needs a text",
        ),
        ("gauge('T', 1, 2)", "gauge is no longer: use gauges.set"),
        (
            "sounds.play('x', {volumen = 1})",
            "there is no option volumen here; there are volume, loop",
        ),
    ] {
        engine.exec("t", code);
        let actions = engine.take_actions();
        assert!(error_text(&actions).contains(said), "{code}: {actions:?}");
    }
}

/// Words, effects and a light need no numbers.
#[test]
fn a_gauge_takes_what_its_kind_has() {
    let mut engine = engine("");
    let mut level = |given: &str| {
        engine.exec("t", &format!("gauges.set('G', {{{given}}})"));
        match engine.take_actions().remove(0) {
            Action::Gauge { level, .. } => level,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(level("value = 3, max = 4"), Some((3.0, 4.0)));
    assert_eq!(level("kind = 'number', value = 3"), Some((3.0, 0.0)));
    assert_eq!(level("kind = 'light', on = true"), Some((1.0, 1.0)));
    assert_eq!(level("kind = 'light', on = false"), Some((0.0, 1.0)));
    assert_eq!(level("kind = 'text', text = 'hi'"), Some((0.0, 0.0)));
    assert_eq!(level("kind = 'effects', items = {'a'}"), Some((0.0, 0.0)));
}

#[test]
fn a_sound_is_stopped_by_what_playing_it_gave() {
    let mut engine = engine("");
    engine.exec("t", "rain = sounds.play('rain.ogg', {loop = true})");
    let id = match engine.take_actions().remove(0) {
        Action::PlaySound { id, .. } => id,
        other => panic!("{other:?}"),
    };
    engine.exec("t", "rain:stop()");
    assert_eq!(engine.take_actions(), [Action::StopSound(id)]);
}
