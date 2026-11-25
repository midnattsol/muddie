//! Tests of adding rooms the map has not, moving between known rooms and the words of exits.

use super::*;

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
