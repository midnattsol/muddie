//! Tests of learning from what the server shows: names, exits and sessions.

use super::*;

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
    let rooms = include_str!("../../testdata/catedral/rooms.tsv");
    let session = include_str!("../../testdata/catedral/session.tsv");
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
