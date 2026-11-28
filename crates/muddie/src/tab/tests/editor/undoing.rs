//! What is deleted in the editor, had back.

use super::*;

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

/// A script that is not UTF-8, as one written in Latin-1, is had back as it
/// was once deleted, not empty.
#[test]
fn a_script_in_latin1_deleted_is_had_back_whole() {
    let dir = std::env::temp_dir().join(format!("muddie-undo-latin1-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Test", &crate::profile::Profile::default())
        .unwrap();
    let file = profiles.scripts_dir("Test").join("old.luau");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let bytes = b"-- ca\xf1a\n".to_vec();
    std::fs::write(&file, &bytes).unwrap();
    let mut harness = window(Some(profiles));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Test".into())));
    app.open_automation(automation::SCRIPTS);
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let index = (app.editor.script_files.iter())
        .position(|path| path.ends_with("old.luau"))
        .unwrap();
    app.delete_rows(&[index]);
    assert!(!file.exists());
    app.undo_delete();
    let back = std::fs::read(&file).unwrap_or_default();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(back, bytes);
}
