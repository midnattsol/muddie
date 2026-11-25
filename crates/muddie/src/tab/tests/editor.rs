//! The editor: rules, variables and scripts.

use super::*;

mod code;
mod dragging;
mod folders;

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
    app.open_automation(automation::VARIABLES);
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
    harness
        .get_by_role_and_label(egui::accesskit::Role::Switch, "Value")
        .click();
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
    harness.state_mut().tab_mut().editor.script_text = r#"macro("f9", "kick")"#.into();
    harness.run_steps(2);
    harness.get_by_label("Run").click();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(
        matches!(&app.editor.script_ran, Some((_, None))),
        "{:?}",
        app.editor.script_ran
    );
    assert_eq!(app.editor.script_saved, r#"macro("f9", "kick")"#);
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
        r#"macro("f9", "kick", {tags = {"Combat"}})"#,
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
            on("gmcp", "Map.Canvas", function() cells += 1 end)
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
    harness
        .state_mut()
        .tab_mut()
        .open_automation(automation::TRIGGERS);
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
    let next_sent = |sent: &mut tokio::sync::mpsc::UnboundedReceiver<Command>| {
        std::iter::from_fn(|| sent.try_recv().ok())
            .find(|command| matches!(command, Command::Send(_)))
    };
    assert_eq!(
        next_sent(&mut sent),
        Some(Command::Send("eat bread".into()))
    );
    // Reloading scripts keeps the rules working.
    assert!(harness.state_mut().tab_mut().do_client_command("/reload"));
    events
        .send(Event::Line(
            panels::styled_lines("Again you are hungry.").remove(0),
        ))
        .unwrap();
    harness.run_steps(2);
    assert_eq!(
        next_sent(&mut sent),
        Some(Command::Send("eat bread".into()))
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A list with a folder holding two aliases, and one outside it.
fn list_with_a_folder<'a>() -> Harness<'a, App> {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 900.0));
    let app = harness.state_mut().tab_mut();
    for (name, folder) in [("Alpha", "Travel"), ("Beta", "Travel"), ("Gamma", "")] {
        app.rules.aliases.push(alias(&name.to_lowercase(), |rule| {
            rule.name = name.into();
            rule.folder = folder.into();
        }));
    }
    app.open_automation(automation::ALIASES);
    harness.run_steps(3);
    harness
}

/// Every change to the rules is undone with Ctrl+Z and redone with
/// Ctrl+Shift+Z or Ctrl+Y, saying what; typing on in one field is one step,
/// undone from the field itself.
#[test]
fn changes_in_the_editor_are_undone_and_redone() {
    let mut harness = list_with_a_folder();
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    app.editor.selected[automation::ALIASES] = 2;
    harness.run_steps(2);
    let said = |harness: &Harness<'_, App>| harness.state().tab().voice.said.last().cloned();
    let on = |harness: &Harness<'_, App>| harness.state().tab().editor.draft.aliases[2].enabled;
    // A switch: off, undone, redone, undone again.
    harness
        .get_by_role_and_label(egui::accesskit::Role::Switch, "Enabled")
        .click();
    harness.run_steps(2);
    assert!(!on(&harness));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run_steps(2);
    assert!(on(&harness));
    assert_eq!(said(&harness).as_deref(), Some("Undone: Gamma."));
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    harness.run_steps(2);
    assert!(!on(&harness));
    assert_eq!(said(&harness).as_deref(), Some("Redone: Gamma."));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Y);
    harness.run_steps(2);
    assert!(!on(&harness));
    // Typing in the name, in two goes close together, is one step, undone
    // from the field, which keeps the keyboard.
    harness.key_press(Key::F2);
    harness.run_steps(2);
    harness.get_by_label("Name").type_text("Road");
    harness.run_steps(2);
    harness.get_by_label("Name").type_text("s");
    harness.run_steps(2);
    let name =
        |harness: &Harness<'_, App>| harness.state().tab().editor.draft.aliases[2].name.clone();
    assert_eq!(name(&harness), "Roads");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run_steps(2);
    assert_eq!(name(&harness), "Gamma");
    assert!(harness.get_by_label("Name").accesskit_node().is_focused());
    // Everything undone, there is nothing more, and that is said.
    for _ in 0..3 {
        harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        harness.run_steps(2);
    }
    assert_eq!(said(&harness).as_deref(), Some("Nothing to undo."));
    assert!(on(&harness));
}

/// Undone, what changed in another rule is chosen, to be seen: and a
/// folder had back is chosen itself.
#[test]
fn what_is_undone_is_chosen() {
    let mut harness = list_with_a_folder();
    let app = harness.state_mut().tab_mut();
    app.editor.draft.aliases[0].enabled = false;
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    app.editor.selected[automation::ALIASES] = 2;
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert!(app.editor.draft.aliases[0].enabled);
    assert_eq!(app.chosen(), [0]);
    // A folder deleted and had back is chosen.
    let app = harness.state_mut().tab_mut();
    app.delete_folder(Kind::Alias, "Travel");
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run_steps(2);
    let app = harness.state().tab();
    assert_eq!(app.section_folders(Kind::Alias), ["Travel"]);
    assert_eq!(app.editor.folder.as_deref(), Some("Travel"));
}

/// The bar of a deletion is about it: once something else is changed,
/// its Undo would undo that, and the bar goes.
#[test]
fn a_deletions_bar_goes_once_something_else_changes() {
    let mut harness = list_with_a_folder();
    let app = harness.state_mut().tab_mut();
    app.editor.selected[automation::ALIASES] = 2;
    harness.run_steps(2);
    harness.key_press(Key::Delete);
    harness.run_steps(2);
    harness.get_by_label("Undo");
    let app = harness.state_mut().tab_mut();
    app.editor.draft.aliases[0].enabled = false;
    harness.run_steps(2);
    assert!(harness.query_by_label("Undo").is_none());
}

/// Moves the pointer, pressing or letting go of its button if asked.
fn pointer(harness: &mut Harness<'_, App>, at: egui::Pos2, pressed: Option<bool>) {
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
}

/// A rule that fails is marked in the list as a script that fails is.
#[test]
fn a_rule_that_fails_is_marked_in_its_list() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let mut rules = app.rules.clone();
    rules.triggers.push(trigger("hello", |rule| {
        rule.name = "Broken".into();
        rule.luau = true;
        rule.code = "this is not luau".into();
    }));
    rules
        .triggers
        .push(trigger("bye", |rule| rule.name = "Fine".into()));
    app.reload_rules(Some(rules));
    app.restart_scripts();
    app.open_automation(automation::TRIGGERS);
    let marked: Vec<(String, bool)> = (app.automation_rows().into_iter())
        .map(|row| (row.title, row.problem))
        .collect();
    assert_eq!(marked, [("Broken".into(), true), ("Fine".into(), false)]);
}
