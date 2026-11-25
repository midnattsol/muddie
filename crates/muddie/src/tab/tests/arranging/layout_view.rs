//! The view of the layout: arrangements, dragging in its picture, and naming panels and their gauges.

use super::*;

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
    // Its name is a heading, not a stop of the keyboard: Rename…, in its
    // menu, is the way to it then.
    let heading = harness.get_by_role_and_label(egui::accesskit::Role::Heading, "Mundo");
    let focus = egui::accesskit::Action::Focus;
    assert!(!heading.accesskit_node().data().supports_action(focus));
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
