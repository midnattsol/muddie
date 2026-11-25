//! Tests of rules as they run: triggers, aliases, keys, timers, gauges and templates.

use super::*;

#[test]
fn triggers_do_what_their_fields_say() {
    let rules = Rules {
        triggers: vec![
            trigger("tells you (what?)", |rule| {
                rule.send = "smile; say \"hi\" \\o/".into();
                rule.when.speak = "a tell".into();
                rule.when.sound = "tell.ogg".into();
            }),
            trigger(r"^(\w+) attacks (\w+)", |rule| {
                rule.when.regex = true;
                rule.send = "kill %1".into();
                rule.when.speak = "%1 hits %2%3".into();
                rule.when.highlight = "red".into();
            }),
            trigger("spam", |rule| rule.when.gag = true),
            trigger("spam", |rule| {
                rule.enabled = false;
                rule.send = "never".into();
            }),
            Trigger::default(),
        ],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    // A plain pattern is literal: "(what?)" is not a group.
    assert!(engine.on_line("Ana tells you (what?) now"));
    assert!(engine.on_line("Ana tells you what now"));
    assert_eq!(
        engine.take_actions(),
        [
            send("smile"),
            send("say \"hi\" \\o/"),
            Action::Speak {
                text: "a tell".into(),
                interrupt: false
            },
            Action::PlaySound {
                file: "tell.ogg".into(),
                volume: 1.0,
                looped: false,
                owner: "rules".into()
            },
        ]
    );
    assert!(engine.on_line("Orc attacks Ana!"));
    assert_eq!(
        engine.take_actions(),
        [
            send("kill Orc"),
            Action::Speak {
                text: "Orc hits Ana".into(),
                interrupt: false
            }
        ]
    );
    assert!(!engine.on_line("buy spam today"));
    assert_eq!(engine.take_actions(), []);
}

#[test]
fn aliases_and_keys() {
    let rules = Rules {
        aliases: vec![
            alias("k", |rule| rule.send = "kill %1; loot".into()),
            alias(r"^g (\w+) (\w+)$", |rule| {
                rule.when.regex = true;
                rule.send = "give %1 to %2".into();
            }),
        ],
        macros: vec![
            key("F5", |rule| rule.send = "look".into()),
            // Sent as the other rules send, whatever separates typed ones.
            key("F7", |rule| rule.send = "say a;;b; look".into()),
            key("f6", |rule| {
                rule.enabled = false;
                rule.send = "never".into();
            }),
        ],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    for input in ["k orc chief", "k", "kick", "g sword ana"] {
        engine.on_input(input);
    }
    assert_eq!(
        engine.take_actions(),
        [
            send("kill orc chief"),
            send("loot"),
            send("kill "),
            send("loot"),
            send("kick"),
            send("give sword to ana"),
        ]
    );
    assert!(engine.on_key("f5"));
    assert!(!engine.on_key("f6"));
    assert_eq!(engine.take_actions(), [send("look")]);
    engine.exec("t", "separator(false)");
    assert!(engine.on_key("f7"));
    assert_eq!(engine.take_actions(), [send("say a;b"), send("look")]);
    // Switched to Luau, a rule starts from what it ran.
    assert_eq!(
        crate::rules::commands_as_luau("kill %1 $target", true),
        "send(expand(\"kill \" .. (m[1] or \"\") .. \" $target\"))\n"
    );
}

#[test]
fn templates_are_filled_from_a_match() {
    let alias = alias("k", |_| {});
    let regex = regex::Regex::new(&alias.when.regex_source()).unwrap();
    let captures = regex.captures("k orc chief").unwrap();
    assert_eq!(
        fill("kill %1; say 100%% of %1 %9", &captures),
        "kill orc chief; say 100%% of orc chief "
    );
    assert!(regex.captures("kick").is_none());
    let trigger = trigger("a (b)?", |rule| {
        rule.send = "x".into();
        rule.when.gag = true;
    });
    assert_eq!(trigger.when.regex_source(), "a \\(b\\)\\?");
    assert_eq!(
        trigger.summary(),
        [(Part::Send, "x".to_owned()), (Part::Gag, String::new())]
    );
}

#[test]
fn gauges_follow_values_the_server_reports() {
    let gauge = |value: &str, max: &str| Gauge {
        name: "Salud".into(),
        value: value.into(),
        max: max.into(),
        colour: "green".into(),
        ..Gauge::starter()
    };
    let rules = Rules {
        gauges: vec![
            gauge("Char.Vitals.health.current", "Char.Vitals.health.max"),
            Gauge {
                name: "Oro".into(),
                ..gauge("Player.Status.gold", "1000")
            },
        ],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    engine.on_gmcp(
        "Char.Vitals",
        r#"{"health": {"current": 25, "max": "100"}}"#,
    );
    let look = muddie_script::GaugeLook {
        colour: "green".into(),
        kind: muddie_script::GaugeKind::Cells,
        steps: vec![(30.0, "red".into())],
        ..muddie_script::GaugeLook::default()
    };
    assert_eq!(
        engine.take_actions(),
        [Action::Gauge {
            name: "Salud".into(),
            level: Some((25.0, 100.0)),
            look: look.clone(),
            owner: "rules".into(),
            belongs: muddie_script::Belongs {
                place: Some("Salud".into()),
                tags: Vec::new(),
            },
            panel: String::new(),
        }]
    );
    // A maximum can be a plain number; a value not there yet shows nothing.
    engine.on_gmcp("Player.Status", r#"{"level": 3}"#);
    assert_eq!(engine.take_actions(), []);
    engine.on_gmcp("Player.Status", r#"{"gold": 40}"#);
    assert_eq!(
        engine.take_actions(),
        [Action::Gauge {
            name: "Oro".into(),
            level: Some((40.0, 1000.0)),
            look,
            owner: "rules".into(),
            belongs: muddie_script::Belongs {
                place: Some("Oro".into()),
                tags: Vec::new(),
            },
            panel: String::new(),
        }]
    );
}

/// For a MUD that only says it in its text: a trigger keeps it in a
/// variable, and a gauge follows the variable, a field inside it too.
#[test]
fn gauges_follow_variables_that_triggers_set() {
    let rules = Rules {
        triggers: vec![trigger(r"^Hp: (\d+)/(\d+)", |rule| {
            rule.when.regex = true;
            rule.luau = true;
            rule.code = "vars.hp = tonumber(m[1]) vars.me = {most = tonumber(m[2])}".into();
        })],
        gauges: vec![Gauge {
            name: "Vida".into(),
            value: "hp".into(),
            max: "me.most".into(),
            ..Gauge::starter()
        }],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    engine.take_actions();
    engine.on_line("Hp: 25/80");
    let shown: Vec<_> = (engine.take_actions().into_iter())
        .filter_map(|action| match action {
            Action::Gauge { name, level, .. } => Some((name, level)),
            _ => None,
        })
        .collect();
    assert_eq!(shown.last(), Some(&("Vida".to_owned(), Some((25.0, 80.0)))));
}

/// A gauge of effects follows a list a script keeps, and shows none when
/// there is none.
#[test]
fn a_gauge_of_effects_follows_a_list() {
    let rules = Rules {
        gauges: vec![Gauge {
            name: "Efectos".into(),
            value: "estados".into(),
            kind: GaugeKind::Effects,
            ..Gauge::starter()
        }],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    let shown = |engine: &mut Engine| -> Vec<Vec<String>> {
        (engine.take_actions().into_iter())
            .filter_map(|action| match action {
                Action::Gauge { look, .. } => {
                    Some(look.items.into_iter().map(|item| item.name).collect())
                }
                _ => None,
            })
            .collect()
    };
    engine.exec(
        "t",
        r#"vars.estados = {{name = "Enfermedad Dañadora", colour = "red"}}"#,
    );
    assert_eq!(shown(&mut engine), [vec!["Enfermedad Dañadora".to_owned()]]);
    engine.exec("t", "vars.estados = nil");
    assert_eq!(shown(&mut engine), [Vec::<String>::new()]);
}

#[test]
fn rules_run_their_scripts_and_take_commands_on_several_lines() {
    let rules = Rules {
        triggers: vec![trigger(r"^(\w+) attacks", |rule| {
            rule.when.regex = true;
            rule.send = "kill %1\nshield; parry".into();
            rule.code = "send(\"never sent\")".into();
        })],
        macros: vec![key("f5", |rule| {
            rule.luau = true;
            rule.code = "send(\"score\")".into();
            rule.send = "never sent".into();
        })],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    engine.on_line("Orc attacks");
    assert_eq!(
        engine.take_actions(),
        ["kill Orc", "shield", "parry"].map(send)
    );
    // A rule does one or the other, whatever the other holds.
    assert!(engine.on_key("f5"));
    assert_eq!(engine.take_actions(), [send("score")]);
    // A rule whose script is wrong is reported by name, and alone.
    let broken = Rules {
        triggers: vec![trigger("x", |rule| {
            rule.name = "Broken".into();
            rule.code = "this is not Luau".into();
            rule.luau = true;
        })],
        ..rules
    };
    let chunks = broken.chunks();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].0, "trigger Broken");
}

#[test]
fn timers_go_off_once_or_again_and_again() {
    use std::time::{Duration, Instant};
    let rules = Rules {
        timers: vec![
            timer(20, |rule| rule.send = "save\nscore".into()),
            timer(30, |rule| {
                rule.when.repeat = false;
                rule.luau = true;
                rule.code = "send(\"once\")".into();
            }),
            timer(600, |rule| {
                rule.enabled = false;
                rule.send = "never".into();
            }),
        ],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    let start = Instant::now();
    engine.tick(start + Duration::from_millis(2100));
    assert_eq!(engine.take_actions(), ["save", "score"].map(send));
    engine.tick(start + Duration::from_millis(4200));
    let mut sent = engine.take_actions();
    sent.sort_by_key(|action| format!("{action:?}"));
    assert_eq!(sent, ["once", "save", "score"].map(send));
    engine.tick(start + Duration::from_millis(6300));
    assert_eq!(engine.take_actions(), ["save", "score"].map(send));
    assert_eq!(rules.timers[0].when.interval(), "2 s");
    assert_eq!(Timer::default().when.interval(), "1 min");
    assert_eq!(timer(905, |_| {}).when.interval(), "1 min 30.5 s");
}

#[test]
fn commands_of_rules_take_the_values_of_variables() {
    let rules = Rules {
        triggers: vec![trigger(r"^(\w+) arrives", |rule| {
            rule.when.regex = true;
            rule.send = "tell $friend %1 is here; $5 stays".into();
        })],
        timers: vec![timer(10, |rule| rule.send = "cast $spell".into())],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    engine
        .set_variable("friend", Some(serde_json::json!("Ana")))
        .unwrap();
    engine
        .set_variable("spell", Some(serde_json::json!("shield")))
        .unwrap();
    engine.on_line("Orc arrives");
    assert_eq!(
        engine.take_actions(),
        ["tell Ana Orc is here", "$5 stays"].map(send)
    );
    engine.tick(std::time::Instant::now() + std::time::Duration::from_millis(1100));
    assert_eq!(engine.take_actions(), [send("cast shield")]);
}

#[test]
fn gauges_come_in_kinds_and_change_colour_by_level() {
    let gauge = |name: &str, kind, value: &str| Gauge {
        name: name.into(),
        enabled: true,
        kind,
        value: value.into(),
        ..Gauge::default()
    };
    let rules = Rules {
        gauges: vec![
            gauge("Poisoned", GaugeKind::Light, "Char.Status.poisoned"),
            gauge("Stance", GaugeKind::Text, "Char.Status.stance"),
            gauge("Gold", GaugeKind::Number, "Char.Status.gold"),
        ],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    engine.on_gmcp(
        "Char.Status",
        r#"{"poisoned": "yes", "stance": "defensive", "gold": 40}"#,
    );
    let actions = engine.take_actions();
    type Seen<'a> = (&'a str, Option<(f64, f64)>, &'a str, &'a str);
    let seen: Vec<Seen> = (actions.iter())
        .map(|action| match action {
            Action::Gauge {
                name, level, look, ..
            } => (name.as_str(), *level, look.kind.key(), look.text.as_str()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("Poisoned", Some((1.0, 1.0)), "light", ""),
            ("Stance", Some((0.0, 0.0)), "text", "defensive"),
            ("Gold", Some((40.0, 0.0)), "number", ""),
        ]
    );
    // What is not so puts the light out, whichever way it is said.
    for off in ["false", "0", "\"no\"", "\"\""] {
        engine.on_gmcp("Char.Status", &format!(r#"{{"poisoned": {off}}}"#));
        let Some(Action::Gauge { level, .. }) = engine.take_actions().into_iter().next() else {
            panic!("no gauge for {off}");
        };
        assert_eq!(level, Some((0.0, 1.0)), "{off}");
    }
}

/// How a gauge is announced is kept with it, unless it is as usual, and
/// told to the engine.
#[test]
fn how_a_gauge_is_announced_is_kept_and_told() {
    let mut gauge = Gauge {
        name: "HP".into(),
        value: "Char.hp".into(),
        max: "10".into(),
        ..Gauge::starter()
    };
    assert!(!toml::to_string(&gauge).unwrap().contains("announce"));
    gauge.announce = Announce::Lowest;
    let written = toml::to_string(&gauge).unwrap();
    assert!(written.contains("announce = \"lowest\""), "{written}");
    assert_eq!(toml::from_str::<Gauge>(&written).unwrap(), gauge);
    let mut engine = run(&Rules {
        gauges: vec![gauge],
        ..Rules::default()
    });
    engine.on_gmcp("Char", r#"{"hp": 4}"#);
    match engine.take_actions().remove(0) {
        Action::Gauge { look, .. } => {
            assert_eq!(look.announce, muddie_script::Announce::Lowest)
        }
        other => panic!("{other:?}"),
    }
}
