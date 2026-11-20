//! The MUD's channels, each in its tab.

use super::*;

#[test]
fn channels_have_a_tab_each_in_the_panel_of_channels() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    // A script says what was said on which channel; and a channel
    // can be made known before anyone speaks on it.
    app.scripts.exec(
        "t",
        r#"channel("chat", "Ana: hola") channel("mercenario") channel("chat", "Luis: buenas")"#,
    );
    app.run_actions();
    // The usual message of GMCP names its channel too.
    let said = r#"{"channel": "bando", "talker": "Eva", "text": "Eva: al norte"}"#;
    let (events, _sent) = fake_connection(app);
    events
        .send(Event::Gmcp {
            package: "Comm.Channel.Text".into(),
            data: said.into(),
        })
        .unwrap();
    harness.run_steps(3);
    let app = harness.state().tab();
    let names: Vec<&str> = app
        .panels
        .channels
        .iter()
        .map(|channel| channel.name.as_str())
        .collect();
    assert_eq!(names, ["chat", "mercenario", "bando"]);
    assert_eq!(app.panels.channels[0].log.len(), 2);
    // One panel, not one for each: the panel of channels shows all
    // of it, and a tab for each channel.
    assert_eq!(
        app.layout
            .panels
            .iter()
            .filter(|panel| panel.kind == layout::Kind::Text)
            .count(),
        1
    );
    harness.get_by_label("Luis: buenas");
    harness.get_by_label("Eva: al norte");
    // A channel's tab shows what was said on it alone.
    harness.get_by_label("mercenario");
    harness.get_by_label("chat").click();
    harness.run_steps(3);
    harness.get_by_label("Ana: hola");
    assert!(harness.query_by_label("Eva: al norte").is_none());
    // What is said on another meanwhile marks its tab, until looked at.
    let app = harness.state_mut().tab_mut();
    app.scripts.exec("t", r#"channel("bando", "Eva: esperad")"#);
    app.run_actions();
    harness.run_steps(2);
    let unread = |app: &Tab, name: &str| {
        app.panels
            .channels
            .iter()
            .any(|c| c.name == name && c.unread)
    };
    assert!(unread(harness.state().tab(), "bando") && !unread(harness.state().tab(), "chat"));
    harness.get_by_label("bando, new").click();
    harness.run_steps(3);
    assert!(!unread(harness.state().tab(), "bando"));
    harness.get_by_label("Eva: esperad");
}

