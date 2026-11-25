//! The editor's folders moved by dragging and by the keys, and the menu of a line.

use super::*;

/// Drags from one place to another, and lets go there.
fn drag(harness: &mut Harness<'_, App>, from: egui::Pos2, to: egui::Pos2) {
    pointer(harness, from, Some(true));
    pointer(harness, from + egui::vec2(0.0, 12.0), None);
    pointer(harness, to, None);
    pointer(harness, to, Some(false));
}

/// Aliases in a folder in a folder, and a folder beside it.
fn nested_folders<'a>() -> Harness<'a, App> {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 900.0));
    let app = harness.state_mut().tab_mut();
    for (name, folder) in [
        ("Alpha", "Travel/Roads"),
        ("Beta", "Travel"),
        ("Gamma", "Chat"),
    ] {
        app.rules.aliases.push(alias(&name.to_lowercase(), |rule| {
            rule.name = name.into();
            rule.folder = folder.into();
        }));
    }
    app.open_automation(automation::ALIASES);
    harness.run_steps(3);
    harness
}

fn line_of(harness: &Harness<'_, App>, name: &str) -> egui::Rect {
    harness
        .get_by_role_and_label(egui::accesskit::Role::TreeItem, name)
        .rect()
}

/// A folder in a folder is dragged out of every folder onto the strip
/// that is shown for it over the list; let go out of the list, or onto
/// itself, nothing moves.
#[test]
fn a_nested_folder_is_dragged_to_the_top_level() {
    let mut harness = nested_folders();
    let folders = |harness: &Harness<'_, App>| harness.state().tab().section_folders(Kind::Alias);
    let roads = line_of(&harness, "Roads").center();
    // Let go over the details: nowhere, nothing moves.
    drag(&mut harness, roads, egui::pos2(1000.0, 400.0));
    assert!(folders(&harness).contains(&"Travel/Roads".to_owned()));
    // Onto itself: nothing either.
    drag(&mut harness, roads, roads + egui::vec2(4.0, 2.0));
    assert!(folders(&harness).contains(&"Travel/Roads".to_owned()));
    // Held, the strip to the top level is shown; let go on it, it is out.
    pointer(&mut harness, roads, Some(true));
    pointer(&mut harness, roads + egui::vec2(0.0, 12.0), None);
    // Over the toolbar, where the search is.
    let strip = harness.get_by_label("Search").rect().center();
    pointer(&mut harness, strip, None);
    pointer(&mut harness, strip, Some(false));
    assert!(
        folders(&harness).contains(&"Roads".to_owned()),
        "{:?}",
        folders(&harness)
    );
    let app = harness.state().tab();
    assert_eq!(app.editor.draft.aliases[0].folder, "Roads");
    assert_eq!(app.editor.folder.as_deref(), Some("Roads"));
}

/// Let go on the top edge of a folder, a folder goes before it, at its
/// level; and onto its middle, into it.
#[test]
fn a_folder_dragged_to_a_folders_edge_goes_beside_it() {
    let mut harness = nested_folders();
    let folders = |harness: &Harness<'_, App>| harness.state().tab().section_folders(Kind::Alias);
    let roads = line_of(&harness, "Roads").center();
    let chat = line_of(&harness, "Chat");
    drag(
        &mut harness,
        roads,
        chat.center_top() + egui::vec2(0.0, 2.0),
    );
    assert_eq!(folders(&harness), ["Roads", "Chat", "Travel"]);
    let roads = line_of(&harness, "Roads").center();
    let travel = line_of(&harness, "Travel").center();
    drag(&mut harness, roads, travel);
    let now = folders(&harness);
    assert!(
        now.contains(&"Travel/Roads".to_owned()) && !now.contains(&"Roads".to_owned()),
        "{now:?}"
    );
}

/// By the keys, a folder goes up and down among those beside it, and out
/// of the folder it is in, which is said; and from its menu, into another.
#[test]
fn folders_are_moved_by_the_keys() {
    let mut harness = nested_folders();
    let folders = |harness: &Harness<'_, App>| harness.state().tab().section_folders(Kind::Alias);
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    app.choose_folder("Travel/Roads");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(egui::accesskit::Role::TreeItem, "Roads")
        .focus();
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    harness.run_steps(2);
    assert!(folders(&harness).contains(&"Roads".to_owned()));
    let app = harness.state().tab();
    assert_eq!(
        app.voice.said.last().map(String::as_str),
        Some("Moved to the top level.")
    );
    let roads = harness.get_by_role_and_label(egui::accesskit::Role::TreeItem, "Roads");
    assert!(
        roads.accesskit_node().is_focused(),
        "the keyboard stays on it"
    );
    // Up, among those at the top level.
    let before = folders(&harness);
    harness.key_press_modifiers(Modifiers::ALT, Key::ArrowUp);
    harness.run_steps(2);
    let after = folders(&harness);
    let place = |list: &[String]| list.iter().position(|folder| folder == "Roads");
    assert_eq!(
        place(&after),
        place(&before).map(|at| at - 1),
        "{before:?} {after:?}"
    );
    let said = harness
        .state()
        .tab()
        .voice
        .said
        .last()
        .cloned()
        .unwrap_or_default();
    assert!(said.starts_with("Moved up, "), "{said}");
    // From its menu, into Chat.
    harness
        .get_by_role_and_label(egui::accesskit::Role::TreeItem, "Roads")
        .click_secondary();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(egui::accesskit::Role::MenuItem, "Chat")
        .click();
    harness.run_steps(2);
    assert!(folders(&harness).contains(&"Chat/Roads".to_owned()));
    let app = harness.state().tab();
    assert_eq!(
        app.voice.said.last().map(String::as_str),
        Some("Moved into Chat.")
    );
}

