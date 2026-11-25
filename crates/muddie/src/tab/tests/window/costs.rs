//! What drawing costs: long scrollback, bursts of lines, idle windows, and the measurements.

use super::*;

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

#[test]
fn reading_back_shows_the_latest_lines_over_the_view_without_moving_it() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1100.0, 700.0));
    let app = harness.state_mut().tab_mut();
    (0..400).for_each(|n| app.show_local(&format!("line {n} of the scrollback")));
    harness.run_steps(8);
    // Over the output, which fills the window but for its edges.
    harness.hover_at(egui::pos2(400.0, 300.0));
    let wheel = |harness: &mut Harness<'_, App>, lines: f32| {
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, lines),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(12);
    };
    let shown = |harness: &Harness<'_, App>| harness.query_by_label("Latest lines").is_some();
    // A step back is still following the end.
    wheel(&mut harness, 1.0);
    assert!(!shown(&harness));
    // Further back, the latest lines are shown over the foot of the view,
    // and they stay while it is moved up and down near there...
    wheel(&mut harness, 8.0);
    assert!(shown(&harness));
    for _ in 0..6 {
        wheel(&mut harness, -1.0);
        wheel(&mut harness, 1.0);
        assert!(shown(&harness), "the latest lines came and went");
    }
    // ...until the view is back at the end, however long the scroll
    // takes to settle on a busy machine.
    wheel(&mut harness, -40.0);
    for _ in 0..60 {
        if !shown(&harness) {
            break;
        }
        harness.run_steps(5);
    }
    assert!(!shown(&harness));
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
    app.open_automation(automation::TRIGGERS);
    cost(&mut harness, "editor with 300 triggers");
    let app = harness.state_mut().tab_mut();
    for n in 0..120 {
        app.saw_data(
            &format!("Pkg.Number{n}"),
            false,
            r#"{"a": 1, "b": {"c": [1, 2, 3], "d": "text"}}"#,
        );
    }
    app.set_view(View::Data);
    cost(&mut harness, "debug with 120 packages");
}

/// A window left alone draws nothing: no view asks for frames while
/// nothing changes, but for what is due (here, the keep-alive, in a
/// minute). Waited first for what settles a moment after a change.
#[test]
fn an_idle_window_draws_nothing() {
    type Setup = fn(&mut Harness<'_, App>);
    let views: [(&str, Setup); 3] = [
        ("game", |_| {}),
        ("editor", |harness| {
            let app = harness.state_mut().tab_mut();
            (app.rules.triggers).push(trigger("x", |rule| rule.folder = "F".into()));
            app.open_automation(automation::TRIGGERS);
        }),
        ("incoming data", |harness| {
            harness.state_mut().tab_mut().set_view(View::Data)
        }),
    ];
    for (name, setup) in views {
        let mut harness = connected_demo_window();
        // What the client said is over the corner until its time is up,
        // and asks for the frame that takes it away: that is due.
        harness.state_mut().tab_mut().clear_notices();
        setup(&mut harness);
        harness.run_steps(10);
        // What settles a moment after a change waits for that moment as
        // it is, which a busy machine makes longer: up to two seconds.
        let wait_now = |harness: &Harness<'_, App>| {
            (harness
                .output()
                .viewport_output
                .get(&egui::ViewportId::ROOT))
            .map(|viewport| viewport.repaint_delay)
            .unwrap_or_default()
        };
        let mut wait = wait_now(&harness);
        for _ in 0..6 {
            if wait.as_secs() >= 30 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
            harness.run_steps(3);
            wait = wait_now(&harness);
        }
        assert!(
            wait.as_secs() >= 30,
            "the {name} asks for a frame in {wait:?}"
        );
    }
}

/// Says what a line costs going through the triggers, with none and with
/// three hundred, and how long a burst of lines takes to be shown:
/// `cargo test -p muddie line_costs -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check"]
fn line_costs() {
    for count in [0usize, 300] {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        let mut rules = app.rules.clone();
        for n in 0..count {
            rules.triggers.push(trigger(
                &if n % 2 == 0 {
                    format!("pattern {n} here")
                } else {
                    format!(r"^(\w+) attacks you with (\w+) {n}")
                },
                |rule| {
                    rule.when.regex = n % 2 == 1;
                    rule.send = "say hi".into();
                    rule.when.highlight = if n % 3 == 0 {
                        "yellow".into()
                    } else {
                        String::new()
                    };
                },
            ));
        }
        let started = Instant::now();
        app.reload_rules(Some(rules.clone()));
        let first = started.elapsed();
        let started = Instant::now();
        app.reload_rules(Some(rules));
        eprintln!(
            "{count} triggers: rules put to work in {:.1} ms, again in {:.1} ms",
            first.as_secs_f64() * 1000.0,
            started.elapsed().as_secs_f64() * 1000.0
        );
        let (events, _sent) = fake_connection(app);
        harness.run_steps(3);
        let lines: Vec<_> = (0..5000)
            .map(|n| {
                panels::styled_lines(&format!(
                    "\x1b[32mAn orc number {n} arrives\x1b[0m from the north, looking for a fight"
                ))
                .remove(0)
            })
            .collect();
        // The triggers alone, as each line goes through them, once their
        // set is made.
        let app = harness.state_mut().tab_mut();
        app.scripts.on_line("start");
        std::thread::sleep(std::time::Duration::from_millis(1500));
        app.scripts.on_line("start");
        let started = Instant::now();
        for line in &lines {
            std::hint::black_box(app.scripts.on_styled_line(line, false));
        }
        let triggers = started.elapsed();
        // The whole way, drawing included, as they arrive at once.
        for line in &lines {
            events.send(Event::Line(line.clone())).unwrap();
        }
        let start = harness.state().tab().output.total_pushed();
        let started = Instant::now();
        let mut frames = 0;
        while harness.state().tab().output.total_pushed() < start + 5000 && frames < 2000 {
            harness.step();
            frames += 1;
        }
        let whole = started.elapsed();
        eprintln!(
            "{count} triggers: {:.1} µs a line in the triggers; 5000 lines shown in {:.0} ms over {frames} frames",
            triggers.as_secs_f64() * 1e6 / 5000.0,
            whole.as_secs_f64() * 1000.0
        );
    }
}
