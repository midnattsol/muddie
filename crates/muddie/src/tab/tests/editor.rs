//! The editor: rules, variables and scripts.

use super::*;

#[test]
fn variables_are_the_sessions_and_kept_only_with_a_saved_profile() {
    let dir = std::env::temp_dir().join(format!("muddie-app-vars-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    let app = harness.state_mut().tab_mut();
    // No profile: they last through the scripts starting again, and
    // nothing is written anywhere.
    app.scripts.exec("t", r#"vars.target = "orc""#);
    app.load_scripts();
    assert_eq!(
        app.scripts.variables(),
        [("target".to_owned(), serde_json::json!("orc"))]
    );
    assert!(!dir.join("data.json").exists());
    // Another session does not see them.
    let mut other = window(Some(Profiles::at(dir.clone())));
    assert!(other.state_mut().tab_mut().scripts.variables().is_empty());
    // Saved as a profile, they go with it, and are there the next time.
    let app = harness.state_mut().tab_mut();
    app.host = "mud.example.org".into();
    app.port = "4000".into();
    app.profile_name = "Reinos".into();
    app.save_profile();
    assert_eq!(app.profile.as_deref(), Some("Reinos"));
    assert_eq!(app.scripts.variables().len(), 1);
    let mut later = window(Some(Profiles::at(dir.clone())));
    let app = later.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    assert_eq!(
        app.scripts.variables(),
        [("target".to_owned(), serde_json::json!("orc"))]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_variable_is_edited_as_its_type() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.scripts
        .set_variable("bag", Some(serde_json::json!({"gold": 5})))
        .unwrap();
    app.open_automation(5);
    harness.run_steps(3);
    // A table: a key and a value a row, added to and typed over.
    harness.get_by_label("Add a field").click();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    app.editor.variable_items[1] = ("keys".into(), "3".into());
    harness.run_steps(2);
    let kept = |app: &Tab| app.scripts.variables()[0].1.clone();
    assert_eq!(
        kept(harness.state().tab()),
        serde_json::json!({"gold": 5, "keys": 3})
    );
    // Made an array, it keeps its values and loses their keys.
    harness.get_by_label("array").click();
    harness.run_steps(3);
    assert_eq!(kept(harness.state().tab()), serde_json::json!([5, 3]));
    // A boolean is a switch.
    harness.get_by_label("boolean").click();
    harness.run_steps(3);
    assert_eq!(kept(harness.state().tab()), serde_json::json!(true));
    harness.get_by_label("Value").click();
    harness.run_steps(3);
    assert_eq!(kept(harness.state().tab()), serde_json::json!(false));
}

#[test]
fn a_script_is_run_from_its_button_and_says_how_it_went() {
    let dir = std::env::temp_dir().join(format!("muddie-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Test", &crate::profile::Profile::default())
        .unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Test".into())));
    app.open_automation(automation::SCRIPTS);
    app.add_script(None);
    harness.run_steps(3);
    // Typed, and run at once from its button: it takes effect...
    harness.state_mut().tab_mut().editor.script_text = r#"bind("f9", "kick")"#.into();
    harness.run_steps(2);
    harness.get_by_label("Run").click();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(
        matches!(&app.editor.script_ran, Some((_, None))),
        "{:?}",
        app.editor.script_ran
    );
    assert_eq!(app.editor.script_saved, r#"bind("f9", "kick")"#);
    harness.get_by_label_contains("Last run: ");
    // ...and one that fails says why, there, not among the lines of
    // the game.
    let before = output_lines(harness.state().tab()).len();
    harness.state_mut().tab_mut().editor.script_text = "error('boom')".into();
    harness.run_steps(2);
    harness.get_by_label("Run").click();
    harness.run_steps(3);
    let app = harness.state().tab();
    let Some((_, Some(error))) = &app.editor.script_ran else {
        panic!("the error was not kept: {:?}", app.editor.script_ran);
    };
    assert!(error.contains("boom"), "{error}");
    assert_eq!(output_lines(app).len(), before);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_script_switched_off_is_kept_and_not_run() {
    let dir = std::env::temp_dir().join(format!("muddie-scriptoff-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Test", &crate::profile::Profile::default())
        .unwrap();
    let scripts = profiles.scripts_dir("Test");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::write(
        scripts.join("keys.luau"),
        r#"bind("f9", "kick", {tags = {"Combat"}})"#,
    )
    .unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Test".into())));
    assert_eq!(app.scripts.tags(), ["combat"]);
    // Switched off beside its name, it stops being run once saved...
    app.open_automation(automation::SCRIPTS);
    harness.run_steps(3);
    let index = (harness.state().tab().editor.script_files.iter())
        .position(|file| file.ends_with("keys.luau"))
        .unwrap();
    harness
        .state_mut()
        .tab_mut()
        .select_rule(automation::SCRIPTS, index);
    harness.run_steps(3);
    harness
        .get_by_role_and_label(egui::accesskit::Role::Switch, "Enabled")
        .click();
    harness.run_steps(2);
    std::thread::sleep(Duration::from_millis(1000));
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert!(!app.rules.script("keys").enabled);
    assert!(!app.scripts.on_key("f9"));
    // ...and is still there, to be switched on again.
    assert!(scripts.join("keys.luau").is_file());
    harness
        .get_by_role_and_label(egui::accesskit::Role::Switch, "Enabled")
        .click();
    harness.run_steps(2);
    std::thread::sleep(Duration::from_millis(1000));
    harness.run_steps(3);
    assert!(harness.state_mut().tab_mut().scripts.on_key("f9"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn saving_a_rule_or_a_script_leaves_what_the_other_scripts_keep() {
    let dir = std::env::temp_dir().join(format!("muddie-partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Test", &crate::profile::Profile::default())
        .unwrap();
    let scripts = profiles.scripts_dir("Test");
    std::fs::create_dir_all(&scripts).unwrap();
    // One script keeps what it is told, as one drawing a map would;
    // another is the one being worked on.
    std::fs::write(
        scripts.join("canvas.luau"),
        r#"cells = cells or 0
            on_gmcp("Map.Canvas", function() cells += 1 end)
            alias("^cells$", function() echo("cells " .. cells) end)"#,
    )
    .unwrap();
    std::fs::write(
        scripts.join("other.luau"),
        r#"alias("^v$", function() echo("v1") end)"#,
    )
    .unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Test".into())));
    let said = |app: &mut Tab, typed: &str| {
        app.scripts.on_input(typed);
        app.run_actions();
        output_lines(app).last().cloned().unwrap_or_default()
    };
    app.scripts.on_gmcp("Map.Canvas.Full", "{}");
    app.scripts.on_gmcp("Map.Canvas.Patch", "{}");
    assert_eq!(said(app, "cells"), "cells 2");
    // A rule is made and saved: it takes effect, and the scripts are
    // not run again for it.
    app.editor.draft = app.rules.clone();
    app.editor.draft.aliases.push(alias("hi", |rule| {
        rule.name = "hi".into();
        rule.send = "say hello".into();
    }));
    app.save_rules();
    assert_eq!(said(app, "cells"), "cells 2");
    let (_events, mut sent) = fake_connection(app);
    app.scripts.on_input("hi");
    app.run_actions();
    assert_eq!(
        sent.try_recv().ok(),
        Some(Command::Send("say hello".into()))
    );
    // Saved again, the rule is there once, not twice.
    app.save_rules();
    app.scripts.on_input("hi");
    app.run_actions();
    assert_eq!(
        sent.try_recv().ok(),
        Some(Command::Send("say hello".into()))
    );
    assert!(sent.try_recv().is_err());
    // The other script is changed and run again from the editor: it
    // is the new one, and the first still has what it kept.
    app.open_automation(automation::SCRIPTS);
    harness.run_steps(3);
    let index = (harness.state().tab().editor.script_files.iter())
        .position(|file| file.ends_with("other.luau"))
        .unwrap();
    harness
        .state_mut()
        .tab_mut()
        .select_rule(automation::SCRIPTS, index);
    harness.run_steps(3);
    harness.state_mut().tab_mut().editor.script_text =
        r#"alias("^v$", function() echo("v2") end)"#.into();
    harness.run_steps(2);
    harness.get_by_label("Run").click();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert_eq!(said(app, "v"), "v2");
    assert_eq!(said(app, "cells"), "cells 2");
    // Switched off, a script is stopped; it can still be run by hand,
    // and running everything again starts it only if it is on.
    app.editor.draft = app.rules.clone();
    app.editor.draft.set_script(crate::rules::ScriptEntry {
        path: "other".into(),
        enabled: false,
        tags: Vec::new(),
    });
    app.save_rules();
    let before = output_lines(app).len();
    app.scripts.on_input("v");
    app.run_actions();
    assert_eq!(output_lines(app).len(), before, "it was not stopped");
    assert_eq!(said(app, "cells"), "cells 2");
    app.run_scripts();
    assert_eq!(said(app, "v"), "v2");
    app.load_scripts();
    let before = output_lines(app).len();
    app.scripts.on_input("v");
    app.run_actions();
    assert_eq!(
        output_lines(app).len(),
        before,
        "one switched off was started with the rest"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn what_is_deleted_can_be_had_back() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.rules.triggers.push(trigger("attacks you", |rule| {
        rule.name = "Fight back".into();
        rule.send = "kill".into();
    }));
    app.scripts
        .set_variable("target", Some(serde_json::json!("orc")))
        .unwrap();
    app.open_automation(automation::TRIGGERS);
    harness.run_steps(3);
    // Deleted, it is gone, and the editor says so with a way back.
    harness.get_by_label("Delete").click();
    harness.run_steps(3);
    assert!(harness.state().tab().editor.draft.triggers.is_empty());
    harness.get_by_label_contains("Deleted Fight back.");
    harness.get_by_label("Undo").click();
    harness.run_steps(3);
    assert_eq!(
        harness.state().tab().editor.draft.triggers[0].name,
        "Fight back"
    );
    // A variable too, with the keys that undo anywhere.
    harness
        .state_mut()
        .tab_mut()
        .open_automation(automation::VARIABLES);
    harness.run_steps(3);
    harness.get_by_label("Delete").click();
    harness.run_steps(3);
    assert!(harness.state().tab().scripts.variables().is_empty());
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::Z);
    harness.run_steps(3);
    let kept = harness.state().tab().scripts.variables();
    assert_eq!(kept, [("target".to_owned(), serde_json::json!("orc"))]);
}

#[test]
fn rules_made_in_the_window_are_saved_and_take_effect() {
    let dir = std::env::temp_dir().join(format!("muddie-app-rules-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    harness.state_mut().tab_mut().open_automation(0);
    harness.run_steps(4);
    harness.get_by_label("Add").click();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    app.editor.draft.triggers[0].when.pattern = "you are hungry".into();
    app.editor.draft.triggers[0].send = "eat bread".into();
    // Edits are saved once they stop for a moment, with no button.
    harness.run_steps(2);
    assert!(!dir.join("rules.toml").exists(), "not while still editing");
    std::thread::sleep(Duration::from_millis(1000));
    harness.run_steps(2);
    assert_eq!(
        Rules::load(&dir.join("rules.toml")).unwrap().triggers.len(),
        1
    );
    // Closing the window saves at once.
    let app = harness.state_mut().tab_mut();
    app.editor.draft.triggers[0].name = "Hunger".into();
    app.close_automation();
    assert_eq!(
        Rules::load(&dir.join("rules.toml")).unwrap().triggers[0].name,
        "Hunger"
    );

    let app = harness.state_mut().tab_mut();
    let (events, mut sent) = fake_connection(app);
    events
        .send(Event::Line(
            panels::styled_lines("Suddenly you are hungry.").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    assert_eq!(sent.try_recv().unwrap(), Command::Send("eat bread".into()));
    // Reloading scripts keeps the rules working.
    assert!(harness.state_mut().tab_mut().do_client_command("/reload"));
    events
        .send(Event::Line(
            panels::styled_lines("Again you are hungry.").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    assert_eq!(sent.try_recv().unwrap(), Command::Send("eat bread".into()));
    std::fs::remove_dir_all(&dir).unwrap();
}

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
    std::fs::write(scripts.join("hunt.luau"), r#"bind("f9", "kick")"#).unwrap();
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
