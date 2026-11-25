//! The editor of code and of what a rule does: its size, its keys, and scripts restarting.

use super::*;

#[test]
fn restarting_the_scripts_says_when_and_which_failed() {
    let dir = std::env::temp_dir().join(format!("muddie-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Test", &crate::profile::Profile::default())
        .unwrap();
    let scripts = profiles.scripts_dir("Test");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::write(scripts.join("good.luau"), "local x = 1").unwrap();
    std::fs::write(scripts.join("broken.luau"), "error('boom')").unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Test".into())));
    app.open_automation(automation::SCRIPTS);
    harness.run_steps(3);
    harness.get_by_label("Restart scripts").click();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert_eq!(app.editor.failed, ["broken.luau"]);
    // In the foot of the sections: how many failed, and when.
    let at = harness.state().tab().editor.restarted.clone().unwrap();
    harness.get_by_label("1 failed");
    harness.get_by_label(&format!("Restarted at {at}"));
    // Said over the corner, as the lines of the game are not in sight,
    // and kept as something that could not be done.
    let (said, _) = harness.state().tab().messages();
    let last = said.last().unwrap();
    assert!(
        last.problem && last.text.contains("1 failed"),
        "{}",
        last.text
    );
    harness.get_by_label(&last.text.clone());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// What a rule does is written in an editor which fits what it holds, up to
/// a point, and which is made taller from its grip, by the keyboard too,
/// saying how many lines it shows, and given back its own height with Home.
#[test]
fn the_editor_of_what_a_rule_does_is_made_taller_from_its_grip() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 900.0));
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    app.open_automation(automation::TRIGGERS);
    app.add_rule();
    harness.run_steps(4);
    let tall = |harness: &Harness<'_, App>| {
        let typing = egui::accesskit::Role::MultilineTextInput;
        harness
            .get_by_role_and_label(typing, "Commands")
            .rect()
            .height()
    };
    let short = tall(&harness);
    // Thirty lines do not make it taller than twenty: it scrolls.
    let app = harness.state_mut().tab_mut();
    app.editor.draft.triggers[0].send = vec!["look"; 30].join("\n");
    harness.run_steps(3);
    let grip = harness.get_by_label("Editor height");
    let lines = grip.accesskit_node().description();
    assert_eq!(lines.as_deref(), Some("20 lines"));
    assert!(short < tall(&harness));
    // Down, with the keyboard on it, a line more, which is said.
    grip.focus();
    harness.run_steps(2);
    harness.key_press(Key::ArrowDown);
    harness.run_steps(3);
    let app = harness.state().tab();
    assert_eq!(app.voice.said.last().map(String::as_str), Some("21 lines"));
    assert!(app.editor.code.heights[0] > 0.0);
    let grip = harness.get_by_label("Editor height");
    let lines = grip.accesskit_node().description();
    assert_eq!(lines.as_deref(), Some("21 lines"));
    // Home: it fits what it holds again.
    harness.key_press(Key::Home);
    harness.run_steps(3);
    assert_eq!(harness.state().tab().editor.code.heights[0], 0.0);
}

/// The editor opens larger, over the window, with the keyboard in it; and
/// Escape gives the keyboard back to where it was opened, with what was
/// written there kept.
#[test]
fn the_editor_of_what_a_rule_does_opens_larger() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 900.0));
    let app = harness.state_mut().tab_mut();
    app.open_automation(automation::TRIGGERS);
    app.add_rule();
    harness.run_steps(4);
    harness.get_by_label("Expand editor").focus();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(4);
    assert!(harness.state().tab().editor.code.expanded);
    let dialog = harness.get_by_role(egui::accesskit::Role::Dialog);
    let title = dialog.accesskit_node().label();
    assert_eq!(title.as_deref(), Some("Commands · New trigger"));
    let editors = harness.query_all_by_label("Commands").collect::<Vec<_>>();
    let typing = editors
        .iter()
        .find(|node| node.accesskit_node().is_focused());
    assert!(typing.is_some(), "the keyboard is in the larger editor");
    harness.key_press(Key::Escape);
    harness.run_steps(4);
    assert!(!harness.state().tab().editor.code.expanded);
    let expand = harness.get_by_label("Expand editor");
    assert!(expand.accesskit_node().is_focused());
}

/// In an editor of code Tab types a tab; Escape, and then Tab, goes on to
/// what is next, the grip of its height, and not back to the top of the
/// window.
#[test]
fn escape_and_tab_leave_an_editor_of_code_for_what_is_next() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 1200.0));
    let app = harness.state_mut().tab_mut();
    app.open_automation(automation::TRIGGERS);
    app.add_rule();
    harness.run_steps(4);
    let typing = egui::accesskit::Role::MultilineTextInput;
    harness.get_by_role_and_label(typing, "Commands").focus();
    harness.run_steps(2);
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    assert_eq!(harness.state().tab().editor.draft.triggers[0].send, "\t");
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    let editor = harness.get_by_role_and_label(typing, "Commands");
    assert!(
        editor.accesskit_node().is_focused(),
        "Escape leaves the keyboard there"
    );
    harness.key_press(Key::Tab);
    harness.run_steps(3);
    assert_eq!(harness.state().tab().editor.draft.triggers[0].send, "\t");
    let grip = harness.get_by_label("Editor height");
    assert!(grip.accesskit_node().is_focused());
}

/// Where there is room under the page, the editor of what a rule does
/// takes it, and the page is not scrolled for it.
#[test]
fn the_editor_of_code_takes_the_room_under_the_page() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 1400.0));
    let app = harness.state_mut().tab_mut();
    app.open_automation(automation::ALIASES);
    app.add_rule();
    harness.run_steps(6);
    let typing = egui::accesskit::Role::MultilineTextInput;
    let editor = harness.get_by_role_and_label(typing, "Commands").rect();
    assert!(editor.height() > 400.0, "{}", editor.height());
    let grip = harness.get_by_label("Editor height").rect();
    assert!(grip.bottom() < 1400.0);
    // In a short window it is as tall as what it holds.
    harness.set_size(egui::vec2(1400.0, 700.0));
    harness.run_steps(6);
    let editor = harness.get_by_role_and_label(typing, "Commands").rect();
    assert!(editor.height() < 100.0, "{}", editor.height());
}

/// From an editor of code Escape gives Tab back, and Escape again leaves
/// the editor, as from anywhere else.
#[test]
fn escape_twice_from_an_editor_of_code_leaves_the_editor() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 900.0));
    let app = harness.state_mut().tab_mut();
    app.open_automation(automation::ALIASES);
    app.add_rule();
    harness.run_steps(3);
    let typing = egui::accesskit::Role::MultilineTextInput;
    harness.get_by_role_and_label(typing, "Commands").focus();
    harness.run_steps(2);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(harness.state().tab().editor.open);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().tab().editor.open);
}
