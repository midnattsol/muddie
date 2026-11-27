//! What opens over the game, met with the keyboard as the rest is: the menus,
//! the palette and the small windows. Each is gone through with Tab both ways,
//! and with the arrows a menu is: what is met has a name and a role, is not
//! only for the pointer and is not said twice; and what each should have is
//! met.

use super::*;

/// What is met going through with `key`, held with `modifiers`, as many
/// times as there are stops: the name of each, and what is wrong with it.
fn met(
    harness: &mut Harness<'_, App>,
    view: &str,
    key: Key,
    modifiers: Modifiers,
) -> (Vec<String>, Vec<String>) {
    let (mut names, mut problems, mut seen) = (Vec::new(), Vec::new(), Vec::new());
    // From where the keyboard is.
    for press in 0..60 {
        if press > 0 {
            harness.key_press_modifiers(modifiers, key);
            harness.run_steps(3);
        }
        let Some((id, role, name, about)) = said(harness) else {
            problems.push(format!("{view}: the keyboard went nowhere"));
            break;
        };
        if seen.contains(&id) {
            break;
        }
        seen.push(id);
        if name.trim().is_empty() {
            problems.push(format!("{view}: a stop with no name, a {role:?}"));
        }
        if matches!(role, Role::Splitter | Role::Image | Role::Unknown) {
            problems.push(format!("{view}: a stop at a {role:?}, {name:?}"));
        }
        if !about.is_empty() && name.contains(&about) {
            problems.push(format!("{view}: {name:?} is also said to be {about:?}"));
        }
        names.push(name);
    }
    (names, problems)
}

/// Goes through what is open both ways with Tab, and says what is wrong, and
/// whether `wanted` was all met.
fn through(harness: &mut Harness<'_, App>, view: &str, wanted: &[&str]) -> Vec<String> {
    let (on, mut problems) = met(harness, view, Key::Tab, Modifiers::NONE);
    let (back, wrong) = met(
        harness,
        &format!("{view}, back"),
        Key::Tab,
        Modifiers::SHIFT,
    );
    problems.extend(wrong);
    for name in wanted {
        for (way, names) in [("on", &on), ("back", &back)] {
            if !names.iter().any(|met| met.contains(name)) {
                problems.push(format!("{view}: {name:?} not met going {way}: {names:?}"));
            }
        }
    }
    problems
}

/// Exploring the map, from the player's room, with a room beside it chosen.
fn exploring(harness: &mut Harness<'_, App>) {
    to_the_line(harness);
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::E);
    harness.run_steps(3);
    harness.key_press(Key::ArrowRight);
    harness.run_steps(3);
}

