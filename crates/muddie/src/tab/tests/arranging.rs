//! The panels: what they show, where they are, and the channels.

use super::*;

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
    harness.get_by_label_contains("Efectos: Efecto número 1, Efecto número 2,");
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

#[test]
fn a_panel_is_hidden_from_its_own_menu_with_no_mode_to_enter() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.started = true;
    harness.run_steps(3);
    // Every panel has its handle and its menu, all the time.
    harness.get_by_label("Move Gauges");
    harness.get_by_label("Gauges options").click();
    harness.run_steps(2);
    // From there it is hidden, to be shown again from the palette.
    harness.get_by_label("Hide Gauges").click();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert!(
        !app.layout
            .panels
            .iter()
            .find(|panel| panel.name == "Gauges")
            .unwrap()
            .visible
    );
}

#[test]
fn a_panel_is_dragged_by_its_handle_to_another_place() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(4);
    // The chat, above, taken by its dots and let go on the upper part of
    // the map, at the right.
    let from = harness.get_by_label("Move Gauges").rect().center();
    let map = harness.get_by_label("Move Map").rect().center() + egui::vec2(60.0, 60.0);
    harness.hover_at(from);
    harness.run_steps(1);
    harness.drag_at(from);
    harness.run_steps(1);
    for step in 1..=10 {
        let at = from + (map - from) * (step as f32 / 10.0);
        harness.hover_at(at);
        harness.run_steps(1);
    }
    harness.drop_at(map);
    harness.run_steps(4);
    let app = harness.state().tab();
    let side = |name: &str| {
        (app.layout.panels.iter())
            .find(|panel| panel.name == name)
            .map(|panel| panel.side)
    };
    assert_eq!(side("Gauges"), Some(layout::Side::Right));
    assert!(app.panels.dragged.is_none());
}

#[test]
fn a_panels_menu_is_reached_from_the_keyboard() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    harness.get_by_label("Gauges options").focus();
    harness.run_steps(2);
    assert!(harness.get_by_label("Gauges options").is_focused());
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    harness.get_by_label("Hide Gauges");
}

#[test]
fn a_gauge_is_changed_where_it_is_shown() {
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
    // One the MUD made by itself is shown, and not to be changed here.
    app.gauges.set("MV", Some((10.0, 20.0)));
    harness.run_steps(3);
    assert!(harness.query_by_label("Edit MV").is_none());
    // One made in the editor opens its details over the game.
    harness.get_by_label("Edit Salud").click();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let salud = app.rules.gauges.len() - 1;
    assert_eq!(
        (app.editor.gauge_here, app.view()),
        (Some(salud), View::Game)
    );
    app.editor.draft.gauges[salud].colour = "red".into();
    harness.run_steps(2);
    // Closed, what was changed is saved, as in the editor.
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.editor.gauge_here.is_none());
    assert_eq!(app.rules.gauges.last().unwrap().colour, "red");
}

#[test]
fn a_side_is_put_away_with_its_panels_and_brought_back_as_it_was() {
    use layout::Side;
    let mut harness = window(None);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert_eq!(
        app.layout.sides(),
        [(Side::Bottom, true), (Side::Right, true)]
    );
    let map_on_screen = |app: &Tab| {
        let map = app
            .layout
            .panels
            .iter()
            .find(|panel| panel.kind == layout::Kind::Map);
        let id = map.unwrap().id;
        app.panels.rects.iter().any(|(shown, _)| *shown == id)
    };
    assert!(map_on_screen(app));
    // The button in the header of its first panel, as it is told to
    // screen readers.
    harness.get_by_label("Collapse the right side").click();
    harness.run_steps(2);
    let app = harness.state().tab();
    assert_eq!(
        app.layout.sides(),
        [(Side::Bottom, true), (Side::Right, false)]
    );
    assert!(!map_on_screen(app) && !app.map_shown());
    // Each panel is still as it was: only the side is away.
    assert!(app.layout.panels.iter().all(|panel| panel.visible));
    assert!(harness.query_by_label("Collapse the right side").is_none());
    // The arrow of its strip brings it back, and so does the name of
    // any of its panels.
    harness.get_by_label("Expand the right side").click();
    harness.run_steps(2);
    assert!(map_on_screen(harness.state().tab()));
    harness.get_by_label("Collapse the right side").click();
    harness.run_steps(2);
    assert!(!map_on_screen(harness.state().tab()));
    harness.get_by_label("Show Map").click();
    harness.run_steps(2);
    assert!(map_on_screen(harness.state().tab()));
    harness.get_by_label("Collapse the bottom side").click();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert_eq!(
        app.layout.sides(),
        [(Side::Bottom, false), (Side::Right, true)]
    );
    // Asked for, a panel on a side put away brings the side back.
    app.toggle_side(Side::Right);
    app.set_map_shown(true);
    assert!(app.layout.collapsed == [Side::Bottom]);
    // It is kept with the rest of the panels.
    let file = std::env::temp_dir().join(format!("muddie-sides-{}.toml", std::process::id()));
    app.layout.save(&file).unwrap();
    assert_eq!(
        layout::Layout::load(&file).unwrap().collapsed,
        [Side::Bottom]
    );
    std::fs::remove_file(&file).unwrap();
}

