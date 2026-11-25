//! Waiting in the middle of a script.

use super::*;

/// What is echoed, sent and gone wrong since it was last asked.
fn said(engine: &mut Engine) -> Vec<String> {
    (engine.take_actions().into_iter())
        .filter_map(|action| match action {
            Action::Echo(said) => Some(said.text),
            Action::Send(text) => Some(format!("> {text}")),
            Action::Error(error) => Some(error),
            _ => None,
        })
        .collect()
}

fn runs(engine: &mut Engine, code: &str) -> Vec<String> {
    engine.exec("t", code);
    said(engine)
}

fn later(engine: &mut Engine, seconds: u64) -> Vec<String> {
    engine.tick(Instant::now() + Duration::from_secs(seconds));
    said(engine)
}

fn line(engine: &mut Engine, text: &str) -> Vec<String> {
    engine.on_line(text);
    said(engine)
}

const NONE: [&str; 0] = [];

#[test]
fn a_script_waits_for_a_while() {
    let mut engine = engine("");
    assert_eq!(
        runs(
            &mut engine,
            r#"echo("one") wait(5) echo("two") wait(5) echo("three")"#
        ),
        ["one"]
    );
    assert_eq!(later(&mut engine, 4), NONE);
    assert_eq!(later(&mut engine, 6), ["two"]);
    assert_eq!(later(&mut engine, 12), ["three"]);
    assert_eq!(later(&mut engine, 60), NONE);
}

/// As drinking a potion and seeing what comes of it.
#[test]
fn a_script_waits_for_a_line() {
    let mut engine = engine(
        r#"alias("^cure$", function()
            send("drink potion")
            local m, line = wait("line", "^You feel (\\w+)", {timeout = 3})
            if not m then return echo("nothing") end
            echo(`{m[1]}: {line.text}`)
            line:hide()
        end)"#,
    );
    engine.on_input("cure");
    assert_eq!(said(&mut engine), ["> drink potion"]);
    assert_eq!(line(&mut engine, "An orc arrives."), NONE);
    assert!(!engine.on_line("You feel better"));
    assert_eq!(said(&mut engine), ["better: You feel better"]);
    assert_eq!(line(&mut engine, "You feel worse"), NONE);
    assert_eq!(later(&mut engine, 9), NONE);
    engine.on_input("cure");
    assert_eq!(said(&mut engine), ["> drink potion"]);
    assert_eq!(later(&mut engine, 4), ["nothing"]);
    assert_eq!(line(&mut engine, "You feel better"), NONE);
}

/// The line a trigger stops on is not the one it waits for.
#[test]
fn what_waits_for_a_line_waits_for_another() {
    let mut engine = engine(
        r#"trigger("^a$", function(m, line)
            echo("a")
            wait("line", "^a$")
            echo("again")
            line:hide()
        end, {once = true})"#,
    );
    assert_eq!(line(&mut engine, "a"), ["a"]);
    let heard = line(&mut engine, "a");
    assert_eq!(heard[0], "again");
    // That of before is over.
    assert!(
        heard[1].contains("is for the line a trigger is looking at"),
        "{heard:?}"
    );
    let mut engine = super::engine(
        r#"trigger("^x", function(m, l) wait("line", "p", {prompt = true}) echo("prompt") end)"#,
    );
    line(&mut engine, "x");
    assert_eq!(line(&mut engine, "p"), NONE);
    let prompt = Line {
        spans: vec![Span {
            text: "p".into(),
            style: Style::default(),
        }],
        ..Line::default()
    };
    engine.on_styled_line(&prompt, true);
    assert_eq!(said(&mut engine), ["prompt"]);
}

