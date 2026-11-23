//! What the MUD sends besides text, and what scripts make of it.

use super::*;

#[test]
fn gmcp_data_reaches_handlers_and_is_remembered() {
    let mut engine = engine(
        r#"on_gmcp("char", function(data, package) echo(`{package}: {data.hp}/{data.maxhp}`) end)
        on_gmcp("Char.Vitals", function(data) gauge("HP", data.hp, data.maxhp) end)
        on_gmcp("Room.Info", function(data) echo(`{data.name} {#data.exits} {data.area}`) end)"#,
    );
    engine.on_gmcp("Char.Vitals", r#"{"hp": 80, "maxhp": 100}"#);
    assert_eq!(
        engine.take_actions(),
        [
            Action::Echo("Char.Vitals: 80/100".into()),
            Action::Gauge {
                name: "HP".into(),
                level: Some((80.0, 100.0)),
                look: GaugeLook::default(),
                owner: "test".into(),
            },
        ]
    );
    engine.on_gmcp(
        "Room.Info",
        r#"{"name": "Plaza", "exits": ["n", "s"], "area": null}"#,
    );
    assert_eq!(engine.take_actions(), [Action::Echo("Plaza 2 nil".into())]);
    // Packages that only share a prefix are not "under" the handler's.
    engine.on_gmcp("Charms.List", "[]");
    engine.on_gmcp("Core.Ping", "");
    assert_eq!(engine.take_actions(), []);

    engine.exec(
        "lua",
        "echo(`{gmcp('char.vitals').hp} {gmcp('Nope')} {gmcp('Core.Ping')}`)",
    );
    assert_eq!(engine.take_actions(), [Action::Echo("80 nil nil".into())]);
    engine.on_gmcp("Char.Vitals", "{not json");
    assert!(error_text(&engine.take_actions()).contains("bad data for Char.Vitals"));
}

/// A variable is followed as what the MUD reports is: its handlers are told
/// once the script that changed it is done, with its value as it is then,
/// whoever changed it.
#[test]
fn variables_are_followed_when_they_change() {
    let mut engine = engine(
        r#"on_var("hp", function(value, name) echo(`{name} {value}`) end)
        on_var("bag", function(bag) echo(`gold {bag and bag.gold}`); vars.told = true end)
        on_var("told", function(value) echo(`told {value}`) end)"#,
    );
    // Twice in one script: told once, of the last.
    engine.exec("t", "vars.hp = 10 vars.hp = 12 vars.other = 1");
    assert_eq!(engine.take_actions(), [Action::Echo("hp 12".into())]);
    // A handler that changes a variable has it told in turn.
    engine.exec("t", "vars.bag = {gold = 5}");
    assert_eq!(
        engine.take_actions(),
        [
            Action::Echo("gold 5".into()),
            Action::Echo("told true".into())
        ]
    );
    // Changed in the editor, forgotten too.
    engine
        .set_variable("hp", Some(serde_json::json!(30)))
        .unwrap();
    engine.set_variable("hp", None).unwrap();
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("hp 30".into()), Action::Echo("hp nil".into())]
    );
    // Taken away, it is told no more.
    engine.exec(
        "t",
        "cancel(on_var('hp', function() echo('again') end)) vars.hp = 1",
    );
    assert_eq!(engine.take_actions(), [Action::Echo("hp 1".into())]);
}

/// A gauge of effects is given them as a script or a MUD writes them: a
/// list in its order, or a table by name in the order of the names, each
/// a name or a table; one that is false is not on.
#[test]
fn a_gauge_of_effects_takes_them_as_lists_or_tables() {
    let mut engine = engine("");
    let items = |engine: &mut Engine, given: &str| -> Vec<(String, String, String)> {
        engine.exec(
            "t",
            &format!("gauge('Efectos', 0, 0, {{kind = 'effects', items = {given}}})"),
        );
        match engine.take_actions().as_slice() {
            [Action::Gauge { look, .. }] => (look.items.iter())
                .map(|item| (item.name.clone(), item.about.clone(), item.colour.clone()))
                .collect(),
            other => panic!("{other:?}"),
        }
    };
    let it = |name: &str, about: &str, colour: &str| {
        (name.to_owned(), about.to_owned(), colour.to_owned())
    };
    assert_eq!(
        items(
            &mut engine,
            r#"{"Ciego", {name = "Veneno", about = "1 de daño", colour = "red"}}"#
        ),
        [it("Ciego", "", ""), it("Veneno", "1 de daño", "red")]
    );
    assert_eq!(
        items(
            &mut engine,
            r#"{prisa = true, lento = false, bendito = {about = "+2"}}"#
        ),
        [it("bendito", "+2", ""), it("prisa", "", "")]
    );
    assert_eq!(items(&mut engine, "{}"), []);
}

