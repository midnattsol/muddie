//! What the map draws and how it is said: tidying, appearance, levels, areas and doors.

use super::*;

/// A map with all in one area, as Reinos de Leyenda's is: tidying it would
/// draw every room again, so it is done only when told "all".
#[test]
fn a_very_large_area_is_tidied_only_once_the_user_says_so() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let rooms: Vec<Room> = (1..=2001)
        .map(|id: RoomId| Room {
            id,
            name: "Camino".into(),
            area: "Mapa Completo".into(),
            pos: Pos {
                // Tangled: ten rooms on each cell.
                x: id as i32 * 3 % 30,
                y: 0,
                z: 0,
            },
            exits: [(
                "e".to_owned(),
                Exit {
                    to: id % 2001 + 1,
                    ..Exit::default()
                },
            )]
            .into(),
            cost: 0,
            avoid: false,
            colour: None,
            note: String::new(),
            renamed: false,
            added: false,
            shape: Default::default(),
            symbol: String::new(),
            styled: false,
        })
        .collect();
    app.map.borrow_mut().replace_all(rooms, None).unwrap();
    assert!(app.map.borrow_mut().set_current(1));
    let x_of_2 = |app: &Tab| app.map.borrow().room(2).unwrap().pos.x;
    // Chosen, it is asked first, in a dialog: nothing is drawn yet.
    assert!(app.do_client_command("/map tidy mapa completo"));
    assert_eq!(app.tidy_asked.as_deref(), Some("Mapa Completo"));
    assert_eq!(x_of_2(app), 6);
    // Told to, it is drawn again.
    app.tidy("Mapa Completo");
    assert_eq!(x_of_2(app), 4);
    // Tidy, it is not drawn again: the exit that closes the ring points
    // the wrong way however it is drawn.
    app.tidy("Mapa Completo");
    assert_eq!(last_said(app), "The map is as tidy as it gets.");
}

/// The appearance of the map, from its menu, and of a room, from the
/// room's: for it, or for every room called as it is; a room of no colour
/// or shape of its own takes the map's.
#[test]
fn the_appearance_of_the_map_and_of_rooms_is_chosen() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.started = true;
    let (_events, _sent) = fake_connection(app);
    desert(app);
    let called = app.map.borrow().called("Dunas");
    assert!(called.len() > 1);
    let shape =
        |harness: &Harness<'_, App>, id| harness.state().tab().map.borrow().room(id).unwrap().shape;
    // For the room alone.
    harness.state_mut().tab_mut().open_look(called[0]);
    harness.run_steps(2);
    harness.get_by_label("Circle").click();
    harness.run_steps(1);
    harness.get_by_label("Apply").click();
    harness.run_steps(2);
    assert_eq!(
        shape(&harness, called[0]),
        Some(muddie_mapper::Shape::Circle)
    );
    assert_eq!(shape(&harness, called[1]), None);
    // For all called so, with a symbol.
    harness.state_mut().tab_mut().open_look(called[1]);
    harness.run_steps(2);
    harness.get_by_label("Diamond").click();
    harness.get_by_label("A character or two").click();
    harness.run_steps(1);
    harness.get_by_label("A character or two").type_text("~");
    harness
        .get_by_label(&format!("All {} rooms called Dunas", called.len()))
        .click();
    harness.run_steps(1);
    harness.get_by_label("Apply").click();
    harness.run_steps(2);
    let map = harness.state().tab().map.borrow();
    for id in &called {
        let room = map.room(*id).unwrap();
        assert_eq!(
            (room.shape, room.symbol.as_str(), room.styled),
            (Some(muddie_mapper::Shape::Diamond), "~", true)
        );
    }
    drop(map);
    assert!(harness.state().tab().look.is_none());
    // The map's own, for rooms of none of their own; one given back the
    // map's shape draws in it.
    harness.state_mut().tab_mut().open_map_appearance();
    harness.run_steps(2);
    harness.get_by_label("Hexagon").click();
    harness.get_by_label("blue").click();
    harness.run_steps(1);
    harness.get_by_label("Apply").click();
    harness.run_steps(2);
    let appearance = harness.state().tab().map.borrow().appearance();
    assert_eq!(appearance.shape, muddie_mapper::Shape::Hexagon);
    assert!(appearance.colour.is_some());
    harness.state_mut().tab_mut().open_look(called[0]);
    harness.run_steps(2);
    harness.get_by_label("The map's").click();
    harness.run_steps(1);
    harness.get_by_label("Apply").click();
    harness.run_steps(2);
    assert_eq!(shape(&harness, called[0]), None);
}

