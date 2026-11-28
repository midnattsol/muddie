//! Tests of placing rooms, editing, routes, tidying and keeping the map.

use super::*;

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
    assert!(map.set_cost(2, 4).unwrap());
    assert_eq!(via(&map, 4).unwrap(), [5, 6, 4]);
    assert!(map.set_cost(2, 0).unwrap());
    assert_eq!(map.room(2).unwrap().cost, 0);
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
    map.edit_exit(1, "n", |exit| exit.blocked = true).unwrap();
    assert_eq!(commands(&map).unwrap(), ["e", "n", "w"]);
    map.edit_exit(3, "n", |exit| exit.blocked = true).unwrap();
    assert_eq!(commands(&map), None);
    assert!(!map.edit_exit(1, "up", |exit| exit.blocked = true).unwrap());
    assert!(!map.edit_exit(99, "n", |exit| exit.blocked = true).unwrap());
}

#[test]
fn server_reports_keep_what_the_user_set_on_exits() {
    let mut map = Map::in_memory();
    map.visit(info(1, "Hall", &[("n", 2), ("e", 3)])).unwrap();
    map.edit_exit(1, "n", |exit| exit.blocked = true).unwrap();
    map.set_exit(1, "enter portal", 9).unwrap();
    // The server now says north leads elsewhere and east is gone.
    map.visit(info(1, "Hall", &[("n", 5)])).unwrap();
    let hall = map.room(1).unwrap();
    assert_eq!((hall.exits["n"].to, hall.exits["n"].blocked), (5, true));
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
fn tidying_redraws_a_tangled_area_on_its_levels() {
    let mut map = town();
    let mut dock = info(30, "Dock", &[("e", 31)]);
    dock.area = "town".into();
    map.set_room(dock).unwrap();
    map.set_room(info(31, "Pier", &[])).unwrap();
    for (id, at) in [
        (1, pos(7, -3, 0)),
        (2, pos(7, -3, 0)),
        (3, pos(0, 9, 0)),
        (30, pos(5, 5, 0)),
        (31, pos(-4, 0, 0)),
    ] {
        map.move_room(id, at).unwrap();
    }
    assert!(matches!(map.tidy("town").unwrap(), Tidied::Moved(5)));
    let at = |id| map.room(id).unwrap().pos;
    // The player is on the tower top, which keeps its place; the rest is
    // drawn the way the exits say, east of it, on its own level: up and
    // down move no room.
    assert_eq!(at(4), pos(1, 1, 1));
    assert_eq!(
        [at(1), at(2), at(3)],
        [pos(3, 0, 0), pos(3, 1, 0), pos(4, 1, 0)]
    );
    // The dock and pier connect to nothing else: a separate island.
    assert_eq!([at(30), at(31)], [pos(6, 0, 0), pos(7, 0, 0)]);
    assert_eq!(map.tidy("town").unwrap(), Tidied::Moved(0));
    assert_eq!(map.tidy("nowhere").unwrap(), Tidied::Moved(0));
}

#[test]
fn tidying_leaves_a_drawing_it_would_make_worse() {
    // Drawn by hand: the road east is two steps long, with a hill between
    // that it goes round, and every exit points its way.
    let mut map = Map::in_memory();
    for room in [
        info(1, "Road", &[("e", 2), ("ne", 3)]),
        info(2, "Road", &[("w", 1), ("nw", 3)]),
        info(3, "Hill", &[("sw", 1), ("se", 2)]),
    ] {
        map.set_room(room).unwrap();
    }
    for (id, at) in [(1, pos(0, 0, 0)), (2, pos(2, 0, 0)), (3, pos(1, 1, 0))] {
        map.move_room(id, at).unwrap();
    }
    // Drawn again from scratch, the road would be a step long, and the
    // hill's exits would point the wrong way: it is left as it is.
    let kept = map.tidy("town").unwrap();
    assert_eq!(
        kept,
        Tidied::Kept {
            now: Drawing::default(),
            then: Drawing {
                wrong_exits: 2,
                stacked: 0
            }
        }
    );
    assert_eq!(map.room(2).unwrap().pos, pos(2, 0, 0));
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
        assert!(map.set_cost(1, 2).unwrap());
        assert!(map.set_avoid(2, true).unwrap());
        let door = |exit: &mut Exit| {
            *exit = Exit {
                cost: 4,
                blocked: true,
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
        blocked: true,
        command: Some("open door;n".into()),
        custom: false,
        door: None,
    };
    assert_eq!(gate.exits["n"], expected);
    assert!(gate.exits["leap"].custom);
    assert_eq!(map.room(1).unwrap().pos, pos(4, 4, 0));
    assert_eq!(
        (map.room(1).unwrap().cost, map.room(2).unwrap().avoid),
        (2, true)
    );

    let mut map = map;
    map.clear().unwrap();
    drop(map);
    assert_eq!(Map::open(&path).unwrap().rooms().count(), 0);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn rooms_not_saved_while_the_file_is_busy_are_saved_later() {
    let path = std::env::temp_dir().join(format!("muddie-map-busy-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let mut map = Map::open(&path).unwrap();
        map.visit(info(1, "Gate", &[])).unwrap();
        // Another Muddie writing the same map, for a moment.
        let other = rusqlite::Connection::open(&path).unwrap();
        other.execute_batch("BEGIN EXCLUSIVE").unwrap();
        assert!(map.flush().is_err());
        other.execute_batch("ROLLBACK").unwrap();
        map.flush().unwrap();
    }
    let kept = Map::open(&path).unwrap().rooms().count();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(kept, 1, "the room visited while the file was busy was lost");
}

#[test]
fn the_highest_cost_is_kept() {
    let path = std::env::temp_dir().join(format!("muddie-map-cost-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let mut map = Map::open(&path).unwrap();
        map.visit(info(1, "Gate", &[])).unwrap();
        map.set_cost(1, u32::MAX).unwrap();
        map.flush().unwrap();
    }
    let cost = Map::open(&path).unwrap().room(1).unwrap().cost;
    std::fs::remove_file(&path).unwrap();
    assert_eq!(cost, u32::MAX);
}
