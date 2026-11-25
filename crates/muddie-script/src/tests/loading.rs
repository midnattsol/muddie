//! Loading scripts and modules, running them again, and what they keep.

use super::*;

#[test]
fn require_loads_modules_once_from_the_script_folder_only() {
    let dir = std::env::temp_dir().join(format!("muddie-require-test-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("combat")).unwrap();
    std::fs::write(
        dir.join("util.luau"),
        "loads = (loads or 0) + 1\nreturn {double = function(n) return n * 2 end}",
    )
    .unwrap();
    std::fs::write(
        dir.join("combat/init.luau"),
        "return require('util').double(21)",
    )
    .unwrap();
    std::fs::write(dir.join("loop.luau"), "return require('loop')").unwrap();
    std::fs::write(
        dir.join("main.luau"),
        "echo(`{require('util').double(4)} {require('combat')} {require('util') == require('util')} {loads}`)",
    )
    .unwrap();
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.load_file(&dir.join("main.luau"));
    assert_eq!(engine.take_actions(), [Action::Echo("8 42 true 1".into())]);
    for (name, problem) in [
        ("../util", "invalid module name"),
        ("/etc/passwd", "invalid module name"),
        ("missing", "module not found"),
        ("loop", "circular require"),
    ] {
        engine.exec("lua", &format!("require('{name}')"));
        assert!(
            error_text(&engine.take_actions()).contains(problem),
            "{name}"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn remembered_values_survive_a_new_engine() {
    let file = std::env::temp_dir().join(format!("muddie-memory-test-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let mut engine = engine("");
    engine.use_memory_file(file.clone());
    engine.exec(
        "test",
        r#"vars.kills = 41
        vars.friends = {"ana", "luis"}
        vars.config = {speed = "fast"}
        vars.gone = 1
        vars.gone = nil"#,
    );
    assert_eq!(engine.take_actions(), []);
    let mut engine = self::engine("");
    engine.use_memory_file(file.clone());
    engine.exec(
        "test",
        r#"echo(`{vars.kills + 1} {vars.friends[2]} {vars.config.speed} {vars.gone}`)"#,
    );
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("42 luis fast nil".into())]
    );

    std::fs::write(&file, "not json").unwrap();
    let mut engine = self::engine("");
    engine.use_memory_file(file.clone());
    assert!(error_text(&engine.take_actions()).contains("not loaded"));
    // The damaged file is left alone rather than overwritten.
    engine.exec("test", r#"vars.x = 1"#);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "not json");
    std::fs::remove_file(&file).unwrap();
}

/// The scripts shipped as examples must at least load without error.
#[test]
fn example_scripts_load() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/scripts");
    let mut files: Vec<PathBuf> = (std::fs::read_dir(&folder).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "luau")
        })
        .collect();
    files.sort();
    assert!(
        files.len() >= 5,
        "examples not found in {}",
        folder.display()
    );
    for file in files {
        let mut engine = Engine::new(Rc::default()).unwrap();
        engine.load_file(&file);
        // An example does nothing on loading: anything would be an
        // error it reported.
        assert_eq!(engine.take_actions(), [], "{}", file.display());
    }
}

#[test]
fn variables_are_a_table_that_is_kept_and_go_into_commands() {
    let file = std::env::temp_dir().join(format!("muddie-vars-test-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.use_memory_file(file.clone());
    engine.exec(
        "test",
        r#"
        vars.target = "orc"
        vars.kills = (vars.kills or 0) + 2
        macro("f1", "kill $target; say $kills down, $5 left, $nothing")
        local names = {}
        for name, value in vars do table.insert(names, name .. "=" .. tostring(value)) end
        table.sort(names)
        echo(table.concat(names, " ") .. " | " .. expand("$target!"))
        "#,
    );
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("kills=2 target=orc | orc!".into())]
    );
    engine.on_key("f1");
    assert_eq!(
        engine.take_actions(),
        [send("kill orc"), send("say 2 down, $5 left, $nothing")]
    );
    // Into tables and lists, as far as there is something there.
    engine.exec(
        "test",
        r#"
        vars.enemy = {name = "troll", bag = {"axe", "gold"}}
        echo(expand("hit $enemy.name with $enemy.bag.1; loot $enemy.bag; $enemy.none.x"))
        "#,
    );
    let said = engine.take_actions();
    let Action::Echo(said) = &said[0] else {
        panic!("{said:?}")
    };
    assert!(
        said.starts_with("hit troll with axe; loot axe gold; {"),
        "{said}"
    );
    assert!(said.ends_with(".none.x"), "{said}");
    engine.set_variable("enemy", None).unwrap();
    // The editor reads and changes the same values.
    assert_eq!(
        engine.variables()[0],
        ("kills".to_owned(), serde_json::json!(2))
    );
    engine
        .set_variable("target", Some(serde_json::json!("troll")))
        .unwrap();
    engine.set_variable("kills", None).unwrap();
    // They are there the next time.
    let mut later = Engine::new(Rc::default()).unwrap();
    later.use_memory_file(file.clone());
    later.exec(
        "test",
        r#"echo(tostring(vars.target) .. tostring(vars.kills))"#,
    );
    assert_eq!(later.take_actions(), [Action::Echo("trollnil".into())]);
    std::fs::remove_file(&file).unwrap();
}

#[test]
fn a_script_is_run_again_without_the_others_starting_over() {
    let dir = std::env::temp_dir().join(format!("muddie-script-owner-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (keeper, edited) = (dir.join("keeper.luau"), dir.join("edited.luau"));
    // One keeps something it was told; the other is being worked on.
    std::fs::write(
        &keeper,
        r#"canvas = canvas or {}
        on("gmcp", "Map.Canvas", function(data) table.insert(canvas, data.cell) end)
        alias("^count$", function() send("kept " .. #canvas) end)"#,
    )
    .unwrap();
    std::fs::write(
        &edited,
        r#"trigger("orc", function() send("flee v1") end)
        macro("f9", function() after(0.01, function() send("late v1") end) end)
        every(0.01, function() send("tick v1") end)"#,
    )
    .unwrap();
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.load_file(&keeper);
    engine.load_file(&edited);
    engine.on_gmcp("Map.Canvas.Patch", r#"{"cell": "a"}"#);
    engine.on_gmcp("Map.Canvas.Patch", r#"{"cell": "b"}"#);
    // The second is changed and run again by itself.
    std::fs::write(&edited, r#"trigger("orc", function() send("flee v2") end)"#).unwrap();
    engine.reload_file(&edited);
    engine.take_actions();
    // What it made before is gone, what it makes now is there...
    engine.on_line("an orc");
    assert_eq!(engine.take_actions(), [send("flee v2")]);
    assert!(!engine.on_key("f9"));
    engine.tick(Instant::now() + Duration::from_secs(1));
    assert_eq!(engine.take_actions(), []);
    // ...and the first has all it had, and goes on being told.
    engine.on_gmcp("Map.Canvas.Patch", r#"{"cell": "c"}"#);
    engine.on_input("count");
    assert_eq!(engine.take_actions(), [send("kept 3")]);
    // What a script makes later, from what it made, is its own too.
    std::fs::write(
        &edited,
        r#"macro("f9", function() trigger("troll", function() send("run") end) end)"#,
    )
    .unwrap();
    engine.reload_file(&edited);
    assert!(engine.on_key("f9"));
    engine.on_line("a troll");
    assert_eq!(engine.take_actions(), [send("run")]);
    // Stopped, it makes nothing more, and what it showed goes.
    engine.unload_file(&edited);
    engine.on_line("a troll");
    let owner = crate::owner_of(&edited);
    assert_eq!(engine.take_actions(), [Action::Unloaded(owner)]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A name the API no longer has says, called, what does it now.
#[test]
fn a_retired_name_says_what_to_use_instead() {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.exec("old", r#"supports("Comm 1")"#);
    engine.exec("old map", "map.relayout()");
    let errors: Vec<String> = (engine.take_actions().into_iter())
        .filter_map(|action| match action {
            Action::Error(error) => Some(error),
            _ => None,
        })
        .collect();
    assert!(
        errors[0].contains("supports is no longer: use support_gmcp(module, ...)"),
        "{errors:?}"
    );
    assert!(
        errors[1].contains("map.relayout is no longer: use map.tidy(area)"),
        "{errors:?}"
    );
}

/// Nothing a script made, nor the API itself, keeps the runtime once the
/// engine is gone: the map it read is let go, with the file it is kept in.
#[test]
fn an_engine_gone_lets_go_of_its_map() {
    let map = Rc::new(RefCell::new(Map::in_memory()));
    let mut engine = Engine::new(Rc::clone(&map)).unwrap();
    engine.exec(
        "test",
        r#"
        trigger("^x$", function() send("y") end)
        alias("^a$", function() send("b") end)
        every(5, function() send("c") end)
        kept = { panel_size, send }
        "#,
    );
    assert_eq!(engine.take_actions(), [], "setup script failed");
    drop(engine);
    assert_eq!(Rc::strong_count(&map), 1);
}
