//! Tests of the map: placing, editing, routes and keeping it.

use super::*;

pub(crate) fn info(id: RoomId, name: &str, exits: &[(&str, RoomId)]) -> RoomInfo {
    RoomInfo {
        id,
        name: name.into(),
        area: "town".into(),
        exits: (exits.iter())
            .map(|(direction, to)| ((*direction).to_owned(), *to))
            .collect(),
        pos: None,
    }
}

fn pos(x: i32, y: i32, z: i32) -> Pos {
    Pos { x, y, z }
}

/// 1 -n-> 2 -e-> 3 -u-> 4, plus a one-way shortcut 1 -portal-> 4.
fn town() -> Map {
    let mut map = Map::in_memory();
    for room in [
        info(1, "Gate", &[("n", 2), ("portal", 4)]),
        info(2, "Square", &[("s", 1), ("e", 3)]),
        info(3, "Tower", &[("w", 2), ("u", 4)]),
        info(4, "Tower Top", &[("d", 3)]),
    ] {
        map.visit(room).unwrap();
    }
    map
}

#[test]
fn rooms_are_placed_by_the_direction_walked() {
    let map = town();
    let at = |id| map.room(id).unwrap().pos;
    assert_eq!(
        [at(1), at(2), at(3), at(4)],
        [pos(0, 0, 0), pos(0, 1, 0), pos(1, 1, 0), pos(1, 1, 1)]
    );
    assert_eq!(map.current().unwrap().name, "Tower Top");
}

#[test]
fn room_reached_by_an_odd_exit_is_placed_beside_a_neighbour() {
    let mut map = Map::in_memory();
    map.visit(info(1, "Gate", &[("portal", 9)])).unwrap();
    map.visit(info(2, "Square", &[("s", 1)])).unwrap();
    assert_eq!(map.room(2).unwrap().pos, pos(0, 1, 0));
}

#[test]
fn taken_cell_stretches_the_link_instead_of_stacking() {
    let mut map = Map::in_memory();
    // Going n, e, s, w should close a square, but here it does not:
    // room 5 is a different room from the start.
    for room in [
        info(1, "A", &[("n", 2)]),
        info(2, "B", &[("e", 3)]),
        info(3, "C", &[("s", 4)]),
        info(4, "D", &[("w", 5)]),
        info(5, "E", &[]),
    ] {
        map.visit(room).unwrap();
    }
    assert_eq!(map.room(1).unwrap().pos, pos(0, 0, 0));
    assert_eq!(map.room(5).unwrap().pos, pos(-1, 0, 0));
}

#[test]
fn unanchored_room_starts_an_island_clear_of_the_area() {
    let mut map = town();
    map.visit(info(50, "Teleport Pad", &[])).unwrap();
    assert_eq!(map.room(50).unwrap().pos, pos(3, 0, 0));
    let mut elsewhere = info(60, "Other Land", &[]);
    elsewhere.area = "island".into();
    map.visit(elsewhere).unwrap();
    assert_eq!(map.room(60).unwrap().pos, pos(0, 0, 0));
}

#[test]
fn rooms_can_be_added_edited_and_removed_without_visiting() {
    let mut map = town();
    map.set_room(info(5, "Cellar", &[("u", 2)])).unwrap();
    assert_eq!(map.current().unwrap().id, 4);
    assert_eq!(map.room(5).unwrap().pos, pos(0, 1, -1));

    assert!(map.set_exit(2, "Down", 5).unwrap());
    assert!(!map.set_exit(99, "n", 1).unwrap());
    let directions: Vec<String> = (map.route(4, 5).unwrap().into_iter())
        .map(|step| step.command)
        .collect();
    assert_eq!(directions, ["d", "w", "d"]);

    assert!(map.remove_exit(2, "d").unwrap());
    assert_eq!(map.route(4, 5), None);
    assert!(map.move_room(5, pos(9, 9, 9)).unwrap());
    assert_eq!(map.room(5).unwrap().pos, pos(9, 9, 9));

    assert!(map.remove_room(4).unwrap());
    assert!(!map.remove_room(4).unwrap());
    assert!(map.current().is_none());
    assert_eq!(map.room(3).unwrap().exits["u"].to, 4);
}

