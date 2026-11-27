use super::*;

fn keys(text: &str) -> Press {
    Press::parse(text).unwrap_or_else(|| panic!("{text} are keys"))
}

#[test]
fn keys_are_read_as_written_and_kept_in_lower_case() {
    assert_eq!(keys("Ctrl+Shift+P").stored(), "ctrl+shift+p");
    assert_eq!(keys("alt+UP").stored(), "alt+up");
    assert_eq!(keys("f5").stored(), "f5");
    assert_eq!(keys("numpad8").stored(), "numpad8");
    assert_eq!(keys("shift+numpadup").stored(), "shift+numpadup");
    assert_eq!(keys("ctrl").stored(), "ctrl");
    assert_eq!(keys("ctrl++").stored(), "ctrl+plus");
    assert_eq!(keys("option+esc").stored(), "alt+escape");
    assert_eq!(keys("pgdn"), keys("pagedown"));
    for wrong in ["", "ctrl+nothing", "hyper+k", "numpad99", "shift+ctrl"] {
        assert!(Press::parse(wrong).is_none(), "{wrong}");
    }
    // Read back as kept, they are the same keys.
    for text in [
        "ctrl+alt+shift+k",
        "alt+1",
        "space",
        "numpadenter",
        "shift+f10",
    ] {
        assert_eq!(keys(&keys(text).stored()), keys(text));
    }
    assert_eq!(
        keys("alt+up").written(),
        if MAC { "Option+↑" } else { "Alt+↑" }
    );
    assert_eq!(keys("numpadadd").written(), "Keypad +");
}

#[test]
fn keys_are_taken_where_they_do_not_get_in_the_way() {
    // Anywhere, keys that type or move the cursor are not taken.
    assert_eq!(PALETTE.check(&keys("p")), Err(Wrong::Types));
    assert_eq!(PALETTE.check(&keys("shift+p")), Err(Wrong::Types));
    assert_eq!(PALETTE.check(&keys("up")), Err(Wrong::Types));
    assert_eq!(PALETTE.check(&keys("ctrl+p")), Ok(()));
    assert_eq!(PALETTE.check(&keys("f9")), Ok(()));
    assert_eq!(PALETTE.check(&keys("numpad5")), Ok(()));
    // Nor those that go through the window, anywhere.
    assert_eq!(PALETTE.check(&keys("tab")), Err(Wrong::Kept));
    assert_eq!(MAP_NORTH.check(&keys("escape")), Err(Wrong::Kept));
    assert_eq!(MAP_NORTH.check(&keys("shift+f10")), Err(Wrong::Kept));
    // The map, explored, has the keyboard: any other key will do.
    assert_eq!(MAP_NORTH.check(&keys("k")), Ok(()));
    assert_eq!(MAP_NORTH.check(&keys("shift+up")), Ok(()));
    // The command line: keys that do not type, but Enter sends.
    assert_eq!(HISTORY_PREVIOUS.check(&keys("pageup")), Ok(()));
    assert_eq!(HISTORY_PREVIOUS.check(&keys("k")), Err(Wrong::Types));
    assert_eq!(HISTORY_PREVIOUS.check(&keys("enter")), Err(Wrong::Kept));
    // Held alone, only to silence.
    assert_eq!(HUSH.check(&keys("shift")), Ok(()));
    assert_eq!(PALETTE.check(&keys("ctrl")), Err(Wrong::Alone));
}

