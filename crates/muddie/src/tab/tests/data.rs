//! What the MUD sends besides text: kept to look at, and MXP.

use super::*;

#[test]
fn what_the_mud_sends_is_kept_to_look_at_and_made_into_gauges() {
    let mut harness = window(None);
    let (events, _sent) = fake_connection(harness.state_mut().tab_mut());
    // Kept from the start, as when it is asked to go on while it is not
    // on screen.
    harness.state_mut().tab_mut().data_view.keep = true;
    let vitals = |hp: u32| Event::Gmcp {
        package: "Char.Vitals".into(),
        data: format!(r#"{{"health": {{"current": {hp}, "max": 100}}}}"#),
    };
    events.send(vitals(40)).unwrap();
    events.send(vitals(62)).unwrap();
    let hunger = Event::Msdp {
        variable: "HUNGER".into(),
        data: "\"7\"".into(),
    };
    events.send(hunger).unwrap();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let seen: Vec<(&str, bool, u32)> = app
        .data_view
        .seen
        .iter()
        .map(|seen| (seen.name.as_str(), seen.msdp, seen.count))
        .collect();
    assert_eq!(seen, [("Char.Vitals", false, 2), ("HUNGER", true, 1)]);
    assert!(
        app.data_view.seen[0].json.contains("62")
            && app.data_view.seen[0].earlier[0].1.contains("40")
    );
    // The editor shows it, and a number there becomes a gauge with
    // its maximum found, in a panel of gauges.
    app.set_view(2);
    harness.run_steps(3);
    assert!(
        harness
            .query_all_by_label("Char.Vitals.health.current")
            .next()
            .is_some()
    );
    harness
        .get_all_by_label("Make a gauge")
        .next()
        .unwrap()
        .click();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let made = app
        .editor
        .draft
        .gauges
        .last()
        .expect("a gauge was made")
        .clone();
    assert_eq!(
        (made.name.as_str(), made.value.as_str(), made.max.as_str()),
        (
            "health",
            "Char.Vitals.health.current",
            "Char.Vitals.health.max"
        )
    );
    assert_eq!(app.view(), 1, "it is shown in the editor to be finished");
    // Saved, it follows the value, in the panel it was put in.
    app.close_automation();
    events.send(vitals(55)).unwrap();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.gauge_is_in("health", made.panel));
    assert!(app.gauges.summary().contains("health 55 of 100"));
}

#[test]
fn what_arrives_is_kept_only_while_it_is_looked_at() {
    let mut harness = window(None);
    let (events, mut sent) = fake_connection(harness.state_mut().tab_mut());
    let vitals = || Event::Gmcp {
        package: "Char.Vitals".into(),
        data: r#"{"hp": 5}"#.into(),
    };
    // With it off the screen, nothing is kept.
    events.send(vitals()).unwrap();
    harness.run_steps(3);
    assert!(harness.state().tab().data_view.seen.is_empty());
    // Open, it is: by the protocol each thing came by. The connection
    // is told to say what it otherwise passes over.
    harness.state_mut().tab_mut().set_view(2);
    harness.run_steps(2);
    let told: Vec<Command> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
    assert!(matches!(told.last(), Some(Command::Trace(true))));
    events.send(vitals()).unwrap();
    let tag = Event::Other {
        protocol: "MXP".into(),
        name: "send".into(),
        data: "<send href=\"look\">".into(),
    };
    events.send(tag).unwrap();
    events
        .send(Event::Protocol {
            name: "GMCP".into(),
            on: true,
        })
        .unwrap();
    harness.run_steps(3);
    let kept = |app: &Tab| -> Vec<String> {
        (app.data_view.seen.iter())
            .map(|seen| format!("{} {}", seen.protocol, seen.name))
            .collect()
    };
    assert_eq!(
        kept(harness.state().tab()),
        ["GMCP Char.Vitals", "MXP send"]
    );
    harness.get_by_label("GMCP: In use");
    // Choosing a protocol lists what came by it alone.
    harness.get_by_label("MXP: Not offered by this MUD").click();
    harness.run_steps(3);
    assert!(harness.query_by_label("Char.Vitals").is_none());
    // In the list, and laid out beside it.
    assert!(harness.query_all_by_label("send").count() >= 2);
    // Closed again, it stops, unless asked to go on.
    harness.state_mut().tab_mut().set_view(0);
    harness.run_steps(2);
    events.send(vitals()).unwrap();
    harness.run_steps(3);
    assert_eq!(harness.state().tab().data_view.seen[0].count, 1);
    harness.state_mut().tab_mut().data_view.keep = true;
    events.send(vitals()).unwrap();
    harness.run_steps(3);
    assert_eq!(harness.state().tab().data_view.seen[0].count, 2);
}

#[test]
fn what_mxp_asks_for_reaches_the_panels_the_gauges_and_the_command_line() {
    use muddie_core::Mxp;
    let mut harness = window(None);
    let (events, _sent) = fake_connection(harness.state_mut().tab_mut());
    let send = |event: Event| events.send(event).unwrap();
    let value = |name: &str, value: &str| {
        Event::Mxp(Mxp::Value {
            name: name.into(),
            value: value.into(),
        })
    };
    // A gauge follows values by name, once they are known.
    send(Event::Mxp(Mxp::Gauge {
        entity: "hp".into(),
        max: "maxhp".into(),
        caption: "Health".into(),
        colour: "red".into(),
        bar: true,
    }));
    send(value("hp", "62"));
    send(value("maxhp", "100"));
    // A frame is a panel by its name, and what is sent to it goes there.
    send(Event::Mxp(Mxp::Frame {
        name: "Scores".into(),
        open: true,
        title: String::new(),
        align: "left".into(),
    }));
    let mut line = panels::styled_lines("Ana 12").remove(0);
    line.dest = Some("Scores".into());
    send(Event::Line(line.clone()));
    send(Event::Line(panels::styled_lines("in the game").remove(0)));
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert_eq!(app.gauges.summary(), "Health 62 of 100");
    let panel = (app.layout.panels.iter())
        .find(|panel| panel.name == "Scores")
        .cloned()
        .unwrap();
    assert!(panel.visible && panel.side == layout::Side::Left);
    assert_eq!(app.text_of(panel.id).log.text(0), "Ana 12");
    assert_eq!(output_lines(app), ["in the game"]);
    // Emptied, and closed, when asked.
    send(Event::Mxp(Mxp::Clear("Scores".into())));
    send(Event::Mxp(Mxp::Frame {
        name: "scores".into(),
        open: false,
        title: String::new(),
        align: String::new(),
    }));
    // Where to go on is said, and left to the player.
    send(Event::Mxp(Mxp::Relocate {
        host: "mud.example".into(),
        port: "4001".into(),
    }));
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    let panel = (app.layout.panels.iter())
        .find(|panel| panel.name == "Scores")
        .cloned()
        .unwrap();
    assert!(!panel.visible && app.text_of(panel.id).log.len() == 0);
    assert!(
        output_lines(app)
            .iter()
            .any(|line| line.contains("mud.example, port 4001"))
    );
    assert!(app.connection.is_some());
    // A link that prompts puts its command in the command line.
    app.link_clicked(crate::output::Clicked::Prompt("tell guard".into()));
    assert_eq!(app.input, "tell guard ");
}