#[test]
fn route_prefers_cheap_rooms_and_keeps_out_of_avoided_ones() {
    // Two ways from 1 to 4: short through 2, long through 5 and 6.
    let mut map = Map::in_memory();
    for room in [
        info(1, "Start", &[("n", 2), ("e", 5)]),
        info(2, "Swamp", &[("n", 4)]),
        info(5, "Road", &[("n", 6)]),
        info(6, "Bridge", &[("w", 4)]),
        info(4, "End", &[]),
    ] {
        map.set_room(room).unwrap();
    }
    let via = |map: &Map, to| -> Option<Vec<RoomId>> {
        Some(map.route(1, to)?.into_iter().map(|step| step.to).collect())
    };
    assert_eq!(via(&map, 4).unwrap(), [2, 4]);
    assert!(map.set_weight(2, 5).unwrap());
    assert_eq!(via(&map, 4).unwrap(), [5, 6, 4]);
    assert!(map.set_weight(2, 0).unwrap());
    assert_eq!(map.room(2).unwrap().weight, 1);
    assert_eq!(via(&map, 4).unwrap(), [2, 4]);

    assert!(map.set_avoid(2, true).unwrap());
    assert_eq!(via(&map, 4).unwrap(), [5, 6, 4]);
    assert_eq!(
        via(&map, 2).unwrap(),
        [2],
        "an avoided room can still be the destination"
    );
    map.set_avoid(6, true).unwrap();
    assert_eq!(via(&map, 4), None);

    // What the server reports later does not undo the user's marks.
    map.visit(info(2, "Swamp (dry)", &[("n", 4)])).unwrap();
    assert!(map.room(2).unwrap().avoid);
}

#[test]
fn island_is_lined_up_when_its_connection_is_found() {
    let mut map = Map::in_memory();
    map.visit(info(1, "Gate", &[("n", 2)])).unwrap();
    // Teleported: rooms 10 and 11 are drawn apart from the gate.
    map.visit(info(10, "Dock", &[("e", 11)])).unwrap();
    map.visit(info(11, "Pier", &[("w", 10)])).unwrap();
    assert_eq!(map.room(10).unwrap().pos, pos(2, 0, 0));
    // Back at the gate, then north: room 2 turns out to lead east to
    // the dock, so the dock's island belongs beside it.
    map.visit(info(1, "Gate", &[("n", 2)])).unwrap();
    map.visit(info(2, "Square", &[("s", 1), ("e", 10)]))
        .unwrap();
    assert_eq!(map.room(2).unwrap().pos, pos(0, 1, 0));
    assert_eq!(map.room(10).unwrap().pos, pos(1, 1, 0));
    assert_eq!(map.room(11).unwrap().pos, pos(2, 1, 0));
    assert_eq!(map.room(1).unwrap().pos, pos(0, 0, 0));
}

#[test]
fn walking_into_an_island_moves_it_but_a_loop_is_left_alone() {
    let mut map = Map::in_memory();
    map.visit(info(1, "Gate", &[("n", 2)])).unwrap();
    map.visit(info(20, "Cave", &[("s", 21)])).unwrap();
    map.visit(info(21, "Pit", &[("n", 20)])).unwrap();
    // The gate gains an exit west into the cave: walk through it.
    map.visit(info(1, "Gate", &[("n", 2), ("w", 20)])).unwrap();
    map.visit(info(20, "Cave", &[("s", 21), ("e", 1)])).unwrap();
    assert_eq!(map.room(20).unwrap().pos, pos(-1, 0, 0));
    assert_eq!(map.room(21).unwrap().pos, pos(-1, -1, 0));

    // A second link between rooms already joined does not move them,
    // even though it does not fit the grid.
    map.visit(info(21, "Pit", &[("n", 20), ("e", 1)])).unwrap();
    map.visit(info(1, "Gate", &[("n", 2), ("w", 20)])).unwrap();
    assert_eq!(map.room(21).unwrap().pos, pos(-1, -1, 0));
    assert_eq!(map.room(1).unwrap().pos, pos(0, 0, 0));
}

