//! What a screen reader user meets with the keyboard, forward and back:
//! nothing that is only for the pointer, nothing said twice, and the name
//! of what is come to.

use super::*;
use egui::accesskit::Role;

/// What is said of where the keyboard is: its kind, name and description.
fn said(harness: &Harness<'_, App>) -> Option<(String, Role, String, String)> {
    harness
        .query_all(egui_kittest::kittest::by())
        .find(|node| node.is_focused())
        .map(|node| {
            let node = node.accesskit_node();
            (
                format!("{:?}", node.id()),
                node.role(),
                node.label().unwrap_or_default().to_string(),
                node.description().unwrap_or_default().to_string(),
            )
        })
}

/// Goes round with Tab, with `modifiers`, and says what should not be met.
fn round(harness: &mut Harness<'_, App>, view: &str, modifiers: Modifiers) -> Vec<String> {
    let (mut problems, mut seen) = (Vec::new(), Vec::new());
    for _ in 0..120 {
        harness.key_press_modifiers(modifiers, Key::Tab);
        harness.run_steps(3);
        let Some((id, role, name, about)) = said(harness) else {
            problems.push(format!("{view}: Tab left the keyboard nowhere"));
            break;
        };
        if seen.contains(&id) {
            break;
        }
        seen.push(id);
        if matches!(role, Role::Splitter | Role::Image | Role::Unknown) {
            problems.push(format!("{view}: a stop at a {role:?}, {name:?}"));
        }
        if !about.is_empty() && name.contains(&about) {
            problems.push(format!("{view}: {name:?} is also said to be {about:?}"));
        }
    }
    problems
}

fn both_ways(harness: &mut Harness<'_, App>, view: &str) -> Vec<String> {
    let mut problems = round(harness, view, Modifiers::NONE);
    problems.extend(round(harness, &format!("{view}, back"), Modifiers::SHIFT));
    problems
}

fn to_the_line(harness: &mut Harness<'_, App>) {
    let tab = harness.state_mut().tab_mut();
    tab.reader.open = false;
    tab.input.clear();
    tab.focus_line = true;
    harness.run_steps(4);
}

/// Playing with every panel in use, and a screen reader.
fn playing<'a>() -> Harness<'a, App> {
    let mut harness = connected_demo_window();
    harness.state_mut().tab_mut().voice.pretend_reader = true;
    harness.run_steps(3);
    to_the_line(&mut harness);
    harness
}

