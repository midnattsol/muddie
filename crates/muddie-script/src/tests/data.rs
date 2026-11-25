//! What the MUD sends besides text, and what scripts make of it.

use super::*;

#[test]
fn gmcp_data_reaches_handlers_and_is_remembered() {
    let mut engine = engine(
        r#"on("gmcp", "char", function(data, package) echo(`{package}: {data.hp}/{data.maxhp}`) end)
        on("gmcp", "Char.Vitals", function(data)
            gauges.set("HP", {value = data.hp, max = data.maxhp, place = "Vitals/HP", tags = "fight", panel = "Puntos"})
        end)
        on("gmcp", "Room.Info", function(data) echo(`{data.name} {#data.exits} {data.area}`) end)"#,
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
                belongs: Belongs {
                    place: Some("Vitals/HP".into()),
                    tags: vec!["fight".into()],
                },
                panel: "Puntos".into(),
            },
        ]
    );
    engine.on_gmcp(
        "Room.Info",
        r#"{"name": "Plaza", "exits": ["n", "s"], "area": null}"#,
    );
    assert_eq!(engine.take_actions(), [Action::Echo("Plaza 2 nil".into())]);
    // A gauge shown is switched by its place, as a trigger is.
    engine.exec("t", r#"echo(table.concat(gauges.list(), ","))"#);
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("Vitals,Vitals/HP".into())]
    );
    // Packages that only share a prefix are not "under" the handler's.
    engine.on_gmcp("Charms.List", "[]");
    engine.on_gmcp("Core.Ping", "");
    assert_eq!(engine.take_actions(), []);

    engine.exec(
        "lua",
        "echo(`{gmcp.get('char.vitals').hp} {gmcp.get('Nope')} {gmcp.get('Core.Ping')}`)",
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
        r#"on("var", "hp", function(value, name) echo(`{name} {value}`) end)
        on("var", "bag", function(bag) echo(`gold {bag and bag.gold}`); vars.told = true end)
        on("var", "told", function(value) echo(`told {value}`) end)"#,
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
        "on('var', 'hp', function() echo('again') end):remove() vars.hp = 1",
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
            &format!("gauges.set('Efectos', {{kind = 'effects', items = {given}}})"),
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
        r#"gmcp.send("Core.Ping")
        gmcp.send("Core.Supports.Add", {"Char.Items 1"})
        gmcp.send("Char.Login", {name = "ana"})
        gauges.remove("HP")
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
                belongs: Belongs::default(),
                panel: String::new(),
            },
            Action::Walk(42),
        ]
    );
    engine.exec("test", r#"gauges.set("HP", {value = 5})"#);
    assert!(error_text(&engine.take_actions()).contains("both a value and a max"));
}

