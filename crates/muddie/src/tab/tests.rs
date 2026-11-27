//! Tests of a session, driven as a user would: through the window, with
//! the connection stood in for.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use muddie_mapper::{Pos, RoomInfo};

use super::input::PARTIAL_SETTLE;
use super::*;
use crate::app::App;
use crate::rules::{Alias, Macro, Timer, Trigger};

/// A trigger on `pattern`, with what `with` gives it.
fn trigger(pattern: &str, with: impl FnOnce(&mut Trigger)) -> Trigger {
    let mut rule = Trigger::default();
    rule.when.pattern = pattern.into();
    with(&mut rule);
    rule
}

/// An alias on `pattern`, with what `with` gives it.
fn alias(pattern: &str, with: impl FnOnce(&mut Alias)) -> Alias {
    let mut rule = Alias::default();
    rule.when.pattern = pattern.into();
    with(&mut rule);
    rule
}

mod aloud;
mod arranging;
mod asking;
mod channels;
mod connection;
mod data;
mod editor;
mod input;
mod keyboard;
mod media;
mod official;
mod places;
mod profiles;
mod screenshots;
mod stops;
mod window;

/// A window that is not drawn, using `profiles` as its configuration.
fn window<'a>(profiles: Option<Profiles>) -> Harness<'a, App> {
    // Tests read the interface's text, so it must not follow the
    // language of whatever machine they run on.
    let english = |cc: &eframe::CreationContext<'_>, startup| {
        let app = App::new(cc, startup, Default::default());
        crate::i18n::set_language("en");
        app
    };
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1100.0, 700.0))
        .build_eframe(|cc| {
            let startup = Startup {
                profiles,
                profile: None,
                target: None,
                speech: false,
                scripts: Vec::new(),
                download: Default::default(),
            };
            english(cc, startup)
        });
    // A new session opens on the welcome screen; most tests are about
    // a session in use.
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(2);
    harness
}

/// What the client has said, over the corner and under the bell.
/// The map, as a screen reader finds it: a group called Map that says
/// its keys.
fn map_widget<'h, 'a>(harness: &'h Harness<'a, App>) -> Option<egui_kittest::Node<'h>> {
    (harness.query_all_by_role(egui::accesskit::Role::Group)).find(|node| {
        let node = node.accesskit_node();
        node.label().as_deref() == Some("Map")
            && node.description().is_some_and(|about| about.contains("F1"))
    })
}

/// The button that edits a gauge, named by what the gauge says.
fn gauge_button<'h, 'a>(
    harness: &'h Harness<'a, App>,
    name: &str,
) -> Option<egui_kittest::Node<'h>> {
    (harness.query_all_by_role(egui::accesskit::Role::Button)).find(|node| {
        node.accesskit_node()
            .label()
            .is_some_and(|label| label.starts_with(name))
    })
}

fn notes(app: &Tab) -> Vec<String> {
    (app.messages().0.iter())
        .map(|message| message.text.clone())
        .collect()
}

fn output_lines(app: &Tab) -> Vec<String> {
    (0..app.output.len())
        .map(|row| app.output.text(row).to_owned())
        .collect()
}

/// Puts the window in the state of having a live connection, and
/// returns the two ends a real session would hold.
fn fake_connection(
    app: &mut Tab,
) -> (
    std::sync::mpsc::Sender<Event>,
    tokio::sync::mpsc::UnboundedReceiver<Command>,
) {
    let (events, receiver) = channel();
    let (commands, sent) = tokio::sync::mpsc::unbounded_channel();
    app.sizes.window_untold = true;
    app.connection = Some(Connection {
        commands,
        events: receiver,
    });
    (events, sent)
}

/// A window with every panel in use, as after some minutes of play.
fn demo_window<'a>() -> Harness<'a, App> {
    demo(Harness::builder())
}

/// The same, drawn with the GPU, for pictures of it: a machine with none,
/// such as the CI's, has the others, which are not drawn.
fn drawn_demo_window<'a>() -> Harness<'a, App> {
    demo(Harness::builder().wgpu())
}