#[test]
fn scripts_can_send_gmcp_and_drive_panels() {
    let mut engine = engine("");
    engine.exec(
        "test",
        r#"send_gmcp("Core.Ping")
        send_gmcp("Core.Supports.Add", {"Char.Items 1"})
        send_gmcp("Char.Login", {name = "ana"})
        gauge("HP")
        chat("Ana: hola")
        map.go(42)"#,
    );
    let gmcp = |package: &str, data: &str| Action::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    assert_eq!(
        engine.take_actions(),
        [
            gmcp("Core.Ping", ""),
            gmcp("Core.Supports.Add", r#"["Char.Items 1"]"#),
            gmcp("Char.Login", r#"{"name":"ana"}"#),
            Action::Gauge {
                name: "HP".into(),
                level: None,
                look: GaugeLook::default(),
                owner: "test".into(),
            },
            Action::Chat("Ana: hola".into()),
            Action::Walk(42),
        ]
    );
    engine.exec("test", r#"gauge("HP", 5)"#);
    assert!(error_text(&engine.take_actions()).contains("both a value and a max"));
}

#[test]
fn msdp_and_mssp_are_available_to_scripts() {
    let mut engine = engine(
        r#"on_msdp("HEALTH", function(value, name) echo(`{name}={value}`) end)
        on_gmcp("health", function() echo("wrong kind") end)"#,
    );
    engine.on_msdp("HEALTH", r#""80""#);
    engine.on_msdp("ROOM", r#"{"VNUM":"7","EXITS":{"n":"8"}}"#);
    engine.on_mssp(&[("NAME".into(), "FakeMUD".into())]);
    assert_eq!(engine.take_actions(), [Action::Echo("HEALTH=80".into())]);
    engine.exec(
        "test",
        r#"echo(`{msdp("room").EXITS.n} {gmcp("room")} {mssp().NAME} {mssp().PLAYERS}`)
        send_msdp("REPORT", "HEALTH", "MANA")"#,
    );
    assert_eq!(
        engine.take_actions(),
        [
            Action::Echo("8 nil FakeMUD nil".into()),
            Action::Msdp {
                variable: "REPORT".into(),
                values: vec!["HEALTH".into(), "MANA".into()]
            },
        ]
    );
}

#[test]
fn client_events_modules_and_completions() {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.exec(
        "test",
        r#"
        support_gmcp("Map.Render 1", "Dev 1")
        on_event("panel", function(size) echo(size.columns .. "x" .. size.rows) end)
        on_event("connect", function() echo("in") end)
        completions("cmds", {"north", "look"})
        completions("cmds")
        "#,
    );
    engine.on_event("panel", r#"{"columns": 40, "rows": 30}"#);
    engine.on_event("connect", "");
    assert_eq!(
        engine.take_actions(),
        [
            Action::Completions {
                name: "cmds".into(),
                words: vec!["north".into(), "look".into()]
            },
            Action::Completions {
                name: "cmds".into(),
                words: Vec::new()
            },
            Action::Echo("40x30".into()),
            Action::Echo("in".into()),
        ]
    );
    // A package of the same name from the server is not the event.
    engine.on_gmcp("panel", r#"{"columns": 1, "rows": 1}"#);
    assert_eq!(engine.take_actions(), []);
    assert_eq!(engine.supports(), ["Map.Render 1", "Dev 1"]);
}

#[test]
fn the_modules_a_script_asks_for_go_with_it() {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.exec_owned("one", "one", r#"support_gmcp("Comm 1", "Room 1")"#);
    engine.exec_owned(
        "two",
        "two",
        r#"support_gmcp("Room 1") support_gmcp("Room 1")"#,
    );
    assert_eq!(engine.supports(), ["Comm 1", "Room 1"]);
    engine.unload("one");
    assert_eq!(engine.supports(), ["Room 1"]);
    engine.unload("two");
    assert!(engine.supports().is_empty());
}

#[test]
fn sounds_are_requested_with_their_options() {
    let mut engine = engine("");
    engine.exec(
        "test",
        r#"play("alert.ogg") play("rain.ogg", {volume = 4, loop = true}) stop_sounds()"#,
    );
    let sound = |file: &str, volume, looped| Action::PlaySound {
        file: file.into(),
        volume,
        looped,
    };
    assert_eq!(
        engine.take_actions(),
        [
            sound("alert.ogg", 1.0, false),
            sound("rain.ogg", 1.0, true),
            Action::StopSounds
        ]
    );
}

#[test]
fn the_size_of_a_panel_can_be_asked_for() {
    let mut engine = engine(
        r#"alias("^size (\\S+)$", function(m)
            local size = panel_size(m[1] ~= "map" and m[1] or nil)
            send(size and (size.columns .. "x" .. size.rows) or "none")
        end)"#,
    );
    engine.on_input("size map");
    engine.set_panel_size("Map", 42, 20);
    engine.set_panel_size("Radar", 12, 6);
    engine.on_input("size map");
    engine.on_input("size radar");
    engine.on_input("size other");
    assert_eq!(
        engine.take_actions(),
        [send("none"), send("42x20"), send("12x6"), send("none")]
    );
}

#[test]
fn a_package_that_sends_only_what_changed_is_merged_when_asked() {
    let mut engine = engine(
        r#"merge_gmcp("Char.Status")
        on_gmcp("Char.Status", function(data) echo(`came {data.pvs.min} {data.pe}`) end)"#,
    );
    engine.on_gmcp(
        "Char.Status",
        r#"{"pvs": {"min": 80, "max": 100}, "pe": 5}"#,
    );
    engine.on_gmcp("Char.Status", r#"{"pvs": {"min": 70}}"#);
    engine.exec(
        "lua",
        "local s = gmcp('Char.Status') echo(`{s.pvs.min}/{s.pvs.max} {s.pe}`)",
    );
    assert_eq!(
        engine.take_actions(),
        [
            Action::Echo("came 80 5".into()),
            // Handlers get what came.
            Action::Echo("came 70 nil".into()),
            Action::Echo("70/100 5".into()),
        ]
    );
    // Others are as they come.
    engine.on_gmcp("Room.Info", r#"{"exits": ["n", "s"]}"#);
    engine.on_gmcp("Room.Info", r#"{"name": "Plaza"}"#);
    engine.exec("lua", "echo(`{gmcp('Room.Info').exits}`)");
    assert_eq!(engine.take_actions(), [Action::Echo("nil".into())]);
}

#[test]
fn scripts_show_their_text_in_the_language_of_the_interface() {
    let mut engine = engine("");
    let shown = |engine: &mut Engine, code: &str| {
        crate::set_interface_language(code);
        engine.exec(
            "test",
            r#"echo(tr({en = "Map", es = "Mapa"}))
            echo(tr({fr = "Carte", de = "Karte"}))
            echo(tr("Same"))"#,
        );
        engine.take_actions()
    };
    let echoed = |texts: [&str; 3]| texts.map(|text| Action::Echo(text.into())).to_vec();
    assert_eq!(shown(&mut engine, "es"), echoed(["Mapa", "Karte", "Same"]));
    // Of a country's language, the language's; else English; else the
    // first by code.
    assert_eq!(
        shown(&mut engine, "es_AR"),
        echoed(["Mapa", "Karte", "Same"])
    );
    assert_eq!(
        shown(&mut engine, "pt_BR"),
        echoed(["Map", "Karte", "Same"])
    );
    assert_eq!(shown(&mut engine, "fr"), echoed(["Map", "Carte", "Same"]));
    // What a script offers to do with the map is named so too, in the
    // language the interface is in when shown, and chosen by that name.
    engine.exec(
        "test",
        r#"map.action({en = "Sync", es = "Sincronizar"}, function() echo("synced") end)"#,
    );
    engine.take_actions();
    let named = |engine: &Engine| engine.map_tools()[0].label.clone();
    assert_eq!(named(&engine), "Sync");
    crate::set_interface_language("es");
    assert_eq!(named(&engine), "Sincronizar");
    assert!(engine.use_map_tool("Sincronizar"));
    assert_eq!(engine.take_actions(), [Action::Echo("synced".into())]);
    crate::set_interface_language("");
}
