//! The gauges as data: asking for them, and listening to them.

use super::*;

/// What ran `code`.
fn having(code: &str) -> Engine {
    let mut engine = engine("");
    engine.exec("t", code);
    let wrong = engine.take_actions();
    let wrong: Vec<_> = wrong
        .iter()
        .filter(|action| matches!(action, Action::Error(_)))
        .collect();
    assert!(wrong.is_empty(), "{wrong:?}");
    engine
}

/// What is echoed, and gone wrong, as `code` runs.
fn echoes(engine: &mut Engine, code: &str) -> Vec<String> {
    engine.exec("t", code);
    said(engine)
}

fn said(engine: &mut Engine) -> Vec<String> {
    (engine.take_actions().into_iter())
        .filter_map(|action| match action {
            Action::Echo(said) => Some(said.text),
            Action::Error(error) => Some(error),
            _ => None,
        })
        .collect()
}

const LOW: [&str; 0] = [];

#[test]
fn what_a_gauge_is_can_be_asked_for() {
    let mut engine = having(
        r##"gauges.set("HP", {value = 40, max = 80})
        gauges.set("Gold", {kind = "number", value = 7})
        gauges.set("Hungry", {kind = "light", on = true})
        gauges.set("Mood", {kind = "text", text = "calm", place = "Me/Mood"})
        gauges.set("On you", {kind = "effects", items = {"Blessed", {name = "Poisoned", about = "ow", colour = "red"}}})
        function tell(g) return `{g.name} {g.kind} {g.value} {g.max} {g.percent} {g.on} {g.text} {g.place}` end"##,
    );
    engine.take_actions();
    for (name, is) in [
        ("HP", "HP bar 40 80 50 nil nil nil"),
        (" hp", "HP bar 40 80 50 nil nil nil"),
        ("Gold", "Gold number 7 nil nil nil nil nil"),
        ("Hungry", "Hungry light nil nil nil true nil nil"),
        ("Mood", "Mood text nil nil nil nil calm Me/Mood"),
        ("Me/Mood", "Mood text nil nil nil nil calm Me/Mood"),
    ] {
        assert_eq!(
            echoes(&mut engine, &format!("echo(tell(gauges.get('{name}')))")),
            [is]
        );
    }
    assert_eq!(
        echoes(
            &mut engine,
            r#"local i = gauges.get("On you").items
            echo(`{#i} {i[1].name} {i[1].about} {i[2].name} {i[2].about} {i[2].colour}`)
            echo(tostring(gauges.get("None")))
            local names = {}
            for _, g in gauges.all() do table.insert(names, `{g.name}={g.kind}`) end
            echo(table.concat(names, " "))"#
        ),
        [
            "2 Blessed nil Poisoned ow red",
            "nil",
            "HP=bar Gold=number Hungry=light Mood=text On you=effects"
        ]
    );
    // As it is set, at once; gone once removed; and off, as if not there.
    assert_eq!(
        echoes(
            &mut engine,
            r#"gauges.set("HP", {value = 20, max = 80}) echo(tell(gauges.get("HP")))
            gauges.remove("HP") echo(tostring(gauges.get("HP")))
            gauges.disable("Me") echo(tostring(gauges.get("Mood"))) echo(tostring(#gauges.all()))"#
        ),
        ["HP bar 20 80 25 nil nil nil", "nil", "nil", "3"]
    );
}

#[test]
fn who_listens_to_a_gauge_is_told_how_it_changes() {
    let mut engine = having(
        r#"on("gauge", "HP", function(g, change) echo(`HP {change.was.value} to {g.value}`) end)
        on("gauge", function(g) echo(`some {g.name}`) end)"#,
    );
    let set = |value: i32| format!(r#"gauges.set("HP", {{value = {value}, max = 100}})"#);
    // As it starts, and the same again, are not news.
    assert_eq!(echoes(&mut engine, &set(80)), LOW);
    assert_eq!(echoes(&mut engine, &set(80)), LOW);
    assert_eq!(echoes(&mut engine, &set(70)), ["HP 80 to 70", "some HP"]);
    // All a script changes is one change.
    let twice = format!("{} {}", set(60), set(50));
    assert_eq!(echoes(&mut engine, &twice), ["HP 70 to 50", "some HP"]);
    // Another owner's is the same gauge.
    engine.exec("other", &set(45));
    assert_eq!(said(&mut engine), ["HP 50 to 45", "some HP"]);
    // In another session it starts again; it is there to be switched.
    engine.exec(
        "t",
        r#"gauges.set("M", {kind = "text", text = "a", place = "Me/M"})"#,
    );
    engine.forget_gauges();
    assert_eq!(
        echoes(&mut engine, r#"echo(tostring(gauges.get("HP")))"#),
        ["nil"]
    );
    assert_eq!(echoes(&mut engine, r#"echo(gauges.list()[2])"#), ["Me/M"]);
    assert_eq!(echoes(&mut engine, &set(9)), LOW);
}

/// A level is told once as it is passed, and again only after coming clear
/// of it.
#[test]
fn a_level_of_a_gauge_is_told_once() {
    let mut engine = having(
        r#"on("gauge", "HP", function(g) echo("low") end, {under = 30})
        on("gauge", "hp", function(g) echo("full") end, {over = 100})"#,
    );
    let mut at = |value: i32| {
        let code = format!(r#"gauges.set("HP", {{value = {value}, max = 100}})"#);
        echoes(&mut engine, &code)
    };
    assert_eq!(at(60), LOW);
    assert_eq!(at(30), LOW);
    assert_eq!(at(29), ["low"]);
    assert_eq!(at(20), LOW);
    // It wavers.
    assert_eq!(at(32), LOW);
    assert_eq!(at(28), LOW);
    assert_eq!(at(35), LOW);
    assert_eq!(at(10), ["low"]);
    assert_eq!(at(99), LOW);
    assert_eq!(at(100), ["full"]);
    assert_eq!(at(98), LOW);
    assert_eq!(at(100), LOW);
    assert_eq!(at(94), LOW);
    assert_eq!(at(100), ["full"]);
}

/// Low as it starts is not news, until it comes clear.
#[test]
fn a_gauge_that_starts_past_a_level_is_not_told() {
    let mut engine = having(r#"on("gauge", "HP", function(g) echo("low") end, {under = 30})"#);
    let mut at = |value: i32| {
        let code = format!(r#"gauges.set("HP", {{value = {value}, max = 100}})"#);
        echoes(&mut engine, &code)
    };
    assert_eq!(at(20), LOW);
    assert_eq!(at(10), LOW);
    assert_eq!(at(50), LOW);
    assert_eq!(at(20), ["low"]);
}

#[test]
fn of_effects_it_is_told_which_came_and_which_are_gone() {
    let mut engine = having(
        r#"function names(list) local n = {} for _, e in list do table.insert(n, e.name) end return table.concat(n, ",") end
        on("gauge", "On you", function(g, change) echo(`+{names(change.came)} -{names(change.gone)}`) end)"#,
    );
    let mut on = |items: &str| {
        let code = format!(r#"gauges.set("On you", {{kind = "effects", items = {{{items}}}}})"#);
        echoes(&mut engine, &code)
    };
    assert_eq!(on(r#""A", "B""#), LOW);
    assert_eq!(on(r#""B", "C", "D""#), ["+C,D -A"]);
    assert_eq!(on(""), ["+ -B,C,D"]);
    assert_eq!(on(""), LOW);
}

/// Two that set each other's gauges do not go on forever, and one switched
/// off is not told of.
#[test]
fn listening_to_a_gauge_ends_and_is_switched() {
    let mut engine = having(
        r#"on("gauge", "A", function(g) gauges.set("B", {kind = "number", value = g.value + 1}) end)
        on("gauge", "B", function(g) gauges.set("A", {kind = "number", value = g.value + 1}) end)
        gauges.set("A", {kind = "number", value = 1}) gauges.set("B", {kind = "number", value = 1})"#,
    );
    engine.take_actions();
    assert_eq!(
        echoes(
            &mut engine,
            r#"gauges.set("A", {kind = "number", value = 5})"#
        ),
        LOW
    );
    assert_eq!(
        echoes(&mut engine, r#"echo(tostring(gauges.get("A").value > 5))"#),
        ["true"]
    );
    let mut engine = having(
        r#"made = on("gauge", "M", function(g) echo(g.text) end, {place = "Told"})
        gauges.set("M", {kind = "text", text = "a", place = "Me/M"})"#,
    );
    engine.take_actions();
    let mut now = |before: &str, text: &str| {
        let set = r#"gauges.set("M", {kind = "text", place = "Me/M", text = "#;
        echoes(&mut engine, &format!(r#"{before} {set}"{text}"}})"#))
    };
    assert_eq!(now("", "b"), ["b"]);
    assert_eq!(now("gauges.disable('Me')", "c"), LOW);
    assert_eq!(now("gauges.enable('Me')", "d"), ["d"]);
    assert_eq!(now("made:disable()", "e"), LOW);
    assert_eq!(now("made:enable()", "f"), ["f"]);
    assert_eq!(now("triggers.disable('Told')", "g"), LOW);
    assert_eq!(now("made:remove()", "h"), LOW);
}

#[test]
fn what_the_client_makes_is_told_of_too() {
    let mut engine = having(r#"on("gauge", "HP", function(g) echo(`{g.percent}`) end)"#);
    engine.know_gauge("HP", (50.0, 100.0), GaugeLook::default());
    engine.know_gauge("HP", (40.0, 100.0), GaugeLook::default());
    assert_eq!(said(&mut engine), ["40"]);
    for (code, wrong) in [
        (r#"on("gmcp", "X", print, {under = 3})"#, "no option under"),
        (r#"on("gauge", "X", print, {below = 3})"#, "no option below"),
    ] {
        assert!(echoes(&mut engine, code)[0].contains(wrong), "{code}");
    }
}
