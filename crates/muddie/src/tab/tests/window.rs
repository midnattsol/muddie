//! The window: its menu, its messages, what opens and stays, what is
//! announced, and how much drawing costs.

use super::*;

mod costs;
mod reading;

#[test]
fn what_the_client_says_can_be_read_again() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.note("Connected.");
    // An answer said in the command line for a moment is kept too.
    app.scripts
        .exec("t", r#"macro("f9", "kick", {tags = {"Combat"}})"#);
    app.run_command("/tag Combat off");
    app.run_command("/map find nowhere");
    harness.run_steps(3);
    let (said, unread) = harness.state().tab().messages();
    let texts: Vec<(&str, bool)> = said
        .iter()
        .map(|message| (message.text.as_str(), message.problem))
        .collect();
    assert_eq!(
        texts,
        [
            ("Connected.", false),
            ("Tag combat off.", false),
            ("Nothing on the map has nowhere.", true)
        ]
    );
    assert_eq!(unread, 3);
    // The bell says how many are new, and opening it shows them.
    harness.get_by_label("Messages, 3 new").click();
    harness.run_steps(3);
    // Over the corner for a moment, and under the bell.
    assert!(
        harness
            .query_all_by_label("Nothing on the map has nowhere.")
            .count()
            >= 2
    );
    assert_eq!(harness.state().tab().messages().1, 0);
}

#[test]
fn the_one_menu_opens_and_does_what_it_says() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    harness.get_by_label("Menu").click();
    harness.run_steps(3);
    harness.get_by_label("Keyboard shortcuts");
    harness.get_by_label("Settings").click();
    harness.run_steps(3);
    // The control named by its row; the row's words are hidden from a
    // screen reader, which hears them in the control.
    harness.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Language");
}

#[test]
fn what_a_click_opens_stays_open() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.started = true;
    app.rules.gauges.push(crate::rules::Gauge {
        name: "Salud".into(),
        value: "Char.Vitals.hp".into(),
        max: "100".into(),
        ..crate::rules::Gauge::starter()
    });
    app.gauges.set("Salud", Some((40.0, 100.0)));
    app.note("Connected.");
    harness.run_steps(3);
    let steady = |harness: &mut Harness<'_, crate::app::App>, what: &str, shown: &str| {
        // It may be laid out a few frames late; once there, it stays.
        harness.run_steps(4);
        for frame in 0..6 {
            assert!(
                harness.query_all_by_label(shown).next().is_some(),
                "{what}: gone at frame {frame}"
            );
            harness.step();
        }
        harness.key_press(Key::Escape);
        harness.run_steps(3);
    };
    let session = harness
        .get_all_by_role(egui::accesskit::Role::Tab)
        .find(|tab| {
            tab.accesskit_node()
                .label()
                .is_some_and(|label| label.contains("Not connected"))
        })
        .expect("the session's tab");
    session.click_secondary();
    steady(&mut harness, "session menu", "Save as profile");
    harness.get_by_label("Menu").click();
    steady(&mut harness, "menu", "Keyboard shortcuts");
    harness.get_by_label("Messages, 1 new").click();
    steady(&mut harness, "messages", "Clear");
    harness.get_by_label("Gauges options").click();
    steady(&mut harness, "panel menu", "Hide Gauges");
    gauge_button(&harness, "Salud")
        .expect("a gauge to edit")
        .click();
    steady(&mut harness, "gauge", "Open in the editor");
    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.get_by_label("Settings").click();
    steady(&mut harness, "settings", "Language");
}

#[test]
fn what_is_said_aloud_is_the_game_the_channels_and_the_client() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    let (events, _sent) = fake_connection(app);
    events
        .send(Event::Line(
            panels::styled_lines("An orc arrives.").remove(0),
        ))
        .unwrap();
    // A channel of the MUD's: said with its name before it.
    events
        .send(Event::Gmcp {
            package: "Comm.Channel.Text".into(),
            data: r#"{"channel": "ooc", "text": "Ana: hola"}"#.into(),
        })
        .unwrap();
    harness.run_steps(3);
    let said = |harness: &Harness<'_, App>| harness.state().tab().voice.said.clone();
    assert_eq!(said(&harness), ["An orc arrives.", "ooc: Ana: hola"]);
    // Kept quiet from the palette, a channel is not read; and silenced,
    // nothing is.
    let ctx = harness.ctx.clone();
    let app = harness.state_mut().tab_mut();
    let quiet = (app.palette_acts().into_iter())
        .find(|item| item.usage == "Read the ooc channel aloud (on)")
        .expect("the palette keeps a channel quiet");
    let act = quiet.act.expect("it does something");
    app.act(act, &ctx);
    app.voice.said.clear();
    events
        .send(Event::Gmcp {
            package: "Comm.Channel.Text".into(),
            data: r#"{"channel": "ooc", "text": "Ana: adiós"}"#.into(),
        })
        .unwrap();
    harness.run_steps(3);
    assert!(said(&harness).is_empty());
    // The game not read, Muddie still says what it says of itself.
    let app = harness.state_mut().tab_mut();
    app.voice.set_muted(true);
    events
        .send(Event::Line(
            panels::styled_lines("A troll arrives.").remove(0),
        ))
        .unwrap();
    harness.run_steps(3);
    assert!(said(&harness).is_empty());
    harness.state_mut().tab_mut().note("Still heard.");
    assert_eq!(said(&harness), ["Still heard."]);
    // Read again: a tell that comes as a line and over GMCP is read once.
    let app = harness.state_mut().tab_mut();
    app.voice.set_muted(false);
    app.set_channel_quiet("ooc", false);
    app.voice.said.clear();
    events
        .send(Event::Line(panels::styled_lines("Ana: otra vez").remove(0)))
        .unwrap();
    events
        .send(Event::Gmcp {
            package: "Comm.Channel.Text".into(),
            data: r#"{"channel": "ooc", "text": "Ana: otra vez"}"#.into(),
        })
        .unwrap();
    harness.run_steps(3);
    assert_eq!(said(&harness), ["Ana: otra vez"]);
}

