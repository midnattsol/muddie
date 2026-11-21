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
    // The control named by its row; the row's words are hidden from a
    // screen reader, which hears them in the control.
    harness.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Language");
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
                harness.query_all_by_label(shown).next().is_some(),
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
fn what_is_said_aloud_is_the_game_the_channels_and_the_client() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    let (events, _sent) = fake_connection(app);
    events
        .send(Event::Line(
            panels::styled_lines("An orc arrives.").remove(0),
        ))
        .unwrap();
    // A channel of the MUD's: said with its name before it.
    events
        .send(Event::Gmcp {
            package: "Comm.Channel.Text".into(),
            data: r#"{"channel": "ooc", "text": "Ana: hola"}"#.into(),
        })
        .unwrap();
    harness.run_steps(3);
    let said = |harness: &Harness<'_, App>| harness.state().tab().voice.said.clone();
    assert_eq!(said(&harness), ["An orc arrives.", "ooc: Ana: hola"]);
    // Kept quiet, a channel is not read; and silenced, nothing is.
    let app = harness.state_mut().tab_mut();
    app.prefs.quiet_channels.push("OOC".into());
    app.voice.said.clear();
    events
        .send(Event::Gmcp {
            package: "Comm.Channel.Text".into(),
            data: r#"{"channel": "ooc", "text": "Ana: adiós"}"#.into(),
        })
        .unwrap();
    harness.run_steps(3);
    assert!(said(&harness).is_empty());
    let app = harness.state_mut().tab_mut();
    app.voice.set_muted(true);
    app.note("Nobody hears this.");
    assert!(said(&harness).is_empty());
}

#[test]
fn what_the_palette_finds_is_said_once_typing_pauses() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    app.toggle_palette();
    harness.run_steps(3);
    let field = "Command palette";
    (harness.get_by_role_and_label(egui::accesskit::Role::TextInput, field)).type_text("zzzz");
    harness.run_steps(2);
    // Nothing yet: what is typed may still change.
    assert!(harness.state().tab().voice.said.is_empty());
    std::thread::sleep(std::time::Duration::from_millis(600));
    harness.run_steps(2);
    assert_eq!(harness.state().tab().voice.said, ["No matches."]);
    assert!(harness.query_by_label("No matches.").is_some());
}

#[test]
fn a_dialog_opened_with_the_keyboard_has_it_and_says_its_name() {
    let mut harness = window(None);
    harness.run_steps(3);
    // Opened after a click, nothing in it is marked.
    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.get_by_label("Settings").click();
    harness.run_steps(4);
    assert!(!harness.get_by_label("Close").accesskit_node().is_focused());
    harness.get_by_role_and_label(egui::accesskit::Role::Dialog, "Settings");
    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);
    // Opened with the keyboard, the keyboard is in it, not in the command
    // line behind it.
    harness.key_press(egui::Key::K);
    harness
        .state_mut()
        .open_settings(crate::app::Section::General);
    harness.run_steps(4);
    assert!(harness.get_by_label("Close").accesskit_node().is_focused());
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

#[test]
fn what_is_said_out_of_sight_of_the_game_is_shown_over_the_corner() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.run_steps(2);
    // In sight, the lines of the game say it, and nothing floats.
    harness.state_mut().tab_mut().note("Said in sight.");
    harness.run_steps(2);
    assert_eq!(
        harness.query_all_by_label("Said in sight.").count(),
        1,
        "said twice"
    );
    // In the editor, a notice says it, moving nothing, and goes.
    harness.state_mut().tab_mut().open_automation(0);
    harness.run_steps(2);
    harness.state_mut().tab_mut().note("Said in the editor.");
    harness.run_steps(2);
    harness.get_by_label("Said in the editor.");
    std::thread::sleep(std::time::Duration::from_millis(6100));
    harness.run_steps(2);
    assert!(harness.query_by_label("Said in the editor.").is_none());
}