/// The levels are over the map's top right corner, a floor above another,
/// only where the area has more than one: the number said as the level it
/// is. Looking at another level, or another area, chosen in the palette,
/// the way back to where the player is is over the top left corner.
#[test]
fn levels_and_other_areas_are_looked_at_over_the_map() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 860.0));
    let app = harness.state_mut().tab_mut();
    desert(app);
    assert!(app.map.borrow_mut().set_current(1));
    harness.run_steps(3);
    assert!(harness.query_by_label("Level 0").is_none());
    let room = |id, name: &str, area: &str, z| muddie_mapper::RoomInfo {
        id,
        name: name.into(),
        area: area.into(),
        exits: Default::default(),
        pos: Some(Pos { x: 0, y: 0, z }),
        colour: None,
    };
    let map = &harness.state().tab().map;
    map.borrow_mut()
        .set_room(room(60, "Pozo", "Desierto", -1))
        .unwrap();
    map.borrow_mut()
        .set_room(room(70, "Palmeras", "Oasis", 0))
        .unwrap();
    harness.run_steps(3);
    harness.get_by_label("Level 0");
    assert!(harness.query_by_label("Back to my position").is_none());
    // Down to the well, and back.
    harness.get_by_label("Down a level").click();
    harness.run_steps(3);
    harness.get_by_label("Level -1");
    harness.get_by_label("Back to my position").click();
    harness.run_steps(3);
    harness.get_by_label("Level 0");
    assert!(harness.query_by_label("Back to my position").is_none());
    // Another area, chosen as it is typed, is shown without going.
    let app = harness.state_mut().tab_mut();
    app.toggle_palette();
    app.palette.query = "show an area".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "The area to show");
    harness.state_mut().tab_mut().palette.query = "oa".into();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert!(!harness.state().tab().palette.open);
    harness.get_by_label("Back to my position");
    assert_eq!(harness.state().tab().map.borrow().current().unwrap().id, 1);
}

/// What the map draws is said too: doors on exits and a room's symbol;
/// an area shown is said, and explored from its room nearest; and the map
/// no longer drawn is no longer explored, its keys given back.
#[test]
fn doors_areas_and_leaving_the_map_are_said() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 860.0));
    let app = harness.state_mut().tab_mut();
    app.started = true;
    app.voice.force_voice();
    desert(app);
    {
        let mut map = app.map.borrow_mut();
        map.edit_exit(3, "n", |exit| exit.door = Some(muddie_mapper::Door::Locked))
            .unwrap();
        assert!(map.set_current(3));
    }
    assert!(app.run_command("/map info 3"));
    let info = last_said(app);
    assert!(info.contains("Doors: north, locked"), "{info}");
    // An area shown, asked for: said, and explored from its nearest room.
    app.voice.said.clear();
    app.run_command("/map show-area desierto");
    harness.run_steps(3);
    let said = harness.state().tab().voice.said.join(" | ");
    assert!(said.starts_with("Desierto: 4 rooms, 1 level."), "{said}");
    assert!(said.contains("Exploring the map from Dunas"), "{said}");
    assert!(harness.state().tab().map_view.exploring());
    // The map hidden while explored: left, and said.
    let app = harness.state_mut().tab_mut();
    app.voice.said.clear();
    app.set_map_shown(false);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(!app.map_view.exploring());
    assert_eq!(app.voice.said, ["Left the map."]);
}