#[test]
fn exit_cost_lock_and_command_shape_the_route() {
    let mut map = Map::in_memory();
    for room in [
        info(1, "Hall", &[("n", 2), ("e", 3)]),
        info(2, "Vault", &[]),
        info(3, "Passage", &[("n", 4)]),
        info(4, "Loft", &[("w", 2)]),
    ] {
        map.set_room(room).unwrap();
    }
    let commands = |map: &Map| -> Option<Vec<String>> {
        Some(
            map.route(1, 2)?
                .into_iter()
                .map(|step| step.command)
                .collect(),
        )
    };
    assert_eq!(commands(&map).unwrap(), ["n"]);
    // A door: the walk must open it first.
    assert!(
        map.edit_exit(1, "North", |exit| exit.command = Some("open door;n".into()))
            .unwrap()
    );
    assert_eq!(commands(&map).unwrap(), ["open door;n"]);
    // Costly enough, the long way round wins.
    map.edit_exit(1, "n", |exit| exit.cost = 5).unwrap();
    assert_eq!(commands(&map).unwrap(), ["e", "n", "w"]);
    map.edit_exit(1, "n", |exit| exit.cost = 0).unwrap();
    map.edit_exit(1, "n", |exit| exit.locked = true).unwrap();
    assert_eq!(commands(&map).unwrap(), ["e", "n", "w"]);
    map.edit_exit(3, "n", |exit| exit.locked = true).unwrap();
    assert_eq!(commands(&map), None);
    assert!(!map.edit_exit(1, "up", |exit| exit.locked = true).unwrap());
    assert!(!map.edit_exit(99, "n", |exit| exit.locked = true).unwrap());
}

#[test]
fn server_reports_keep_what_the_user_set_on_exits() {
    let mut map = Map::in_memory();
    map.visit(info(1, "Hall", &[("n", 2), ("e", 3)])).unwrap();
    map.edit_exit(1, "n", |exit| exit.locked = true).unwrap();
    map.set_exit(1, "enter portal", 9).unwrap();
    // The server now says north leads elsewhere and east is gone.
    map.visit(info(1, "Hall", &[("n", 5)])).unwrap();
    let hall = map.room(1).unwrap();
    assert_eq!((hall.exits["n"].to, hall.exits["n"].locked), (5, true));
    assert!(!hall.exits.contains_key("e"));
    assert_eq!(
        (
            hall.exits["enter portal"].to,
            hall.exits["enter portal"].custom
        ),
        (9, true)
    );
    // Once the server lists the user's exit itself, it is no longer
    // custom, so it goes away when the server later drops it.
    map.visit(info(1, "Hall", &[("enter portal", 9)])).unwrap();
    map.visit(info(1, "Hall", &[])).unwrap();
    assert!(map.room(1).unwrap().exits.is_empty());
}

#[test]
fn relayout_redraws_a_tangled_area_around_the_player() {
    let mut map = town();
    let mut dock = info(30, "Dock", &[("e", 31)]);
    dock.area = "town".into();
    map.set_room(dock).unwrap();
    map.set_room(info(31, "Pier", &[])).unwrap();
    for (id, at) in [
        (1, pos(7, -3, 2)),
        (2, pos(7, -3, 2)),
        (3, pos(0, 9, 0)),
        (31, pos(-4, 0, 0)),
    ] {
        map.move_room(id, at).unwrap();
    }
    // The player is on the tower top, which keeps its place.
    assert_eq!(map.relayout("town").unwrap(), 4);
    let at = |id| map.room(id).unwrap().pos;
    assert_eq!(
        [at(4), at(3), at(2), at(1)],
        [pos(1, 1, 1), pos(1, 1, 0), pos(0, 1, 0), pos(0, 0, 0)]
    );
    // The dock and pier connect to nothing else: a separate island.
    assert_eq!([at(30), at(31)], [pos(3, 0, 0), pos(4, 0, 0)]);
    assert_eq!(map.relayout("town").unwrap(), 0);
    assert_eq!(map.relayout("nowhere").unwrap(), 0);
}

#[test]
fn areas_are_listed_once_in_order() {
    let mut map = town();
    let mut other = info(9, "Raft", &[]);
    other.area = "sea".into();
    map.set_room(other).unwrap();
    assert_eq!(map.areas(), ["sea", "town"]);
}

#[test]
fn text_identifiers_get_stable_ids() {
    assert_eq!(room_id(" 42 "), 42);
    assert_eq!(room_id("-1"), -1);
    let hashed = room_id("a7f3c9");
    assert!((-(1 << 52) - 2..-1).contains(&hashed));
    assert_eq!(hashed, room_id("a7f3c9"));
    assert_ne!(hashed, room_id("a7f3ca"));
    // Pinned: stored maps depend on this value never changing.
    assert_eq!(room_id("plaza"), -4_460_106_186_057_796);
}

