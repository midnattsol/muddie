//! Tests of the gauges: widths, colours, what is said and updating.

use super::*;
use arrange::{Want, arrange};

const BAR: Want = Want {
    min: 140.0,
    max: 320.0,
    height: 31.0,
    bar: true,
};

fn fixed(width: f32) -> Want {
    Want {
        min: width,
        max: width,
        height: 28.0,
        bar: false,
    }
}

/// Bars share a wide panel, no wider than looks well; in a narrow one,
/// each takes its whole width, one under another; in between, in as many
/// columns as fit, those of a row after the first no wider than those of
/// the first.
#[test]
fn bars_share_the_width_of_their_panel() {
    let widths = |room: f32| -> Vec<(f32, f32)> {
        let (placed, _) = arrange(room, GAP, 8.0, &[BAR; 3]);
        placed
            .iter()
            .map(|place| (place.row as f32, place.width))
            .collect()
    };
    // Three in a row at the bottom of a wide window: their widest.
    assert_eq!(widths(1460.0), [(0.0, 320.0); 3]);
    // Three in a row that fits them just: the width shared, to the pixel.
    let shared = widths(600.0);
    assert!(
        shared
            .iter()
            .all(|&(y, width)| y == 0.0 && (185.0..=186.0).contains(&width)),
        "{shared:?}"
    );
    // Beside the output, narrow: one under another, the whole width.
    let narrow = widths(300.0);
    assert!(
        narrow.iter().all(|&(_, width)| width == 300.0),
        "{narrow:?}"
    );
    assert!(narrow[1].0 > narrow[0].0 && narrow[2].0 > narrow[1].0);
    // In between: two columns, the third under the first, no wider.
    let two = widths(400.0);
    assert_eq!((two[0].0, two[1].0), (0.0, 0.0));
    assert!(two[2].0 > 0.0 && two[2].1 <= two[0].1, "{two:?}");
}

