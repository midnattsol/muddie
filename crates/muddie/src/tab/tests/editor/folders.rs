//! The editor's folders and tags: filing, choosing several, and the keys through the tree.

use super::*;

#[test]
fn folders_and_tags_file_what_there_is_and_switch_it() {
    let dir = std::env::temp_dir().join(format!("muddie-folders-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Test", &crate::profile::Profile::default())
        .unwrap();
    let scripts = profiles.scripts_dir("Test");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::write(scripts.join("hunt.luau"), r#"macro("f9", "kick")"#).unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Test".into())));
    app.rules
        .triggers
        .push(trigger("attacks you", |rule| rule.send = "parry".into()));
    app.open_automation(automation::TRIGGERS);
    harness.run_steps(3);
    // A folder is made, and named as it is made.
    harness.get_by_label("New folder").click();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    app.editor.renaming.as_mut().expect("being named").typed = "Combat".into();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.section_folders(Kind::Trigger), ["Combat"]);
    // The trigger goes in it, and switched off with it, it does not run.
    app.editor.draft.triggers[0].folder = "Combat".into();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(egui::accesskit::Role::Switch, "Combat")
        .click();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    app.close_automation();
    assert!(!app.rules.folder_on(Kind::Trigger, "Combat"));
    let (_events, mut sent) = fake_connection(app);
    app.scripts.on_line("An orc attacks you");
    app.run_actions();
    assert!(
        sent.try_recv().is_err(),
        "a trigger in a folder switched off ran"
    );
    // Its details say why.
    app.open_automation(automation::TRIGGERS);
    harness.run_steps(3);
    harness.get_by_label_contains("the folder Combat is switched off");
    // A tag is given by typing it.
    let app = harness.state_mut().tab_mut();
    app.editor
        .draft
        .set_folder_on(Kind::Trigger, "Combat", true);
    harness.run_steps(2);
    harness.get_by_label("Add a tag").click();
    harness.run_steps(1);
    harness.get_by_label("Add a tag").type_text("Combat");
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(
        harness.state().tab().editor.draft.triggers[0].tags,
        ["Combat"]
    );
    // A script goes into a folder on the disk, and runs from there.
    let app = harness.state_mut().tab_mut();
    app.open_automation(automation::SCRIPTS);
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    app.new_folder(Kind::Script, "");
    app.editor.renaming = None;
    assert!(scripts.join(tr("New folder")).is_dir());
    assert!(app.move_script_files("hunt", &format!("{}/hunt", tr("New folder"))));
    assert!(scripts.join(tr("New folder")).join("hunt.luau").is_file());
    assert!(app.scripts.on_key("f9"));
    // Switched off by its folder, it is stopped.
    app.editor
        .draft
        .set_folder_on(Kind::Script, tr("New folder"), false);
    app.save_rules();
    app.scripts.take_actions();
    assert!(!app.scripts.on_key("f9"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn several_are_chosen_with_ctrl_and_shift_and_changed_together() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    for name in ["Alpha", "Beta", "Gamma", "Delta"] {
        app.rules
            .triggers
            .push(trigger(&name.to_lowercase(), |rule| {
                rule.name = name.into()
            }));
    }
    app.rules.set_folder_on(Kind::Trigger, "Combat", true);
    app.open_automation(automation::TRIGGERS);
    harness.run_steps(3);
    let chosen = |harness: &Harness<'_, App>| harness.state().tab().chosen();
    // Ctrl adds one, Shift a range from the one chosen before.
    harness.get_by_label("Alpha").click();
    harness.run_steps(2);
    harness
        .get_by_label("Gamma")
        .click_modifiers(Modifiers::COMMAND);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), [0, 2]);
    harness.get_by_label_contains("2 chosen");
    harness
        .get_by_label("Delta")
        .click_modifiers(Modifiers::SHIFT);
    harness.run_steps(2);
    assert_eq!(chosen(&harness), [2, 3]);
    // Switched off together, and deleted together, to be had back at once.
    harness
        .get_by_role_and_label(egui::accesskit::Role::Switch, "Enabled")
        .click();
    harness.run_steps(2);
    let draft = &harness.state().tab().editor.draft;
    let on: Vec<bool> = draft.triggers.iter().map(|rule| rule.enabled).collect();
    assert_eq!(on, [true, true, false, false]);
    harness.get_by_label("Delete").click();
    harness.run_steps(2);
    assert_eq!(harness.state().tab().editor.draft.triggers.len(), 2);
    harness.get_by_label_contains("Deleted 2 items.");
    harness.get_by_label("Undo").click();
    harness.run_steps(3);
    assert_eq!(harness.state().tab().editor.draft.triggers.len(), 4);
    // Dragged with the mouse onto a folder, one goes into it.
    harness.get_by_label("Beta").click();
    harness.run_steps(2);
    let from = harness.get_by_label("Beta").rect().center();
    let into = (harness.get_by_role_and_label(egui::accesskit::Role::TreeItem, "Combat"))
        .rect()
        .center();
    let pointer = |harness: &mut Harness<'_, App>, at: egui::Pos2, pressed: Option<bool>| {
        harness.event(egui::Event::PointerMoved(at));
        if let Some(pressed) = pressed {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        harness.run_steps(2);
    };
    pointer(&mut harness, from, Some(true));
    pointer(&mut harness, from + egui::vec2(0.0, -12.0), None);
    pointer(&mut harness, into, None);
    pointer(&mut harness, into, Some(false));
    let draft = &harness.state().tab().editor.draft;
    assert_eq!(draft.triggers[1].folder, "Combat");
    // Ctrl+A chooses all that is shown; Escape leaves the one selected.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run_steps(2);
    assert_eq!(chosen(&harness).len(), 4);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert_eq!(chosen(&harness).len(), 1);
    assert!(
        harness.state().tab().editor.open,
        "Escape went back to the game"
    );
}

#[test]
fn a_new_folder_is_named_in_its_line_and_a_name_taken_is_kept_to_change() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.rules.set_folder_on(Kind::Alias, "Travel", true);
    app.open_automation(automation::ALIASES);
    harness.run_steps(3);
    harness.get_by_label("New folder").click();
    harness.run_steps(3);
    // Its name is chosen: what is typed replaces it.
    harness.get_by_label("Folder name").type_text("Travel");
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert!(app.editor.renaming.is_some(), "a name taken was given");
    assert_eq!(app.editor.renaming.as_ref().unwrap().typed, "Travel");
    harness.get_by_label("Folder name").type_text(" home");
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.editor.renaming.is_none());
    assert_eq!(app.section_folders(Kind::Alias), ["Travel", "Travel home"]);
}