#[test]
fn server_coordinates_win_and_revisits_keep_the_position() {
    let mut map = town();
    map.visit(info(2, "Square (night)", &[("s", 1)])).unwrap();
    assert_eq!(map.room(2).unwrap().pos, pos(0, 1, 0));
    assert_eq!(map.room(2).unwrap().name, "Square (night)");
    let mut moved = info(2, "Square", &[]);
    moved.pos = Some(pos(5, 5, 0));
    map.visit(moved).unwrap();
    assert_eq!(map.room(2).unwrap().pos, pos(5, 5, 0));
}

#[test]
fn route_is_the_shortest_and_respects_one_way_exits() {
    let map = town();
    let directions = |from, to| -> Option<Vec<String>> {
        Some(
            map.route(from, to)?
                .into_iter()
                .map(|step| step.command)
                .collect(),
        )
    };
    assert_eq!(directions(1, 4).unwrap(), ["portal"]);
    assert_eq!(directions(4, 1).unwrap(), ["d", "w", "s"]);
    assert_eq!(directions(2, 2).unwrap(), Vec::<String>::new());
    assert_eq!(directions(1, 99), None);
    assert_eq!(
        map.route(4, 1).unwrap()[0],
        Step {
            command: "d".into(),
            to: 3
        }
    );
}

#[test]
fn find_ignores_case_and_sorts() {
    let map = town();
    let names: Vec<&str> = map
        .find("tower")
        .iter()
        .map(|room| room.name.as_str())
        .collect();
    assert_eq!(names, ["Tower", "Tower Top"]);
    assert!(map.find("dungeon").is_empty());
}