fn demo<'a>(builder: egui_kittest::HarnessBuilder<App>) -> Harness<'a, App> {
    let mut harness = builder
        .with_size(egui::vec2(1100.0, 700.0))
        .build_eframe(|cc| {
            let startup = Startup {
                profiles: None,
                profile: None,
                target: None,
                speech: false,
                scripts: Vec::new(),
                download: Default::default(),
            };
            let app = App::new(cc, startup, Default::default());
            crate::i18n::set_language("en");
            app
        });
    let app = harness.state_mut().tab_mut();
    app.started = true;
    app.host = "mud.example.org".into();
    app.port = "4000".into();
    app.tls = true;
    app.profile_name = "Reinos".into();
    app.note("Connected.");
    let text = "\x1b[1;32mPlaza Mayor\x1b[0m\n\
            Una plaza amplia con una fuente en el centro. Al norte se alza la \x1b[33mtorre\x1b[0m \
            del reloj y al este se abre un callejón estrecho que huele a pan recién hecho.\n\
            \x1b[36mSalidas: norte, este, sur\x1b[0m\n\
            \x1b[31mUn orco te ataca\x1b[0m con su \x1b[1;35mhacha rúnica\x1b[0m.\n\
            \x1b[38;5;208mAna te dice: ¿vienes a la torre?\x1b[0m";
    for line in panels::styled_lines(text) {
        app.output.push(&line);
    }
    app.output.push_local("> matar orco");
    app.output
        .set_partial(&panels::styled_lines("HP:80 MP:20> ")[0]);
    app.gauges.set("HP", Some((80.0, 100.0)));
    app.gauges.set("MP", Some((20.0, 50.0)));
    app.gauges.set("MV", Some((140.0, 150.0)));
    for line in panels::styled_lines("\x1b[33m[ooc] Ana: hola a todos\x1b[0m\n[ooc] Luis: buenas") {
        app.channel_line(&line);
    }
    app.show_in_panel("Minimapa", Some("  #\n#-\x1b[1;33m@\x1b[0m-#\n  #"));
    app.input = "matar orco".into();
    {
        let mut map = app.map.borrow_mut();
        let room = |id, name: &str, area: &str, exits: &[(&str, RoomId)]| RoomInfo {
            id,
            name: name.into(),
            area: area.into(),
            exits: (exits.iter())
                .map(|(direction, to)| ((*direction).to_owned(), *to))
                .collect(),
            pos: None,
            colour: None,
        };
        for info in [
            room(1, "Puerta", "ciudad", &[("n", 2)]),
            room(
                2,
                "Plaza Mayor",
                "ciudad",
                &[("s", 1), ("n", 3), ("e", 4), ("w", 9)],
            ),
            room(3, "Torre", "ciudad", &[("s", 2), ("u", 5), ("portal", 20)]),
            room(4, "Callejón", "ciudad", &[("w", 2), ("e", 6), ("ne", 7)]),
            room(6, "Panadería", "ciudad", &[("w", 4)]),
            room(7, "Patio", "ciudad", &[("sw", 4), ("n", 8)]),
            room(8, "Camino", "afueras", &[("s", 7)]),
            room(4, "Callejón", "ciudad", &[("w", 2), ("e", 6), ("ne", 7)]),
            room(
                2,
                "Plaza Mayor",
                "ciudad",
                &[("s", 1), ("n", 3), ("e", 4), ("w", 9)],
            ),
        ] {
            map.visit(info).unwrap();
        }
        map.set_avoid(6, true).unwrap();
        // Coloured as a map taken from Mudlet would be: stone in town,
        // grass outside.
        for (id, colour) in [
            (1, [150, 140, 125]),
            (2, [196, 170, 120]),
            (7, [96, 150, 80]),
            (8, [70, 140, 60]),
        ] {
            map.set_colour(id, Some(colour)).unwrap();
        }
        map.edit_exit(4, "ne", |exit| exit.blocked = true).unwrap();
    }
    harness.run_steps(3);
    harness
}

/// The same window as it looks during play, with the connection open.
fn connected_demo_window<'a>() -> Harness<'a, App> {
    connected(demo_window())
}

/// The same, drawn with the GPU, for pictures of it.
fn connected_drawn_demo_window<'a>() -> Harness<'a, App> {
    connected(drawn_demo_window())
}

fn connected(mut harness: Harness<'_, App>) -> Harness<'_, App> {
    // Dropping the far ends is fine: nothing is sent or received here.
    fake_connection(harness.state_mut().tab_mut());
    harness.run_steps(3);
    harness
}