#[test]
fn the_list_answers_the_keys_any_list_does() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    for name in ["Alpha", "Beta", "Gamma"] {
        app.rules
            .aliases
            .push(alias(&name.to_lowercase(), |rule| rule.name = name.into()));
    }
    app.open_automation(automation::ALIASES);
    harness.run_steps(3);
    let selected = |harness: &Harness<'_, App>| harness.state().tab().chosen();
    // The arrows go down and up it, Home and End to its ends.
    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    assert_eq!(selected(&harness), [1]);
    harness.key_press(Key::End);
    harness.run_steps(2);
    assert_eq!(selected(&harness), [2]);
    harness.key_press(Key::Home);
    harness.run_steps(2);
    assert_eq!(selected(&harness), [0]);
    // F2 gives the name the keys; Escape there leaves the name, not the
    // editor.
    harness.key_press(Key::F2);
    harness.run_steps(2);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(
        harness.state().tab().editor.open,
        "Escape in the name left the editor"
    );
    // Its text is chosen, to be typed over.
    harness.key_press(Key::F2);
    harness.run_steps(2);
    harness.get_by_label("Name").type_text("First");
    harness.run_steps(2);
    assert_eq!(harness.state().tab().editor.draft.aliases[0].name, "First");
    // Another clicked, out of the name, Delete deletes it, to be had back.
    harness.get_by_label("Beta").click();
    harness.run_steps(2);
    harness.key_press(Key::Delete);
    harness.run_steps(2);
    let names = |harness: &Harness<'_, App>| -> Vec<String> {
        let aliases = &harness.state().tab().editor.draft.aliases;
        aliases.iter().map(|rule| rule.name.clone()).collect()
    };
    assert_eq!(names(&harness), ["First", "Gamma"]);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run_steps(2);
    assert_eq!(names(&harness), ["First", "Beta", "Gamma"]);
}