/// The names of the panels by side, as the view of the layout lists them.
fn by_side(app: &Tab) -> Vec<(layout::Side, String)> {
    let listed = app.listed_for_tests();
    (listed.iter())
        .filter_map(|id| app.layout.panel(*id))
        .map(|panel| (panel.side, panel.name.clone()))
        .collect()
}

#[test]
fn the_layout_has_a_view_of_its_own_beside_the_game_and_the_editor() {
    let mut harness = connected_demo_window();
    harness.set_size(egui::vec2(1400.0, 860.0));
    harness.run_steps(2);
    harness.get_by_label("Layout").click();
    harness.run_steps(3);
    assert_eq!(harness.state().tab().view(), View::Layout);
    // The panels by side, the first chosen; a picture of them; the
    // arrangements.
    harness.get_by_label("Picture of the window's panels");
    harness.get_by_role_and_label(egui::accesskit::Role::Button, "Classic");
    // The editor takes its place, and Escape goes back to the game.
    harness.state_mut().tab_mut().set_view(View::Editor);
    harness.run_steps(2);
    assert_eq!(harness.state().tab().view(), View::Editor);
    harness.state_mut().tab_mut().set_view(View::Layout);
    harness.run_steps(2);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert_eq!(harness.state().tab().view(), View::Game);
    // A panel's menu opens it with that panel chosen.
    let map = harness
        .state()
        .tab()
        .layout
        .panels
        .iter()
        .find(|panel| panel.kind == layout::Kind::Map)
        .unwrap()
        .id;
    harness.state_mut().tab_mut().open_layout_view(Some(map));
    harness.run_steps(2);
    assert_eq!(harness.state().tab().layout_view.chosen, Some(map));
}

#[test]
fn panels_are_laid_out_with_the_keyboard() {
    use layout::Side::{Bottom, Right};
    let mut harness = connected_demo_window();
    harness.set_size(egui::vec2(1400.0, 860.0));
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    let before = by_side(app);
    assert_eq!(
        before,
        [
            (Right, "Map".into()),
            (Right, "Minimapa".into()),
            (Bottom, "Gauges".into())
        ]
    );
    // Opened while the keyboard is in use, the keyboard goes to the
    // panel chosen, the first: the map. Down goes to the next one.
    harness.key_press(Key::Tab);
    harness.run_steps(1);
    harness.state_mut().tab_mut().open_layout_view(None);
    harness.run_steps(2);
    // Each says the side it is on, which the arrows go past.
    let map = harness.get_by_role_and_label(egui::accesskit::Role::ListItem, "Map");
    assert_eq!(
        map.accesskit_node().description().as_deref(),
        Some("At the right")
    );
    let minimap = harness.get_by_role_and_label(egui::accesskit::Role::ListItem, "Minimapa");
    assert_eq!(
        minimap.accesskit_node().description().as_deref(),
        Some("Text, At the right")
    );
    let focused = || harness.state().tab().layout_view.chosen;
    let map = focused();
    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    assert_ne!(harness.state().tab().layout_view.chosen, map);
    // Alt and Up move it above the map, and it is said where it is.
    harness.key_press_modifiers(crate::shortcuts::WITH_ARROWS, Key::ArrowUp);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert_eq!(by_side(app)[0], (Right, "Minimapa".into()));
    assert_eq!(
        app.voice.said.last().map(String::as_str),
        Some("Moved up, 1 of 2.")
    );
    // Alt and Right to the next side: below the game.
    harness.key_press_modifiers(crate::shortcuts::WITH_ARROWS, Key::ArrowRight);
    harness.run_steps(2);
    harness.key_press_modifiers(crate::shortcuts::WITH_ARROWS, Key::ArrowRight);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert!(
        by_side(app).contains(&(Bottom, "Minimapa".into())),
        "{:?}",
        by_side(app)
    );
    assert_eq!(
        app.voice.said.last().map(String::as_str),
        Some("Moved: At the bottom.")
    );
}

