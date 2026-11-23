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
        colour: None,
    }
}

fn pos(x: i32, y: i32, z: i32) -> Pos {
    Pos { x, y, z }
}

/// A room as a map taken from elsewhere has it: in `area`, `at`, with its
/// exits.
fn room_at(area: &str, id: RoomId, name: &str, at: Pos, exits: &[(&str, RoomId)]) -> Room {
    Room {
        id,
        name: name.to_owned(),
        area: area.to_owned(),
        pos: at,
        exits: (exits.iter())
            .map(|(way, to)| {
                let exit = Exit {
                    to: *to,
                    ..Exit::default()
                };
                ((*way).to_owned(), exit)
            })
            .collect(),
        weight: 1,
        avoid: false,
        colour: None,
        note: String::new(),
        renamed: false,
        added: false,
        shape: None,
        symbol: String::new(),
        styled: false,
    }
}

/// A room by its number, name and exits.
type Laid<'a> = (RoomId, &'a str, &'a [(&'a str, RoomId)]);

/// A map of those rooms in `area`, taken whole, each in a row east by its
/// number.
fn map_of(area: &str, rooms: &[Laid<'_>]) -> Map {
    let mut map = Map::in_memory();
    let rooms = (rooms.iter())
        .map(|(id, name, exits)| room_at(area, *id, name, pos(*id as i32, 0, 0), exits))
        .collect();
    map.replace_all(rooms, None).unwrap();
    map
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
    assert!(matches!(map.relayout("town").unwrap(), Tidied::Moved(5)));
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
    assert_eq!(map.relayout("town").unwrap(), Tidied::Moved(0));
    assert_eq!(map.relayout("nowhere").unwrap(), Tidied::Moved(0));
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
    let kept = map.relayout("town").unwrap();
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
        assert!(map.set_weight(1, 3).unwrap());
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
    let mut map = map_of(
        "desert",
        &[
            (10, "Dune", &[("e", 11)]),
            (11, "Dune [e,w]", &[("e", 12), ("w", 10)]),
            (12, "Dune", &[("w", 11), ("n", 13)]),
            (13, "Oasis", &[("s", 12)]),
        ],
    );
    // As maps taken from elsewhere are, the exits after a name.
    map.set_naming(Naming {
        exits_between: Some(("[".into(), "]".into())),
        ..Naming::default()
    });
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
fn a_room_led_to_whose_exits_are_not_those_shown_is_not_trusted_if_another_has_them() {
    let mut map = desert();
    // 12 has also an exit by a command, which the server does not show.
    map.set_exit(12, "cavar", 13).unwrap();
    let shown = |map: &mut Map, exits: &[&str]| {
        let exits: Vec<String> = exits.iter().map(|exit| (*exit).to_owned()).collect();
        map.arrive(Sighting {
            name: Some("Dune"),
            exits: Some(&exits),
            came: Some("e"),
        })
    };
    // East of 10 is 11, but the dune shown is as 12 is: the map is wrong
    // there, and the player is looked for around.
    assert!(map.set_current(10));
    assert_eq!(shown(&mut map, &["w", "n"]), Located::Here(12));
    // A dune as none on the map is: the exit is followed.
    assert!(map.set_current(10));
    assert_eq!(shown(&mut map, &["w", "s"]), Located::Here(11));
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
    map.teach_directions(&[
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
        assert_eq!(map.source_reader(), None);
        map.set_source_reader(Some(MUDLET_READER)).unwrap();
        map.set_colour(10, Some([7, 7, 7])).unwrap();
        map.set_colour(12, Some([5, 5, 5])).unwrap();
        map.set_shape(12, Some(Shape::Circle)).unwrap();
        let look = Appearance {
            colour: Some([30, 60, 90]),
            shape: Shape::Hexagon,
        };
        map.set_appearance(look).unwrap();
        map.set_symbol(12, " $$$ ").unwrap();
        map.set_note(13, "  Fresh water  ").unwrap();
        // A room the map had named with something said there.
        assert!(map.rename(11, " Dune crest ").unwrap());
        assert!(!map.rename(99, "Nowhere").unwrap());
        // The newer map has the same rooms and one more, and no exit of the
        // user's; it colours rooms, where the look the user set stays, and
        // names the room the user named as it named it before; it has a
        // door, shut.
        let mut newer: Vec<Room> = desert().rooms().cloned().collect();
        for room in newer.iter_mut().filter(|room| room.id <= 11) {
            room.colour = Some([1, 1, 1]);
        }
        for room in newer.iter_mut().filter(|room| room.id == 13) {
            room.exits.get_mut("s").unwrap().door = Some(Door::Closed);
        }
        let mut more = newer[0].clone();
        more.id = 20;
        newer.push(more);
        let replaced = map.replace_all(newer, Some(12)).unwrap();
        assert_eq!(
            replaced,
            Replaced {
                rooms: desert().rooms().count() + 1,
                new: 1,
                renamed: 1,
                ..Replaced::default()
            }
        );
        assert_eq!(map.current().unwrap().id, 12);
    }
    let mut map = Map::open(&path).unwrap();
    assert_eq!(map.room(11).unwrap().name, "Dune crest");
    // The server reporting the room does not name it otherwise either.
    let room = map.room(11).unwrap();
    let seen = RoomInfo {
        id: 11,
        name: "Ted says: hello".into(),
        area: room.area.clone(),
        exits: (room.exits.iter())
            .map(|(way, exit)| (way.clone(), exit.to))
            .collect(),
        pos: Some(room.pos),
        colour: None,
    };
    map.visit(seen).unwrap();
    assert_eq!(map.room(11).unwrap().name, "Dune crest");
    assert!(map.room(11).unwrap().avoid);
    assert_eq!(map.room(13).unwrap().exits["climb"].to, 10);
    let colour = |id| map.room(id).unwrap().colour;
    assert_eq!(
        (colour(10), colour(11), colour(12)),
        (Some([7, 7, 7]), Some([1, 1, 1]), Some([5, 5, 5]))
    );
    let square = map.room(12).unwrap();
    assert_eq!(
        (square.shape, square.symbol.as_str()),
        (Some(Shape::Circle), "$$")
    );
    assert_eq!(map.room(13).unwrap().exits["s"].door, Some(Door::Closed));
    assert_eq!(
        map.appearance(),
        Appearance {
            colour: Some([30, 60, 90]),
            shape: Shape::Hexagon
        }
    );
    assert_eq!(map.room(10).unwrap().shape, None);
    assert_eq!(map.marked("water"), Some(13));
    assert_eq!(map.room(13).unwrap().note, "Fresh water");
    assert_eq!(map.source_reader(), Some(MUDLET_READER));
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

#[test]
fn a_rooms_colour_stays_when_the_server_says_none_and_is_kept() {
    let path = std::env::temp_dir().join(format!("muddie-colour-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let mut map = Map::open(&path).unwrap();
        let coloured = RoomInfo {
            colour: Some([1, 2, 3]),
            ..info(1, "Gate", &[("n", 2)])
        };
        map.visit(coloured).unwrap();
        map.visit(info(2, "Square", &[("s", 1)])).unwrap();
        map.visit(info(1, "Gate", &[("n", 2)])).unwrap();
        assert_eq!(map.room(1).unwrap().colour, Some([1, 2, 3]));
        assert!(map.set_colour(2, Some([200, 100, 0])).unwrap());
        assert!(!map.set_colour(9, None).unwrap());
    }
    let mut map = Map::open(&path).unwrap();
    assert_eq!(map.room(1).unwrap().colour, Some([1, 2, 3]));
    assert_eq!(map.room(2).unwrap().colour, Some([200, 100, 0]));
    map.set_colour(1, None).unwrap();
    drop(map);
    assert_eq!(Map::open(&path).unwrap().room(1).unwrap().colour, None);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn a_room_is_in_its_area_unless_the_mud_names_its_region() {
    let mut map = desert();
    let rename = |map: &mut Map, id: RoomId, name: &str| {
        let rooms: Vec<Room> = (map.rooms().cloned())
            .map(|mut room| {
                if room.id == id {
                    room.name = name.to_owned();
                }
                room
            })
            .collect();
        map.replace_all(rooms, None).unwrap();
    };
    rename(&mut map, 11, "Anduar: Calle de la Justicia [e,w]");
    rename(&mut map, 12, "Anduar: Plaza Mayor");
    let region = |map: &Map, id| map.region(map.room(id).unwrap()).map(str::to_owned);
    // Of one area, and named in no way of the MUD's: nothing to say.
    assert_eq!(region(&map, 11), None);
    // Taught that a name begins with its region.
    map.set_naming(Naming {
        region_before: Some(":".into()),
        exits_between: Some(("[".into(), "]".into())),
    });
    assert_eq!(region(&map, 11).as_deref(), Some("Anduar"));
    assert_eq!(region(&map, 10).as_deref(), Some("Dune"));
    // A route from the oasis to the first dune goes through Anduar.
    let steps = map.route(13, 10).unwrap();
    assert_eq!(map.regions_along(13, &steps), ["Anduar"]);
    // Untaught again, with several areas, the area is the region.
    map.set_naming(Naming::default());
    let mut rooms: Vec<Room> = map.rooms().cloned().collect();
    rooms
        .iter_mut()
        .filter(|room| room.id == 13)
        .for_each(|room| room.area = "oasis".into());
    map.replace_all(rooms, None).unwrap();
    assert_eq!(region(&map, 11).as_deref(), Some("desert"));
}

#[test]
fn a_name_is_compared_whole_unless_the_mud_writes_exits_after_it() {
    let mut map = desert();
    assert_eq!(map.locate("Dune", None), [10, 11, 12]);
    map.set_naming(Naming::default());
    assert_eq!(map.locate("Dune", None), [10, 12]);
    assert_eq!(map.locate("dune [e,w]", None), [11]);
}

/// As Reinos de Leyenda's map has: a street and a square, and on their very
/// places copies of them, of their names, that only lead to each other and,
/// one way, to the street's next room. The copies are left over: not found
/// by name, nor taken for where the player is; the rooms they copy are.
#[test]
fn copies_of_rooms_no_room_leads_to_are_left_over() {
    let mut map = Map::in_memory();
    let room =
        |id, name, y, exits: &[(&str, RoomId)]| room_at("Anduar", id, name, pos(0, y, 0), exits);
    map.replace_all(
        vec![
            room(1, "Calle", 2, &[("s", 2)]),
            room(2, "Calle", 0, &[("n", 1), ("s", 3)]),
            room(3, "Plaza", -2, &[("n", 2)]),
            room(5, "Calle", 0, &[("n", 1), ("s", 6)]),
            room(6, "Plaza", -2, &[("n", 5)]),
            // Two alike in one place that nothing leads to: the one with
            // more exits stays.
            room(8, "Torre", 10, &[("e", 1)]),
            room(9, "Torre", 10, &[("e", 1), ("w", 1)]),
        ],
        None,
    )
    .unwrap();
    assert_eq!((map.ghost_of(5), map.ghost_of(6)), (Some(2), Some(3)));
    assert_eq!(map.ghost_of(8), Some(9));
    assert!(!map.is_ghost(1) && !map.is_ghost(2) && !map.is_ghost(3) && !map.is_ghost(9));
    assert_eq!(map.called("Plaza"), [3]);
    assert_eq!(map.locate("Calle", None), [1, 2]);
    assert!(map.find("Torre").iter().all(|room| room.id == 9));
    // Entering the square, the map is sure where.
    assert!(map.set_current(1));
    map.sent("s");
    map.arrive(Sighting {
        name: Some("Calle"),
        ..Sighting::default()
    });
    assert_eq!(
        map.arrive(Sighting {
            name: Some("Plaza"),
            came: Some("s"),
            ..Sighting::default()
        }),
        Located::Here(3)
    );
    // A way made to a copy makes it one of the rest again.
    map.set_exit(3, "w", 6).unwrap();
    assert!(!map.is_ghost(6) && !map.is_ghost(5));
}

#[test]
fn a_room_lies_some_way_from_another_and_is_called_short() {
    let mut map = desert();
    // The desert's dunes are in a row from west to east, the oasis at the
    // east end, one place apart.
    assert_eq!(map.bearing(10, 12), Some("e"));
    assert_eq!(map.bearing(12, 10), Some("w"));
    assert_eq!(map.bearing(10, 10), None);
    map.move_room(13, pos(15, 5, 0)).unwrap();
    assert_eq!(map.bearing(10, 13), Some("ne"));
    map.set_naming(Naming {
        region_before: Some(":".into()),
        exits_between: Some(("[".into(), "]".into())),
    });
    let mut street = map.room(11).unwrap().clone();
    street.name = "Anduar: Calle [e,w]".into();
    assert_eq!(map.short_name(&street), "Calle");
    assert_eq!(map.short_name(map.room(10).unwrap()), "Dune");
}

#[test]
fn places_are_found_by_mark_note_and_name_the_nearest_first() {
    // 1 Gate -n-> 2 Square -e-> 3 Tower -u-> 4 Tower Top, and a Tower far
    // off that no way leads to.
    let mut map = town();
    map.visit(info(9, "Tower", &[])).unwrap();
    map.mark("home tower", 4).unwrap();
    map.set_note(2, "A fountain by the tower").unwrap();
    map.visit(info(1, "Gate", &[("n", 2), ("portal", 4)]))
        .unwrap();
    let found = map.search("TOWER", Some(1));
    let said: Vec<(RoomId, FoundBy, Option<usize>, usize)> = (found.into_iter())
        .map(|found| (found.room, found.by, found.moves, found.alike))
        .collect();
    assert_eq!(
        said,
        [
            (4, FoundBy::Mark("home tower".into()), Some(1), 1),
            (2, FoundBy::Note, Some(1), 1),
            // Two rooms are called Tower: the one there is a way to.
            (3, FoundBy::Name, Some(2), 2),
        ]
    );
    assert_eq!(map.marks_of(4), ["home tower"]);
    assert!(map.search("  ", Some(1)).is_empty());
    // Nowhere to look from: all the same, found.
    assert_eq!(map.search("fountain", None)[0].moves, None);
    // A note emptied is no note.
    map.set_note(2, " ").unwrap();
    assert!(map.search("fountain", None).is_empty());
}

#[test]
fn moving_between_known_rooms_leaves_the_map_as_it_was_and_moves_are_counted_as_walked() {
    let mut map = town();
    map.visit(info(1, "Gate", &[("n", 2), ("portal", 4)]))
        .unwrap();
    let revision = map.revision();
    // Walking known rooms changes none: what is worked out of them stays.
    map.visit(info(2, "Square", &[("s", 1), ("e", 3)])).unwrap();
    map.visit(info(1, "Gate", &[("n", 2), ("portal", 4)]))
        .unwrap();
    assert_eq!(map.revision(), revision);
    // The moves to each room are those of its route, the portal included.
    for to in 1..=4 {
        let route = map.route(1, to).map(|steps| steps.len());
        assert_eq!(map.moves_between(1, to), route, "to {to}");
    }
    // A room to avoid is reached, but not gone through.
    map.set_avoid(2, true).unwrap();
    assert_eq!(map.moves_between(1, 2), Some(1));
    // The tower, then, by the portal and down, as a walk would go.
    assert_eq!(map.moves_between(1, 3), Some(2));
    assert_eq!(map.route(1, 3).map(|steps| steps.len()), Some(2));
}

#[test]
fn a_languages_words_are_under_those_taught_and_stay_when_these_go() {
    let mut map = Map::in_memory();
    let words = |pairs: &[(&str, &[&str])]| -> Vec<(String, Vec<String>)> {
        (pairs.iter())
            .map(|(key, said)| {
                (
                    (*key).to_owned(),
                    said.iter().map(|word| (*word).to_owned()).collect(),
                )
            })
            .collect()
    };
    map.set_language(&words(&[
        ("nw", &["noroeste", "no"]),
        ("n", &["norte", "n"]),
    ]));
    assert_eq!(
        (map.direction("Noroeste"), map.command_for("nw")),
        ("nw".to_owned(), "noroeste")
    );
    // English is understood of any MUD.
    assert_eq!(map.direction("north"), "n");
    // A plugin's own word goes over the language's, which still mean it.
    map.teach_directions(&words(&[("nw", &["nor", "noroeste"])]));
    assert_eq!(
        (map.direction("nor"), map.direction("no")),
        ("nw".to_owned(), "nw".to_owned())
    );
    assert_eq!(
        (map.command_for("nw"), map.command_for("n")),
        ("nor", "norte")
    );
    map.forget_taught_directions();
    assert_eq!(
        (map.direction("nor"), map.command_for("nw")),
        ("nor".to_owned(), "noroeste")
    );
    // Each direction known once, in the order exits are listed.
    let keys: Vec<&str> = DIRECTIONS.iter().map(|direction| direction.key).collect();
    assert_eq!(
        keys,
        [
            "n", "ne", "e", "se", "s", "sw", "w", "nw", "u", "d", "in", "out"
        ]
    );
    assert_eq!((direction_rank("w"), direction_rank("portal")), (6, 12));
}

#[test]
fn exits_kept_in_the_muds_words_are_the_maps_once_it_knows_them() {
    let words = |pairs: &[(&str, &[&str])]| -> Vec<(String, Vec<String>)> {
        (pairs.iter())
            .map(|(key, said)| {
                (
                    (*key).to_owned(),
                    said.iter().map(|w| (*w).to_owned()).collect(),
                )
            })
            .collect()
    };
    let spanish = words(&[("out", &["fuera", "fu"]), ("in", &["dentro", "de"])]);
    let exit = |to| Exit {
        to,
        ..Exit::default()
    };
    // As a map taken from elsewhere keeps them: "fu" for out, and in one
    // room both "fu" and "out", to different rooms.
    let mut rooms: Vec<Room> = desert().rooms().cloned().collect();
    for room in rooms.iter_mut() {
        match room.id {
            10 => drop(room.exits.insert("fu".into(), exit(13))),
            12 => {
                room.exits.insert("fu".into(), exit(10));
                room.exits.insert("out".into(), exit(13));
            }
            _ => {}
        }
    }
    // Taken by a map that knows the words: in its own.
    let mut map = Map::in_memory();
    map.set_language(&spanish);
    map.replace_all(rooms.clone(), None).unwrap();
    assert_eq!(map.room(10).unwrap().exits["out"].to, 13);
    assert!(!map.room(10).unwrap().exits.contains_key("fu"));
    // Neither is lost where the room has both.
    let twelve = &map.room(12).unwrap().exits;
    assert_eq!((twelve["out"].to, twelve["fu"].to), (13, 10));
    // Taken before the map knew them: as soon as it does.
    let mut map = Map::in_memory();
    map.replace_all(rooms, None).unwrap();
    assert!(map.room(10).unwrap().exits.contains_key("fu"));
    let revision = map.revision();
    map.set_language(&spanish);
    assert_eq!(map.room(10).unwrap().exits["out"].to, 13);
    assert!(map.revision() > revision);
    // And then nothing is left to change.
    let revision = map.revision();
    map.set_language(&spanish);
    assert_eq!(map.revision(), revision);
    assert_eq!(map.route(10, 13).unwrap()[0].command, "fuera");
}

/// What the player saw, for [`Map::add`].
fn arrival<'a>(name: &'a str, exits: &'a [String], came: &'a str) -> Arrival<'a> {
    Arrival { name, exits, came }
}

fn ways(ways: &[&str]) -> Vec<String> {
    ways.iter().map(|way| (*way).to_owned()).collect()
}

#[test]
fn a_room_the_map_has_not_is_added_where_the_player_went_and_undone() {
    let mut map = desert();
    // Lost, the map adds nothing.
    let grove = ways(&["s", "e"]);
    assert_eq!(
        map.add(arrival("Palm Grove", &grove, "n")),
        Err(Refused::Lost)
    );
    assert!(map.set_current(13));
    assert_eq!(map.add(arrival(" ", &grove, "n")), Err(Refused::Unnamed));
    // Where an exit leads to a room, it is not for adding.
    assert_eq!(
        map.add(arrival("Palm Grove", &grove, "s")),
        Err(Refused::Known(12))
    );
    // North of the oasis, the way north from it; its exits lead nowhere
    // known yet, the way back too.
    let palms = map.add(arrival("Palm Grove", &grove, "n")).unwrap();
    assert_eq!(palms, Added::Made(ADDED_FROM));
    let room = map.room(ADDED_FROM).unwrap().clone();
    assert!(room.added);
    assert_eq!((room.pos, room.area.as_str()), (pos(13, 1, 0), "desert"));
    assert_eq!(room.exits["s"].to, UNEXPLORED);
    assert_eq!(room.exits["e"].to, UNEXPLORED);
    let way = &map.room(13).unwrap().exits["n"];
    assert_eq!((way.to, way.custom), (ADDED_FROM, true));
    assert_eq!(map.current().unwrap().id, ADDED_FROM);
    // Walking back, the oasis is found, not made again; the way back is
    // known now.
    let oasis = ways(&["s", "n"]);
    assert_eq!(
        map.add(arrival("Oasis", &oasis, "south")),
        Ok(Added::Found(13))
    );
    assert_eq!(map.room(ADDED_FROM).unwrap().exits["s"].to, 13);
    assert_eq!(map.rooms().count(), 5);
    // Through a portal, a room the map has anywhere could be the one: it
    // cannot tell, and adds nothing.
    let dune = ways(&["e", "w"]);
    assert_eq!(
        map.add(arrival("Dune", &dune, "portal")),
        Err(Refused::Alike(vec![11]))
    );
    // One it has not is made beside the room left.
    let cave = ways(&["out"]);
    let made = map.add(arrival("Cave", &cave, "enter portal")).unwrap();
    assert_eq!(made, Added::Made(ADDED_FROM + 1));
    assert_eq!(map.room(ADDED_FROM + 1).unwrap().pos, pos(14, 0, 0));
    assert_eq!(
        map.room(13).unwrap().exits["enter portal"].to,
        ADDED_FROM + 1
    );
    assert_eq!(map.added().len(), 2);
    // Undone, the last first: the room made goes, and the room it was
    // reached from is as it was; the map does not know where the player
    // is in a room no longer there.
    let cave = Changed::Made(made.room(), "Cave".into());
    assert_eq!(map.take_told().last(), Some(&cave));
    assert_eq!(map.last_change(), Some(&cave));
    assert_eq!(map.undo_change().unwrap(), Some(cave));
    assert!(map.room(ADDED_FROM + 1).is_none());
    assert!(!map.room(13).unwrap().exits.contains_key("enter portal"));
    assert!(map.current().is_none());
    assert_eq!(
        map.undo_change().unwrap(),
        Some(Changed::Found(13, "Oasis".into()))
    );
    assert_eq!(map.room(ADDED_FROM).unwrap().exits["s"].to, UNEXPLORED);
    map.undo_change().unwrap();
    assert!(map.room(ADDED_FROM).is_none() && !map.room(13).unwrap().exits.contains_key("n"));
    assert_eq!(map.undo_change().unwrap(), None);
    assert_eq!(map.rooms().count(), 4);
}

#[test]
fn a_room_that_way_is_found_unless_it_is_reached_back_from_another() {
    let mut map = desert();
    // East of the oasis, a dune the map has, but drawn apart: no exit
    // leads to it.
    let mut lone = map.room(10).unwrap().clone();
    lone.id = 40;
    lone.pos = pos(15, 0, 0);
    lone.exits.clear();
    lone.exits.insert("w".into(), Exit::default());
    let mut rooms: Vec<Room> = map.rooms().cloned().collect();
    rooms.push(lone);
    map.replace_all(rooms.clone(), Some(13)).unwrap();
    let dune = ways(&["w"]);
    assert_eq!(map.add(arrival("Dune", &dune, "e")), Ok(Added::Found(40)));
    assert_eq!(map.room(13).unwrap().exits["e"].to, 40);
    // Had its way back led to another room, it would not be the one.
    for room in rooms.iter_mut().filter(|room| room.id == 40) {
        room.exits.insert(
            "w".into(),
            Exit {
                to: 10,
                ..Exit::default()
            },
        );
    }
    map.replace_all(rooms, Some(13)).unwrap();
    // The exit found before is the user's, which the map taken keeps.
    map.remove_exit(13, "e").unwrap();
    assert_eq!(
        map.add(arrival("Dune", &dune, "e")),
        Ok(Added::Made(ADDED_FROM))
    );
    assert_eq!(map.room(ADDED_FROM).unwrap().pos, pos(14, 0, 0));
}

#[test]
fn rooms_added_are_kept_when_the_map_is_taken_again_or_joined_with_its_own() {
    let mut map = desert();
    assert!(map.set_current(13));
    // North of the oasis a grove, east of it a spring, down from it a
    // cellar: none of them on the map.
    let grove = map
        .add(arrival("Palm Grove", &ways(&["s", "e"]), "n"))
        .unwrap()
        .room();
    map.set_note(grove, "Shade").unwrap();
    map.mark("palms", grove).unwrap();
    let spring = map
        .add(arrival("Spring", &ways(&["w", "d"]), "e"))
        .unwrap()
        .room();
    let cellar = map
        .add(arrival("Cellar", &ways(&["u"]), "d"))
        .unwrap()
        .room();
    assert_eq!(map.room(cellar).unwrap().pos, pos(14, 1, -1));
    // The map taken again has the grove and the spring now, all of it
    // drawn further east.
    let mut newer: Vec<Room> = desert().rooms().cloned().collect();
    let exit = |to| Exit {
        to,
        ..Exit::default()
    };
    let mut grove_too = newer[0].clone();
    (grove_too.id, grove_too.name) = (30, "Palm Grove".into());
    grove_too.exits = BTreeMap::from([("s".into(), exit(13)), ("e".into(), exit(31))]);
    grove_too.pos = pos(13, 1, 0);
    let mut spring_too = grove_too.clone();
    (spring_too.id, spring_too.name) = (31, "Spring".into());
    spring_too.exits = BTreeMap::from([("w".into(), exit(30)), ("d".into(), exit(99))]);
    spring_too.pos = pos(14, 1, 0);
    newer.extend([grove_too, spring_too]);
    for room in newer.iter_mut() {
        room.pos.x += 100;
        if room.id == 13 {
            room.exits.insert("n".into(), exit(30));
        }
    }
    let replaced = map.replace_all(newer, Some(cellar)).unwrap();
    assert_eq!(
        replaced,
        Replaced {
            rooms: 7,
            new: 2,
            renamed: 0,
            added: 1,
            joined: 2,
        }
    );
    // The grove and the spring are the map's, with what was set on them,
    // and its way down from the spring; the cellar is kept, drawn as the
    // spring it hangs from moved.
    assert!(map.room(grove).is_none() && map.room(spring).is_none());
    assert_eq!(map.room(30).unwrap().note, "Shade");
    assert_eq!(map.marked("palms"), Some(30));
    assert_eq!(map.room(31).unwrap().exits["d"].to, 99);
    let kept = map.room(cellar).unwrap();
    assert!(kept.added);
    assert_eq!(kept.pos, pos(114, 1, -1));
    assert_eq!(map.current().unwrap().id, cellar);
    // Nothing is left to undo: the rooms it would undo are the map's now.
    assert_eq!(map.last_change(), None);
}

/// A road as maps taken from elsewhere have them: 1 "Road", then 2 named
/// with what was said there, 3 with its name cut short, 4 "Gate", each
/// east of the last, and 5 "Square" north of the gate.
fn road() -> Map {
    map_of(
        "road",
        &[
            (1, "Road", &[("e", 2)]),
            (2, "Traveller says: these roads", &[("w", 1), ("e", 3)]),
            (3, "Wall of Am", &[("w", 2), ("e", 4)]),
            (4, "Gate", &[("w", 3), ("n", 5)]),
            (5, "Square", &[("s", 4)]),
        ],
    )
}

/// The server shows `name` with `exits`, reached by `came`.
fn shown(map: &mut Map, name: &str, exits: &[&str], came: Option<&str>) -> Located {
    let exits: Vec<String> = exits.iter().map(|exit| (*exit).to_owned()).collect();
    map.arrive(Sighting {
        name: Some(name),
        exits: Some(&exits),
        came,
    })
}

/// What the server shows, a room by name and exits reached by an exit
/// (none on looking), and where the map then has the player.
type Shown<'a> = (&'a str, &'a [&'a str], Option<&'a str>, Located);

/// Walks the map through what the server shows, step by step.
fn walk(map: &mut Map, steps: &[Shown<'_>]) {
    for (number, (name, exits, came, at)) in steps.iter().enumerate() {
        let got = shown(map, name, exits, *came);
        assert_eq!(&got, at, "step {number}: {name} {exits:?} by {came:?}");
    }
}

fn name_of(map: &Map, id: RoomId) -> &str {
    &map.room(id).unwrap().name
}

use Located::{Here, Nowhere};

#[test]
fn learning_puts_right_the_rooms_named_otherwise_once_borne_out() {
    let mut map = road();
    map.set_learning(true);
    assert!(map.set_current(1));
    // Followed through rooms named otherwise by the way and the exits;
    // looking changes nothing; on to a room as the map has it, those
    // between are named as shown, each told and undone.
    walk(
        &mut map,
        &[
            ("Road", &["w", "e"], Some("e"), Here(2)),
            ("Wall of Amün", &["w", "e"], Some("e"), Here(3)),
            ("Wall of Amün", &["w", "e"], None, Here(3)),
        ],
    );
    assert_eq!(name_of(&map, 2), "Traveller says: these roads");
    walk(&mut map, &[("Gate", &["w", "n"], Some("e"), Here(4))]);
    assert_eq!(
        (name_of(&map, 2), name_of(&map, 3)),
        ("Road", "Wall of Amün")
    );
    assert!(map.room(3).unwrap().renamed);
    assert_eq!(map.take_told().len(), 2);
    let wall = Changed::Renamed {
        room: 3,
        was: "Wall of Am".into(),
        now: "Wall of Amün".into(),
    };
    assert_eq!(map.undo_change().unwrap(), Some(wall));
    assert_eq!(name_of(&map, 3), "Wall of Am");
    // Lost after it, nothing is renamed; exits not those shown are not
    // followed.
    let mut map = road();
    map.set_learning(true);
    assert!(map.set_current(1));
    walk(
        &mut map,
        &[
            ("Road", &["w", "e"], Some("e"), Here(2)),
            ("Forest", &["n"], Some("e"), Nowhere),
        ],
    );
    assert!(map.set_current(1));
    assert_ne!(shown(&mut map, "Road", &["w"], Some("e")), Here(2));
    assert_eq!(name_of(&map, 2), "Traveller says: these roads");
    // While unsure which of two roads, the one way east is followed.
    let mut map = road();
    map.set_learning(true);
    let other = RoomInfo {
        pos: Some(pos(1, 5, 0)),
        ..info(6, "Road", &[("n", 5)])
    };
    map.set_room(other).unwrap();
    let looked = map.arrive(Sighting {
        name: Some("Road"),
        ..Sighting::default()
    });
    assert_eq!(looked, Located::Unsure(vec![1, 6]));
    walk(
        &mut map,
        &[
            ("Road", &["w", "e"], Some("e"), Here(2)),
            ("Wall of Amün", &["w", "e"], Some("e"), Here(3)),
            ("Gate", &["w", "n"], Some("e"), Here(4)),
        ],
    );
    assert_eq!(name_of(&map, 2), "Road");
    // Not learning, only followed.
    let mut map = road();
    assert!(map.set_current(1));
    walk(
        &mut map,
        &[
            ("Road", &["w", "e"], Some("e"), Here(2)),
            ("Wall of Amün", &["w", "e"], Some("e"), Here(3)),
            ("Gate", &["w", "n"], Some("e"), Here(4)),
        ],
    );
    assert_eq!(name_of(&map, 3), "Wall of Am");
    assert!(map.take_told().is_empty() && map.last_change().is_none());
}

#[test]
fn learning_puts_right_exits_lacking_or_leading_elsewhere_and_counts_those_never_shown() {
    // A gate shown with an exit south the map lacks bears out the rooms
    // before it all the same, and is given it, not explored.
    let mut map = road();
    map.set_learning(true);
    assert!(map.set_current(1));
    walk(
        &mut map,
        &[
            ("Road", &["w", "e"], Some("e"), Here(2)),
            ("Wall of Amün", &["w", "e"], Some("e"), Here(3)),
            ("Gate", &["w", "n", "s"], Some("e"), Here(4)),
        ],
    );
    assert_eq!(name_of(&map, 3), "Wall of Amün");
    assert_eq!(map.room(4).unwrap().exits["s"].to, UNEXPLORED);
    let added = Changed::ExitsAdded {
        room: 4,
        exits: vec!["s".into()],
    };
    assert_eq!(map.take_told().last(), Some(&added));
    assert_eq!(map.undo_change().unwrap(), Some(added));
    // Only from where the map knew the player was: told it, not shown it.
    let mut map = road();
    map.set_learning(true);
    assert!(map.set_current(4));
    walk(&mut map, &[("Square", &["s", "e"], Some("n"), Here(5))]);
    assert!(!map.room(5).unwrap().exits.contains_key("e"));
    assert!(map.set_current(4));
    walk(
        &mut map,
        &[
            ("Gate", &["w", "n"], None, Here(4)),
            ("Square", &["s", "e"], Some("n"), Here(5)),
        ],
    );
    assert_eq!(map.room(5).unwrap().exits["e"].to, UNEXPLORED);
    // An exit that led to the room beside, borne out by the next move,
    // leads there.
    let mut map = map_of(
        "fields",
        &[
            (1, "Crossing", &[("n", 2), ("e", 3)]),
            (2, "Hill", &[("s", 1)]),
            (3, "Field", &[("w", 1), ("n", 4)]),
            (4, "Pond", &[("s", 3)]),
        ],
    );
    map.set_learning(true);
    assert!(map.set_current(1));
    walk(
        &mut map,
        &[
            ("Crossing", &["n", "e"], None, Here(1)),
            ("Field", &["w", "n"], Some("n"), Here(3)),
        ],
    );
    assert_eq!(map.room(1).unwrap().exits["n"].to, 2);
    walk(&mut map, &[("Pond", &["s"], Some("n"), Here(4))]);
    let rerouted = Changed::Rerouted {
        room: 1,
        way: "n".into(),
        was: 2,
        now: 3,
    };
    assert_eq!(map.take_told(), std::slice::from_ref(&rerouted));
    assert_eq!(map.undo_change().unwrap(), Some(rerouted));
    assert_eq!(map.room(1).unwrap().exits["n"].to, 2);
    // An exit never shown is counted, not taken away; shown, it starts
    // again.
    let mut map = road();
    map.set_learning(true);
    assert!(map.set_current(3));
    walk(&mut map, &[("Wall of Am", &["w", "e"], None, Here(3))]);
    for _ in 0..3 {
        walk(
            &mut map,
            &[
                ("Gate", &["w"], Some("e"), Here(4)),
                ("Wall of Am", &["w", "e"], Some("w"), Here(3)),
            ],
        );
    }
    assert_eq!(map.unshown_exits(UNSHOWN_AFTER), [(4, "n".to_owned(), 3)]);
    assert_eq!(map.room(4).unwrap().exits["n"].to, 5);
    walk(&mut map, &[("Gate", &["w", "n"], Some("e"), Here(4))]);
    assert!(map.unshown_exits(1).is_empty());
}

#[test]
fn learning_adds_the_rooms_the_map_has_not_from_where_it_knew_the_player_was() {
    let made = ADDED_FROM;
    // Told where the player was, not shown it: nothing is added, and the
    // player is in no room named like the one shown.
    let mut map = road();
    map.set_learning(true);
    assert!(map.set_current(5));
    walk(&mut map, &[("Fountain", &["w"], Some("e"), Nowhere)]);
    assert_eq!(map.rooms().count(), 5);
    // Shown it: east of the square, where there is no exit, the room is
    // made; back and there again, it is known, the way back learnt.
    assert!(map.set_current(5));
    walk(
        &mut map,
        &[
            ("Square", &["s"], None, Here(5)),
            ("Fountain", &["w"], Some("e"), Here(made)),
            ("Square", &["s", "e"], Some("w"), Here(5)),
            ("Fountain", &["w"], Some("e"), Here(made)),
        ],
    );
    assert_eq!(map.take_told()[0], Changed::Made(made, "Fountain".into()));
    // Something said with brackets is not a room.
    walk(&mut map, &[("Square", &["s", "e"], Some("w"), Here(5))]);
    assert_ne!(
        shown(&mut map, "Moony says: ok", &["ok"], Some("n")),
        Here(made + 1)
    );
    assert!(map.room(made + 1).is_none());
    // Not learning, nothing is added.
    let mut map = road();
    assert!(map.set_current(5));
    shown(&mut map, "Square", &["s"], None);
    assert_eq!(shown(&mut map, "Fountain", &["w"], Some("e")), Nowhere);
}

/// A session in Reinos de Leyenda, as recorded with --debug at the
/// Catedral de la Guerra, on the rooms around as the MUD's published map
/// had them before: the rooms as the game wrote them and the moves made,
/// in testdata/catedral. The map follows every step; the room the MUD
/// renamed is put right, the Catedral learns its exit south-west, and the
/// Tesorería that way is added and its way back found.
#[test]
fn learning_on_a_session_recorded_in_reinos_de_leyenda() {
    let rooms = include_str!("../testdata/catedral/rooms.tsv");
    let session = include_str!("../testdata/catedral/session.tsv");
    let mut map = Map::in_memory();
    let rooms: Vec<Room> = (rooms.lines())
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let field: Vec<&str> = line.split('\t').collect();
            let number = |at: usize| field[at].parse::<i32>().unwrap();
            let exits: Vec<(&str, RoomId)> = (field[6].split(','))
                .filter_map(|exit| exit.split_once('='))
                .map(|(way, to)| (way, to.parse().unwrap()))
                .collect();
            let at = pos(number(3), number(4), number(5));
            room_at(field[2], field[0].parse().unwrap(), field[1], at, &exits)
        })
        .collect();
    map.replace_all(rooms, None).unwrap();
    // As the plugin of Reinos de Leyenda sets the map up.
    let words = |pairs: &[(&str, &[&str])]| -> Vec<(String, Vec<String>)> {
        (pairs.iter())
            .map(|(key, said)| {
                (
                    (*key).to_owned(),
                    said.iter().map(|word| (*word).to_owned()).collect(),
                )
            })
            .collect()
    };
    map.set_language(&words(&[
        ("n", &["norte", "n"]),
        ("ne", &["noreste", "ne"]),
        ("e", &["este", "e"]),
        ("se", &["sudeste", "se"]),
        ("s", &["sur", "s"]),
        ("sw", &["sudoeste", "so"]),
        ("w", &["oeste", "o"]),
        ("nw", &["noroeste", "no"]),
        ("u", &["arriba", "ar"]),
        ("d", &["abajo", "ab"]),
        ("in", &["dentro", "de"]),
        ("out", &["fuera", "fu"]),
    ]));
    map.set_naming(Naming {
        region_before: Some(":".into()),
        exits_between: Some(("[".into(), "]".into())),
    });
    map.set_learning(true);
    for (number, step) in session
        .lines()
        .filter(|line| !line.starts_with('#'))
        .enumerate()
    {
        let field: Vec<&str> = step.split('\t').collect();
        let came = (field[0] == "move").then_some(field[1]);
        let (name, list) = field[2].trim_end_matches(']').rsplit_once(" [").unwrap();
        let exits: Vec<String> = (list.split(','))
            .map(|exit| exit.replace(['|', '-'], ""))
            .collect();
        let at = map.arrive(Sighting {
            name: Some(name),
            exits: Some(&exits),
            came,
        });
        assert!(matches!(at, Here(_)), "step {number}: {step} -> {at:?}");
    }
    let tesoreria = ADDED_FROM;
    assert_eq!(
        name_of(&map, 36709),
        "Catedral de la Guerra - Antesala de los Tambores de la Guerra"
    );
    assert_eq!(map.room(36710).unwrap().exits["sw"].to, tesoreria);
    assert_eq!(
        name_of(&map, tesoreria),
        "Catedral de la Guerra - Tesorería"
    );
    assert_eq!(map.room(tesoreria).unwrap().exits["ne"].to, 36710);
    assert_eq!(map.current().unwrap().id, 36705);
}
