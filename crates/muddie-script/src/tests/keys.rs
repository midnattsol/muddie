//! The keys of the client's shortcuts, read and chosen by scripts.

use super::*;

/// Reads keys as a client might: any word but "nothing", in lower case;
/// on the map, any; elsewhere, held with ctrl or a function key.
fn check(name: &str, key: &str) -> Result<String, String> {
    let key = key.to_lowercase();
    match () {
        _ if key.contains("nothing") => Err("is not a key there is".into()),
        _ if name.starts_with("map_") || key.contains("ctrl") || key.starts_with('f') => Ok(key),
        _ => Err("would type".into()),
    }
}

fn known(name: &str, place: &str, keys: &[&str]) -> KnownShortcut {
    KnownShortcut {
        name: name.into(),
        place: place.into(),
        about: format!("does {name}"),
        keys: keys.iter().map(|key| (*key).to_owned()).collect(),
    }
}

fn with_shortcuts() -> Engine {
    let mut engine = Engine::new(Rc::default()).unwrap();
    engine.know_shortcuts(
        vec![
            known("explore", "everywhere", &["alt+e"]),
            known("map_north", "map", &["up", "numpad8"]),
        ],
        check,
    );
    engine
}

#[test]
fn a_script_reads_and_chooses_the_keys_of_shortcuts() {
    let mut engine = with_shortcuts();
    engine.exec(
        "keys.luau",
        r#"echo(table.concat(shortcuts.map_north, " "))
        shortcuts.explore = "F9"
        shortcuts.map_north = {"k", "Up"}
        echo(shortcuts.explore[1])
        for _, shortcut in shortcuts.all() do
            echo(`{shortcut.name} {shortcut.place} {shortcut.about}: {table.concat(shortcut.keys, ", ")}`)
        end"#,
    );
    assert_eq!(
        engine.take_actions(),
        [
            Action::Echo("up numpad8".into()),
            Action::Echo("f9".into()),
            Action::Echo("explore everywhere does explore: f9".into()),
            Action::Echo("map_north map does map_north: k, up".into()),
        ]
    );
    let (changed, chosen) = engine.shortcuts();
    assert_eq!(changed, 2);
    assert_eq!(
        chosen,
        [
            ("keys.luau".into(), "explore".into(), vec!["f9".into()]),
            (
                "keys.luau".into(),
                "map_north".into(),
                vec!["k".into(), "up".into()]
            ),
        ]
    );
    // False leaves it without keys; nil gives it back those it had.
    engine.exec(
        "keys.luau",
        "shortcuts.explore = false shortcuts.map_north = nil",
    );
    assert_eq!(
        engine.shortcuts().1,
        [("keys.luau".into(), "explore".into(), Vec::new())]
    );
    engine.exec(
        "other.luau",
        "echo(#shortcuts.explore) echo(shortcuts.map_north[1])",
    );
    assert_eq!(
        engine.take_actions(),
        [Action::Echo("0".into()), Action::Echo("up".into())]
    );
    // What a script chose goes with it.
    engine.unload("keys.luau");
    assert_eq!(engine.shortcuts(), (5, Vec::new()));
}

#[test]
fn what_is_wrong_with_keys_chosen_is_said() {
    let mut engine = with_shortcuts();
    for (code, said) in [
        (
            "shortcuts.explorer = 'f9'",
            "there is no shortcut explorer; there are explore, map_north",
        ),
        (
            "local _ = shortcuts.nothing",
            "there is no shortcut nothing",
        ),
        ("shortcuts.explore = 'p'", "shortcuts.explore: p would type"),
        (
            "shortcuts.map_north = {'k', 'nothing'}",
            "shortcuts.map_north: nothing is not a key",
        ),
        (
            "shortcuts.explore = 5",
            "is a key, a list of keys, false for none or nil, not number",
        ),
        (
            "shortcuts.explore = {true}",
            "shortcuts.explore is given keys as texts",
        ),
    ] {
        engine.exec("keys.luau", code);
        let actions = engine.take_actions();
        let error = error_text(&actions);
        assert!(error.contains(said), "{code}: {error}");
    }
    // Nothing wrong was kept.
    assert_eq!(engine.shortcuts().1, []);
}