#[test]
fn what_the_palette_finds_is_said_once_typing_pauses() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    app.toggle_palette();
    harness.run_steps(3);
    let field = "Palette";
    (harness.get_by_role_and_label(egui::accesskit::Role::TextInput, field)).type_text("zzzz");
    harness.run_steps(2);
    // Nothing yet: what is typed may still change.
    assert!(harness.state().tab().voice.said.is_empty());
    std::thread::sleep(std::time::Duration::from_millis(600));
    harness.run_steps(2);
    assert_eq!(harness.state().tab().voice.said, ["No matches."]);
    assert!(harness.query_by_label("No matches.").is_some());
}

#[test]
fn a_dialog_opened_with_the_keyboard_has_it_and_says_its_name() {
    let mut harness = window(None);
    harness.run_steps(3);
    // Opened after a click, nothing in it is marked.
    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    harness.get_by_label("Menu").click();
    harness.run_steps(2);
    harness.get_by_label("Settings").click();
    harness.run_steps(4);
    assert!(!harness.get_by_label("Close").accesskit_node().is_focused());
    harness.get_by_role_and_label(egui::accesskit::Role::Dialog, "Settings");
    harness.key_press(egui::Key::Escape);
    harness.run_steps(3);
    // Opened with the keyboard, the keyboard is in it, not in the command
    // line behind it.
    harness.key_press(egui::Key::K);
    harness
        .state_mut()
        .open_settings(crate::app::Section::General);
    harness.run_steps(4);
    assert!(harness.get_by_label("Close").accesskit_node().is_focused());
}

#[test]
fn alt_m_says_where_you_are_and_how_far_a_walk_has_left() {
    let mut harness = connected_demo_window();
    let app = harness.state_mut().tab_mut();
    app.voice.force_voice();
    harness.run_steps(2);
    let app = harness.state_mut().tab_mut();
    app.voice.said.clear();
    let lines = app.lines();
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::M);
    harness.run_steps(2);
    let said = harness.state().tab().voice.said.clone();
    assert_eq!(
        said,
        ["Plaza Mayor, ciudad. Exits: north, east, south, west."]
    );
    // Said only: the output is left as it was.
    assert_eq!(harness.state().tab().lines(), lines);
    let app = harness.state_mut().tab_mut();
    app.voice.said.clear();
    app.walk_to(1);
    harness.key_press_modifiers(crate::shortcuts::WITH_KEY, Key::M);
    harness.run_steps(2);
    let said = harness.state().tab().voice.said.clone();
    assert!(
        said.last()
            .unwrap()
            .ends_with("Going to Puerta, ciudad: 1 move left."),
        "{said:?}"
    );
}

#[test]
fn what_is_said_out_of_sight_of_the_game_is_shown_over_the_corner() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    harness.run_steps(2);
    // In sight, the lines of the game say it, and nothing floats.
    harness.state_mut().tab_mut().note("Said in sight.");
    harness.run_steps(2);
    assert_eq!(
        harness.query_all_by_label("Said in sight.").count(),
        1,
        "said twice"
    );
    // In the editor, a message says it, moving nothing, and goes.
    harness
        .state_mut()
        .tab_mut()
        .open_automation(automation::TRIGGERS);
    harness.run_steps(2);
    harness.state_mut().tab_mut().note("Said in the editor.");
    harness.run_steps(2);
    harness.get_by_label("Said in the editor.");
    std::thread::sleep(std::time::Duration::from_millis(6100));
    harness.run_steps(2);
    assert!(harness.query_by_label("Said in the editor.").is_none());
}

/// A long value in what the MUD sends, with the codes of colours a MUD
/// puts in text, leaves its path the room to be read on one line, and is
/// shown in its colours, cut short, its words alone told to a screen
/// reader.
#[test]
fn a_long_coloured_value_leaves_its_path_room() {
    let mut harness = connected_demo_window();
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.saw_data(
        "Comm.MensajeCanal",
        false,
        r#"{"canal": "chat", "mensaje": "[Chat] Szhysszh | \u001b[1m \u001b[30mGeneral  \u001b[37mde la  \u001b[35mSalsa \u001b[37m|: \u001b[31m pues si lo quieres para autismo y npc, y para lo que haga falta en la partida"}"#,
    );
    app.set_view(View::Data);
    harness.run_steps(3);
    let words = |name: &str| {
        harness
            .get_by_role_and_label(egui::accesskit::Role::Label, name)
            .rect()
    };
    let (path, line) = (
        words("Comm.MensajeCanal.mensaje"),
        words("Comm.MensajeCanal.canal"),
    );
    assert!(
        path.height() <= line.height() + 1.0,
        "the path is cut up: {path:?}"
    );
    let value = harness
        .query_all(egui_kittest::kittest::by())
        .filter(|node| node.accesskit_node().role() == egui::accesskit::Role::Label)
        .filter_map(|node| node.accesskit_node().value())
        .find(|value| value.starts_with("[Chat]"))
        .expect("the value is there");
    assert!(!value.contains('\u{1b}'), "{value:?}");
    assert!(value.ends_with("en la partida"), "told whole: {value:?}");
}
