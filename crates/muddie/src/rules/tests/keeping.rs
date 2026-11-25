//! Tests of how rules are switched, put in folders and kept in their file.

use super::*;

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
    // Timers in seconds, as they are given everywhere.
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("seconds = 300.0")
    );
    // Its Luau as it was once written, `lua` and `script`, is read.
    std::fs::write(
        &path,
        "[[trigger]]\npattern = \"x\"\nlua = true\nscript = \"send('a')\"\n",
    )
    .unwrap();
    let old = &Rules::load(&path).unwrap().triggers[0];
    assert!(old.luau && old.code == "send('a')");
    // A rule written by hand without `enabled` is on.
    std::fs::write(&path, "[[trigger]]\npattern = \"x\"\n").unwrap();
    assert!(Rules::load(&path).unwrap().triggers[0].enabled);
    // A timer saved in tenths, as they were once, or in whole seconds.
    std::fs::write(&path, "[[timer]]\ntenths = 25\n[[timer]]\nseconds = 2\n").unwrap();
    let timers = Rules::load(&path).unwrap().timers;
    assert_eq!((timers[0].when.tenths, timers[1].when.tenths), (25, 20));
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
