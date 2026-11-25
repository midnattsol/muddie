//! Tests of following the player through a map taken from elsewhere: rooms alike, places and what the user set.

use super::*;

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
    // The cheapest first, as routes go: of the same cost, by number.
    assert_eq!(around, [(11, 1), (13, 1), (10, 2)]);
    // A room routes keep off is reached, but not gone through.
    let mut map = map;
    map.set_avoid(11, true).unwrap();
    let around: Vec<RoomId> = map.around(12, 2).into_iter().map(|(id, _)| id).collect();
    assert_eq!(around, [11, 13]);
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
    // A room taken off the map takes its marks with it.
    let mut gone = desert();
    gone.mark("Well", 12).unwrap();
    assert!(gone.remove_room(12).unwrap());
    assert!(gone.marks().is_empty() && gone.marked("well").is_none());
    assert!(map.rooms_meant("sea").is_empty());
    // Of several, the room meant is the nearest; with no whereabouts, the
    // first.
    assert_eq!(map.room_meant("dune"), Some(10));
    assert!(map.set_current(13));
    assert_eq!(map.room_meant("dune"), Some(12));
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
