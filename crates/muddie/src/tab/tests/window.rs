//! The window: its menu, its messages, what opens and stays, what is
//! announced, and how much drawing costs.

use super::*;

#[test]
fn what_the_client_says_can_be_read_again() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.note("Connected.");
    // An answer said in the command line for a moment is kept too.
    app.scripts
        .exec("t", r#"bind("f9", "kick", {tags = {"Combat"}})"#);
    app.run_command("/tag Combat off");
    app.run_command("/map find nowhere");
    harness.run_steps(3);
    let (said, unread) = harness.state().tab().notices();
    let texts: Vec<(&str, bool)> = said
        .iter()
        .map(|notice| (notice.text.as_str(), notice.problem))
        .collect();
    assert_eq!(
        texts,
        [
            ("Connected.", false),
            ("Tag combat off.", false),
            ("No room matches.", true)
        ]
    );
    assert_eq!(unread, 3);
    // The bell says how many are new, and opening it shows them.
    harness.get_by_label("Messages, 3 new").click();
    harness.run_steps(3);
    harness.get_by_label("No room matches.");
    assert_eq!(harness.state().tab().notices().1, 0);
}

#[test]
fn the_one_menu_opens_and_does_what_it_says() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    harness.get_by_label("Menu").click();
    harness.run_steps(3);
    harness.get_by_label("Keyboard shortcuts");
    harness.get_by_label("Settings").click();
    harness.run_steps(3);
    harness.get_by_label("Language");
}

#[test]
fn what_a_click_opens_stays_open() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.started = true;
    app.rules.gauges.push(crate::rules::Gauge {
        name: "Salud".into(),
        value: "Char.Vitals.hp".into(),
        max: "100".into(),
        ..crate::rules::Gauge::starter()
    });
    app.gauges.set("Salud", Some((40.0, 100.0)));
    app.note("Connected.");
    harness.run_steps(3);
    let steady = |harness: &mut Harness<'_, crate::app::App>, what: &str, shown: &str| {
        // It may be laid out a few frames late; once there, it stays.
        harness.run_steps(4);
        for frame in 0..6 {
            assert!(
                harness.query_by_label(shown).is_some(),
                "{what}: gone at frame {frame}"
            );
            harness.step();
        }
        harness.key_press(Key::Escape);
        harness.run_steps(3);
    };
    let session = harness
        .get_all_by_role(egui::accesskit::Role::Tab)
        .find(|tab| {
            tab.accesskit_node()
                .label()
                .is_some_and(|label| label.contains("Not connected"))
        })
        .expect("the session's tab");
    session.click_secondary();
    steady(&mut harness, "session menu", "Save as profile");
    harness.get_by_label("Menu").click();
    steady(&mut harness, "menu", "Keyboard shortcuts");
    harness.get_by_label("Messages, 1 new").click();
    steady(&mut harness, "messages", "Clear");
    harness.get_by_label("Gauges options").click();
    steady(&mut harness, "panel menu", "Hide Gauges");
    harness.get_by_label("Edit Salud").click();
    steady(&mut harness, "gauge", "Open in the editor");
    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.get_by_label("Settings").click();
    steady(&mut harness, "settings", "Language");
}

