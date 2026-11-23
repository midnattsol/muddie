//! Tests of the rules made in the editor.

use std::rc::Rc;

use muddie_script::{Action, Engine};

use super::*;

/// The rules running as a session runs them: switched as saved.
fn run(rules: &Rules) -> Engine {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.set_switches(rules.switched_off());
    engine.exec("rules", &rules.to_luau());
    assert_eq!(engine.take_actions(), [], "the generated script failed");
    engine
}

fn send(text: &str) -> Action {
    Action::Send(text.into())
}

/// A trigger on `pattern`, with what `with` gives it.
fn trigger(pattern: &str, with: impl FnOnce(&mut Trigger)) -> Trigger {
    let mut rule = Trigger::default();
    rule.when.pattern = pattern.into();
    with(&mut rule);
    rule
}

fn alias(pattern: &str, with: impl FnOnce(&mut Alias)) -> Alias {
    let mut rule = Alias::default();
    rule.when.pattern = pattern.into();
    with(&mut rule);
    rule
}

fn key(key: &str, with: impl FnOnce(&mut Macro)) -> Macro {
    let mut rule = Macro::default();
    rule.when.key = key.into();
    with(&mut rule);
    rule
}

fn timer(tenths: u32, with: impl FnOnce(&mut Timer)) -> Timer {
    let mut rule = Timer::default();
    rule.when.tenths = tenths;
    with(&mut rule);
    rule
}

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
            alias("k", |rule| rule.send = "kill %1; loot".into()),
            alias(r"^g (\w+) (\w+)$", |rule| {
                rule.when.regex = true;
                rule.send = "give %1 to %2".into();
            }),
        ],
        macros: vec![
            key("F5", |rule| rule.send = "look".into()),
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
        [(Part::Send, "x".to_owned()), (Part::Hide, String::new())]
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
            owner: "rules".into(),
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
            rule.lua = true;
            rule.script = "vars.hp = tonumber(m[1]) vars.me = {most = tonumber(m[2])}".into();
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

#[test]
fn rules_run_their_scripts_and_take_commands_on_several_lines() {
    let rules = Rules {
        triggers: vec![trigger(r"^(\w+) attacks", |rule| {
            rule.when.regex = true;
            rule.send = "kill %1\nshield; parry".into();
            rule.script = "send(\"never sent\")".into();
        })],
        macros: vec![key("f5", |rule| {
            rule.lua = true;
            rule.script = "send(\"score\")".into();
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
            rule.script = "this is not Luau".into();
            rule.lua = true;
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
                rule.lua = true;
                rule.script = "send(\"once\")".into();
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
fn rules_are_switched_by_their_folder_or_their_tags() {
    let mut rules = Rules {
        triggers: vec![trigger("attacks", |rule| {
            rule.send = "parry".into();
            rule.folder = "Combat/Defence".into();
        })],
        aliases: vec![alias("k", |rule| {
            rule.send = "kill %1".into();
            rule.tags = vec!["Combat".into()];
        })],
        macros: vec![
            key("f1", |rule| {
                rule.send = "kick".into();
                rule.name = "Fight".into();
                rule.tags = vec!["combat".into()];
            }),
            key("f1", |rule| rule.send = "look".into()),
        ],
        ..Rules::default()
    };
    assert_eq!(rules.tag_names(), ["Combat"]);
    assert_eq!(
        rules.folder_paths(Kind::Trigger),
        ["Combat", "Combat/Defence"]
    );
    // Saved as off, they start off: the folder holds the trigger, the
    // tag the alias and the first key, which leaves the second.
    rules.set_folder_on(Kind::Trigger, "Combat", false);
    rules.set_tag_on("COMBAT", false);
    let mut engine = run(&rules);
    engine.on_line("An orc attacks");
    engine.on_input("k orc");
    engine.on_key("f1");
    assert_eq!(engine.take_actions(), ["k orc", "look"].map(send));
    // Switched on for the session, they all are.
    let mut switches = engine.switches();
    switches.set_tag("combat", true);
    switches.set_place(Kind::Trigger, "Combat", true);
    engine.set_switches(switches);
    engine.on_line("An orc attacks");
    engine.on_input("k orc");
    engine.on_key("f1");
    assert_eq!(
        engine.take_actions(),
        ["parry", "kill orc", "kick"].map(send)
    );
    rules.set_tag_on("combat", true);
    assert!(rules.tags_off.is_empty());
}

#[test]
fn folders_move_and_go_with_what_they_hold() {
    let mut rules = Rules {
        triggers: vec![
            trigger("a", |rule| rule.folder = "Combat".into()),
            trigger("b", |rule| rule.folder = "Combat/Healing".into()),
            trigger("c", |_| {}),
        ],
        ..Rules::default()
    };
    rules.set_folder_on(Kind::Trigger, "Combat/Healing", false);
    rules.move_folder(Kind::Trigger, "Combat", "Fighting");
    let folders: Vec<&str> = rules
        .triggers
        .iter()
        .map(|rule| rule.folder.as_str())
        .collect();
    assert_eq!(folders, ["Fighting", "Fighting/Healing", ""]);
    assert!(!rules.folder_on(Kind::Trigger, "Fighting/Healing"));
    // Taken away, what it held goes up a level.
    rules.remove_folder(Kind::Trigger, "Fighting");
    let folders: Vec<&str> = rules
        .triggers
        .iter()
        .map(|rule| rule.folder.as_str())
        .collect();
    assert_eq!(folders, ["", "Healing", ""]);
    assert_eq!(rules.folder_paths(Kind::Trigger), ["Healing"]);
}

#[test]
fn a_folder_is_put_before_or_after_another() {
    let mut rules = Rules {
        triggers: vec![
            trigger("a", |rule| rule.folder = "Combat".into()),
            trigger("b", |rule| rule.folder = "Travel".into()),
            trigger("c", |rule| rule.folder = "Chat".into()),
        ],
        ..Rules::default()
    };
    // Implied by what they hold, in the order of their names, they are
    // kept to have their places.
    assert_eq!(
        rules.folder_paths(Kind::Trigger),
        ["Chat", "Combat", "Travel"]
    );
    rules.place_folder(Kind::Trigger, "Travel", "Combat", false);
    assert_eq!(
        rules.folder_paths(Kind::Trigger),
        ["Chat", "Travel", "Combat"]
    );
    rules.place_folder(Kind::Trigger, "Chat", "Combat", true);
    assert_eq!(
        rules.folder_paths(Kind::Trigger),
        ["Travel", "Combat", "Chat"]
    );
}

#[test]
fn rules_alike_in_one_folder_have_places_of_their_own() {
    let rules = vec![
        trigger("spam", |_| {}),
        trigger("spam", |rule| rule.enabled = false),
        trigger("spam", |rule| rule.folder = "Junk".into()),
    ];
    let places: Vec<String> = (0..3).map(|index| RuleList::place(&rules, index)).collect();
    assert_eq!(places, ["spam", "spam #2", "Junk/spam"]);
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
            } => (
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
}

#[test]
fn rules_round_trip_through_their_file() {
    let path = std::env::temp_dir().join(format!("muddie-rules-{}/rules.toml", std::process::id()));
    assert_eq!(Rules::load(&path).unwrap(), Rules::default());
    let rules = Rules {
        tags_off: vec!["combat".into()],
        triggers: vec![trigger("a \"quoted\" [pattern]", |rule| {
            rule.when.gag = true;
            rule.folder = "Combat".into();
            rule.tags = vec!["Combat".into(), "loud".into()];
        })],
        aliases: vec![alias("k", |rule| {
            rule.enabled = false;
            rule.send = "kill %1".into();
        })],
        macros: vec![key("ctrl+k", |rule| {
            rule.name = "Kick".into();
            rule.send = "kick".into();
        })],
        timers: vec![timer(3000, |rule| {
            rule.name = "Save".into();
            rule.when.repeat = false;
            rule.send = "save".into();
        })],
        gauges: vec![Gauge {
            name: "Salud".into(),
            value: "Char.Vitals.health.current".into(),
            max: "Char.Vitals.health.max".into(),
            colour: "red".into(),
            ..Gauge::starter()
        }],
        folders: vec![Folder {
            kind: Kind::Trigger,
            path: "Combat".into(),
            enabled: false,
        }],
        scripts: vec![ScriptEntry {
            path: "Combat/keys".into(),
            enabled: false,
            tags: vec!["combat".into()],
        }],
    };
    rules.save(&path).unwrap();
    assert_eq!(Rules::load(&path).unwrap(), rules);
    // Macros are kept as such.
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("[[macro]]")
    );
    // A rule written by hand without `enabled` is on.
    std::fs::write(&path, "[[trigger]]\npattern = \"x\"\n").unwrap();
    assert!(Rules::load(&path).unwrap().triggers[0].enabled);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
