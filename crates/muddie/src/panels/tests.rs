//! Tests of the gauges: widths, colours, what is said and updating.

use super::*;

/// Bars share a wide panel, no wider than looks well; in a narrow
/// one, each takes its whole width, one under another.
#[test]
fn bars_share_the_width_of_their_panel() {
    // Three in a row at the bottom of a wide window: their widest.
    assert_eq!(bar_width(1460.0, 3), 320.0);
    // Three in a row that fits them just: the width shared.
    assert_eq!(bar_width(600.0, 3), (600.0 - 2.0 * GAP) / 3.0);
    // Beside the output, narrow: one under another, the whole width.
    assert_eq!(bar_width(300.0, 3), 300.0);
    // In between: two columns.
    assert_eq!(bar_width(400.0, 3), (400.0 - GAP) / 2.0);
    assert_eq!(bar_width(500.0, 0), BAR_WIDTH);
}

#[test]
fn colours_are_named_as_scripts_name_them() {
    let theme = theme::current();
    assert_eq!(colour_named("Bright Red"), Some(theme.terminal[9]));
    assert_eq!(colour_named("green"), Some(theme.terminal[2]));
    assert_eq!(colour_named("#102030"), Some(Color32::from_rgb(16, 32, 48)));
    assert!(colour_named("purple").is_none() && colour_named("").is_none());
}

#[test]
fn a_gauge_is_said_when_it_falls_under_a_warning_and_not_when_it_rises() {
    let mut gauges = Gauges::default();
    let look = GaugeLook {
        steps: vec![(60.0, "yellow".into()), (30.0, "red".into())],
        ..GaugeLook::default()
    };
    let mut set = |value: f64| {
        let look = Some(look.clone());
        (gauges.set_by(
            "Health",
            Some((value, 100.0)),
            look,
            None,
            Belongs::default(),
            String::new(),
        ))
        .map(|warning| warning.said)
    };
    // As it starts is not news; above every warning, nothing either.
    assert_eq!(set(50.0), None);
    assert_eq!(set(80.0), None);
    assert_eq!(set(55.0).as_deref(), Some("Health 55 of 100"));
    assert_eq!(set(50.0), None);
    assert_eq!(set(25.0).as_deref(), Some("Health 25 of 100"));
    assert_eq!(set(45.0), None);
    assert_eq!(set(20.0).as_deref(), Some("Health 20 of 100"));
    // Under the lowest, each tenth more it falls is said.
    assert_eq!(set(15.0), None);
    assert_eq!(set(9.0).as_deref(), Some("Health 9 of 100"));
    // Wavering about a warning, it is said once, until clear of it.
    assert_eq!(set(31.0), None);
    assert_eq!(set(29.0), None);
    assert_eq!(set(31.0), None);
    assert_eq!(set(40.0), None);
    assert_eq!(set(29.0).as_deref(), Some("Health 29 of 100"));
}

/// A light, words or effects are said when what they say changes.
#[test]
fn a_light_words_and_effects_are_said_when_they_change() {
    let mut gauges = Gauges::default();
    let light = GaugeLook {
        kind: GaugeKind::Light,
        ..GaugeLook::default()
    };
    let mut set = |value: f64| {
        (gauges.set_by(
            "Poisoned",
            Some((value, 1.0)),
            Some(light.clone()),
            None,
            Belongs::default(),
            String::new(),
        ))
        .map(|warning| (warning.said, warning.urgent))
    };
    assert_eq!(set(0.0), None);
    assert_eq!(set(1.0), Some(("Poisoned on".to_owned(), false)));
    assert_eq!(set(1.0), None);
    assert_eq!(set(0.0), Some(("Poisoned off".to_owned(), false)));
}

#[test]
fn gauges_update_in_place_and_summarise() {
    let mut gauges = Gauges::default();
    gauges.set("HP", Some((80.0, 100.0)));
    gauges.set("MP", Some((5.0, 50.0)));
    gauges.set("HP", Some((70.5, 100.0)));
    assert_eq!(gauges.summary(), "HP 70.5 of 100, MP 5 of 50");
    gauges.set("HP", None);
    gauges.set("XP", None);
    assert_eq!(gauges.summary(), "MP 5 of 50");
}

/// Asked for all together, effects are said by their names; where each is
/// shown, with what it is.
#[test]
fn all_the_gauges_are_said_without_what_each_effect_is() {
    let mut gauges = Gauges::default();
    let look = GaugeLook {
        kind: GaugeKind::Effects,
        items: vec![muddie_script::Effect {
            name: "Poisoned".into(),
            about: "1 damage a second".into(),
            ..Default::default()
        }],
        ..GaugeLook::default()
    };
    let level = Some((0.0, 0.0));
    gauges.set_by(
        "Effects",
        level,
        Some(look),
        None,
        Belongs::default(),
        String::new(),
    );
    assert_eq!(gauges.summary(), "Effects: Poisoned");
    assert_eq!(gauges.0[0].said(), "Effects: Poisoned (1 damage a second)");
}