#[test]
fn lines_are_announced_through_a_live_region_when_asked() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.set_announce(true);
    app.voice.set_enabled(true);
    let (events, _sent) = fake_connection(app);
    // Let the notice that speech is on be announced first.
    harness.run_steps(2);
    assert_eq!(harness.state().tab().announcement, "Speech on.");
    events
        .send(Event::Line(
            panels::styled_lines("An orc arrives.").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    // The line is in the scrollback too; the announcement is the node
    // marked as a live region.
    let announced = (harness.query_all_by_label("An orc arrives."))
        .filter(|node| node.accesskit_node().live() == egui::accesskit::Live::Polite)
        .count();
    assert_eq!(announced, 1);
    // The same line again still counts as a change to announce.
    events
        .send(Event::Line(
            panels::styled_lines("An orc arrives.").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    assert_eq!(
        harness.state().tab().announcement,
        "An orc arrives.\u{200b}"
    );
    // Speaking directly instead leaves nothing to announce.
    harness.state_mut().tab_mut().voice.set_announce(false);
    harness.run_steps(2);
    assert!(!harness.state().tab().voice.announcing());
}

#[test]
fn long_scrollback_is_not_laid_out_all_at_once_when_the_text_changes() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    (0..6000).for_each(|n| {
        app.show_local(&format!(
            "line {n} of a long session, long enough to matter"
        ))
    });
    harness.run_steps(3);
    // A change of the text's size: every row may be another height.
    let mut settings = harness.state().tab().settings.clone();
    settings.font_size = 19.0;
    let ctx = harness.ctx.clone();
    crate::theme::apply(&ctx, &settings).unwrap();
    harness.state_mut().tab_mut().settings_changed(&settings);
    harness.step();
    // One frame does not pay for six thousand rows: it lays out a
    // share, and what is on the screen.
    let measured = harness.state().tab().output.measured();
    assert!(measured < 3000, "{measured} rows were laid out in a frame");
    // And the end stays in view while the rest catches up.
    harness.run_steps(40);
    assert!(harness.query_by_label("Latest lines").is_none());
}

#[test]
fn a_burst_of_lines_is_measured_a_share_a_frame() {
    let mut harness = window(None);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    (0..6000).for_each(|n| app.show_local(&format!("line {n} of a burst of text")));
    // Laid out all at once, each row would hold its laid out text for
    // the frame; a share is, the newest, which are the ones in view.
    harness.step();
    let share = crate::output::REMEASURED_PER_FRAME;
    let measured = harness.state().tab().output.measured();
    assert!(
        measured > 0 && measured <= share + 100,
        "{measured} rows measured in a frame"
    );
    // The view follows to the end, where the newest already are; a
    // frame or two later, as the size of what it holds settles.
    // However many frames that takes on a busy machine.
    for _ in 0..12 {
        harness.step();
        if harness
            .query_by_label("line 5999 of a burst of text")
            .is_some()
        {
            break;
        }
    }
    assert!(harness.query_by_label("Latest lines").is_none());
    harness.get_by_label("line 5999 of a burst of text");
    // The rest follow, and the end stays in view meanwhile.
    harness.run_steps(6000 / share + 2);
    assert_eq!(
        harness.state().tab().output.measured(),
        harness.state().tab().output.len()
    );
    assert!(harness.query_by_label("Latest lines").is_none());
}

#[test]
fn arriving_lines_do_not_flash_the_view_of_the_latest_lines() {
    let mut harness = window(None);
    for burst in 0..12 {
        let app = harness.state_mut().tab_mut();
        (0..9).for_each(|n| app.show_local(&format!("burst {burst} line {n}")));
        // Frame by frame: the view must not come up even for one.
        for _ in 0..3 {
            harness.step();
            assert!(
                harness.query_by_label("Latest lines").is_none(),
                "shown during burst {burst}"
            );
        }
    }
}

/// Says what a frame costs in the views that could be slow, to look
/// for what to make faster:
/// `cargo test -p muddie frame_costs -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check"]
fn frame_costs() {
    let cost = |harness: &mut Harness<'_, App>, what: &str| {
        // Once what a burst of text left to be measured has been: that is
        // done a share a frame, and is not what a frame costs after.
        harness.run_steps(90);
        let started = Instant::now();
        for _ in 0..30 {
            harness.step();
        }
        println!(
            "{what}: {:.2} ms a frame",
            started.elapsed().as_secs_f64() * 1000.0 / 30.0
        );
    };
    let mut harness = window(None);
    harness.set_size(egui::vec2(1600.0, 1000.0));
    cost(&mut harness, "empty session");
    let app = harness.state_mut().tab_mut();
    for n in 0..20_000 {
        app.show_local(&format!(
            "\x1b[32mline {n}\x1b[0m of a long session, with some text to wrap now and then"
        ));
    }
    cost(&mut harness, "20000 lines of scrollback");
    let app = harness.state_mut().tab_mut();
    for n in 0..2000 {
        let line = panels::styled_lines(&format!("[ooc] someone says thing {n}")).remove(0);
        app.channel_line(&line);
        app.add_to_panel("Combat", &line);
    }
    app.gauges.set("HP", Some((80.0, 100.0)));
    cost(&mut harness, "with panels of 2000 lines and gauges");
    let app = harness.state_mut().tab_mut();
    for n in 0..300 {
        app.rules
            .triggers
            .push(trigger(&format!("pattern {n}"), |rule| {
                rule.name = format!("Trigger {n}");
                rule.send = "say hi".into();
                rule.folder = format!("Folder {}", n % 6);
                rule.tags = vec![format!("tag {}", n % 4)];
            }));
    }
    app.open_automation(0);
    cost(&mut harness, "editor with 300 triggers");
    let app = harness.state_mut().tab_mut();
    for n in 0..120 {
        app.saw_data(
            &format!("Pkg.Number{n}"),
            false,
            r#"{"a": 1, "b": {"c": [1, 2, 3], "d": "text"}}"#,
        );
    }
    app.set_view(2);
    cost(&mut harness, "debug with 120 packages");
}