#[test]
fn playing_nothing_for_the_pointer_or_said_twice_is_come_to() {
    let mut harness = playing();
    let mut problems = round(&mut harness, "game, back", Modifiers::SHIFT);
    // From each panel on, which Tab alone does not go to.
    for presses in 1..5 {
        to_the_line(&mut harness);
        for _ in 0..presses {
            harness.key_press(Key::F6);
            harness.run_steps(3);
        }
        problems.extend(both_ways(&mut harness, &format!("place {presses}")));
    }
    // With a side put away.
    for name in ["Collapse the right side", "Collapse the bottom side"] {
        harness.get_all_by_label(name).next().unwrap().click();
        harness.run_steps(4);
        to_the_line(&mut harness);
        problems.extend(both_ways(&mut harness, name));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn what_is_only_for_the_pointer_is_not_told_to_a_screen_reader() {
    let pointed = |harness: &Harness<'_, App>| {
        (harness.query_all(egui_kittest::kittest::by()))
            .filter(|node| !node.accesskit_node().is_hidden())
            .filter(|node| {
                let node = node.accesskit_node();
                let name = node.label().unwrap_or_default().to_string();
                node.role() == Role::Splitter || name.starts_with("Move ")
            })
            .count()
    };
    let mut harness = playing();
    assert_eq!(pointed(&harness), 0);
    // It is there for who has none.
    harness.state_mut().tab_mut().voice.pretend_reader = false;
    harness.run_steps(3);
    assert!(pointed(&harness) > 3);
}

/// A gauge to press is one thing to a screen reader: the button, with what
/// the gauge says, and not its drawing too.
#[test]
fn a_gauge_to_edit_is_told_once() {
    let mut harness = connected_demo_window();
    let app = harness.state_mut().tab_mut();
    for (name, kind) in [
        ("Bar", muddie_script::GaugeKind::Bar),
        ("Number", muddie_script::GaugeKind::Number),
        ("Light", muddie_script::GaugeKind::Light),
    ] {
        app.rules.gauges.push(crate::rules::Gauge {
            name: name.into(),
            kind,
            value: "Told.value".into(),
            max: "10".into(),
            ..Default::default()
        });
    }
    app.run_rules();
    app.scripts.on_gmcp("Told", r#"{"value": 7}"#);
    app.run_actions();
    harness.run_steps(6);
    for name in ["Bar", "Number", "Light"] {
        let told: Vec<(Role, String)> = (harness.query_all(egui_kittest::kittest::by()))
            .filter(|node| !node.accesskit_node().is_hidden())
            .map(|node| {
                let node = node.accesskit_node();
                (node.role(), node.label().unwrap_or_default().to_string())
            })
            .filter(|(_, label)| label.starts_with(name))
            .collect();
        let roles: Vec<Role> = told.iter().map(|(role, _)| *role).collect();
        assert_eq!(roles, [Role::Button], "{told:?}");
    }
}

/// What a screen reader is told of that says `name`, in the tree as it goes
/// through it.
fn told_of(harness: &Harness<'_, App>, name: &str) -> Vec<String> {
    (harness.query_all(egui_kittest::kittest::by()))
        .map(|node| node.accesskit_node())
        // Hidden, with all that is in it.
        .filter(|node| {
            std::iter::successors(Some(*node), |node| node.parent()).all(|node| !node.is_hidden())
        })
        .map(|node| node.label().unwrap_or_default().to_string())
        .filter(|label| label.contains(name))
        .collect()
}

/// With a screen reader the panels of gauges are drawn, and not there for
/// it: not told, nor come to with F6 or Tab, alone, as a tab of others or
/// in a side put away. They are asked for with Alt+0.
#[test]
fn a_panel_of_gauges_is_not_there_for_a_screen_reader() {
    let mut harness = playing();
    let app = harness.state_mut().tab_mut();
    app.gauges.set("Vigour", Some((7.0, 10.0)));
    // Beside the output, where Tab goes by.
    let is_gauges = |panel: &&layout::PanelSpec| panel.kind == layout::Kind::Gauges;
    let id = app.layout.panels.iter().find(is_gauges).unwrap().id;
    app.drop_panel(id, None, layout::Side::Right);
    assert!(app.gauges_said().contains("Vigour 7 of 10"));
    harness.run_steps(4);
    for name in ["Vigour", "Gauges"] {
        assert_eq!(told_of(&harness, name), [] as [String; 0]);
    }
    assert!(!told_of(&harness, "Minimapa").is_empty());
    to_the_line(&mut harness);
    // Back from the command line, past every panel.
    let mut stops = Vec::new();
    for _ in 0..20 {
        harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
        harness.run_steps(3);
        stops.extend(said(&harness).map(|said| said.2));
    }
    assert!(stops.contains(&"Minimapa options".to_owned()), "{stops:?}");
    let of_gauges = |stop: &&String| stop.contains("Gauges") || stop.contains("Vigour");
    assert_eq!(stops.iter().find(of_gauges), None);
    // Drawn all the same, and pressed with the pointer.
    let options = harness.get_by_label("Gauges options").rect();
    harness.hover_at(options.center());
    harness.drag_at(options.center());
    harness.drop_at(options.center());
    harness.run_steps(4);
    assert!(harness.query_by_label("Edit the gauges").is_some());
    harness.key_press(Key::Escape);
    harness.run_steps(4);
    // As a tab of another panel, which is what is come to.
    let app = harness.state_mut().tab_mut();
    let kind = |app: &Tab, kind: layout::Kind| {
        let found = app
            .layout
            .panels
            .iter()
            .position(|panel| panel.kind == kind);
        found.unwrap()
    };
    let (gauges, text) = (
        kind(app, layout::Kind::Gauges),
        kind(app, layout::Kind::Text),
    );
    let side = app.layout.panels[text].side;
    for index in [gauges, text] {
        app.layout.panels[index].stack = 77;
        app.layout.panels[index].side = side;
    }
    let shown = app.layout.panels[gauges].id;
    app.panels.shown_tabs.push((77, shown));
    harness.run_steps(4);
    assert_eq!(told_of(&harness, "Gauges"), [] as [String; 0]);
    assert!(!told_of(&harness, "Minimapa").is_empty());
    to_the_line(&mut harness);
    let mut come = Vec::new();
    for _ in 0..4 {
        harness.key_press(Key::F6);
        harness.run_steps(3);
        come.extend(said(&harness).map(|said| said.2));
    }
    assert!(come.contains(&"Minimapa".to_owned()), "{come:?}");
    assert!(both_ways(&mut harness, "tabs").is_empty());
    // With its side put away: the others bring it back.
    harness.state_mut().tab_mut().toggle_side(side);
    harness.run_steps(4);
    assert_eq!(told_of(&harness, "Gauges"), [] as [String; 0]);
    assert!(!told_of(&harness, "Show Minimapa").is_empty());
    // It is there again once the reader goes.
    harness.state_mut().tab_mut().voice.pretend_reader = false;
    harness.run_steps(4);
    assert!(!told_of(&harness, "Show Gauges").is_empty());
}

#[test]
fn f6_says_which_panel_it_comes_to_and_tab_goes_on_into_it() {
    let mut harness = playing();
    let mut come = Vec::new();
    for _ in 0..4 {
        harness.key_press(Key::F6);
        for _ in 0..3 {
            harness.run_steps(1);
            let (_, role, name, _) = said(&harness).unwrap();
            if come.last() != Some(&(role, name.clone())) {
                come.push((role, name));
            }
        }
    }
    // Nothing else has the keyboard on the way, in any frame.
    let come: Vec<_> = (come.iter())
        .map(|(role, name)| (*role, name.as_str()))
        .collect();
    assert_eq!(
        come,
        [
            (Role::MultilineTextInput, "Output"),
            (Role::Heading, "Map"),
            (Role::Heading, "Minimapa"),
            (Role::TextInput, "Command"),
        ]
    );
    to_the_line(&mut harness);
    for _ in 0..3 {
        harness.key_press(Key::F6);
        harness.run_steps(3);
    }
    harness.key_press(Key::Tab);
    harness.run_steps(3);
    assert_eq!(said(&harness).unwrap().2, "Minimapa options");
    // The name is not a stop on the way back.
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    harness.run_steps(3);
    assert_ne!(said(&harness).unwrap().1, Role::Heading);
}

/// Presses `key` and says what has the keyboard in each frame drawn since.
fn frames(harness: &mut Harness<'_, App>, key: Key, modifiers: Modifiers) -> Vec<(Role, String)> {
    harness.input_mut().events.push(egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
    let mut met = Vec::new();
    for _ in 0..4 {
        harness.step();
        if let Some((_, role, name, _)) = said(harness) {
            met.push((role, name));
        }
    }
    met.dedup();
    met
}

#[test]
fn the_output_is_come_to_as_text_and_nothing_before_it() {
    let text = (Role::MultilineTextInput, "Output".to_owned());
    let line = (Role::TextInput, "Command".to_owned());
    for (key, modifiers) in [
        (Key::Tab, Modifiers::NONE),
        (Key::Tab, Modifiers::SHIFT),
        (Key::F6, Modifiers::NONE),
    ] {
        let mut harness = playing();
        let met = frames(&mut harness, key, modifiers);
        assert_eq!(met.last(), Some(&text), "{key:?} {modifiers:?}");
        let between: Vec<_> = (met.iter())
            .filter(|at| **at != text && **at != line)
            .collect();
        assert!(between.is_empty(), "{key:?} {modifiers:?}: {between:?}");
    }
    // Coming on from a panel, with Tab.
    let mut harness = playing();
    for _ in 0..3 {
        harness.key_press(Key::F6);
        harness.run_steps(3);
    }
    let mut met = Vec::new();
    for _ in 0..4 {
        met.extend(frames(&mut harness, Key::Tab, Modifiers::NONE));
        if met.last() == Some(&text) {
            break;
        }
    }
    assert_eq!(met.last(), Some(&text), "{met:?}");
    let odd = |(role, name): &&(Role, String)| name == "Output" && *role != text.0;
    assert_eq!(met.iter().find(odd), None);
}

#[test]
fn going_back_to_the_line_and_completing_come_to_nothing_else() {
    let line = (Role::TextInput, "Command".to_owned());
    let text = (Role::MultilineTextInput, "Output".to_owned());
    for key in [Key::Tab, Key::Escape, Key::Enter] {
        let mut harness = playing();
        frames(&mut harness, Key::Tab, Modifiers::NONE);
        let met = frames(&mut harness, key, Modifiers::NONE);
        assert_eq!(met.last(), Some(&line), "{key:?}");
        assert!(met.iter().all(|at| *at == line || *at == text), "{met:?}");
    }
    // With something typed, Tab completes it and stays.
    let mut harness = playing();
    harness.state_mut().tab_mut().input = "hach".into();
    harness.run_steps(2);
    let met = frames(&mut harness, Key::Tab, Modifiers::NONE);
    assert_eq!(met, [line]);
    assert_eq!(harness.state().tab().input, "hacha");
}

#[test]
fn a_side_is_resized_with_the_pointer_with_a_screen_reader_too() {
    let mut harness = playing();
    let width = |harness: &Harness<'_, App>| {
        let layout = &harness.state().tab().layout;
        (layout.panels.iter())
            .find(|panel| panel.side == layout::Side::Right)
            .unwrap()
            .size
    };
    let before = width(&harness);
    let size = harness.ctx.content_rect();
    let from = egui::pos2(size.right() - before, size.center().y - 60.0);
    harness.drag_at(from);
    harness.run_steps(2);
    for step in 1..=4 {
        harness.hover_at(from - egui::vec2(10.0 * step as f32, 0.0));
        harness.run_steps(1);
    }
    harness.drop_at(from - egui::vec2(40.0, 0.0));
    harness.run_steps(3);
    let after = width(&harness);
    assert!((after - before - 40.0).abs() < 3.0, "{before} to {after}");
    // It stays so as the reader goes.
    harness.state_mut().tab_mut().voice.pretend_reader = false;
    harness.run_steps(3);
    assert!((width(&harness) - after).abs() < 1.0);
}

#[test]
fn in_the_other_windows_nothing_for_the_pointer_or_said_twice_is_come_to() {
    let dir = std::env::temp_dir().join(format!("muddie-stops-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Reinos", &crate::profile::Profile::default())
        .unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1300.0, 860.0));
    harness.state_mut().tab_mut().voice.pretend_reader = true;
    harness.state_mut().tab_mut().started = false;
    harness.run_steps(3);
    let mut problems = both_ways(&mut harness, "welcome");
    let (_events, _sent) = fake_connection(harness.state_mut().tab_mut());
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    for section in automation::Section::ALL {
        harness.state_mut().tab_mut().open_automation(section);
        harness.run_steps(3);
        problems.extend(both_ways(&mut harness, &format!("editor {section:?}")));
    }
    harness.state_mut().tab_mut().set_view(View::Layout);
    harness.run_steps(3);
    problems.extend(both_ways(&mut harness, "layout"));
    harness.state_mut().tab_mut().set_view(View::Game);
    harness.run_steps(2);
    for section in crate::app::Section::ALL {
        harness.state_mut().open_settings(section);
        harness.run_steps(3);
        let view = format!("settings {}", section.name());
        problems.extend(both_ways(&mut harness, &view));
    }
    harness.state_mut().close_settings_for_tests();
    harness.run_steps(2);
    harness.state_mut().edit_profile("Reinos");
    harness.run_steps(3);
    for tab in ["Connection", "Session", "Plugins"] {
        harness.get_by_role_and_label(Role::Tab, tab).click();
        harness.run_steps(3);
        problems.extend(both_ways(&mut harness, &format!("profile {tab}")));
    }
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// From the command line, Shift+Tab goes back through the window as it is
/// seen: the panels below, the output, and those beside and above it, with
/// something typed or not; and F6 goes to each.
#[test]
fn every_panel_is_come_to_whatever_side_it_is_on() {
    use layout::Side;
    for (side, reader) in [Side::Top, Side::Left, Side::Right, Side::Bottom]
        .into_iter()
        .flat_map(|side| [(side, false), (side, true)])
    {
        let mut harness = connected_demo_window();
        harness.state_mut().tab_mut().voice.pretend_reader = reader;
        let app = harness.state_mut().tab_mut();
        let is_text = |panel: &&layout::PanelSpec| panel.kind == layout::Kind::Text;
        let id = app.layout.panels.iter().find(is_text).unwrap().id;
        app.layout.panels.retain(|panel| panel.id == id);
        app.drop_panel(id, None, side);
        harness.run_steps(6);
        for typed in ["", "look"] {
            to_the_line(&mut harness);
            harness.state_mut().tab_mut().input = typed.into();
            let mut met = Vec::new();
            for _ in 0..16 {
                harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
                harness.run_steps(3);
                met.extend(said(&harness).map(|said| said.2));
            }
            let at = |name: &str| met.iter().position(|met| met.contains(name));
            let (panel, output) = (at("Minimapa options"), at("Output"));
            let case = format!("{side:?}, reader {reader}, {typed:?}: {met:?}");
            assert!(panel.is_some() && output.is_some(), "{case}");
            // As it is seen, from the bottom up.
            assert_eq!(panel < output, side == Side::Bottom, "{case}");
            assert_eq!(harness.state().tab().input, typed, "{case}");
        }
        to_the_line(&mut harness);
        let mut met = Vec::new();
        for _ in 0..3 {
            harness.key_press(Key::F6);
            harness.run_steps(3);
            met.extend(said(&harness).map(|said| said.2));
        }
        assert!(met.contains(&"Minimapa".to_owned()), "{side:?}: {met:?}");
    }
}

mod small;
