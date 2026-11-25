//! Images of the map and of its menu.

use super::*;

/// The map: as walked, every area on a level, the menu of a room, a room
/// avoided.
pub(super) fn shows_map() {
    // The first is the image named itself.
    let path = std::env::var_os("MUDDIE_SCREENSHOT").map(PathBuf::from);
    let mut harness = connected_drawn_demo_window();
    assert!(harness.query_by_label("Connection").is_none());
    let image = harness.render().expect("rendering failed");
    assert_eq!((image.width(), image.height()), (1100, 700));
    if let Some(path) = &path {
        image.save(path).expect("could not write the screenshot");
    }
    // A second view: every area on the level.
    harness.state_mut().tab_mut().map_view.all_areas = true;
    harness.run_steps(3);
    let image = harness.render().expect("rendering failed");
    if let Some(path) = &path {
        (image.save(path.with_extension("all-areas.png"))).expect("could not write the screenshot");
    }
    // The panels at the right put away, and back.
    harness.get_by_label("Collapse the right side").click();
    harness.run_steps(3);
    let image = harness.render().expect("rendering failed");
    if let Some(path) = &path {
        (image.save(path.with_extension("side-away.png"))).expect("could not write the screenshot");
    }
    harness.get_by_label("Collapse the bottom side").click();
    harness.run_steps(3);
    let image = harness.render().expect("rendering failed");
    if let Some(path) = &path {
        (image.save(path.with_extension("sides-away.png")))
            .expect("could not write the screenshot");
    }
    harness.get_by_label("Expand the bottom side").click();
    harness.run_steps(3);
    harness.get_by_label("Show Map").click();
    harness.run_steps(3);
    // A third: the menu of the room under the pointer. The map is
    // centred on the player's room, so that is the room it opens on.
    map_widget(&harness).unwrap().click_secondary();
    harness.run_steps(3);
    assert!(harness.query_by_label("Keep routes off").is_some());
    let image = harness.render().expect("rendering failed");
    if let Some(path) = &path {
        let path = path.with_extension("map-menu.png");
        image.save(path).expect("could not write the screenshot");
    }
    // Choosing to avoid the room marks it on the map.
    harness.get_by_label("Keep routes off").click();
    harness.run_steps(3);
    assert!(harness.state().tab().map.borrow().room(2).unwrap().avoid);
    // Close up, with the rooms' names, while a walk is under way, the
    // map alone at its side.
    let app = harness.state_mut().tab_mut();
    app.layout.panels.retain(|panel| panel.name != "Minimapa");
    // Beside the player's room, one the map added of itself, dashed round.
    {
        let mut map = app.map.borrow_mut();
        let here = map.current().map(|room| room.id).unwrap();
        let exits = ["e".to_owned()];
        let added = ["w", "s", "n", "e"].into_iter().find_map(|way| {
            let seen = muddie_mapper::Arrival {
                name: "Claro",
                exits: &exits,
                came: way,
            };
            map.add(seen).ok()
        });
        assert!(added.is_some());
        map.set_current(here);
        // Rooms drawn in shapes of their own, one with a symbol, and doors.
        let mut others: Vec<RoomId> = (map.rooms())
            .map(|room| room.id)
            .filter(|id| *id != here && *id < muddie_mapper::ADDED_FROM)
            .collect();
        others.sort_unstable();
        let shapes = [
            (muddie_mapper::Shape::Circle, "$"),
            (muddie_mapper::Shape::Diamond, ""),
            (muddie_mapper::Shape::Hexagon, "+"),
        ];
        for (id, (shape, symbol)) in others.iter().zip(shapes) {
            map.set_shape(*id, Some(shape)).unwrap();
            map.set_symbol(*id, symbol).unwrap();
        }
        map.set_colour(others[0], Some([224, 192, 64])).unwrap();
        let ways: Vec<String> = map.room(here).unwrap().exits.keys().cloned().collect();
        for (way, door) in ways
            .iter()
            .zip([muddie_mapper::Door::Closed, muddie_mapper::Door::Locked])
        {
            map.edit_exit(here, way, |exit| exit.door = Some(door))
                .unwrap();
        }
    }
    let route = app.map.borrow().route(2, 6).unwrap();
    app.walk = Some((
        muddie_mapper::Walk::new(2, route),
        std::time::Instant::now(),
    ));
    app.map_view.zoom_to(80.0);
    harness.run_steps(3);
    save(&mut harness, "map-walk");
    harness.state_mut().tab_mut().walk = None;
    // The appearance of a room, from its menu, and of the map, from its.
    let here = harness.state().tab().map.borrow().current().unwrap().id;
    harness.state_mut().tab_mut().open_look(here);
    harness.run_steps(3);
    save(&mut harness, "room-appearance");
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    harness.state_mut().tab_mut().open_map_appearance();
    harness.run_steps(3);
    save(&mut harness, "map-appearance");
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    // A room chosen with the keys, east of the player's, told under it.
    let view = map_widget(&harness).unwrap().rect().center();
    harness.hover_at(view);
    harness.run_steps(1);
    harness.drag_at(view);
    harness.run_steps(1);
    harness.drop_at(view);
    harness.run_steps(2);
    harness.key_press(egui::Key::ArrowRight);
    harness.run_steps(3);
    save(&mut harness, "map-selected");
    // Searching the map, over it, with what was found said.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    harness.run_steps(3);
    harness.event(egui::Event::Text("pa".into()));
    harness.run_steps(2);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(3);
    save(&mut harness, "map-search");
    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);
    // An area of more than one level shows which, and the way between.
    let belfry = muddie_mapper::RoomInfo {
        id: 5,
        name: "Campanario".into(),
        area: "ciudad".into(),
        exits: [("d".to_owned(), 3)].into(),
        pos: None,
        colour: None,
    };
    (harness.state().tab().map.borrow_mut().set_room(belfry)).unwrap();
    harness.run_steps(3);
    save(&mut harness, "map-levels");
    // The map placed the rooms as walked: the patio is north-east of
    // the alley, one cell east of the square.
    let map = harness.state().tab().map.borrow();
    assert_eq!(map.room(7).unwrap().pos, Pos { x: 2, y: 2, z: 0 });
}

/// The menu of the map's panel, with what a plugin offers under its name,
/// and its name being changed in the header.
pub(super) fn shows_map_menu() {
    let mut harness = connected_drawn_demo_window();
    harness.set_size(egui::vec2(1500.0, 900.0));
    let app = harness.state_mut().tab_mut();
    let plugin = &crate::plugins::fixture::reinos();
    for (name, code) in &plugin.scripts {
        app.scripts
            .exec_owned(&format!("{}/{name}", plugin.folder()), name, code);
    }
    app.run_actions();
    harness.run_steps(4);
    harness.get_by_label("Map options").click();
    harness.run_steps(4);
    save(&mut harness, "map-panel-menu");
    harness.key_press(egui::Key::Escape);
    let app = harness.state_mut().tab_mut();
    let map = app
        .layout
        .panels
        .iter()
        .find(|panel| panel.kind == layout::Kind::Map);
    let id = map.unwrap().id;
    app.rename_panel(id);
    harness.run_steps(4);
    save(&mut harness, "map-panel-rename");
}