#[test]
fn arrangements_put_the_panels_where_they_say_and_keep_them() {
    use layout::Side::{Bottom, Left, Right};
    let mut harness = connected_demo_window();
    harness.set_size(egui::vec2(1400.0, 1400.0));
    harness.state_mut().tab_mut().open_layout_view(None);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "Wide screen")
        .click();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert_eq!(
        by_side(app),
        [
            (Left, "Map".into()),
            (Right, "Minimapa".into()),
            (Bottom, "Gauges".into())
        ]
    );
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "The game alone")
        .click();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert!(app.layout.panels.iter().all(|panel| !panel.visible));
    assert_eq!(app.layout.panels.len(), 3);
    harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "Classic")
        .click();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    assert_eq!(
        by_side(app),
        [
            (Right, "Map".into()),
            (Right, "Minimapa".into()),
            (Bottom, "Gauges".into())
        ]
    );
    assert!(app.layout.panels.iter().all(|panel| panel.visible));
    // Two panels share a place as tabs, and part again.
    let ids: Vec<u32> = app.layout.panels.iter().map(|panel| panel.id).collect();
    app.share_place_for_tests(ids[1], ids[0]);
    let stacks: Vec<u32> = app.layout.panels.iter().map(|panel| panel.stack).collect();
    assert!(stacks[0] != 0 && stacks[0] == stacks[1], "{stacks:?}");
    app.share_place_for_tests(ids[1], 0);
    assert_eq!(
        app.layout
            .panels
            .iter()
            .find(|panel| panel.id == ids[1])
            .unwrap()
            .stack,
        0
    );
}

#[test]
fn panels_are_dragged_in_the_picture_of_the_window_seeing_how_it_would_be() {
    use layout::Side::{Left, Right};
    let mut harness = connected_demo_window();
    harness.set_size(egui::vec2(1400.0, 1000.0));
    harness.state_mut().tab_mut().open_layout_view(None);
    harness.run_steps(3);
    let picture = harness
        .get_by_label("Picture of the window's panels")
        .rect();
    let (map_at, left_edge) = (
        egui::pos2(picture.right() - 20.0, picture.top() + 20.0),
        egui::pos2(picture.left() + 6.0, picture.center().y),
    );
    let pointer = |harness: &mut Harness<'_, App>, at: egui::Pos2, pressed: Option<bool>| {
        harness.event(egui::Event::PointerMoved(at));
        if let Some(pressed) = pressed {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        harness.run_steps(2);
    };
    pointer(&mut harness, map_at, Some(true));
    pointer(&mut harness, map_at + egui::vec2(-30.0, 10.0), None);
    pointer(&mut harness, left_edge, None);
    // Meanwhile it is only seen: the panels are as they were.
    let side_of_map = |app: &Tab| {
        (app.layout.panels.iter())
            .find(|panel| panel.kind == layout::Kind::Map)
            .map(|panel| panel.side)
    };
    assert_eq!(side_of_map(harness.state().tab()), Some(Right));
    assert!(harness.state().tab().layout_view.dragged.is_some());
    pointer(&mut harness, left_edge, Some(false));
    let app = harness.state().tab();
    assert_eq!(side_of_map(app), Some(Left));
    assert!(app.layout_view.dragged.is_none());
    // Taken again and Escape: back where it was, and the view stays.
    let picture = harness
        .get_by_label("Picture of the window's panels")
        .rect();
    let map_at = egui::pos2(picture.left() + 20.0, picture.top() + 20.0);
    let right_edge = egui::pos2(picture.right() - 6.0, picture.center().y);
    pointer(&mut harness, map_at, Some(true));
    pointer(&mut harness, map_at + egui::vec2(30.0, 10.0), None);
    pointer(&mut harness, right_edge, None);
    assert!(harness.state().tab().layout_view.dragged.is_some());
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    pointer(&mut harness, right_edge, Some(false));
    let app = harness.state().tab();
    assert_eq!(side_of_map(app), Some(Left));
    assert!(app.layout_view.dragged.is_none() && app.layout_view.open);
}