#[test]
fn a_script_waits_for_what_on_listens_to() {
    let mut engine = engine("");
    engine.exec(
        "t",
        r#"local data, package = wait("gmcp", "Char.Vitals") echo(`{data.hp} {package}`)
        echo(`connect {wait("connect")}`)
        local g, change = wait("gauge", "HP", {under = 30, timeout = 9}) echo(`{g.value} {change.was.value}`)
        local c = wait("channel", "chat") echo(c.text)
        echo(tostring(wait("var", "x", {timeout = 2})))"#,
    );
    engine.on_gmcp("Char", r#"{"hp": 1}"#);
    assert_eq!(said(&mut engine), NONE);
    engine.on_gmcp("Char.Vitals", r#"{"hp": 7}"#);
    assert_eq!(said(&mut engine), ["7 Char.Vitals"]);
    engine.on_event("connect", "");
    assert_eq!(said(&mut engine), ["connect true"]);
    for value in [80, 40, 20] {
        engine.exec(
            "o",
            &format!(r#"gauges.set("HP", {{value = {value}, max = 100}})"#),
        );
    }
    assert_eq!(said(&mut engine), ["20 40"]);
    engine.exec("o", r#"channels.say("chat", "hi")"#);
    assert_eq!(said(&mut engine), ["hi"]);
    assert_eq!(later(&mut engine, 3), ["nil"]);
    // Nothing is left listening.
    engine.exec("o", "vars.x = 1");
    assert_eq!(said(&mut engine), NONE);
}

#[test]
fn lines_are_gathered_until_they_end() {
    let mut engine = engine(
        r#"trigger("^You have:$", function()
            local things = {}
            for line in lines({till = "^$", timeout = 2}) do table.insert(things, line.text) line:mute() end
            echo(table.concat(things, ","))
        end)"#,
    );
    for text in ["You have:", "a sword", "a shield"] {
        assert_eq!(line(&mut engine, text), NONE);
    }
    assert_eq!(engine.line_heard(), Heard::Not);
    assert_eq!(line(&mut engine, ""), ["a sword,a shield"]);
    assert_eq!(line(&mut engine, "a bow"), NONE);
    assert_eq!(engine.line_heard(), Heard::AsIs);
    // Cut short by the time.
    line(&mut engine, "You have:");
    line(&mut engine, "a bow");
    assert_eq!(later(&mut engine, 3), ["a bow"]);
    // So many; and left before, nothing waits any more.
    engine.exec(
        "t",
        r#"for l in lines({count = 2}) do echo(l.text) end echo("end")"#,
    );
    assert_eq!(line(&mut engine, "1"), ["1"]);
    assert_eq!(line(&mut engine, "2"), ["2", "end"]);
    engine.exec("t", r#"for l in lines({count = 9}) do break end"#);
    assert_eq!(line(&mut engine, "1"), NONE);
    assert!(engine.state.borrow().waiting.is_empty());
}

/// What waits goes with its script, and a mistake after waiting is said of
/// the script that made it.
#[test]
fn what_waits_is_of_its_script() {
    let mut engine = engine("");
    let wait =
        r#"wait(1) trigger("^t$", function() echo("t") end) wait("line", "go") echo("gone")"#;
    engine.exec("one", wait);
    later(&mut engine, 2);
    assert_eq!(line(&mut engine, "t"), ["t"]);
    engine.unload("one");
    assert_eq!(line(&mut engine, "t"), NONE);
    assert_eq!(line(&mut engine, "go"), NONE);
    engine.exec("one", "wait(1)\nerror('oops')");
    let said = later(&mut engine, 2);
    assert!(said[0].starts_with("one:2: oops"), "{said:?}");
    // Too long is too long after waiting too, and what waited before goes on.
    engine.exec("one", "wait(1) while true do end");
    let said = later(&mut engine, 2);
    assert!(said[0].contains("ran too long"), "{said:?}");
    // Those it makes itself go on by themselves.
    assert_eq!(
        runs(
            &mut engine,
            r#"coroutine.wrap(function() wait(1) echo("inside") end)() echo("out")"#
        ),
        ["out"]
    );
    assert_eq!(later(&mut engine, 2), ["inside"]);
}

#[test]
fn where_nothing_can_wait_it_is_said() {
    let mut engine = engine("");
    for (code, wrong) in [
        (
            "table.sort({2, 1}, function(a, b) wait(1) return a < b end)",
            "nothing can wait here",
        ),
        (r#"wait("nothing")"#, "there is no nothing to listen to"),
        (r#"wait("send")"#, r#"there is on("send", ...)"#),
        (r#"wait("line", "(")"#, "regex parse error"),
        (
            r#"wait("line", "a", {timeot = 1})"#,
            "there is no option timeot",
        ),
        (
            r#"wait("gmcp", "A", {under = 1})"#,
            "there is no option under",
        ),
        ("wait(-1)", "positive"),
        ("lines()", "lines needs till, count or timeout"),
        ("lines({til = 1})", "there is no option til"),
        ("coroutine.yield()", "wait(...) stops for a while"),
    ] {
        let said = runs(&mut engine, code);
        assert!(
            said.len() == 1 && said[0].contains(wrong),
            "{code}: {said:?}"
        );
    }
    engine.exec("t", r#"on("send", function() wait(1) end)"#);
    assert_eq!(engine.on_send("x").as_deref(), Some("x"));
    assert!(said(&mut engine)[0].contains("nothing can wait here"));
    assert!(engine.state.borrow().waiting.is_empty());
}
