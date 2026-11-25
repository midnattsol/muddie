//! Handling panels where they are shown: hiding, dragging, menus, putting a side away and the keyboard between places.

use super::*;

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
    assert!(gauge_button(&harness, "MV").is_none());
    // One made in the editor opens its details over the game.
    gauge_button(&harness, "Salud")
        .expect("a gauge to edit")
        .click();
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

/// F6 goes from the command line to the output and to each panel, and
/// Shift+F6 back: not through all there is, as Tab does.
#[test]
fn f6_goes_from_place_to_place() {
    let mut harness = window(None);
    let (events, _sent) = fake_connection(harness.state_mut().tab_mut());
    events
        .send(Event::Line(panels::styled_lines("The Temple.").remove(0)))
        .unwrap();
    harness.run_steps(3);
    assert!(harness.get_by_label("Command").is_focused());
    harness.key_press(Key::F6);
    harness.run_steps(3);
    let text = egui::accesskit::Role::MultilineTextInput;
    assert!(harness.get_by_role_and_label(text, "Output").is_focused());
    harness.key_press(Key::F6);
    harness.run_steps(3);
    let first = harness.state().tab().layout.panels[0].shown_name();
    let heading = egui::accesskit::Role::Heading;
    assert!(harness.get_by_role_and_label(heading, &first).is_focused());
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::F6);
    harness.run_steps(3);
    assert!(harness.get_by_role_and_label(text, "Output").is_focused());
}

/// What takes away the control the keyboard was on says what it did, and
/// gives the keyboard to what brings it back.
#[test]
fn collapsing_and_hiding_are_said_and_keep_the_keyboard() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.started = true;
    app.voice.force_voice();
    harness.run_steps(3);
    harness.get_by_label("Collapse the right side").click();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert_eq!(
        app.voice.said.last().map(String::as_str),
        Some("Collapsed. The strip at its edge expands it again.")
    );
    assert!(harness.get_by_label("Expand the right side").is_focused());
    let app = harness.state_mut().tab_mut();
    let id = app.layout.panels[0].id;
    let name = app.layout.panels[0].shown_name();
    app.show_panel(id, false);
    assert_eq!(
        app.voice.said.last(),
        Some(&format!(
            "{name} hidden. Show {name}, in the palette, brings it back."
        ))
    );
}
