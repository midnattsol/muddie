//! What a script may not do, and what happens when it goes wrong.

use super::*;

#[test]
fn sandbox_has_no_file_or_process_access() {
    let mut engine = engine("");
    engine.exec(
        "probe",
        r#"echo(`{io} {package} {debug} {os.execute} {os.remove} {loadfile} {dofile}`)"#,
    );
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("nil nil nil nil nil nil nil".into())]
    );
    engine.exec("probe", "send = nil; string.rep = nil");
    assert!(error_text(&engine.take_actions()).contains("readonly"));
}

#[test]
fn runaway_script_is_stopped() {
    let mut engine = engine(r#"trigger("loop", function() while true do end end)"#);
    let started = Instant::now();
    engine.on_line("loop");
    let said = format!("{:?}", engine.take_actions());
    assert!(said.contains("ran too long"), "{said}");
    assert!(started.elapsed() < TIME_BUDGET * 4);
    // The engine keeps working afterwards.
    engine.exec("lua", "send('alive')");
    assert_eq!(engine.take_actions(), [send("alive")]);
}

#[test]
fn errors_are_reported_and_globals_persist_between_chunks() {
    let mut engine = engine("counter = 41");
    engine.exec("lua", "counter += 1; send(tostring(counter))");
    assert_eq!(engine.take_actions(), [send("42")]);
    engine.exec("lua", "trigger('(', print)");
    assert!(error_text(&engine.take_actions()).contains("regex parse error"));
    engine.exec("lua", "error('boom')");
    assert_eq!(error_text(&engine.take_actions()), "lua:1: boom");
}

#[test]
fn code_is_checked_without_being_run() {
    let engine = Engine::new(Rc::default()).unwrap();
    assert_eq!(
        engine.check("send(\"north\")\nif m then echo(m[1]) end"),
        None
    );
    let (line, message) = engine.check("local x = 1\n\nif x then\n  send(\n").unwrap();
    assert_eq!(line, 5);
    assert!(
        !message.is_empty() && !message.contains("code:"),
        "{message}"
    );
    // Nothing ran: no action was asked for.
    let mut engine = engine;
    engine.check("send(\"north\")");
    assert_eq!(engine.take_actions(), []);
}

#[test]
fn what_runs_too_long_is_taken_away_and_stops_the_game_once() {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.exec("t", r#"trigger(".", function() while true do end end)"#);
    let started = Instant::now();
    for _ in 0..4 {
        engine.on_line("a line");
    }
    assert!(
        started.elapsed() < TIME_BUDGET * 2,
        "{:?}",
        started.elapsed()
    );
    let errors: Vec<String> = (engine.take_actions().into_iter())
        .filter_map(|action| match action {
            Action::Error(error) => Some(error),
            _ => None,
        })
        .collect();
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[1].contains("taken away"), "{errors:?}");
}
