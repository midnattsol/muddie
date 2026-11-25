//! How what scripts, gauges, channels and panels say is heard: the same
//! rules for all of it.

use super::*;

/// What is said as `code` runs, and what of it cuts in.
fn heard(app: &mut Tab, code: &str) -> (Vec<String>, Vec<String>) {
    app.voice.said.clear();
    app.voice.cut.clear();
    app.scripts.exec("t", code);
    app.run_actions();
    (
        std::mem::take(&mut app.voice.said),
        std::mem::take(&mut app.voice.cut),
    )
}

/// Everything a script makes be heard, each told `options`; `n` makes each
/// text its own.
fn ways(n: usize, options: &str) -> Vec<String> {
    vec![
        format!(r#"speak("spoken {n}", {{{options}}})"#),
        format!(r#"echo("echoed {n}", {{{options}}})"#),
        format!(r#"channels.say("chat", "said {n}", {{{options}}})"#),
        format!(r#"panels.add("Tells", "added {n}", {{{options}}})"#),
        format!(
            r#"gauges.set("G{n}", {{kind = "text", text = "calm", {options}}})
            gauges.set("G{n}", {{kind = "text", text = "angry", {options}}})"#
        ),
    ]
}

#[test]
fn everything_is_heard_as_it_is_told() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    for (n, (options, times, cuts)) in [
        ("", 1, 0),
        (r#"aloud = "usual""#, 1, 0),
        (r#"aloud = "wait""#, 1, 0),
        (r#"aloud = "cut""#, 1, 1),
        (r#"aloud = "never""#, 0, 0),
        (r#"aloud = "never", always = true"#, 0, 0),
    ]
    .into_iter()
    .enumerate()
    {
        for code in ways(n, options) {
            let (said, cut) = heard(app, &code);
            assert_eq!((said.len(), cut.len()), (times, cuts), "{code}: {said:?}");
        }
    }
}

/// The game not read, nothing of it is heard but what is asked to be.
#[test]
fn what_is_always_heard_is_while_the_game_is_not_read() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.set_muted(true);
    for (n, (options, times)) in [("", 0), (r#"aloud = "cut""#, 0), ("always = true", 1)]
        .into_iter()
        .enumerate()
    {
        for code in ways(n, options) {
            let (said, _) = heard(app, &code);
            assert_eq!(said.len(), times, "{code}: {said:?}");
        }
    }
    // What the client tells is not the game.
    assert_eq!(heard(app, r#"notify("kept")"#).0, ["kept"]);
}

/// Only the session on screen is heard, whatever is asked.
#[test]
fn a_session_out_of_sight_says_nothing() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.away = true;
    for (n, options) in ["", r#"aloud = "cut", always = true"#]
        .into_iter()
        .enumerate()
    {
        for code in ways(n, options) {
            assert_eq!(heard(app, &code).0, [] as [String; 0], "{code}");
        }
    }
    assert_eq!(heard(app, r#"notify("kept")"#).0, ["kept"]);
}

/// A notice is kept as it is told, heard or not.
#[test]
fn a_notice_is_heard_as_it_is_told() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    for (options, times, cuts) in [
        ("", 1, 0),
        (r#"aloud = "cut""#, 1, 1),
        (r#"aloud = "never""#, 0, 0),
    ] {
        let before = notes(app).len();
        let (said, cut) = heard(app, &format!(r#"notify("hi", {{{options}}})"#));
        assert_eq!((said.len(), cut.len()), (times, cuts), "{options}");
        assert_eq!(notes(app).len(), before + 1, "{options}");
    }
}

/// The very words read as a line are not said again by a script, unless
/// they are to cut in; others that are part of them are.
#[test]
fn what_was_just_read_is_not_said_again() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.line("Ana tells you: come");
    assert_eq!(heard(app, r#"speak("Ana tells you: come")"#).0.len(), 0);
    assert_eq!(heard(app, r#"speak("Ana")"#).0, ["Ana"]);
    assert_eq!(
        heard(app, r#"speak("Ana tells you: come", {aloud = "cut"})"#).1,
        ["Ana tells you: come"]
    );
}

/// What a script tells is in the language of the interface, of the texts
/// it is given by language.
#[test]
fn a_script_tells_in_the_language_of_the_interface() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    for what in ["speak", "echo", "notify"] {
        let said = heard(
            app,
            &format!(r#"{what}({{en = "Hello {what}", es = "Hola"}})"#),
        )
        .0;
        assert_eq!(said, [format!("Hello {what}")]);
    }
}

/// What is read in place of a line is heard as it is told too.
#[test]
fn what_is_read_in_place_of_a_line_is_heard_as_it_is_told() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    for (muted, options, times, cuts) in [
        (false, "", 1, 0),
        (false, r#"aloud = "cut""#, 1, 1),
        (false, r#"aloud = "never""#, 0, 0),
        (true, r#"aloud = "cut""#, 0, 0),
        (true, "always = true", 1, 0),
    ] {
        let app = harness.state_mut().tab_mut();
        app.voice.set_muted(muted);
        let name = format!("orc {times} {cuts} {muted}");
        heard(
            app,
            &format!(
                r#"trigger("^{name}$", function(m, line)
                    line:read(m[0]:upper(), {{{options}}})
                end)"#
            ),
        );
        let line = panels::styled_lines(&name).remove(0);
        events.send(Event::Line(line)).unwrap();
        harness.run_steps(2);
        let app = harness.state_mut().tab_mut();
        assert_eq!(
            (app.voice.said.len(), app.voice.cut.len()),
            (times, cuts),
            "{muted} {options}"
        );
    }
}

/// A gauge the MUD asks for with MXP is told of to scripts, and one of a
/// session that ended is not.
#[test]
fn what_the_client_makes_a_gauge_of_is_data_too() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    heard(
        app,
        r#"on("gauge", "HP", function(g) speak(`now {g.percent}`) end)"#,
    );
    let set = |app: &mut Tab, value: f64| {
        let look = Some(muddie_script::GaugeLook::default());
        let none = Default::default();
        app.set_gauge("HP", Some((value, 10.0)), look, None, none, String::new());
        app.run_actions();
    };
    set(app, 10.0);
    set(app, 5.0);
    assert_eq!(app.voice.said, ["now 50"]);
    app.forget_connection();
    set(app, 4.0);
    assert_eq!(app.voice.said, ["now 50"]);
}