#[test]
fn the_menus_are_gone_through_with_the_keys() {
    let mut harness = playing();
    let mut problems = Vec::new();
    // The menu of a room, opened from the keys.
    exploring(&mut harness);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(3);
    let room = [
        "Go here",
        "The way there",
        "I am here",
        "Mark…",
        "Note…",
        "Keep routes off",
        "Appearance…",
        "Tidy this area",
    ];
    for (key, view) in [
        (Key::ArrowDown, "room menu, down"),
        (Key::ArrowUp, "room menu, up"),
    ] {
        let (names, wrong) = met(&mut harness, view, key, Modifiers::NONE);
        problems.extend(wrong);
        for name in room {
            if !names.iter().any(|met| met == name) {
                problems.push(format!("{view}: {name:?} not met: {names:?}"));
            }
        }
    }
    problems.extend(through(&mut harness, "room menu", &room));
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(
        map_widget(&harness).unwrap().is_focused(),
        "back to the map"
    );
    // The menu of a panel, from its button.
    to_the_line(&mut harness);
    harness.get_by_label("Map options").focus();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let panel = [
        "Rename…",
        "Move in Layout…",
        "Map appearance…",
        "Hide",
        "Remove",
    ];
    let (names, wrong) = met(&mut harness, "panel menu", Key::ArrowDown, Modifiers::NONE);
    problems.extend(wrong);
    for name in panel {
        if !names.iter().any(|met| met.starts_with(name)) {
            problems.push(format!("panel menu: {name:?} not met: {names:?}"));
        }
    }
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_palette_and_the_small_windows_are_gone_through_with_tab() {
    let mut harness = playing();
    let mut problems = Vec::new();
    // The palette, as it opens and with a word typed.
    to_the_line(&mut harness);
    let (held, key) = crate::shortcuts::PALETTE.first();
    harness.key_press_modifiers(held, key);
    harness.run_steps(3);
    problems.extend(through(&mut harness, "palette", &["Palette"]));
    harness.state_mut().tab_mut().palette.query = "map".into();
    harness.run_steps(3);
    problems.extend(through(&mut harness, "palette, map", &["Palette"]));
    // What it found is gone through with the arrows, each said, the keyboard
    // staying in the field.
    let mut found = Vec::new();
    for _ in 0..3 {
        harness.state_mut().tab_mut().voice.said.clear();
        harness.key_press(Key::ArrowDown);
        harness.run_steps(3);
        found.extend(harness.state().tab().voice.said.clone());
    }
    found.dedup();
    let (_, _, field, _) = said(&harness).unwrap_or_default();
    if found.len() != 3 || field != "Palette" {
        problems.push(format!(
            "palette: what it found is not gone through: {found:?}, at {field:?}"
        ));
    }
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    // Marking and noting a room, over the map.
    for (shortcut, name) in [
        (crate::shortcuts::MAP_MARK, "Mark"),
        (crate::shortcuts::MAP_NOTE, "Note"),
    ] {
        exploring(&mut harness);
        let (held, key) = shortcut.first();
        harness.key_press_modifiers(held, key);
        harness.run_steps(3);
        let (id, role, said, _) = said(&harness).unwrap_or_default();
        if role != Role::TextInput || !said.contains(name) {
            problems.push(format!(
                "{name}: the keyboard is at a {role:?}, {said:?} ({id})"
            ));
        }
        harness.key_press(Key::Escape);
        harness.run_steps(3);
        harness.key_press(Key::Escape);
        harness.run_steps(3);
    }
    // The look of a room and of the map.
    let app = harness.state_mut().tab_mut();
    let here = app.map.borrow().current().map(|room| room.id).unwrap();
    app.open_look(here);
    harness.run_steps(3);
    problems.extend(through(&mut harness, "room's appearance", &["Close"]));
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    harness.state_mut().tab_mut().open_map_appearance();
    harness.run_steps(3);
    problems.extend(through(&mut harness, "map appearance", &["Close"]));
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    // A gauge of the editor, opened over the game.
    let app = harness.state_mut().tab_mut();
    app.rules.gauges.push(crate::rules::Gauge {
        name: "Vigour".into(),
        value: "Told.value".into(),
        max: "10".into(),
        ..Default::default()
    });
    app.run_rules();
    app.scripts.on_gmcp("Told", r#"{"value": 7}"#);
    app.run_actions();
    let key = app.gauges_said();
    assert!(key.contains("Vigour"), "{key}");
    app.edit_gauge_here("Vigour");
    harness.run_steps(4);
    problems.extend(through(
        &mut harness,
        "gauge over the game",
        &["Name", "Value", "Read aloud", "Open in the editor", "Close"],
    ));
    harness.key_press(Key::Escape);
    harness.run_steps(3);
    // Naming a panel again, in its header.
    let app = harness.state_mut().tab_mut();
    let map = app
        .layout
        .panels
        .iter()
        .find(|panel| panel.kind == layout::Kind::Map)
        .unwrap()
        .id;
    app.rename_panel(map);
    harness.run_steps(3);
    let (_, role, said, _) = said(&harness).unwrap_or_default();
    if role != Role::TextInput || said != "Name of the panel" {
        problems.push(format!("renaming: the keyboard is at a {role:?}, {said:?}"));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
