//! What is said of a call to what there is not.

use super::*;

fn wrong(code: &str) -> String {
    let mut engine = engine("");
    engine.exec("t", code);
    let actions = engine.take_actions();
    let errors: Vec<&Action> = (actions.iter())
        .filter(|action| matches!(action, Action::Error(_)))
        .collect();
    match errors.as_slice() {
        [Action::Error(error)] => error.clone(),
        other => panic!("{code}: {other:?}"),
    }
}

#[test]
fn a_call_to_what_there_is_not_says_what() {
    for (code, said) in [
        (
            r#"support_gmcp("Comm 1")"#,
            "t:1: there is no function support_gmcp",
        ),
        ("\necho('x')\n  ech 'x'", "t:3: there is no function ech"),
        (
            "gauges.sett('HP', {})",
            "t:1: there is no function gauges.sett",
        ),
        (
            "local x = gauges.sett {}",
            "t:1: there is no function gauges.sett",
        ),
        (
            "echo(tostring(nothing(1)))",
            "t:1: there is no function nothing",
        ),
        ("if isnt(1) then end", "t:1: there is no function isnt"),
        ("local text = 'a' text:nope()", "t:1: attempt to call"),
        ("local f f()", "t:1: there is no function f"),
        (
            "echo('call(1)') nope() -- other(2)",
            "t:1: there is no function nope",
        ),
        // Either may be it.
        ("one(two())", "t:1: attempt to call a nil value"),
        ("(nil)()", "t:1: attempt to call a nil value"),
    ] {
        assert!(wrong(code).starts_with(said), "{code}: {}", wrong(code));
    }
}

/// In what runs later too, and in a module.
#[test]
fn a_call_to_what_there_is_not_says_what_wherever_it_is() {
    let mut engine = engine("");
    engine.exec_owned("one/a.luau", "a.luau", "after(1, function()\n nope() end)");
    engine.exec_owned(
        "two/a.luau",
        "a.luau",
        "after(1, function()\n wait(1)\n other() end)",
    );
    let said = |engine: &mut Engine| -> Vec<String> {
        (engine.take_actions().into_iter())
            .filter_map(|action| match action {
                Action::Error(error) => Some(error),
                _ => None,
            })
            .collect()
    };
    engine.tick(Instant::now() + Duration::from_secs(2));
    assert_eq!(said(&mut engine), ["a.luau:2: there is no function nope"]);
    engine.tick(Instant::now() + Duration::from_secs(4));
    assert_eq!(said(&mut engine), ["a.luau:3: there is no function other"]);
    engine.offer_module("one/util.luau", "return {go = function()\n missing() end}");
    engine.exec_owned("one/a.luau", "a.luau", "require('util').go()");
    assert_eq!(said(&mut engine), ["util:2: there is no function missing"]);
}
