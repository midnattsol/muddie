//! Triggers, aliases, keys and timers, and switching them.

use super::*;

#[test]
fn trigger_gets_groups_and_the_line() {
    let mut engine = engine(
        r#"trigger("^(?<who>\\w+) tells you: (.*)$", function(m, line)
            send(`reply {m.who} got {m[2]}`)
            echo(m[0] == line and "whole" or "partial")
        end)"#,
    );
    assert!(engine.on_line("Ana tells you: hola"));
    assert_eq!(
        engine.take_actions(),
        [send("reply Ana got hola"), Action::Echo("whole".into())]
    );
    assert!(engine.on_line("Nothing here"));
    assert_eq!(engine.take_actions(), []);
}

#[test]
fn gag_hides_only_the_line_that_fired() {
    let mut engine = engine(r#"trigger("spam", function() gag() end)"#);
    assert!(!engine.on_line("some spam here"));
    assert!(engine.on_line("a normal line"));
}

#[test]
fn alias_replaces_input_and_its_sends_are_not_expanded() {
    let mut engine =
        engine(r#"alias("^k (.+)$", function(m) send("k " .. m[1]) send("kill " .. m[1]) end)"#);
    engine.on_input("k orc");
    assert_eq!(engine.take_actions(), [send("k orc"), send("kill orc")]);
    engine.on_input("look");
    assert_eq!(engine.take_actions(), [send("look")]);
}

#[test]
fn timers_fire_repeat_and_cancel() {
    let mut engine = engine(
        r#"after(1, function() send("once") end)
        ticker = every(2, function() send("tick") end)
        alias("^stop$", function() echo(tostring(cancel(ticker))) end)"#,
    );
    let start = Instant::now();
    let at = |seconds: f64| start + Duration::from_secs_f64(seconds);
    engine.tick(at(0.5));
    assert_eq!(engine.take_actions(), []);
    engine.tick(at(1.5));
    assert_eq!(engine.take_actions(), [send("once")]);
    engine.tick(at(2.5));
    engine.tick(at(3.0));
    assert_eq!(engine.take_actions(), [send("tick")]);
    engine.tick(at(5.0));
    assert_eq!(engine.take_actions(), [send("tick")]);
    assert!(engine.next_deadline().is_some());
    engine.on_input("stop");
    assert_eq!(engine.take_actions(), [Action::Echo("true".into())]);
    assert_eq!(engine.next_deadline(), None);
}

#[test]
fn callbacks_may_change_triggers_while_a_line_is_dispatched() {
    let mut engine = engine(
        r#"local second
        trigger("go", function() cancel(second) trigger("go", function() send("late") end) end)
        second = trigger("go", function() send("never") end)"#,
    );
    engine.on_line("go");
    assert_eq!(engine.take_actions(), []);
    engine.on_line("go");
    assert_eq!(engine.take_actions(), [send("late")]);
}

#[test]
fn several_commands_on_one_line_each_go_through_aliases() {
    let mut engine = engine(r#"alias("^k (.+)$", function(m) send("kill " .. m[1]) end)"#);
    engine.on_input("n; k orc ;say hi;; bye;");
    assert_eq!(
        engine.take_actions(),
        [send("n"), send("kill orc"), send("say hi; bye"), send("")]
    );
    engine.on_input("");
    assert_eq!(engine.take_actions(), [send("")]);
    engine.exec("lua", "separator('|')");
    engine.on_input("n|s;e");
    assert_eq!(engine.take_actions(), [send("n"), send("s;e")]);
    engine.exec("lua", "separator(false)");
    engine.on_input("n|s;e");
    assert_eq!(engine.take_actions(), [send("n|s;e")]);
    // Given back, the usual `;`.
    engine.exec("lua", "separator()");
    engine.on_input("n|s;e");
    assert_eq!(engine.take_actions(), [send("n|s"), send("e")]);
}

#[test]
fn tags_switch_triggers_off_and_once_fires_a_single_time() {
    let mut engine = engine(
        r#"trigger("hit", function() send("flee") end, {tags = {"Combat"}})
        trigger("hit", function() send("first blood") end, {once = true})
        alias("^x$", function() send("special") end, {tags = "combat"})"#,
    );
    engine.on_line("you are hit");
    assert_eq!(engine.take_actions(), [send("flee"), send("first blood")]);
    engine.exec("lua", "tags.disable('combat')");
    engine.on_line("you are hit");
    engine.on_input("x");
    assert_eq!(engine.take_actions(), [send("x")]);
    engine.exec("lua", "tags.enable('COMBAT')");
    engine.on_line("you are hit");
    engine.on_input("x");
    assert_eq!(engine.take_actions(), [send("flee"), send("special")]);
}

#[test]
fn a_thing_is_switched_by_its_place_or_its_folder_and_only_of_its_kind() {
    let mut engine = engine(
        r#"trigger("hit", function() send("flee") end, {place = "Combat/Flee"})
        trigger("hit", function() send("quaff") end, {place = "Combat/Healing/Quaff"})
        alias("^hit$", function() send("kick") end, {place = "Combat/Kick"})"#,
    );
    engine.exec("t", r#"triggers.disable("Combat/Healing")"#);
    engine.on_line("you are hit");
    assert_eq!(engine.take_actions(), [send("flee")]);
    engine.exec("t", r#"triggers.disable("Combat")"#);
    engine.on_line("you are hit");
    engine.on_input("hit");
    assert_eq!(engine.take_actions(), [send("kick")]);
    engine.exec(
        "t",
        r#"echo(tostring(triggers.enabled("Combat/Flee")) .. table.concat(triggers.list(), ","))"#,
    );
    assert_eq!(
        engine.take_actions(),
        [Action::Echo(
            "falseCombat,Combat/Flee,Combat/Healing,Combat/Healing/Quaff".into()
        )]
    );
}

#[test]
fn trigger_can_read_the_colours_of_its_line() {
    use muddie_core::{Span, Style};
    let mut engine = engine(
        r#"trigger("orc", function()
            local parts = {}
            for _, span in line_spans() do
                table.insert(parts, `{span.text}:{span.fg}:{span.bold}`)
            end
            echo(table.concat(parts, "|") .. " " .. #line_spans())
        end)"#,
    );
    let span = |text: &str, style: Style| Span {
        text: text.into(),
        style,
    };
    let line = Line {
        spans: vec![
            span("An ", Style::default()),
            span(
                "orc",
                Style {
                    fg: Color::Indexed(1),
                    bold: true,
                    ..Style::default()
                },
            ),
            span(
                "!",
                Style {
                    fg: Color::Rgb(255, 128, 0),
                    ..Style::default()
                },
            ),
        ],
        links: Vec::new(),
        dest: None,
    };
    assert_eq!(engine.on_styled_line(&line), Some(line.clone()));
    assert_eq!(
        engine.take_actions(),
        [Action::Echo(
            "An :nil:false|orc:1:true|!:#ff8000:false 3".into()
        )]
    );
    // Outside a trigger there is no line to describe.
    engine.exec("lua", "echo(tostring(#line_spans()))");
    assert_eq!(engine.take_actions(), [Action::Echo("0".into())]);
}

#[test]
fn trigger_can_recolour_its_match_or_gag_the_line() {
    use muddie_core::{Span, Style};
    let mut engine = engine(
        r##"trigger("An (\\w+)", function() highlight("bright yellow", "#102030") end)
        trigger("^secret", function() gag() end)"##,
    );
    let plain = |text: &str| Line {
        spans: vec![Span {
            text: text.into(),
            style: Style::default(),
        }],
        links: Vec::new(),
        dest: None,
    };
    let shown = engine.on_styled_line(&plain("An orc!")).unwrap();
    let styles: Vec<(&str, Color, Color)> = (shown.spans.iter())
        .map(|span| (span.text.as_str(), span.style.fg, span.style.bg))
        .collect();
    assert_eq!(
        styles,
        [
            ("An orc", Color::Indexed(11), Color::Rgb(0x10, 0x20, 0x30)),
            ("!", Color::Default, Color::Default),
        ]
    );
    assert_eq!(engine.on_styled_line(&plain("secret plans")), None);
    engine.exec("lua", "highlight('plaid')");
    assert!(error_text(&engine.take_actions()).contains("a colour is"));
}

#[test]
fn keys_run_their_binding() {
    let mut engine = engine(
        r#"f5 = bind("F5", "look;inventory")
        bind("Shift + CTRL+k", function() send("kick") end)"#,
    );
    assert!(engine.on_key("f5"));
    assert!(engine.on_key("ctrl+shift+k"));
    assert!(!engine.on_key("f6"));
    assert_eq!(
        engine.take_actions(),
        [send("look"), send("inventory"), send("kick")]
    );
    engine.exec("lua", "echo(`{cancel(f5)} {cancel(f5)}`)");
    assert!(!engine.on_key("f5"));
    assert_eq!(engine.take_actions(), [Action::Echo("true false".into())]);
    assert_eq!(key_name("Shift+Alt+Ctrl+Enter"), "ctrl+alt+shift+enter");
}

#[test]
fn switches_cover_keys_and_timers_and_are_told_about() {
    let mut engine = engine(
        r#"
        trigger("hit", function() send("flee") end, {tags = {"combat"}})
        bind("f1", "kick", {place = "Fighting", tags = {"combat"}})
        bind("f1", "look", {place = "Exploring"})
        bind("f2", "score")
        every(1, function() send("tick") end, {place = "Pulse", tags = {"combat"}})
        tags.disable("combat")
        "#,
    );
    assert_eq!(engine.tags(), ["combat"]);
    assert!(!engine.switches().tag_on("combat"));
    // The same key does what the first bound that is on does, or nothing.
    assert!(engine.on_key("f1"));
    assert_eq!(engine.take_actions(), [send("look")]);
    let mut switches = engine.switches();
    switches.set_place(Kind::Macro, "Exploring", false);
    engine.set_switches(switches);
    assert!(!engine.on_key("f1"), "the key is left to be typed");
    assert!(engine.on_key("f2"));
    engine.take_actions();
    // A timer switched off lets its turns pass, and comes back.
    let start = Instant::now();
    engine.tick(start + Duration::from_millis(1100));
    assert_eq!(engine.take_actions(), []);
    engine.exec(
        "t",
        r#"echo(tostring(tags.toggle("combat")) .. tostring(tags.enabled("combat")))"#,
    );
    assert_eq!(engine.take_actions(), [Action::Echo("truetrue".into())]);
    engine.tick(start + Duration::from_millis(2200));
    assert_eq!(engine.take_actions(), [send("tick")]);
    assert!(engine.on_key("f1"));
    assert_eq!(engine.take_actions(), [send("kick")]);
    // Bound again under the same name, a key does the new thing instead.
    engine.exec("t", r#"bind("f1", "bash", {place = "Fighting"})"#);
    assert!(engine.on_key("f1"));
    assert_eq!(engine.take_actions(), [send("bash")]);
}

#[test]
fn failing_repeating_timer_is_dropped() {
    let mut engine = engine(r#"every(1, function() error("bad") end)"#);
    engine.tick(Instant::now() + Duration::from_secs(2));
    assert!(error_text(&engine.take_actions()).contains("bad"));
    assert_eq!(engine.next_deadline(), None);
}

#[test]
fn many_triggers_fire_as_few_do_and_follow_changes() {
    // Enough to be tried as one set: those that match fire, in order.
    let mut code = String::new();
    for n in 0..30 {
        code.push_str(&format!(
            "trigger(\"^word {n}$\", function() send(\"saw {n}\") end)\n"
        ));
    }
    code.push_str("trigger(\"word\", function() send(\"any\") end)\n");
    let mut engine = engine(&code);
    engine.on_line("word 7");
    assert_eq!(engine.take_actions(), [send("saw 7"), send("any")]);
    // One added, and one taken away, are seen at the next line.
    engine.exec("t", r#"trigger("^word 7$", function() send("again") end)"#);
    // The ids count from 1: this is the one of "word 3".
    engine.exec("t", r#"cancel(4)"#);
    engine.on_line("word 7");
    assert_eq!(
        engine.take_actions(),
        [send("saw 7"), send("any"), send("again")]
    );
    engine.on_line("word 3");
    assert_eq!(engine.take_actions(), [send("any")]);
}

/// Many patterns with `\w`, every letter of every language, are put in one
/// set all the same, on a thread of their own, and fire as each would: too
/// big for what the regex crate allows by default, they used to leave every
/// line tried on every trigger.
#[test]
fn many_wide_patterns_are_put_in_one_set() {
    // In several goes: each takes a while to compile, unoptimised.
    let mut engine = engine("");
    for start in (0..300).step_by(30) {
        let mut code = String::new();
        for n in start..start + 30 {
            code.push_str(&format!(
                "trigger(\"^(\\\\w+) attacks you with (\\\\w+) {n}$\", function(m) send(m[1] .. \" {n}\") end)\n"
            ));
        }
        engine.exec("test", &code);
        assert_eq!(engine.take_actions(), []);
    }
    engine.on_line("nothing to see");
    assert!(
        engine.trigger_set_made(),
        "the patterns were not put in one set"
    );
    engine.on_line("Ñandú attacks you with garras 250");
    assert_eq!(engine.take_actions(), [send("Ñandú 250")]);
}

/// A trigger's pattern is checked when it is made, and compiled only once
/// a line may match it: those that never do hold nothing compiled.
#[test]
fn patterns_are_compiled_only_once_tried() {
    let mut code = String::new();
    for n in 0..30 {
        code.push_str(&format!("trigger(\"^word {n}$\", function() end)\n"));
    }
    let mut engine = engine(&code);
    engine.on_line("nothing");
    assert!(engine.trigger_set_made());
    engine.on_line("word 7");
    let compiled: Vec<bool> = (engine.state.borrow().triggers.iter())
        .map(|trigger| trigger.pattern.is_compiled())
        .collect();
    assert_eq!(
        compiled.iter().filter(|done| **done).count(),
        1,
        "{compiled:?}"
    );
    assert!(compiled[7]);
    // One added: while the set is made again, the one before says for the
    // others, and only the new one is tried on its own.
    engine.exec("t", r#"trigger("^new one$", function() end)"#);
    engine.on_line("something else");
    let compiled: Vec<bool> = (engine.state.borrow().triggers.iter())
        .map(|trigger| trigger.pattern.is_compiled())
        .collect();
    let count = compiled.iter().filter(|done| **done).count();
    assert_eq!(count, 2, "{compiled:?}");
    // A pattern that is not one is said at once, as before.
    engine.exec("t", r#"trigger("(unclosed", function() end)"#);
    let actions = engine.take_actions();
    assert!(
        format!("{actions:?}").contains("unclosed group"),
        "{actions:?}"
    );
}