/// Held over a closed folder for a moment, what is dragged opens it; and
/// Escape gives the drag up, nothing moved.
#[test]
fn a_closed_folder_opens_under_what_is_dragged_and_escape_gives_up() {
    let mut harness = nested_folders();
    let app = harness.state_mut().tab_mut();
    app.fold(Kind::Alias, "Chat", true);
    harness.run_steps(2);
    let travel = line_of(&harness, "Travel").center();
    let chat = line_of(&harness, "Chat").center();
    pointer(&mut harness, travel, Some(true));
    pointer(&mut harness, travel + egui::vec2(0.0, 12.0), None);
    pointer(&mut harness, chat, None);
    std::thread::sleep(std::time::Duration::from_millis(800));
    pointer(&mut harness, chat, None);
    assert!(
        harness.state().tab().folder_open(Kind::Alias, "Chat"),
        "the folder opened"
    );
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    pointer(&mut harness, chat, Some(false));
    let folders = harness.state().tab().section_folders(Kind::Alias);
    assert!(folders.contains(&"Travel".to_owned()), "{folders:?}");
    assert!(
        harness.state().tab().editor.open,
        "Escape closed the editor too"
    );
}

/// The right button opens a menu as it is pressed, as the desktop does, on
/// a folder and on a rule; let go, it stays open; and Escape closes the
/// menu alone, not the editor under it.
#[test]
fn a_menu_opens_as_the_right_button_is_pressed() {
    let mut harness = list_with_a_folder();
    let menu_item = |harness: &Harness<'_, App>, name: &str| {
        let item = egui::accesskit::Role::MenuItem;
        harness.query_by_role_and_label(item, name).is_some()
    };
    for (line, item) in [("Travel", "Rename…"), ("Gamma", "Delete")] {
        let at = line_of(&harness, line).center();
        harness.event(egui::Event::PointerMoved(at));
        harness.step();
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        // Shown while the button is still held: the frame after the press,
        // the one in which egui measures it.
        harness.event(button(true));
        harness.run_steps(2);
        assert!(menu_item(&harness, item), "no menu on pressing on {line}");
        harness.event(button(false));
        harness.run_steps(2);
        assert!(
            menu_item(&harness, item),
            "the menu of {line} closed on letting go"
        );
        harness.key_press(Key::Escape);
        harness.run_steps(2);
        assert!(!menu_item(&harness, item));
        assert!(
            harness.state().tab().editor.open,
            "Escape closed the editor too"
        );
        let app = harness.state().tab();
        let untouched = app.editor.folder.is_none() && app.chosen() == [0];
        assert!(untouched, "the menu chose what it was opened on");
    }
}

/// The menu of a line not chosen acts on that line, and leaves what is
/// chosen, and its details, as they were; of one among several chosen, on
/// all of them.
#[test]
fn a_menu_acts_on_its_line_and_leaves_the_choice_alone() {
    let mut harness = list_with_a_folder();
    let open_menu = |harness: &mut Harness<'_, App>, line: &str| {
        let at = line_of(harness, line).center();
        harness.event(egui::Event::PointerMoved(at));
        harness.step();
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            harness.run_steps(2);
        }
    };
    let item = |harness: &Harness<'_, App>, name: &str| {
        harness
            .get_by_role_and_label(egui::accesskit::Role::MenuItem, name)
            .click();
    };
    // Gamma, the third, is chosen; Beta's menu switches Beta off alone.
    harness.state_mut().tab_mut().editor.selected[automation::ALIASES] = 2;
    harness.run_steps(2);
    open_menu(&mut harness, "Beta");
    item(&harness, "Disable");
    harness.run_steps(2);
    let app = harness.state().tab();
    let on: Vec<bool> = app
        .editor
        .draft
        .aliases
        .iter()
        .map(|rule| rule.enabled)
        .collect();
    assert_eq!(on, [true, false, true]);
    assert_eq!(app.chosen(), [2], "what was chosen still is");
    // Deleted from its menu, Alpha goes, and Gamma, still chosen, is now
    // the second.
    open_menu(&mut harness, "Alpha");
    item(&harness, "Delete");
    harness.run_steps(2);
    let app = harness.state().tab();
    assert_eq!(app.editor.draft.aliases[app.chosen()[0]].name, "Gamma");
    // A folder's menu leaves the choice alone too.
    open_menu(&mut harness, "Travel");
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(harness.state().tab().editor.folder.is_none());
}