/// A folder is chosen as a thing of the list is, by a click on it, and
/// then shows its own details; its arrow opens and closes it.
#[test]
fn a_folder_is_chosen_by_a_click_and_shows_its_details() {
    let mut harness = list_with_a_folder();
    let folder = harness.get_by_role_and_label(egui::accesskit::Role::TreeItem, "Travel");
    folder.click();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert_eq!(app.editor.folder.as_deref(), Some("Travel"));
    assert!(app.chosen().is_empty(), "no rule is chosen with it");
    let folder = harness.get_by_role_and_label(egui::accesskit::Role::TreeItem, "Travel");
    assert_eq!(folder.accesskit_node().is_selected(), Some(true));
    assert_eq!(folder.accesskit_node().level(), Some(1));
    harness.get_by_label("2 items");
    harness.get_by_label("Add here");
    // It is still open: a click on its name does not close it.
    assert!(harness.state().tab().folder_open(Kind::Alias, "Travel"));
    // Its arrow closes it.
    let arrow = folder.rect().left_center() + egui::vec2(12.0, 0.0);
    harness.hover_at(arrow);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: arrow,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run_steps(3);
    assert!(!harness.state().tab().folder_open(Kind::Alias, "Travel"));
}

/// The list is one stop of Tab, as a tree is: the arrows go through its
/// folders and its things, left and right close and open a folder and go
/// out of it, Delete deletes a folder, to be had back, and F2 names it.
#[test]
fn the_keys_go_through_folders_as_through_a_tree() {
    let mut harness = list_with_a_folder();
    let tree = egui::accesskit::Role::TreeItem;
    // Only the one chosen is stopped at by Tab.
    let app = harness.state_mut().tab_mut();
    app.editor.selected[automation::ALIASES] = 2;
    harness.run_steps(2);
    let stops = harness
        .query_all_by_role(tree)
        .filter(|node| {
            (node.accesskit_node().data()).supports_action(egui::accesskit::Action::Focus)
        })
        .count();
    assert_eq!(stops, 1);
    harness.get_by_role_and_label(tree, "Gamma").focus();
    harness.run_steps(2);
    let on = |harness: &Harness<'_, App>, name: &str| {
        harness
            .get_by_role_and_label(egui::accesskit::Role::TreeItem, name)
            .accesskit_node()
            .is_focused()
    };
    // Up, from Gamma: Beta, Alpha, then the folder, with the keyboard.
    for name in ["Beta", "Alpha", "Travel"] {
        harness.key_press(Key::ArrowUp);
        harness.run_steps(2);
        assert!(on(&harness, name), "the keyboard is not on {name}");
    }
    assert_eq!(
        harness.state().tab().editor.folder.as_deref(),
        Some("Travel")
    );
    // Left closes it; Down then skips what is in it.
    harness.key_press(Key::ArrowLeft);
    harness.run_steps(2);
    assert!(!harness.state().tab().folder_open(Kind::Alias, "Travel"));
    harness.key_press(Key::ArrowDown);
    harness.run_steps(2);
    assert!(on(&harness, "Gamma"));
    // Back up, Right opens it, and Right again goes into it.
    harness.key_press(Key::ArrowUp);
    harness.key_press(Key::ArrowRight);
    harness.run_steps(2);
    assert!(harness.state().tab().folder_open(Kind::Alias, "Travel"));
    harness.key_press(Key::ArrowRight);
    harness.run_steps(2);
    assert!(on(&harness, "Alpha"));
    // Left from in it goes to the folder.
    harness.key_press(Key::ArrowLeft);
    harness.run_steps(2);
    assert!(on(&harness, "Travel"));
    // F2 names it.
    harness.key_press(Key::F2);
    harness.run_steps(2);
    harness.get_by_label("Folder name").type_text("Roads");
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert_eq!(app.section_folders(Kind::Alias), ["Roads"]);
    assert_eq!(app.editor.folder.as_deref(), Some("Roads"));
    assert!(on(&harness, "Roads"), "the keyboard is back on the folder");
    // Delete deletes it; what it held is kept, out of it, to be had back.
    harness.key_press(Key::Delete);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.section_folders(Kind::Alias).is_empty());
    assert!(
        app.editor
            .draft
            .aliases
            .iter()
            .all(|rule| rule.folder.is_empty())
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert_eq!(app.section_folders(Kind::Alias), ["Roads"]);
}