#[test]
fn before_a_session_is_anyones_its_layout_and_rules_say_whose_they_would_be() {
    // On the welcome screen, with no profile chosen.
    let mut harness = window(None);
    harness.state_mut().tab_mut().started = false;
    harness.state_mut().tab_mut().set_view(View::Layout);
    harness.run_steps(3);
    // Not the panels of a session that may never have them: why, and the
    // ways on.
    assert!(
        harness
            .query_by_label("Picture of the window's panels")
            .is_none()
    );
    harness.get_by_label("No layout yet");
    harness.get_by_label("Choose a profile").click();
    harness.run_steps(2);
    assert!(harness.state().profiles_shown());
    harness.state_mut().close_profiles_for_tests();
    harness.run_steps(2);
    // Asked for, those of sessions without a profile, saying so.
    harness
        .get_by_label("Edit the layout of sessions without a profile")
        .click();
    harness.run_steps(3);
    harness.get_by_label("Picture of the window's panels");
    harness.get_by_label_contains("of sessions without a profile");
    // The editor goes the same way, once asked for.
    harness.state_mut().tab_mut().set_view(View::Editor);
    harness.run_steps(3);
    assert!(harness.query_by_label("No rules yet").is_none());
    // Escape on it goes back to the game.
    let mut fresh = window(None);
    fresh.state_mut().tab_mut().started = false;
    fresh.state_mut().tab_mut().set_view(View::Editor);
    fresh.run_steps(3);
    fresh.get_by_label("No rules yet");
    fresh.key_press(Key::Escape);
    fresh.run_steps(3);
    assert_eq!(fresh.state().tab().view(), View::Game);
}

/// A panel is named again in its header, from its menu or the palette:
/// Enter keeps the name, Escape the old one.
#[test]
fn a_panel_is_renamed_in_its_header() {
    let mut harness = window(None);
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    let map = (app.layout.panels.iter()).find(|panel| panel.kind == layout::Kind::Map);
    let id = map.unwrap().id;
    let name = |harness: &Harness<'_, App>| {
        let layout = &harness.state().tab().layout;
        layout
            .panels
            .iter()
            .find(|panel| panel.id == id)
            .unwrap()
            .name
            .clone()
    };
    app.rename_panel(id);
    harness.run_steps(3);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness.get_by_label("Name of the panel").type_text("Mundo");
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert_eq!(name(&harness), "Mundo");
    assert!(harness.state().tab().panels.renaming.is_none());
    // Escape leaves it as it was.
    let app = harness.state_mut().tab_mut();
    app.rename_panel(id);
    harness.run_steps(3);
    harness.get_by_label("Name of the panel").type_text("Otro");
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert_eq!(name(&harness), "Mundo");
    // Enter on its name, reached with the keyboard, names it again too.
    harness
        .get_by_role_and_label(egui::accesskit::Role::Label, "Mundo")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    assert!(harness.state().tab().panels.renaming.is_some());
}

/// A panel is named again as typed, in its header or in Layout alike: a
/// name of nothing is none, and one another panel has is said and not
/// given, as scripts tell panels apart by their names.
#[test]
fn a_panel_is_named_again_one_way() {
    let mut harness = window(None);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    let ids: Vec<u32> = app.layout.panels.iter().map(|panel| panel.id).collect();
    let (first, second) = (ids[0], ids[1]);
    let name_of = |app: &Tab, id: u32| app.layout.panel(id).unwrap().shown_name();
    let other = name_of(app, second);
    let before = name_of(app, first);
    app.rename_to(first, "   ");
    assert_eq!(name_of(app, first), before);
    app.rename_to(first, &other.to_uppercase());
    assert_eq!(name_of(app, first), before);
    assert_eq!(
        notes(app).last().unwrap(),
        &format!("There is a panel called {} already.", other.to_uppercase())
    );
    app.rename_to(first, " Mapa grande ");
    assert_eq!(name_of(app, first), "Mapa grande");
    // In the layout view, Escape in its field keeps the name it had, and
    // the view stays; Enter gives the one typed.
    app.open_layout_view(Some(first));
    harness.run_steps(3);
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Name")
        .type_text("Otro");
    harness.run_steps(1);
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert_eq!(name_of(app, first), "Mapa grande");
    assert_eq!(app.view(), View::Layout);
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Name")
        .type_text("Otro");
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert_eq!(name_of(harness.state().tab(), first), "Otro");
}

/// A gauge is put in a panel by its name: one renamed takes its gauges
/// with it, and rules written when they were put in by the panel's id are
/// named as they are read.
#[test]
fn gauges_are_in_their_panels_by_name() {
    let mut harness = window(None);
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    let first = app
        .layout
        .panels
        .iter()
        .find(|panel| panel.kind == layout::Kind::Gauges);
    let (id, name) = first.map(|panel| (panel.id, panel.name.clone())).unwrap();
    app.change_rules(|rules| {
        rules.gauges.push(crate::rules::Gauge {
            name: "Vida".into(),
            panel: name.clone(),
            ..crate::rules::Gauge::starter()
        });
    });
    app.rename_to(id, "Puntos");
    assert_eq!(app.rules.gauges.last().unwrap().panel, "Puntos");
    let old: crate::rules::Rules =
        toml::from_str(&format!("[[gauge]]\nname = \"Vida\"\npanel = {id}\n")).unwrap();
    app.rules = old;
    app.name_gauge_panels();
    assert_eq!(app.rules.gauges[0].panel, "Puntos");
}
