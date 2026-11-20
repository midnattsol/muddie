//! Tests of the rules made in the editor.

use std::rc::Rc;

use muddie_script::{Action, Engine};

use super::*;

fn run(rules: &Rules) -> Engine {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.exec("rules", &rules.to_luau());
    assert_eq!(engine.take_actions(), [], "the generated script failed");
    engine
}

fn send(text: &str) -> Action {
    Action::Send(text.into())
}

#[test]
fn triggers_do_what_their_fields_say() {
    let rules = Rules {
        triggers: vec![
            Trigger {
                enabled: true,
                pattern: "tells you (what?)".into(),
                send: "smile; say \"hi\" \\o/".into(),
                speak: "a tell".into(),
                sound: "tell.ogg".into(),
                ..Trigger::default()
            },
            Trigger {
                enabled: true,
                pattern: r"^(\w+) attacks (\w+)".into(),
                regex: true,
                send: "kill %1".into(),
                speak: "%1 hits %2%3".into(),
                highlight: "red".into(),
                ..Trigger::default()
            },
            Trigger {
                enabled: true,
                pattern: "spam".into(),
                gag: true,
                ..Trigger::default()
            },
            Trigger {
                enabled: false,
                pattern: "spam".into(),
                send: "never".into(),
                ..Trigger::default()
            },
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
                looped: false
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
            Alias {
                enabled: true,
                pattern: "k".into(),
                send: "kill %1; loot".into(),
                ..Alias::default()
            },
            Alias {
                enabled: true,
                pattern: r"^g (\w+) (\w+)$".into(),
                regex: true,
                send: "give %1 to %2".into(),
                ..Alias::default()
            },
        ],
        keys: vec![
            KeyBinding {
                enabled: true,
                key: "F5".into(),
                send: "look".into(),
                ..KeyBinding::default()
            },
            KeyBinding {
                enabled: false,
                key: "f6".into(),
                send: "never".into(),
                ..KeyBinding::default()
            },
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
}

#[test]
fn templates_are_filled_from_a_match() {
    let alias = Alias {
        pattern: "k".into(),
        ..Alias::default()
    };
    let regex = regex::Regex::new(&alias.regex_source()).unwrap();
    let captures = regex.captures("k orc chief").unwrap();
    assert_eq!(
        fill("kill %1; say 100%% of %1 %9", &captures),
        "kill orc chief; say 100%% of orc chief "
    );
    assert!(regex.captures("kick").is_none());
    let trigger = Trigger {
        pattern: "a (b)?".into(),
        send: "x".into(),
        gag: true,
        ..Trigger::default()
    };
    assert_eq!(trigger.regex_source(), "a \\(b\\)\\?");
    assert_eq!(trigger.summary(), "→ x  ·  ∅");
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
        kind: "cells".into(),
        steps: vec![(30.0, "red".into())],
        ..muddie_script::GaugeLook::default()
    };
    assert_eq!(
        engine.take_actions(),
        [Action::Gauge {
            name: "Salud".into(),
            level: Some((25.0, 100.0)),
            look: look.clone(),
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
        }]
    );
}

#[test]
fn rules_run_their_scripts_and_take_commands_on_several_lines() {
    let rules = Rules {
        triggers: vec![Trigger {
            enabled: true,
            pattern: r"^(\w+) attacks".into(),
            regex: true,
            send: "kill %1\nshield; parry".into(),
            script: "if m[1] == \"Orc\" then\n\tsend(\"flee from \" .. line)\nend".into(),
            ..Trigger::default()
        }],
        keys: vec![KeyBinding {
            enabled: true,
            key: "f5".into(),
            send: "look".into(),
            script: "send(\"score\")".into(),
            ..KeyBinding::default()
        }],
        ..Rules::default()
    };
    // Written by an earlier version, with commands and a script both:
    // the commands become the start of the script.
    let mut rules = rules;
    rules.migrate();
    let trigger = &rules.triggers[0];
    assert!(trigger.lua && trigger.send.is_empty());
    assert!(
        trigger
            .script
            .starts_with("send(\"kill \" .. (m[1] or \"\"))\nsend(\"shield\")\n")
    );
    let mut engine = run(&rules);
    engine.on_line("Orc attacks");
    let expected = ["kill Orc", "shield", "parry", "flee from Orc attacks"].map(send);
    assert_eq!(engine.take_actions(), expected);
    assert!(engine.on_key("f5"));
    assert_eq!(engine.take_actions(), ["look", "score"].map(send));
    // A rule does one or the other: with the script chosen, commands
    // left in the other form are not sent.
    let mut either = rules.clone();
    either.triggers[0].send = "never sent".into();
    let mut engine = run(&either);
    engine.on_line("Orc attacks");
    assert_eq!(engine.take_actions(), expected);
    // A rule whose script is wrong is reported by name, and alone.
    let broken = Rules {
        triggers: vec![Trigger {
            name: "Broken".into(),
            enabled: true,
            pattern: "x".into(),
            script: "this is not Luau".into(),
            lua: true,
            ..Trigger::default()
        }],
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
            Timer {
                tenths: 20,
                send: "save\nscore".into(),
                ..Timer::default()
            },
            Timer {
                tenths: 30,
                repeat: false,
                lua: true,
                script: "send(\"once\")".into(),
                ..Timer::default()
            },
            Timer {
                enabled: false,
                send: "never".into(),
                ..Timer::default()
            },
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
    assert_eq!(rules.timers[0].interval(), "2 s");
    assert_eq!(Timer::default().interval(), "1 min");
    let long = Timer {
        tenths: 905,
        ..Timer::default()
    };
    assert_eq!(long.interval(), "1 min 30.5 s");
}

#[test]
fn rules_of_a_group_are_switched_together_whatever_their_kind() {
    let mut rules = Rules {
        triggers: vec![Trigger {
            enabled: true,
            pattern: "attacks".into(),
            send: "parry".into(),
            group: "Combat".into(),
            ..Trigger::default()
        }],
        aliases: vec![Alias {
            enabled: true,
            pattern: "k".into(),
            send: "kill %1".into(),
            group: "Combat".into(),
            ..Alias::default()
        }],
        keys: vec![
            KeyBinding {
                enabled: true,
                key: "f1".into(),
                send: "kick".into(),
                group: "Combat".into(),
                ..KeyBinding::default()
            },
            KeyBinding {
                enabled: true,
                key: "f1".into(),
                send: "look".into(),
                group: "Explore".into(),
                ..KeyBinding::default()
            },
        ],
        ..Rules::default()
    };
    assert_eq!(rules.group_names(), ["Combat", "Explore"]);
    assert!(rules.group_enabled("Combat"));
    // Saved as off, it starts off: nothing of the group takes part.
    rules.set_group_enabled("Combat", false);
    let mut engine = run(&rules);
    engine.on_line("An orc attacks");
    engine.on_input("k orc");
    engine.on_key("f1");
    assert_eq!(engine.take_actions(), ["k orc", "look"].map(send));
    // Switched on for the session, all of it does.
    engine.set_group("Combat", true);
    engine.on_line("An orc attacks");
    engine.on_input("k orc");
    engine.on_key("f1");
    assert_eq!(
        engine.take_actions(),
        ["parry", "kill orc", "kick"].map(send)
    );
    rules.set_group_enabled("Combat", true);
    assert!(
        rules.groups.is_empty(),
        "the usual state is not written down"
    );
}

#[test]
fn commands_of_rules_take_the_values_of_variables() {
    let rules = Rules {
        triggers: vec![Trigger {
            enabled: true,
            pattern: r"^(\w+) arrives".into(),
            regex: true,
            send: "tell $friend %1 is here; $5 stays".into(),
            ..Trigger::default()
        }],
        timers: vec![Timer {
            tenths: 10,
            send: "cast $spell".into(),
            ..Timer::default()
        }],
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
            Action::Gauge { name, level, look } => (
                name.as_str(),
                *level,
                look.kind.as_str(),
                look.text.as_str(),
            ),
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
    // As earlier versions kept them, they are read and brought up to date.
    let old: Rules =
        toml::from_str("[[gauge]]\nname = \"HP\"\nvalue = \"x\"\nsegments = true\nlow = 25\n")
            .unwrap();
    let mut old = old;
    old.migrate();
    let hp = &old.gauges[0];
    assert_eq!(
        (hp.kind, hp.steps[0].under, hp.steps[0].colour.as_str()),
        (GaugeKind::Cells, 25, "red")
    );
    assert!(!toml::to_string(&old).unwrap().contains("segments"));
}

#[test]
fn rules_round_trip_through_their_file() {
    let path = std::env::temp_dir().join(format!("muddie-rules-{}/rules.toml", std::process::id()));
    assert_eq!(Rules::load(&path).unwrap(), Rules::default());
    let rules = Rules {
        triggers: vec![Trigger {
            enabled: true,
            pattern: "a \"quoted\" [pattern]".into(),
            gag: true,
            ..Trigger::default()
        }],
        aliases: vec![Alias {
            enabled: false,
            pattern: "k".into(),
            send: "kill %1".into(),
            ..Alias::default()
        }],
        keys: vec![KeyBinding {
            name: "Kick".into(),
            enabled: true,
            key: "ctrl+k".into(),
            send: "kick".into(),
            ..KeyBinding::default()
        }],
        groups: vec![Group {
            name: "Combat".into(),
            enabled: false,
        }],
        timers: vec![Timer {
            name: "Save".into(),
            tenths: 3000,
            repeat: false,
            send: "save".into(),
            ..Timer::default()
        }],
        gauges: vec![Gauge {
            name: "Salud".into(),
            value: "Char.Vitals.health.current".into(),
            max: "Char.Vitals.health.max".into(),
            colour: "red".into(),
            ..Gauge::starter()
        }],
        scripts_off: vec!["keys.luau".into()],
    };
    rules.save(&path).unwrap();
    assert_eq!(Rules::load(&path).unwrap(), rules);
    // A rule written by hand without `enabled` is on.
    std::fs::write(&path, "[[trigger]]\npattern = \"x\"\n").unwrap();
    assert!(Rules::load(&path).unwrap().triggers[0].enabled);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