#[test]
fn map_survives_reopening() {
    let path = std::env::temp_dir().join(format!("muddie-map-test-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let mut map = Map::open(&path).unwrap();
        for room in [
            info(1, "Gate", &[("n", 2)]),
            info(2, "Square", &[("s", 1), ("e", 3)]),
        ] {
            map.visit(room).unwrap();
        }
        // An update must replace the stored exits, not add to them.
        map.visit(info(2, "Square", &[("s", 1)])).unwrap();
        map.visit(info(3, "Doomed", &[("w", 2)])).unwrap();
        assert!(map.remove_room(3).unwrap());
        assert!(map.move_room(1, pos(4, 4, 0)).unwrap());
        assert!(map.set_weight(1, 3).unwrap());
        assert!(map.set_avoid(2, true).unwrap());
        let door = |exit: &mut Exit| {
            *exit = Exit {
                cost: 4,
                locked: true,
                command: Some("open door;n".into()),
                ..exit.clone()
            }
        };
        assert!(map.edit_exit(1, "n", door).unwrap());
        assert!(map.set_exit(1, "leap", 2).unwrap());
        assert!(map.is_stored());
        // What happens after storing stops must not reach the file.
        map.stop_storing();
        assert!(!map.is_stored());
        map.visit(info(7, "Unsaved", &[])).unwrap();
        map.clear().unwrap();
    }
    let map = Map::open(&path).unwrap();
    assert!(map.current().is_none());
    assert_eq!(map.rooms().count(), 2);
    let square = map.room(2).unwrap();
    assert_eq!(
        (square.name.as_str(), square.area.as_str()),
        ("Square", "town")
    );
    assert_eq!(square.pos, pos(0, 1, 0));
    let exits: Vec<(&str, RoomId)> = (square.exits.iter())
        .map(|(direction, exit)| (direction.as_str(), exit.to))
        .collect();
    assert_eq!(exits, [("s", 1)]);
    let gate = map.room(1).unwrap();
    let expected = Exit {
        to: 2,
        cost: 4,
        locked: true,
        command: Some("open door;n".into()),
        custom: false,
    };
    assert_eq!(gate.exits["n"], expected);
    assert!(gate.exits["leap"].custom);
    assert_eq!(map.room(1).unwrap().pos, pos(4, 4, 0));
    assert_eq!(
        (map.room(1).unwrap().weight, map.room(2).unwrap().avoid),
        (3, true)
    );

    let mut map = map;
    map.clear().unwrap();
    drop(map);
    assert_eq!(Map::open(&path).unwrap().rooms().count(), 0);
    std::fs::remove_file(&path).unwrap();
}

/// A desert of rooms alike, as maps taken from elsewhere have them: a row
/// of three "Dune" rooms, 10 - 11 - 12, joined east and west, with an
/// "Oasis" north of 12 and a "Dune [e,w]" written with its exits.
fn desert() -> Map {
    let mut map = Map::in_memory();
    let room = |id: RoomId, name: &str, exits: &[(&str, RoomId)]| Room {
        id,
        name: name.to_owned(),
        area: "desert".to_owned(),
        pos: pos(id as i32, 0, 0),
        exits: (exits.iter())
            .map(|(way, to)| {
                (
                    (*way).to_owned(),
                    Exit {
                        to: *to,
                        ..Exit::default()
                    },
                )
            })
            .collect(),
        weight: 1,
        avoid: false,
    };
    let rooms = vec![
        room(10, "Dune", &[("e", 11)]),
        room(11, "Dune [e,w]", &[("e", 12), ("w", 10)]),
        room(12, "Dune", &[("w", 11), ("n", 13)]),
        room(13, "Oasis", &[("s", 12)]),
    ];
    map.replace_all(rooms, None).unwrap();
    map
}

#[test]
fn rooms_alike_are_followed_until_a_move_tells_them_apart() {
    let mut map = desert();
    let seen = |name, came| Sighting {
        name: Some(name),
        exits: None,
        came,
    };
    // Three rooms are dunes, one with its exits in its name.
    assert_eq!(
        map.arrive(seen("dune", None)),
        Located::Unsure(vec![10, 11, 12])
    );
    // Going east, the player was in 10 or 11, so is in 11 or 12.
    assert_eq!(
        map.arrive(seen("Dune", Some("e"))),
        Located::Unsure(vec![11, 12])
    );
    // Going east again, only 11 had a dune east of it.
    assert_eq!(map.arrive(seen("Dune", Some("east"))), Located::Here(12));
    // Looking leaves the player where they are; going north leads on.
    assert_eq!(map.arrive(seen("Dune", None)), Located::Here(12));
    assert_eq!(map.arrive(seen("Oasis", Some("n"))), Located::Here(13));
    assert_eq!(map.current().unwrap().id, 13);
}

#[test]
fn exits_tell_rooms_alike_apart_and_one_step_is_found_without_the_way() {
    let mut map = desert();
    let exits = ["w".to_owned(), "n".to_owned()];
    let sighting = Sighting {
        name: Some("Dune"),
        exits: Some(&exits),
        came: None,
    };
    assert_eq!(map.arrive(sighting), Located::Here(12));
    // Without the exit taken, a room next to the last one that is so.
    let sighting = Sighting {
        name: Some("Oasis"),
        ..Sighting::default()
    };
    assert_eq!(map.arrive(sighting), Located::Here(13));
    // A room the map does not have is nowhere; the user can say where.
    let sighting = Sighting {
        name: Some("Sea"),
        ..Sighting::default()
    };
    assert_eq!(map.arrive(sighting), Located::Nowhere);
    assert!(map.current().is_none());
    assert!(map.set_current(11) && !map.set_current(99));
    assert_eq!(map.whereabouts(), Located::Here(11));
}

#[test]
fn the_servers_words_for_directions_are_understood_and_sent() {
    let mut map = desert();
    let words = |direction: &str, said: &[&str]| {
        (
            direction.to_owned(),
            said.iter().map(|word| (*word).to_owned()).collect(),
        )
    };
    map.set_directions(&[
        words("e", &["este", "e"]),
        words("w", &["oeste", "o"]),
        words("n", &["norte", "n"]),
    ]);
    assert_eq!(map.direction("Oeste"), "w");
    assert_eq!(map.direction("o"), "w");
    assert_eq!(map.direction("north"), "n");
    let route = map.route(10, 13).unwrap();
    let commands: Vec<&str> = route.iter().map(|step| step.command.as_str()).collect();
    assert_eq!(commands, ["este", "este", "norte"]);
    // Exits given in those words are the map's.
    let exits = ["o".to_owned(), "norte".to_owned()];
    assert_eq!(map.locate("dune", Some(&exits)), [12]);
    map.set_exit(13, "oeste", 10).unwrap();
    assert!(map.room(13).unwrap().exits.contains_key("w"));
}

#[test]
fn the_nearest_of_several_rooms_is_gone_to() {
    let map = desert();
    let dunes = HashSet::from([10, 11]);
    let (to, steps) = map.route_to_nearest(13, &dunes).unwrap();
    assert_eq!((to, steps.len()), (11, 2));
    let around: Vec<(RoomId, usize)> = (map.around(12, 2).into_iter())
        .map(|(id, steps)| (id, steps.len()))
        .collect();
    // The same number of moves away, in the order of the exits.
    assert_eq!(around, [(13, 1), (11, 1), (10, 2)]);
}

#[test]
fn places_are_meant_by_number_mark_name_or_area() {
    let mut map = desert();
    map.mark("Water", 13).unwrap();
    assert_eq!(map.rooms_meant("12"), [12]);
    assert_eq!(map.rooms_meant("water"), [13]);
    assert_eq!(map.rooms_meant("dune"), [10, 11, 12]);
    assert_eq!(map.rooms_meant("oas"), [13]);
    assert_eq!(map.rooms_meant("Desert oasis"), [13]);
    assert_eq!(map.rooms_meant("desert"), [10, 11, 12, 13]);
    assert!(map.rooms_meant("sea").is_empty());
    // A name given again, in any case, names the room it is given to.
    map.mark("WATER", 12).unwrap();
    assert_eq!(map.marks().len(), 1);
    assert_eq!(map.marked("water"), Some(12));
    assert!(map.unmark("Water").unwrap() && !map.unmark("water").unwrap());
}

#[test]
fn a_map_taken_again_keeps_what_the_user_set_and_where_it_came_from() {
    let path = std::env::temp_dir().join(format!("muddie-taken-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let mut map = Map::open(&path).unwrap();
        map.replace_all(desert().rooms().cloned().collect(), Some(10))
            .unwrap();
        map.set_avoid(11, true).unwrap();
        map.set_exit(13, "climb", 10).unwrap();
        map.mark("water", 13).unwrap();
        map.set_source(Some(("https://example.org/map.zip", Some("v1"))))
            .unwrap();
        // The newer map has the same rooms, and no exit of the user's.
        map.replace_all(desert().rooms().cloned().collect(), Some(12))
            .unwrap();
        assert_eq!(map.current().unwrap().id, 12);
    }
    let map = Map::open(&path).unwrap();
    assert!(map.room(11).unwrap().avoid);
    assert_eq!(map.room(13).unwrap().exits["climb"].to, 10);
    assert_eq!(map.marked("water"), Some(13));
    assert_eq!(
        map.source(),
        Some(("https://example.org/map.zip", Some("v1")))
    );
    drop(map);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn the_room_the_player_was_in_is_where_the_map_starts_next_time() {
    let path = std::env::temp_dir().join(format!("muddie-last-room-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let mut map = Map::open(&path).unwrap();
        map.replace_all(desert().rooms().cloned().collect(), None)
            .unwrap();
        assert!(map.set_current(12));
    }
    let mut map = Map::open(&path).unwrap();
    assert_eq!(map.current().map(|room| room.id), Some(12));
    // The server shows another room: the map does not stay there.
    let seen = Sighting {
        name: Some("Oasis"),
        ..Sighting::default()
    };
    assert_eq!(map.arrive(seen), Located::Here(13));
    drop(map);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn a_move_sent_is_the_exit_taken_when_the_server_does_not_say() {
    let mut map = desert();
    assert!(map.set_current(11));
    let seen = |name| Sighting {
        name: Some(name),
        ..Sighting::default()
    };
    // Rooms alike either way: the move sent tells which.
    map.sent("e");
    assert_eq!(map.arrive(seen("Dune")), Located::Here(12));
    // Looking is no move: the player stays.
    map.sent("look");
    assert_eq!(map.arrive(seen("Dune")), Located::Here(12));
    // A move the server said failed is not taken.
    map.sent("w");
    map.moves_failed();
    assert_eq!(map.arrive(seen("Dune")), Located::Here(12));
    // A move that led nowhere new leaves the player where they were.
    map.sent("s");
    assert_eq!(map.arrive(seen("Dune")), Located::Here(12));
}

#[test]
fn a_name_not_on_the_map_finds_the_rooms_most_like_it_the_nearest_first() {
    let mut map = desert();
    assert!(map.set_current(12));
    // No room is called so: those whose name has it come, the nearest to
    // where the player was first.
    let seen = Sighting {
        name: Some("Dun"),
        ..Sighting::default()
    };
    assert_eq!(map.arrive(seen), Located::Unsure(vec![12, 11, 10]));
    let seen = Sighting {
        name: Some("A quiet oasis"),
        ..Sighting::default()
    };
    assert_eq!(map.arrive(seen), Located::Here(13));
    let seen = Sighting {
        name: Some("Sea"),
        ..Sighting::default()
    };
    assert_eq!(map.arrive(seen), Located::Nowhere);
}
