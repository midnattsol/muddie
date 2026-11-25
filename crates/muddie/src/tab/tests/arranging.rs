//! The panels: what they show, where they are, and the channels.

use super::*;

mod handling;
mod layout_view;

#[test]
fn map_drawn_by_the_server_is_asked_for_at_the_panels_size_and_shown() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let example = include_str!("../../../../../examples/scripts/drawn-map.luau");
    app.scripts.exec("drawn-map", example);
    app.run_actions();
    assert_eq!(
        app.supports.last().map(String::as_str),
        Some("Map.Render 1")
    );
    let (events, mut sent) = fake_connection(app);
    events.send(Event::Connected).unwrap();
    // The size goes out once the panel has kept it for a moment.
    harness.run_steps(2);
    assert!(sent.try_recv().is_err());
    // However busy the machine: it is waited for, not timed.
    let mut answer = None;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        harness.run_steps(2);
        answer = sent.try_recv().ok();
        if answer.is_some() {
            break;
        }
    }
    let Some(Command::Gmcp { package, data }) = answer else {
        panic!("the panel's size was not sent");
    };
    assert_eq!(package, "Map.Render.Size");
    let size: serde_json::Value = serde_json::from_str(&data).unwrap();
    let (width, height) = (
        size["width"].as_u64().unwrap(),
        size["height"].as_u64().unwrap(),
    );
    assert!(
        (20..80).contains(&width) && (5..=60).contains(&height),
        "{data}"
    );
    assert!(sent.try_recv().is_err(), "sent once");

    let drawing = r##"{"width": 5, "height": 3, "lines": ["  #  ", "#-@-#", "  #  "]}"##;
    let render = Event::Gmcp {
        package: "Map.Render".into(),
        data: drawing.into(),
    };
    events.send(render).unwrap();
    harness.run_steps(3);
    let app = harness.state().tab();
    // It is the map's own panel that shows it, not one more.
    assert_eq!(app.layout.panels.len(), 2);
    let map = app
        .layout
        .panels
        .iter()
        .find(|panel| panel.kind == layout::Kind::Map)
        .unwrap();
    let drawn = app
        .panels
        .text
        .iter()
        .find(|(id, _)| *id == map.id)
        .unwrap();
    assert_eq!(drawn.1.fixed.len(), 3);
    assert!(harness.query_by_label("#-@-#").is_some());
}

