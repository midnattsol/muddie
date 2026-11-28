//! Rules during play: gauges following what they show, and switching.

use super::*;

fn levels(engine: &mut Engine) -> Vec<(String, Option<(f64, f64)>)> {
    (engine.take_actions().into_iter())
        .filter_map(|action| match action {
            Action::Gauge { name, level, .. } => Some((name, level)),
            _ => None,
        })
        .collect()
}

/// An MSDP gauge: the MUD reports HEALTH, then HEALTH_MAX, as two
/// variables. Once both are known the gauge is to be shown.
#[test]
fn msdp_gauge_shown_once_its_maximum_comes() {
    let rules = Rules {
        gauges: vec![Gauge {
            name: "Vida".into(),
            value: "HEALTH".into(),
            max: "HEALTH_MAX".into(),
            ..Gauge::starter()
        }],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    engine.take_actions();
    engine.on_msdp("HEALTH", "\"80\"");
    engine.on_msdp("HEALTH_MAX", "\"100\"");
    let after_max = levels(&mut engine);
    // Sanity: the next HEALTH shows it, so both values are readable.
    engine.on_msdp("HEALTH", "\"80\"");
    assert_eq!(
        levels(&mut engine).last(),
        Some(&("Vida".to_owned(), Some((80.0, 100.0)))),
        "sanity"
    );
    assert_eq!(
        after_max.last().cloned(),
        Some(("Vida".to_owned(), Some((80.0, 100.0))))
    );
}

/// A level gained: the maximum grows while the life stays the same.
#[test]
fn gauge_follows_its_maximum_from_a_variable() {
    let rules = Rules {
        gauges: vec![Gauge {
            name: "Vida".into(),
            value: "hp".into(),
            max: "maxhp".into(),
            ..Gauge::starter()
        }],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    engine.exec("t", "vars.hp = 100 vars.maxhp = 100");
    assert_eq!(
        levels(&mut engine).last(),
        Some(&("Vida".to_owned(), Some((100.0, 100.0)))),
        "sanity: the gauge follows variables"
    );
    engine.exec("t", "vars.maxhp = 200");
    assert_eq!(
        levels(&mut engine).last(),
        Some(&("Vida".to_owned(), Some((100.0, 200.0))))
    );
}

/// A trigger at the top switched off, named as a folder is: the
/// triggers in the folder are to go on firing.
#[test]
fn a_rule_switched_off_does_not_switch_off_a_folder_named_alike() {
    let rules = Rules {
        triggers: vec![
            trigger("You are hungry", |rule| {
                rule.folder = "Comida".into();
                rule.send = "eat bread".into();
            }),
            trigger("Comida", |rule| {
                rule.enabled = false;
                rule.send = "never".into();
            }),
        ],
        ..Rules::default()
    };
    let mut engine = run(&rules);
    engine.on_line("You are hungry.");
    assert_eq!(engine.take_actions(), [send("eat bread")]);
}