/// A new folder's name is typed in a field centred on its line.
#[test]
fn a_new_folders_name_is_typed_on_its_line() {
    let mut harness = list_with_a_folder();
    harness.get_by_label("New folder").click();
    harness.run_steps(3);
    let field = harness.get_by_label("Folder name");
    let line = harness
        .query_all_by_role(egui::accesskit::Role::TreeItem)
        .find(|node| {
            node.accesskit_node().data().is_expanded().is_some()
                && node.rect().intersects(field.rect())
        })
        .expect("the field is on a folder's line")
        .rect();
    let (field, line) = (field.rect(), line);
    assert!(
        (field.center().y - line.center().y).abs() < 1.0,
        "{field:?} {line:?}"
    );
}

/// One button closes every folder, and then opens them all, saying which
/// it did; what was chosen in a folder closed gives way to the folder.
#[test]
fn one_button_collapses_and_expands_all_folders() {
    let mut harness = list_with_a_folder();
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    app.editor.selected[automation::ALIASES] = 0;
    harness.run_steps(2);
    harness.get_by_label("Collapse all folders").click();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(!app.folder_open(Kind::Alias, "Travel"));
    assert_eq!(app.editor.folder.as_deref(), Some("Travel"));
    assert_eq!(
        app.voice.said.last().map(String::as_str),
        Some("All folders collapsed.")
    );
    assert!(
        harness.query_by_label("Alpha").is_none(),
        "what is in it is hidden"
    );
    harness.get_by_label("Expand all folders").click();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.folder_open(Kind::Alias, "Travel"));
    assert_eq!(
        app.voice.said.last().map(String::as_str),
        Some("All folders expanded.")
    );
    harness.get_by_label("Alpha");
}

/// A rule moved up or down by the keys says where it now is.
#[test]
fn a_rule_moved_by_the_keys_says_where_it_is() {
    let mut harness = list_with_a_folder();
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    app.editor.selected[automation::ALIASES] = 1;
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::ALT, Key::ArrowUp);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert_eq!(app.editor.draft.aliases[0].name, "Beta");
    assert_eq!(
        app.voice.said.last().map(String::as_str),
        Some("Moved up, 1 of 2.")
    );
}

/// Folders are closed at first, but those the one chosen is in, for it to
/// be seen; those opened are kept open, in the settings, for the profile.
#[test]
fn folders_are_closed_at_first_but_where_the_one_chosen_is() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 900.0));
    let app = harness.state_mut().tab_mut();
    for (name, folder) in [("Alpha", "Travel/Roads"), ("Beta", "Chat"), ("Gamma", "")] {
        app.rules.aliases.push(alias(&name.to_lowercase(), |rule| {
            rule.name = name.into();
            rule.folder = folder.into();
        }));
    }
    app.editor.selected[automation::ALIASES] = 0;
    app.open_automation(automation::ALIASES);
    harness.run_steps(3);
    let tab = harness.state().tab();
    assert!(tab.folder_open(Kind::Alias, "Travel") && tab.folder_open(Kind::Alias, "Travel/Roads"));
    assert!(
        !tab.folder_open(Kind::Alias, "Chat"),
        "a folder is closed at first"
    );
    assert!(harness.query_by_label("Beta").is_none());
    // Opened, it is kept open for the profile, here none.
    harness
        .state_mut()
        .tab_mut()
        .fold(Kind::Alias, "Chat", false);
    harness.run_steps(3);
    let kept = harness
        .state()
        .settings()
        .open_folders
        .get("")
        .cloned()
        .unwrap_or_default();
    assert!(kept.contains(&(Kind::Alias, "Chat".to_owned())), "{kept:?}");
}