#[test]
fn panels_are_kept_fed_by_triggers_and_moved_about() {
    let dir = std::env::temp_dir().join(format!("muddie-app-panels-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    let app = harness.state_mut().tab_mut();
    // What a profile starts with.
    let kinds: Vec<layout::Kind> = app.layout.panels.iter().map(|panel| panel.kind).collect();
    assert_eq!(kinds, [layout::Kind::Map, layout::Kind::Gauges]);
    // A trigger shows its line in a panel, which is made if need be,
    // and with the line hidden it is only there.
    app.rules.triggers.push(trigger("tells you", |rule| {
        rule.when.panel = "Tells".into();
        rule.when.gag = true;
    }));
    for (name, code) in app.rules.chunks() {
        app.scripts.exec(&name, &code);
    }
    let (events, _sent) = fake_connection(app);
    let line = |text: &str| Event::Line(panels::styled_lines(text).remove(0));
    events
        .send(line("\x1b[33mAna tells you: hola\x1b[0m"))
        .unwrap();
    events.send(line("A plain line")).unwrap();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let tells = app
        .layout
        .panels
        .iter()
        .find(|panel| panel.name == "Tells")
        .expect("made")
        .clone();
    let content = &app
        .panels
        .text
        .iter()
        .find(|(id, _)| *id == tells.id)
        .unwrap()
        .1;
    assert_eq!(
        (content.log.len(), content.log.text(0)),
        (1, "Ana tells you: hola")
    );
    assert_eq!(output_lines(app), ["A plain line"]);
    // The channels go to the panel told to show them.
    (app.layout.panels.iter_mut())
        .filter(|panel| panel.id == tells.id)
        .for_each(|panel| panel.channels = true);
    app.channel_line(&panels::styled_lines("[ooc] Luis: buenas").remove(0));
    assert_eq!(
        app.panels
            .text
            .iter()
            .find(|(id, _)| *id == tells.id)
            .unwrap()
            .1
            .log
            .len(),
        2
    );
    // Moved to another side and hidden, it is all there the next time.
    let index = app
        .layout
        .panels
        .iter()
        .position(|panel| panel.id == tells.id)
        .unwrap();
    app.layout.panels[index].side = layout::Side::Left;
    app.set_map_shown(false);
    harness.run_steps(3);
    assert!(
        harness.query_by_label("Where am I").is_none(),
        "the map is hidden"
    );
    let kept = layout::Layout::load(&dir.join("layout.toml")).unwrap();
    assert_eq!(kept.panels[index].side, layout::Side::Left);
    assert!(
        !kept
            .panels
            .iter()
            .any(|panel| panel.kind == layout::Kind::Map && panel.visible)
    );
    // Two panels on one side share it, each with its part.
    let app = harness.state_mut().tab_mut();
    app.set_map_shown(true);
    let map = app
        .layout
        .panels
        .iter()
        .position(|panel| panel.kind == layout::Kind::Map)
        .unwrap();
    app.layout.panels[index].side = app.layout.panels[map].side;
    harness.run_steps(3);
    let app = harness.state().tab();
    let rect = |id| {
        app.panels
            .rects
            .iter()
            .find(|(known, _)| *known == id)
            .unwrap()
            .1
    };
    let (above, below) = (rect(app.layout.panels[map].id), rect(tells.id));
    assert!((above.width() - below.width()).abs() < 1.0 && above.bottom() < below.top());
    assert!(
        (above.height() - below.height()).abs() < 2.0,
        "equal weights, equal parts"
    );
    // Joined as tabs they share one place, and the one out of sight
    // says so when something comes for it.
    let app = harness.state_mut().tab_mut();
    app.layout.panels[index].stack = 1;
    app.layout.panels[map].stack = 1;
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let rect = |app: &Tab, id| {
        app.panels
            .rects
            .iter()
            .find(|(known, _)| *known == id)
            .unwrap()
            .1
    };
    assert_eq!(rect(app, app.layout.panels[map].id), rect(app, tells.id));
    let first = app.layout.panels[map.min(index)].id;
    let hidden = if first == tells.id {
        app.layout.panels[map].id
    } else {
        tells.id
    };
    assert!(app.panels.shown_tabs.contains(&(1, first)));
    if hidden == tells.id {
        app.add_to_panel(
            "Tells",
            &panels::styled_lines("Luis tells you: eh").remove(0),
        );
        assert_eq!(app.panels.unread_tabs, [tells.id]);
        harness.get_by_label("Tells").click();
        harness.run_steps(3);
        let app = harness.state().tab();
        assert!(
            app.panels.shown_tabs.contains(&(1, tells.id)) && app.panels.unread_tabs.is_empty()
        );
    }
    let app = harness.state_mut().tab_mut();
    app.layout.panels[index].stack = 0;
    app.layout.panels[map].stack = 0;
    harness.run_steps(3);
    // Its size is as it was drawn, not a little more each time.
    let before = harness.state().tab().layout.panels[index].size;
    harness.run_steps(5);
    assert_eq!(harness.state().tab().layout.panels[index].size, before);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Effects are labels, read together; as many as fit in the panel, and
/// the rest under one more, which lists them with what each is.
#[test]
fn effects_are_labels_and_those_that_do_not_fit_are_under_one_more() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(900.0, 700.0));
    let effect = |n: u32| muddie_script::Effect {
        name: format!("Efecto número {n}"),
        about: format!("Lo que hace el {n}"),
        ..muddie_script::Effect::default()
    };
    let look = muddie_script::GaugeLook {
        kind: muddie_script::GaugeKind::Effects,
        items: (1..=12).map(effect).collect(),
        ..muddie_script::GaugeLook::default()
    };
    let app = harness.state_mut().tab_mut();
    app.gauges.set_by(
        "Efectos",
        Some((0.0, 0.0)),
        Some(look),
        Some("t"),
        Default::default(),
        String::new(),
    );
    harness.run_steps(3);
    harness.get_by_label_contains(
        "Efectos: Efecto número 1 (Lo que hace el 1), Efecto número 2 (Lo que hace el 2),",
    );
    let more = harness.get_by_label_contains(" more: ");
    let said = more.accesskit_node().label().unwrap_or_default();
    assert!(said.ends_with("Efecto número 12"), "{said}");
    more.click();
    harness.run_steps(3);
    harness.get_by_label("Lo que hace el 12");
}

#[test]
fn gauges_are_shown_in_the_order_of_the_editor() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    let gauge = |name: &str| crate::rules::Gauge {
        name: name.into(),
        value: format!("Char.{name}"),
        ..crate::rules::Gauge::starter()
    };
    app.rules.gauges = vec![gauge("HP"), gauge("MP"), gauge("Gold")];
    // The MUD reports them in an order of its own.
    for name in ["Gold", "HP", "MP"] {
        app.gauges.set(name, Some((1.0, 2.0)));
    }
    harness.run_steps(3);
    assert_eq!(
        harness.state().tab().gauges.summary(),
        "HP 1 of 2, MP 1 of 2, Gold 1 of 2"
    );
    // Moved in the editor with Alt and an arrow, and saved, the panel
    // follows.
    let app = harness.state_mut().tab_mut();
    app.open_automation(automation::GAUGES);
    app.select_rule(automation::GAUGES, 2);
    harness.run_steps(3);
    harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowUp);
    harness.run_steps(3);
    harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowUp);
    harness.run_steps(3);
    let names = |app: &Tab| -> Vec<String> {
        (app.editor.draft.gauges.iter())
            .map(|gauge| gauge.name.clone())
            .collect()
    };
    assert_eq!(names(harness.state().tab()), ["Gold", "HP", "MP"]);
    // At the top it stays; down it goes, and no further than the end.
    harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowUp);
    harness.run_steps(3);
    assert_eq!(names(harness.state().tab()), ["Gold", "HP", "MP"]);
    harness.key_press_modifiers(egui::Modifiers::ALT, Key::ArrowDown);
    harness.run_steps(3);
    assert_eq!(names(harness.state().tab()), ["HP", "Gold", "MP"]);
    // Dropped before another, as dragging does.
    let app = harness.state_mut().tab_mut();
    app.move_rows(&[2], 0);
    assert_eq!(names(app), ["MP", "HP", "Gold"]);
    app.close_automation();
    harness.run_steps(3);
    assert_eq!(
        harness.state().tab().gauges.summary(),
        "MP 1 of 2, HP 1 of 2, Gold 1 of 2"
    );
}

#[test]
fn the_server_is_told_how_much_text_fits() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1100.0, 700.0));
    let (_events, mut sent) = fake_connection(harness.state_mut().tab_mut());
    harness.state_mut().tab_mut().sizes.window_untold = false;
    // Once the size has been the same for a moment, it is told.
    let mut told = |harness: &mut Harness<'_, App>| {
        for _ in 0..40 {
            harness.run_steps(2);
            while let Ok(command) = sent.try_recv() {
                if let Command::Resize(wide, tall) = command {
                    return (wide, tall);
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the size was not told");
    };
    let first = told(&mut harness);
    assert!(
        first.0 > 40 && first.1 > 10,
        "{first:?} is not the size of the text"
    );
    // A wider and shorter window fits more across and less down.
    harness.set_size(egui::vec2(1500.0, 500.0));
    let second = told(&mut harness);
    assert!(
        second.0 > first.0 && second.1 < first.1,
        "{first:?} then {second:?}"
    );
}