#[test]
fn channels_are_given_a_panel_where_there_is_none_for_them() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    // A layout with a map and gauges, and nowhere for text.
    app.layout
        .panels
        .retain(|panel| panel.kind != layout::Kind::Text);
    assert!(!app.layout.panels.iter().any(|panel| panel.channels));
    app.scripts
        .exec("t", r#"channel("chat") channel("chat", "Ana: hola")"#);
    app.run_actions();
    harness.run_steps(3);
    // A panel was made for them, once, and what was said is in it.
    let app = harness.state_mut().tab_mut();
    let made: Vec<_> = app
        .layout
        .panels
        .iter()
        .filter(|panel| panel.channels)
        .cloned()
        .collect();
    assert_eq!(made.len(), 1);
    assert!(made[0].visible && made[0].kind == layout::Kind::Text);
    // Called Channels, whatever the language, under the map.
    assert_eq!(
        (made[0].name.as_str(), made[0].side),
        ("Channels", layout::Side::Right)
    );
    let app = harness.state().tab();
    let right: Vec<&str> = (app.layout.panels.iter())
        .filter(|panel| panel.side == layout::Side::Right)
        .map(|panel| panel.name.as_str())
        .collect();
    assert_eq!(right, ["Map", "Channels"]);
    harness.get_by_label("Ana: hola");
    harness.get_by_label("chat");
    // Hidden, it is not made again.
    let app = harness.state_mut().tab_mut();
    app.layout
        .panels
        .iter_mut()
        .for_each(|panel| panel.visible &= !panel.channels);
    app.scripts.exec("t", r#"channel("chat", "Luis: buenas")"#);
    app.run_actions();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert_eq!(
        app.layout
            .panels
            .iter()
            .filter(|panel| panel.channels)
            .count(),
        1
    );
}

#[test]
fn a_script_turns_a_muds_own_channel_messages_into_tabs() {
    // As a player of a MUD with channels of its own would have it: a
    // layout with no panel of text, and a script for its messages.
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    app.layout
        .panels
        .retain(|panel| panel.kind != layout::Kind::Text);
    let script = r#"supports("Comm 1")

on_gmcp("Comm.MensajeCanal", function(data)
    channel(data.canal, data.mensaje)
end)

on_gmcp("Comm.Canales", function(data)
    for nombre in pairs(data) do
        channel(nombre)
    end
end)"#;
    app.scripts.exec("Channels", script);
    app.run_actions();
    let (events, _sent) = fake_connection(app);
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    events
        .send(gmcp(
            "Comm.Canales",
            r#"{"chat": "chat", "mercenario": "mercenario"}"#,
        ))
        .unwrap();
    let said = "{\"canal\": \"chat\", \"mensaje\": \"[Chat] Naylhena \\u001b[1m\\u001b[30mSacerdotisa \\u001b[0mde la \\u001b[36mcalumnia\\u001b[0m: \\u001b[1m\\u001b[37mre\"}";
    events.send(gmcp("Comm.MensajeCanal", said)).unwrap();
    harness.run_steps(4);
    let app = harness.state().tab();
    let mut names: Vec<&str> = app
        .panels
        .channels
        .iter()
        .map(|channel| channel.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["chat", "mercenario"]);
    assert_eq!(
        app.layout
            .panels
            .iter()
            .filter(|panel| panel.channels && panel.visible)
            .count(),
        1
    );
    // The message, with its colours taken out of the way, in view,
    // and a tab for each channel.
    harness.get_by_label("[Chat] Naylhena Sacerdotisa de la calumnia: re");
    harness.get_by_label("chat");
    harness.get_by_label("mercenario");
}

#[test]
fn a_channel_goes_to_the_panel_a_script_names() {
    let mut harness = window(None);
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    // A panel the player made by hand, and put on the left.
    let id = app.panel_called("Conversaciones");
    let made = app.layout.panel_mut(id).unwrap();
    (made.side, made.size) = (layout::Side::Left, 300.0);
    let panels = app.layout.panels.len();
    // Named by a script, it is the one used, as it is; and one that
    // is not there is made.
    app.scripts.exec(
        "t",
        r#"channel("chat", "Ana: hola", {panel = "conversaciones"})
            channel("bando", "Eva: al norte", {panel = "Conversaciones"})
            channel("susurro", "Luis te susurra: ven", {panel = "Privado"})"#,
    );
    app.run_actions();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.layout.panels.len(), panels + 1);
    let kept = app.layout.panel(id).cloned().unwrap();
    assert!(kept.side == layout::Side::Left && kept.size == 300.0);
    let private = (app.layout.panels.iter())
        .find(|panel| panel.name == "Privado")
        .cloned()
        .unwrap();
    // Each panel has what was said on its channels, and their tabs.
    assert_eq!(app.text_of(id).log.len(), 2);
    assert_eq!(app.text_of(private.id).log.text(0), "Luis te susurra: ven");
    let tabs = |app: &Tab, panel: &str| -> Vec<String> {
        (app.panels.channels.iter())
            .filter(|channel| channel.panel.eq_ignore_ascii_case(panel))
            .map(|channel| channel.name.clone())
            .collect()
    };
    assert_eq!(tabs(app, "Conversaciones"), ["chat", "bando"]);
    assert_eq!(tabs(app, "Privado"), ["susurro"]);
    // The panel of channels, which none of them was sent to, has none.
    harness.get_by_label("bando").click();
    harness.run_steps(3);
    harness.get_by_label("Eva: al norte");
    assert!(harness.query_by_label("Ana: hola").is_none());
}