#[test]
fn msdp_and_mssp_are_available_to_scripts() {
    let mut engine = engine(
        r#"on("msdp", "HEALTH", function(value, name) echo(`{name}={value}`) end)
        on("gmcp", "health", function() echo("wrong kind") end)"#,
    );
    engine.on_msdp("HEALTH", r#""80""#);
    engine.on_msdp("ROOM", r#"{"VNUM":"7","EXITS":{"n":"8"}}"#);
    engine.on_mssp(&[("NAME".into(), "FakeMUD".into())]);
    assert_eq!(engine.take_actions(), [Action::Echo("HEALTH=80".into())]);
    engine.exec(
        "test",
        r#"echo(`{msdp.get("room").EXITS.n} {gmcp.get("room")} {mssp.get().NAME} {mssp.get().PLAYERS}`)
        msdp.send("REPORT", "HEALTH", "MANA")"#,
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
        gmcp.support("Map.Render 1", "Dev 1")
        on("panel", function(size) echo(size.columns .. "x" .. size.rows) end)
        on("connect", function() echo("in") end)
        session.completions("cmds", {"north", "look"})
        session.completions("more", {"west"})
        session.completions("more")
        "#,
    );
    assert_eq!(engine.completions(), ["north", "look"]);
    engine.on_event("panel", r#"{"columns": 40, "rows": 30}"#);
    engine.on_event("connect", "");
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("40x30".into()), Action::Echo("in".into()),]
    );
    // A package of the same name from the server is not the event.
    engine.on_gmcp("panel", r#"{"columns": 1, "rows": 1}"#);
    assert_eq!(engine.take_actions(), []);
    assert_eq!(engine.supports(), ["Map.Render 1", "Dev 1"]);
    // What a script gave to complete goes with it.
    engine.unload("test");
    assert!(engine.completions().is_empty());
}

#[test]
fn the_modules_a_script_asks_for_go_with_it() {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.exec_owned("one", "one", r#"gmcp.support("Comm 1", "Room 1")"#);
    engine.exec_owned(
        "two",
        "two",
        r#"gmcp.support("Room 1") gmcp.support("Room 1")"#,
    );
    assert_eq!(engine.supports(), ["Comm 1", "Room 1"]);
    engine.unload("one");
    assert_eq!(engine.supports(), ["Room 1"]);
    engine.unload("two");
    assert!(engine.supports().is_empty());
}

#[test]
fn what_a_script_sets_for_all_goes_with_it() {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.exec_owned(
        "one",
        "one",
        r#"gmcp.merge("Char.Status") session.separator(false) map.native(false)"#,
    );
    engine.on_input("say a; b");
    assert_eq!(engine.take_actions(), [Action::Send("say a; b".into())]);
    let native = |engine: &mut Engine| {
        engine.exec("ask", r#"send(tostring(map.native()))"#);
        engine.take_actions()
    };
    assert_eq!(native(&mut engine), [Action::Send("false".into())]);
    engine.unload("one");
    engine.on_input("say a; b");
    assert_eq!(
        engine.take_actions(),
        [Action::Send("say a".into()), Action::Send("b".into())]
    );
    assert_eq!(native(&mut engine), [Action::Send("true".into())]);
    // Nor is a package merged any more.
    engine.on_gmcp("Char.Status", r#"{"pe": 5}"#);
    engine.on_gmcp("Char.Status", r#"{"pvs": 7}"#);
    engine.exec("lua", "echo(`{gmcp.get('Char.Status').pe}`)");
    assert_eq!(engine.take_actions(), [Action::Echo("nil".into())]);
}

#[test]
fn sounds_are_requested_with_their_options() {
    let mut engine = engine("");
    engine.exec(
        "test",
        r#"sounds.play("alert.ogg") sounds.play("rain.ogg", {volume = 4, loop = true}) sounds.stop()"#,
    );
    let sound = |id, file: &str, volume, looped| Action::PlaySound {
        id,
        file: file.into(),
        volume,
        looped,
        owner: "test".into(),
    };
    assert_eq!(
        engine.take_actions(),
        [
            sound(1, "alert.ogg", 1.0, false),
            sound(2, "rain.ogg", 1.0, true),
            Action::StopSounds
        ]
    );
}

#[test]
fn the_size_of_a_panel_can_be_asked_for() {
    let mut engine = engine(
        r#"alias("^size (\\S+)$", function(m)
            local size = panels.size(m[1] ~= "map" and m[1] or nil)
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
        r#"gmcp.merge("Char.Status")
        on("gmcp", "Char.Status", function(data) echo(`came {data.pvs.min} {data.pe}`) end)"#,
    );
    engine.on_gmcp(
        "Char.Status",
        r#"{"pvs": {"min": 80, "max": 100}, "pe": 5}"#,
    );
    engine.on_gmcp("Char.Status", r#"{"pvs": {"min": 70}}"#);
    engine.exec(
        "lua",
        "local s = gmcp.get('Char.Status') echo(`{s.pvs.min}/{s.pvs.max} {s.pe}`)",
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
    engine.exec("lua", "echo(`{gmcp.get('Room.Info').exits}`)");
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
    let named = |engine: &Engine| engine.map_actions()[0].label.clone();
    assert_eq!(named(&engine), "Sync");
    crate::set_interface_language("es");
    assert_eq!(named(&engine), "Sincronizar");
    assert!(engine.use_map_action("Sincronizar"));
    assert_eq!(engine.take_actions(), [Action::Echo("synced".into())]);
    crate::set_interface_language("");
}

#[test]
fn handlers_of_data_are_switched_by_their_tags() {
    let mut engine = engine(
        r#"on("gmcp", "Char.Vitals", function(data) echo(`hp {data.hp}`) end, {tags = "fight"})
        on("connect", function() echo("connected") end, {tags = {"fight"}})"#,
    );
    engine.exec("t", r#"echo(table.concat(tags.list(), ","))"#);
    assert_eq!(engine.take_actions(), [Action::Echo("fight".into())]);
    engine.exec("t", r#"tags.disable("fight")"#);
    engine.on_gmcp("Char.Vitals", r#"{"hp": 80}"#);
    engine.on_event("connect", "");
    assert_eq!(engine.take_actions(), []);
    engine.exec("t", r#"tags.enable("fight")"#);
    engine.on_gmcp("Char.Vitals", r#"{"hp": 70}"#);
    engine.on_event("connect", "");
    assert_eq!(
        engine.take_actions(),
        [
            Action::Echo("hp 70".into()),
            Action::Echo("connected".into())
        ]
    );
}

#[test]
fn what_a_script_set_of_the_map_and_the_session_goes_with_it() {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.exec_owned(
        "one",
        "one",
        r#"map.persist(false) map.language("es") panels.set("Mapa", "x") channels.open("ooc")
        map.names({region = true}) map.directions({n = {"norte"}})"#,
    );
    assert!(!engine.persist_map());
    engine.take_actions();
    engine.unload("one");
    assert!(engine.persist_map());
    assert_eq!(
        engine.take_actions(),
        [
            Action::Language(String::new()),
            Action::Panel {
                name: "mapa".into(),
                text: None
            },
            Action::ChannelClosed("ooc".into()),
        ]
    );
}

#[test]
fn a_script_says_how_a_gauge_is_announced() {
    let mut engine = engine("");
    let mut look = |code: &str| {
        engine.exec("t", code);
        engine.take_actions().remove(0)
    };
    for (told, announce) in [
        ("", Announce::Usual),
        (r#", announce = "usual""#, Announce::Usual),
        (r#", announce = "never""#, Announce::Never),
        (r#", announce = "wait""#, Announce::Wait),
        (r#", announce = "lowest""#, Announce::Lowest),
        (r#", announce = "cut""#, Announce::Cut),
    ] {
        match look(&format!(
            r#"gauges.set("HP", {{value = 1, max = 2{told}}})"#
        )) {
            Action::Gauge { look, .. } => assert_eq!(look.announce, announce, "{told}"),
            other => panic!("{other:?}"),
        }
    }
    let wrong = look(r#"gauges.set("HP", {value = 1, max = 2, announce = "shout"})"#);
    assert!(
        matches!(&wrong, Action::Error(said) if said.contains("usual, never, wait, lowest or cut")),
        "{wrong:?}"
    );
}

#[test]
fn a_script_says_what_of_a_channel_is_not_read() {
    let mut engine = engine("");
    let mut look = |code: &str| {
        engine.exec("t", code);
        engine.take_actions().remove(0)
    };
    for (told, quiet) in [("", false), (", {quiet = true}", true)] {
        match look(&format!(r#"channels.say("chat", "hi"{told})"#)) {
            Action::Channel { quiet: said, .. } => assert_eq!(said, quiet),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn scripts_know_whether_a_screen_reader_reads() {
    let mut engine = engine(r#"on("reader", function(on) echo(`now {on} {session.reader}`) end)"#);
    engine.exec("t", "echo(`at first {session.reader}`)");
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("at first false".into())]
    );
    // Told when it changes, and only then.
    assert!(engine.set_screen_reader(true));
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("now true true".into())]
    );
    assert!(!engine.set_screen_reader(true));
    assert!(engine.take_actions().is_empty());
    assert!(engine.set_screen_reader(false));
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("now false false".into())]
    );
}

/// One function listens to everything, switched as a trigger is, and says
/// what there is when it is asked for what there is not.
#[test]
fn everything_is_listened_to_with_one_function() {
    let mut engine = engine(
        r#"on("gmcp", "Char", function(data, name) send(name .. " " .. data.hp) end, {place = "Fight/Life"})
        on("msdp", "HEALTH", function(value, name) send(name .. " " .. value) end, {tags = "fight"})
        on("var", "hp", function(value, name) send(name .. " " .. value) end)
        on("connect", function() send("connected") end)"#,
    );
    let go = |engine: &mut Engine| {
        engine.on_gmcp("Char.Vitals", r#"{"hp": 3}"#);
        engine.on_msdp("HEALTH", r#""4""#);
        engine.exec("t", "vars.hp = (vars.hp or 4) + 1");
        engine.on_event("connect", "");
        engine.take_actions()
    };
    assert_eq!(
        go(&mut engine),
        ["Char.Vitals 3", "HEALTH 4", "hp 5", "connected"].map(send)
    );
    // Switched by its place, a folder above it, or a tag.
    engine.exec("t", r#"triggers.disable("Fight") tags.disable("fight")"#);
    assert_eq!(go(&mut engine), ["hp 6", "connected"].map(send));
    engine.exec(
        "t",
        r#"triggers.enable("Fight") echo(table.concat(triggers.list(), " "))"#,
    );
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("Fight Fight/Life".into())]
    );
    for (code, said) in [
        (
            r#"on("conect", function() end)"#,
            "there is no conect to listen to; there are gmcp, msdp, var, connect, disconnect, reader, panel, mxp",
        ),
        (
            r#"on("gmcp", function() end)"#,
            r#"on("gmcp", ...) needs the name of a package before the function"#,
        ),
        (
            r#"on("connect", "x", function() end)"#,
            r#"on("connect", ...) takes the function next, with no name"#,
        ),
        (r#"on("gmcp", "Char")"#, "on needs a function"),
        (
            r#"on("connect", function() end, {once = true})"#,
            "there is no option once here; there are place, tags",
        ),
        (
            r#"on_gmcp("Char", function() end)"#,
            r#"on_gmcp is no longer: use on("gmcp", package, function)"#,
        ),
    ] {
        engine.exec("t", code);
        match engine.take_actions().as_slice() {
            [Action::Error(error)] => assert!(error.contains(said), "{error}"),
            other => panic!("{code}: {other:?}"),
        }
    }
}

/// A variable is read as it is now, each time: not as it was when the
/// script was loaded, as Luau reads what never changes.
#[test]
fn a_variable_is_read_as_it_is_now() {
    let mut engine = engine("vars.hits = 1");
    engine.exec(
        "t",
        r#"trigger("^x$", function() vars.hits = vars.hits + 1 send(`{vars.hits}`) end)"#,
    );
    for _ in 0..3 {
        engine.on_line("x");
    }
    assert_eq!(engine.take_actions(), ["2", "3", "4"].map(send));
}

/// What a script reads of the session is as it is now; what there is not
/// is an error.
#[test]
fn a_script_is_told_of_the_session() {
    let mut engine = engine(
        r#"alias("^ask$", function()
            send(`{session.connected} {session.host} {session.profile}`)
        end)"#,
    );
    let mut ask = |session: Session| {
        engine.know_session(session);
        engine.on_input("ask");
        engine.take_actions()
    };
    assert_eq!(ask(Session::default()), [send("false  nil")]);
    let now = Session {
        connected: true,
        host: "mud.example.org".into(),
        profile: Some("Ana".into()),
    };
    assert_eq!(ask(now), [send("true mud.example.org Ana")]);
    for (code, said) in [
        ("echo(session.hots)", "there is no session.hots; there are"),
        ("session.host = 'x'", "session.host is asked for, not set"),
        ("gmcp('Char')", "gmcp(...) is no longer: use gmcp.get(...)"),
        (
            "gauges.set('T', {kind = 'text'})",
            "a gauge of the kind text needs a text",
        ),
        ("gauge('T', 1, 2)", "gauge is no longer: use gauges.set"),
        (
            "sounds.play('x', {volumen = 1})",
            "there is no option volumen here; there are volume, loop",
        ),
    ] {
        engine.exec("t", code);
        let actions = engine.take_actions();
        assert!(error_text(&actions).contains(said), "{code}: {actions:?}");
    }
}

/// Words, effects and a light need no numbers.
#[test]
fn a_gauge_takes_what_its_kind_has() {
    let mut engine = engine("");
    let mut level = |given: &str| {
        engine.exec("t", &format!("gauges.set('G', {{{given}}})"));
        match engine.take_actions().remove(0) {
            Action::Gauge { level, .. } => level,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(level("value = 3, max = 4"), Some((3.0, 4.0)));
    assert_eq!(level("kind = 'number', value = 3"), Some((3.0, 0.0)));
    assert_eq!(level("kind = 'light', value = true"), Some((1.0, 1.0)));
    assert_eq!(level("kind = 'light', value = 0"), Some((0.0, 1.0)));
    assert_eq!(level("kind = 'text', text = 'hi'"), Some((0.0, 0.0)));
    assert_eq!(level("kind = 'effects', items = {'a'}"), Some((0.0, 0.0)));
}

#[test]
fn a_sound_is_stopped_by_what_playing_it_gave() {
    let mut engine = engine("");
    engine.exec("t", "rain = sounds.play('rain.ogg', {loop = true})");
    let id = match engine.take_actions().remove(0) {
        Action::PlaySound { id, .. } => id,
        other => panic!("{other:?}"),
    };
    engine.exec("t", "rain:stop()");
    assert_eq!(engine.take_actions(), [Action::StopSound(id)]);
}
