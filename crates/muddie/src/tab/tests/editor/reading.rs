//! The editor as a screen reader goes through it: in the order it is
//! read, and never leaving the keyboard nowhere.

use super::*;

/// Where each of `names` is among what a screen reader goes through, in
/// the order it goes, and with Tab.
fn read_at(harness: &Harness<'_, App>, names: &[&str]) -> Vec<usize> {
    let read: Vec<String> = (harness.query_all(egui_kittest::kittest::by()))
        .map(|node| {
            node.accesskit_node()
                .label()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    (names.iter())
        .map(|name| {
            (read.iter().position(|label| label == name))
                .unwrap_or_else(|| panic!("{name} is not there: {read:?}"))
        })
        .collect()
}

#[test]
fn the_head_of_the_details_is_read_with_what_deletes_last() {
    let dir = std::env::temp_dir().join(format!("muddie-heads-{}", std::process::id()));
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
    let at = read_at(&harness, &["Name", "Enabled", "Run", "Delete"]);
    assert!(at.is_sorted(), "{at:?}");
    let app = harness.state_mut().tab_mut();
    app.scripts
        .set_variable("bag", Some(serde_json::json!(5)))
        .unwrap();
    app.open_automation(automation::VARIABLES);
    harness.run_steps(3);
    let at = read_at(&harness, &["Name", "Delete"]);
    assert!(at.is_sorted(), "{at:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// What the keyboard is on, by name.
fn focused_name(harness: &Harness<'_, App>) -> Option<String> {
    (harness.query_all(egui_kittest::kittest::by()))
        .find(|node| node.is_focused())
        .map(|node| {
            node.accesskit_node()
                .label()
                .unwrap_or_default()
                .to_string()
        })
}

#[test]
fn a_row_or_a_tag_taken_away_leaves_the_keyboard_beside_it() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1400.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.scripts
        .set_variable("bag", Some(serde_json::json!({"gold": 5})))
        .unwrap();
    app.open_automation(automation::VARIABLES);
    harness.run_steps(3);
    // Its name, its value, and then what removes it, which says which.
    let read: Vec<String> = (harness.query_all(egui_kittest::kittest::by()))
        .map(|node| {
            node.accesskit_node()
                .label()
                .unwrap_or_default()
                .to_string()
        })
        .filter(|label| !label.is_empty())
        .collect();
    let row = read.iter().position(|label| label == "Field name").unwrap();
    assert_eq!(
        read[row..].iter().take(3).collect::<Vec<_>>(),
        ["Field name", "Value", "Remove gold"]
    );
    harness.get_by_label("Remove gold").focus();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(focused_name(&harness).as_deref(), Some("Add a field"));
    // A tag taken off: the keyboard goes on to the field to add one.
    let mut tags = vec!["combat".to_owned()];
    let mut ready = false;
    let mut harness = egui_kittest::Harness::new_ui(move |ui| {
        if !std::mem::replace(&mut ready, true) {
            crate::theme::apply(ui.ctx(), &crate::settings::Settings::default()).unwrap();
            return;
        }
        crate::widgets::tag_field(ui, "tags", &mut tags, &[]);
    });
    harness.run_steps(2);
    harness.get_by_label("Remove the tag combat").focus();
    harness.run_steps(2);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    let focused = (harness.query_all(egui_kittest::kittest::by()))
        .find(|node| node.is_focused())
        .map(|node| {
            node.accesskit_node()
                .label()
                .unwrap_or_default()
                .to_string()
        });
    assert_eq!(focused.as_deref(), Some("Add a tag"));
}