#[test]
fn keys_chosen_in_the_settings_and_by_scripts_go_before_their_own() {
    configure(&BTreeMap::from([
        ("palette".into(), "ctrl+shift+p, f9".into()),
        ("hush".into(), String::new()),
    ]));
    assert_eq!(PALETTE.keys(), [keys("ctrl+shift+p"), keys("f9")]);
    assert!(PALETTE.is_changed());
    assert!(HUSH.keys().is_empty());
    assert_eq!(GAUGES.keys(), GAUGES.defaults());
    // A script's go first, and take its keys from what they would meet.
    let gauges = GAUGES.keys()[0].stored();
    script(vec![
        ("keys.luau".into(), "palette".into(), vec!["f2".into()]),
        ("keys.luau".into(), "messages".into(), vec![gauges.clone()]),
    ]);
    assert_eq!(PALETTE.keys(), [keys("f2")]);
    assert_eq!(PALETTE.chosen_keys(), [keys("ctrl+shift+p"), keys("f9")]);
    assert_eq!(PALETTE.script().as_deref(), Some("keys.luau"));
    assert_eq!(MESSAGES.keys(), [keys(&gauges)]);
    assert!(GAUGES.keys().is_empty(), "{:?}", GAUGES.keys());
    // Not from the map, which goes first while it is explored.
    script(vec![(
        "keys.luau".into(),
        "explore".into(),
        vec!["f1".into()],
    )]);
    assert_eq!(MAP_HELP.keys(), [keys("f1")]);
    assert!(KEYS_HELP.keys().is_empty());
    // Gone with the script, the settings' are back.
    script(Vec::new());
    assert_eq!(PALETTE.keys(), [keys("ctrl+shift+p"), keys("f9")]);
    assert_eq!(PALETTE.script(), None);
    configure(&BTreeMap::new());
    assert_eq!(PALETTE.keys(), PALETTE.defaults());
}

#[test]
fn a_key_of_the_keypad_is_told_from_what_egui_makes_of_it() {
    let event = |key, physical| egui::Event::Key {
        key,
        physical_key: Some(physical),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let mut input = egui::InputState::default();
    // Keypad 8 with Num Lock on: egui makes an 8 of it, and types one.
    input.events = vec![event(Key::Num8, Key::Num8), egui::Event::Text("8".into())];
    keypad_pressed(Modifiers::NONE, ["numpad8"]);
    // The 8 of the main block was not pressed.
    assert!(!take(&mut input, &keys("8")));
    assert!(take(&mut input, &keys("numpad8")));
    // Taken, it types nothing.
    assert!(input.events.is_empty());
    assert!(!take(&mut input, &keys("numpad8")));
    // What is left goes to the macros.
    keypad_pressed(Modifiers::NONE, ["numpad2"]);
    assert_eq!(keypad_left(), [(Held::NONE, "numpad2")]);
    assert!(keypad_left().is_empty());
}

#[test]
fn every_shortcut_has_a_name_of_its_own_and_its_keys_can_be_its() {
    let mut names: Vec<&str> = ALL.iter().map(|shortcut| shortcut.id).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), ALL.len());
    for shortcut in ALL {
        for keys in shortcut.defaults() {
            assert_eq!(shortcut.check(keys), Ok(()), "{}: {keys:?}", shortcut.id);
            assert_eq!(Press::parse(&keys.stored()).as_ref(), Some(keys));
        }
        // None of its own keys are another's that it would meet.
        for other in ALL.iter().filter(|other| other.id != shortcut.id) {
            if other.place.meets(shortcut.place) {
                let shared: Vec<&Press> = (shortcut.defaults().iter())
                    .filter(|keys| other.defaults().contains(keys))
                    .collect();
                assert!(
                    shared.is_empty(),
                    "{} and {}: {shared:?}",
                    shortcut.id,
                    other.id
                );
            }
        }
    }
}

/// Prints every shortcut as a row of a table, for the reference:
/// `cargo test -p muddie shortcut_rows -- --ignored --nocapture`.
#[test]
#[ignore = "prints the table of shortcuts"]
fn shortcut_rows() {
    for shortcut in ALL {
        let keys: Vec<String> = shortcut.defaults().iter().map(Press::stored).collect();
        println!(
            "ROW\t{}\t{}\t{}\t{}",
            shortcut.id,
            shortcut.place.id(),
            keys.join(", "),
            shortcut.about()
        );
    }
}
