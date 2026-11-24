//! Every window gone through with Tab alone, as a screen reader user does:
//! the keyboard is never held in one place, never lost, and each stop has
//! a name to say.

use super::*;

/// Presses Tab until the keyboard comes back where it started, and says
/// what was wrong on the way: a stop it does not leave, the keyboard
/// gone nowhere, a stop with no name.
fn walk(harness: &mut Harness<'_, App>, view: &str) -> Vec<String> {
    let focused = |harness: &Harness<'_, App>| {
        harness
            .query_all(egui_kittest::kittest::by())
            .find(|node| node.is_focused())
            .map(|node| {
                let node = node.accesskit_node();
                (
                    node.id(),
                    node.label().unwrap_or_default(),
                    format!("{:?}", node.role()),
                )
            })
    };
    let mut problems = Vec::new();
    let mut seen = Vec::new();
    let mut held = 0;
    for _ in 0..120 {
        harness.key_press(Key::Tab);
        harness.run_steps(2);
        let Some((id, label, role)) = focused(harness) else {
            problems.push(format!("{view}: Tab left the keyboard nowhere"));
            break;
        };
        if label.trim().is_empty() {
            problems.push(format!("{view}: a {role} with no name"));
        }
        if seen.last().is_some_and(|(last, _)| *last == id) {
            held += 1;
            // An editor of code keeps Tab, and says Escape lets it go.
            if role == "MultilineTextInput" && held == 1 {
                harness.key_press(Key::Escape);
                harness.run_steps(2);
                continue;
            }
            if held == 2 {
                problems.push(format!("{view}: Tab does not leave {role} {label:?}"));
                break;
            }
            continue;
        }
        held = 0;
        if seen.iter().any(|(known, _)| *known == id) {
            break;
        }
        seen.push((id, label));
    }
    eprintln!(
        "WALK {view}: {:?}",
        seen.iter()
            .map(|(_, label)| label.as_str())
            .collect::<Vec<_>>()
    );
    if seen.len() < 2 {
        problems.push(format!("{view}: Tab reached {} stops", seen.len()));
    }
    problems
}

#[test]
fn every_window_is_gone_through_with_tab_alone() {
    let dir = std::env::temp_dir().join(format!("muddie-walk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Reinos", &crate::profile::Profile::default())
        .unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1300.0, 860.0));
    let mut problems = Vec::new();
    // The welcome screen, as a session opens.
    harness.state_mut().tab_mut().started = false;
    harness.run_steps(3);
    problems.extend(walk(&mut harness, "welcome"));
    // Playing.
    let (events, _sent) = fake_connection(harness.state_mut().tab_mut());
    events
        .send(Event::Line(panels::styled_lines("The Temple.").remove(0)))
        .unwrap();
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    problems.extend(walk(&mut harness, "game"));
    // The messages.
    harness.state_mut().tab_mut().note("Connected.");
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::N);
    harness.run_steps(3);
    problems.extend(walk(&mut harness, "messages"));
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    // The editor, each section.
    for section in automation::Section::ALL {
        harness.state_mut().tab_mut().open_automation(section);
        harness.run_steps(3);
        problems.extend(walk(&mut harness, &format!("editor {section:?}")));
    }
    harness.state_mut().tab_mut().set_view(View::Layout);
    harness.run_steps(3);
    problems.extend(walk(&mut harness, "layout"));
    harness.state_mut().tab_mut().set_view(View::Game);
    harness.run_steps(2);
    // The settings, each page.
    for section in crate::app::Section::ALL {
        harness.state_mut().open_settings(section);
        harness.run_steps(3);
        problems.extend(walk(&mut harness, &format!("settings {}", section.name())));
    }
    harness.state_mut().close_settings_for_tests();
    harness.run_steps(2);
    // The profiles, each tab of one.
    harness.state_mut().edit_profile("Reinos");
    harness.run_steps(3);
    for tab in ["Connection", "Session", "Plugins"] {
        harness
            .get_by_role_and_label(egui::accesskit::Role::Tab, tab)
            .click();
        harness.run_steps(3);
        problems.extend(walk(&mut harness, &format!("profile {tab}")));
    }
    // A new profile.
    harness
        .get_all_by_label("New profile")
        .next()
        .unwrap()
        .click();
    harness.run_steps(3);
    problems.extend(walk(&mut harness, "new profile"));
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
