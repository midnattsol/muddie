//! The map, from scripts.

use super::*;

#[test]
fn script_can_load_the_map_from_text() {
    // A MUD with no GMCP: the room is read from its title and exits
    // lines, and identified by the title since there is no number.
    let mut engine = engine(
        r#"map.native(false)
        local title
        trigger("^\\[ (.+) \\]$", function(m) title = m[1] end)
        trigger("^Exits: (.+)$", function(m)
            local exits = {}
            for direction, to in m[1]:gmatch("(%a+)=(%a+)") do exits[direction] = to end
            map.visit({id = title, name = title, area = "town", exits = exits})
        end)"#,
    );
    assert!(!engine.native_map());
    for line in [
        "[ Gate ]",
        "Exits: north=Square",
        "[ Square ]",
        "Exits: south=Gate east=Tower",
    ] {
        engine.on_line(line);
    }
    let (gate, square) = (room_id("Gate"), room_id("Square"));
    assert_eq!(
        engine.take_actions(),
        [Action::Entered(gate), Action::Entered(square)]
    );
    {
        let map = engine.map.borrow();
        assert_eq!(map.current().unwrap().name, "Square");
        assert_eq!(map.room(gate).unwrap().exits["n"].to, square);
        let (from, to) = (map.room(gate).unwrap().pos, map.room(square).unwrap().pos);
        assert_eq!((to.x - from.x, to.y - from.y), (0, 1));
    }
    engine.exec(
        "test",
        r#"local here = map.room()
        echo(`{here.name} {here.exits.s == map.id("Gate")} {map.room("Tower")}`)
        echo(table.concat(map.path("Gate"), ","))
        map.walk("Gate")"#,
    );
    assert_eq!(
        engine.take_actions(),
        [
            Action::Echo("Square true nil".into()),
            Action::Echo("s".into()),
            Action::Walk(gate),
        ]
    );
}

#[test]
fn script_can_take_the_whole_map_from_the_server() {
    // The server sends each area complete: nothing is saved, and the
    // previous area is dropped when a new one arrives.
    let mut engine = engine(
        r#"map.native(false)
        map.persist(false)
        on_gmcp("Area.Map", function(area)
            map.clear()
            for _, room in area.rooms do
                map.set_room({id = room.id, name = room.name, area = area.name,
                              x = room.x, y = room.y, exits = room.exits})
            end
        end)
        on_gmcp("Area.Position", function(id) map.visit(map.room(id)) end)"#,
    );
    assert!(!engine.persist_map());
    engine.on_gmcp(
        "Area.Map",
        r#"{"name": "Docks", "rooms": [
            {"id": 1, "name": "Pier", "x": 0, "y": 0, "exits": {"e": 2}},
            {"id": 2, "name": "Wharf", "x": 1, "y": 0, "exits": {"w": 1}}]}"#,
    );
    engine.on_gmcp("Area.Position", "2");
    assert_eq!(engine.take_actions(), [Action::Entered(2)]);
    {
        let map = engine.map.borrow();
        assert_eq!(map.rooms().count(), 2);
        assert_eq!(map.current().unwrap().name, "Wharf");
        assert_eq!(map.room(2).unwrap().area, "Docks");
    }
    engine.on_gmcp(
        "Area.Map",
        r#"{"name": "Sea", "rooms": [{"id": 9, "name": "Raft", "x": 0, "y": 0}]}"#,
    );
    let map = engine.map.borrow();
    assert_eq!(map.rooms().count(), 1);
    assert!(map.current().is_none());
}