/// Whatever the width and the gauges, each is inside the panel, none over
/// another, in the order they have, row after row; and a wider panel never
/// has more rows.
#[test]
fn gauges_are_put_in_order_inside_the_panel() {
    let mixes: [&[Want]; 4] = [
        &[BAR; 5],
        &[BAR, BAR, BAR, fixed(95.0)],
        &[fixed(60.0), BAR, fixed(250.0), BAR, fixed(40.0)],
        &[fixed(400.0), BAR],
    ];
    for wants in mixes {
        let mut rows_before = usize::MAX;
        for room in 100..1500 {
            let room = room as f32;
            let (placed, height) = arrange(room, GAP, 8.0, wants);
            let mut rows = 1;
            for (at, place) in placed.iter().enumerate() {
                assert!(
                    place.x >= 0.0 && place.x + place.width <= room,
                    "{room}: {placed:?}"
                );
                assert!(
                    place.y + wants[at].height <= height + 0.5,
                    "{room}: {placed:?}"
                );
                if at > 0 {
                    let before = placed[at - 1];
                    match place.row {
                        row if row == before.row + 1 => {
                            rows += 1;
                            assert!(place.y > before.y, "{room}: {placed:?}");
                        }
                        row if row == before.row => assert!(
                            place.x >= before.x + before.width + GAP,
                            "{room}: {placed:?}"
                        ),
                        _ => panic!("{room}: rows out of order: {placed:?}"),
                    }
                }
            }
            assert!(
                rows <= rows_before,
                "{room}: {rows} rows, {rows_before} before"
            );
            rows_before = rows;
        }
    }
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
        .map(|said| said.text)
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
        .map(|said| (said.text, said.aloud.how == How::Cut))
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

/// What a gauge says as it falls, and whether it cuts in, as it is told.
#[test]
fn a_gauge_is_heard_as_it_is_told() {
    let falls = |how: How| -> Vec<(String, bool)> {
        let mut gauges = Gauges::default();
        let look = GaugeLook {
            steps: vec![(60.0, "yellow".into()), (30.0, "red".into())],
            aloud: Aloud { how, always: false },
            ..GaugeLook::default()
        };
        [80.0, 55.0, 42.0, 25.0, 12.0]
            .into_iter()
            .filter_map(|value| {
                gauges.set_by(
                    "HP",
                    Some((value, 100.0)),
                    Some(look.clone()),
                    None,
                    Belongs::default(),
                    String::new(),
                )
            })
            .map(|said| (said.text, said.aloud.how == How::Cut))
            .collect()
    };
    let said = |urgent: [bool; 4]| -> Vec<(String, bool)> {
        [55, 42, 25, 12]
            .into_iter()
            .zip(urgent)
            .map(|(value, urgent)| (format!("HP {value} of 100"), urgent))
            .collect()
    };
    assert_eq!(falls(How::Usual), said([true; 4]));
    assert_eq!(falls(How::Cut), said([true; 4]));
    assert_eq!(falls(How::Wait), said([false; 4]));
    assert_eq!(falls(How::Lowest), said([false, false, true, true]));
    assert_eq!(falls(How::Never), []);
}

/// One of words, with no steps, waits unless it is told to cut in.
#[test]
fn a_gauge_of_words_is_heard_as_it_is_told() {
    let changes = |how: How| -> Vec<bool> {
        let mut gauges = Gauges::default();
        ["a", "b"]
            .into_iter()
            .filter_map(|text| {
                let look = GaugeLook {
                    kind: GaugeKind::Text,
                    text: text.into(),
                    aloud: Aloud { how, always: false },
                    ..GaugeLook::default()
                };
                let level = Some((0.0, 0.0));
                gauges.set_by(
                    "T",
                    level,
                    Some(look),
                    None,
                    Belongs::default(),
                    String::new(),
                )
            })
            .map(|said| said.aloud.how == How::Cut)
            .collect()
    };
    assert_eq!(changes(How::Usual), [false]);
    assert_eq!(changes(How::Wait), [false]);
    assert_eq!(changes(How::Lowest), [false]);
    assert_eq!(changes(How::Cut), [true]);
    assert_eq!(changes(How::Never), [] as [bool; 0]);
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

/// Where three bars and the effects the character is in are drawn in a
/// panel `room` wide, by what each tells.
fn drawn_at(room: f32) -> Vec<Rect> {
    use egui_kittest::kittest::Queryable;
    let harness = drawing_at(room);
    let labels = [
        "HP 80 of 150",
        "MP 20 of 150",
        "MV 140 of 150",
        "States: Poisoned",
    ];
    labels
        .map(|label| harness.get_by_label(label).rect())
        .to_vec()
}

/// A panel `room` wide with three bars and the effects the character is in.
fn drawing_at(room: f32) -> egui_kittest::Harness<'static> {
    use std::sync::{Arc, Mutex};
    let mut gauges = Gauges::default();
    for (name, value) in [("HP", 80.0), ("MP", 20.0), ("MV", 140.0)] {
        gauges.set_by(
            name,
            Some((value, 150.0)),
            None,
            None,
            Belongs::default(),
            String::new(),
        );
    }
    let effects = GaugeLook {
        kind: GaugeKind::Effects,
        items: vec![muddie_script::Effect {
            name: "Poisoned".into(),
            ..Default::default()
        }],
        ..GaugeLook::default()
    };
    gauges.set_by(
        "States",
        Some((0.0, 0.0)),
        Some(effects),
        None,
        Belongs::default(),
        String::new(),
    );
    let gauges = Arc::new(Mutex::new(gauges));
    let shown = Arc::clone(&gauges);
    let mut ready = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(vec2(room + 16.0, 300.0))
        .build_ui(move |ui| {
            // Its fonts, given before anything is drawn with them.
            if !std::mem::replace(&mut ready, true) {
                crate::theme::apply(ui.ctx(), &crate::settings::Settings::default()).unwrap();
                return;
            }
            shown.lock().unwrap().show(ui, |_| true, |_| false);
        });
    harness.run_steps(3);
    harness
}

/// Gauges go on to another row only where they do not fit, and the same
/// at every width: a bar is not put under the others with room beside
/// them, and the effects go beside the bars wherever both fit.
#[test]
fn gauges_go_on_to_another_row_only_where_they_do_not_fit() {
    let mut rows_before = usize::MAX;
    for room in (300..=1100).step_by(7) {
        let rects = drawn_at(room as f32);
        let bars = &rects[..3];
        let rows = {
            // Those whose middles are level are in a row.
            let mut middles: Vec<f32> = rects.iter().map(|rect| rect.center().y).collect();
            middles.dedup_by(|one, other| (*one - *other).abs() < 2.0);
            middles.len()
        };
        // Wider, never more rows.
        assert!(
            rows <= rows_before,
            "{room}: {rows} rows, {rows_before} before"
        );
        rows_before = rows;
        // Three bars fit in a row from 3 × 140 and two gaps.
        if room as f32 >= 3.0 * 140.0 + 2.0 * GAP {
            assert!(
                bars.iter()
                    .all(|bar| (bar.center().y - bars[0].center().y).abs() < 2.0),
                "{room}: {bars:?}"
            );
        }
        // Nothing beyond the panel's right edge.
        let left = rects
            .iter()
            .map(|rect| rect.left())
            .fold(f32::INFINITY, f32::min);
        assert!(
            rects
                .iter()
                .all(|rect| rect.right() <= left + room as f32 + 0.5),
            "{room}: {rects:?}"
        );
    }
    // Room for the bars at their narrowest and the effects: beside them.
    let rects = drawn_at(700.0);
    assert!(
        (rects[3].center().y - rects[0].center().y).abs() < 2.0,
        "{rects:?}"
    );
}

/// Images of a panel of gauges at several widths, for looking at by eye:
/// `MUDDIE_SCREENSHOT=/tmp/muddie.png cargo test -p muddie -- --ignored
/// gauges_at_widths`.
#[test]
#[ignore = "needs a graphics adapter; writes images for visual review"]
fn gauges_at_widths() {
    let Some(path) = std::env::var_os("MUDDIE_SCREENSHOT").map(std::path::PathBuf::from) else {
        return;
    };
    for room in [300.0, 468.0, 700.0, 1100.0] {
        let mut harness = drawing_at(room);
        let image = harness.render().expect("rendering failed");
        image
            .save(path.with_extension(format!("gauges-{room}.png")))
            .unwrap();
    }
}

/// A bar tells how full it is, or a screen reader takes it for busy.
#[test]
fn a_bar_tells_how_full_it_is() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let harness = drawing_at(700.0);
    let bar = harness.get_by_label("HP 80 of 150");
    let node = bar.accesskit_node();
    assert_eq!(node.role(), Role::ProgressIndicator);
    assert_eq!(
        (
            node.numeric_value(),
            node.min_numeric_value(),
            node.max_numeric_value()
        ),
        (Some(80.0), Some(0.0), Some(150.0))
    );
}