#[test]
fn script_can_edit_and_query_the_map() {
    let mut engine = engine(
        r#"map.set_room({id = 1, name = "Gate", exits = {n = 2}})
        map.set_room({id = 2, name = "Square", x = 5, y = 5, exits = {south = 1}})
        map.set_room({id = 3, name = "Old Square"})"#,
    );
    assert!(engine.native_map());
    engine.exec(
        "test",
        r#"echo(`{map.room()} {map.path(2)} {#map.find("square")} {map.find("square")[2].id}`)
        echo(table.concat(map.path(2, 1), ",") .. " " .. tostring(map.path(3, 1)))
        echo(`{map.set_exit(2, "enter portal", 3)} {map.set_exit(9, "n", 1)}`)
        echo(table.concat(map.path(3, 1), ";"))
        echo(`{map.move(3, 7, 8)} {map.room(3).x},{map.room(3).y},{map.room(3).z}`)
        echo(`{map.remove_exit(2, "enter portal")} {map.remove_room(3)} {map.room(3)}`)
        map.set_room({id = 4, name = "Detour", area = "wilds", exits = {n = 2}})
        map.set_exit(1, "e", 4)
        echo(`{map.avoid(2, true)} {map.room(2).avoid} {map.path(2, 1)[1]} {map.path(4, 1)[1]}`)
        echo(`{map.set_weight(4, 9)} {map.room(4).weight} {table.concat(map.areas(), "+")}`)
        map.avoid(2, false)
        map.set_exit(1, "gate", 2, {command = "open gate;n", cost = 0})
        map.edit_exit(1, "n", {locked = true})
        local gate = map.exit(1, "gate")
        echo(`{map.path(2, 1)[1]} {gate.custom} {gate.locked} {map.exit(1, "n").locked} {map.exit(1, "x")}`)
        echo(`{map.edit_exit(1, "gate", {command = ""})} {map.exit(1, "gate").command} {map.edit_exit(1, "x", {})}`)
        echo(`{map.relayout("nowhere")} {map.relayout()}`)
        panel("Minimap", "+-+\n| |")
        panel("Minimap")
        map.visit({name = "No id"})"#,
    );
    let actions = engine.take_actions();
    let echoes: Vec<&str> = (actions.iter())
        .filter_map(|action| match action {
            Action::Echo(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        echoes,
        [
            "nil nil 2 2",
            "n nil",
            "true false",
            "n;enter portal",
            "true 7,8,0",
            "true true nil",
            "true true n e",
            "true 9 +wilds",
            "open gate;n true false true nil",
            "true nil false",
            "0 0",
        ]
    );
    assert_eq!(
        actions[11..13],
        [
            Action::Panel {
                name: "Minimap".into(),
                text: Some("+-+\n| |".into())
            },
            Action::Panel {
                name: "Minimap".into(),
                text: None
            },
        ]
    );
    assert!(error_text(&actions[13..]).contains("a room id must be"));
}

/// The text mapper example maps a room given in its expected format.
#[test]
fn text_mapper_example_maps_rooms() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/scripts");
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.load_file(&folder.join("text-mapper.luau"));
    engine.on_line("[ Market Square ]");
    engine.on_line("Exits: north=Gate east=Alley");
    let map = engine.map.borrow();
    let room = map.current().unwrap();
    assert_eq!((room.name.as_str(), room.exits.len()), ("Market Square", 2));
}

#[test]
fn script_can_follow_a_server_that_shows_rooms_by_name_only() {
    // A server in Spanish shows each room's name and exits, and the move
    // made: two rooms alike, told apart by where the player goes.
    let mut engine = engine(
        r#"map.directions({n = {"norte", "n"}, s = {"sur", "s"}, e = {"este", "e"}, w = {"oeste", "o"}})
        for _, room in {{1, "Dune", {e = 2}}, {2, "Dune", {o = 1, norte = 3}}, {3, "Oasis", {sur = 2}}} do
            map.set_room({id = room[1], name = room[2], area = "desert", exits = room[3]})
        end
        on_gmcp("Room.Actual", function(seen) map.arrive(seen) end)
        on_gmcp("Room.Fallido", function() map.stop() end)"#,
    );
    engine.exec("test", r#"map.import("https://mud.example.org/map.zip")"#);
    assert_eq!(
        engine.take_actions(),
        [Action::ImportMap("https://mud.example.org/map.zip".into())]
    );
    engine.on_gmcp("Room.Actual", r#"{"name": "Dune"}"#);
    assert_eq!(engine.take_actions(), [Action::LostTrack]);
    engine.on_gmcp("Room.Actual", r#"{"name": "Dune", "came": "este"}"#);
    engine.on_gmcp(
        "Room.Actual",
        r#"{"name": "Oasis", "came": "norte", "exits": ["s"]}"#,
    );
    engine.on_gmcp("Room.Fallido", "");
    assert_eq!(
        engine.take_actions(),
        [Action::Entered(2), Action::Entered(3), Action::StopWalk]
    );
    engine.exec(
        "test",
        r#"echo(table.concat(map.path(1), ","))
        echo(tostring(map.here(1)) .. " " .. tostring(map.here(7)))
        echo(map.exit(2, "Oeste").to)"#,
    );
    assert_eq!(
        engine.take_actions(),
        [
            Action::Echo("sur,oeste".into()),
            Action::Entered(1),
            Action::Echo("true false".into()),
            Action::Echo("1".into()),
        ]
    );
}
