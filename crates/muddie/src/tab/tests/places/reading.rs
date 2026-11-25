//! What is heard of Reinos de Leyenda with its plugin: what changes, and
//! nothing twice.

use super::*;

/// A session with the plugin, and what to send it.
struct Heard {
    harness: Harness<'static, App>,
    events: std::sync::mpsc::Sender<Event>,
}

impl Heard {
    fn start() -> Self {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        let (events, _sent) = fake_connection(app);
        app.voice.away = false;
        reinos(app);
        app.voice.said.clear();
        Self { harness, events }
    }

    /// What is said as a package comes, and what of it cuts in.
    fn gmcp(&mut self, package: &str, data: &str) -> (Vec<String>, Vec<String>) {
        let event = Event::Gmcp {
            package: package.into(),
            data: data.into(),
        };
        self.events.send(event).unwrap();
        self.harness.run_steps(2);
        let voice = &mut self.harness.state_mut().tab_mut().voice;
        (
            std::mem::take(&mut voice.said),
            std::mem::take(&mut voice.cut),
        )
    }
}

const SET: &str = "Char.Status.Estados.set";
const ADD: &str = "Char.Status.Estados.add";
const REMOVE: &str = "Char.Status.Estados.remove";
const BLEEDING: &str =
    r#"{"nombre":"sangrado","corto":"Sangrado","tipo":"enfermedades","ayuda":"Pierdes vida."}"#;
const STUNNED: &str = r#"{"nombre":"aturdido","corto":"Aturdido","tipo":"incapacidades","ayuda":"No puedes actuar."}"#;

#[test]
fn of_the_states_only_what_changes_is_said() {
    let mut heard = Heard::start();
    let mut said = |package: &str, data: &str| heard.gmcp(package, data).0;
    // Those there are on logging in are not news.
    assert_eq!(
        said(SET, &format!(r#"{{"sangrado":{BLEEDING}}}"#)),
        [] as [String; 0]
    );
    assert_eq!(said(ADD, STUNNED), ["Aturdido."]);
    // The same again is none.
    assert_eq!(said(ADD, STUNNED), [] as [String; 0]);
    assert_eq!(
        said(REMOVE, r#"{"nombre":"aturdido"}"#),
        ["No longer: Aturdido."]
    );
    assert_eq!(said(REMOVE, r#"{"nombre":"aturdido"}"#), [] as [String; 0]);
    // All of them sent again: what is not as it was.
    assert_eq!(
        said(SET, &format!(r#"{{"aturdido":{STUNNED}}}"#)),
        ["Aturdido.", "No longer: Sangrado."]
    );
    // After connecting again, they are not news either.
    let app = heard.harness.state_mut().tab_mut();
    app.scripts.on_event("disconnect", "");
    app.run_actions();
    let set = format!(r#"{{"sangrado":{BLEEDING}}}"#);
    assert_eq!(heard.gmcp(SET, &set).0, [] as [String; 0]);
}

/// With the reading switched off, nothing is said of the states, which are
/// shown all the same.
#[test]
fn the_reading_can_be_switched_off_alone() {
    let mut heard = Heard::start();
    heard.gmcp(SET, "{}");
    heard
        .harness
        .state_mut()
        .tab_mut()
        .scripts
        .unload("Lectura.luau");
    assert_eq!(heard.gmcp(ADD, STUNNED).0, [] as [String; 0]);
    let summary = heard.harness.state().tab().gauges_said();
    assert_eq!(summary, "Efectos: Aturdido");
}

/// Life cuts in only when it is about to run out; energy and faith wait.
#[test]
fn life_cuts_in_only_under_its_lowest_step() {
    let mut heard = Heard::start();
    let status = |pvs: u32, pe: u32, fe: u32| {
        format!(
            r#"{{"pvs":{{"min":{pvs},"max":100}},"pe":{{"min":{pe},"max":100}},"fe":{{"min":{fe},"max":100}}}}"#
        )
    };
    assert_eq!(
        heard.gmcp("Char.Status", &status(100, 100, 100)).0,
        [] as [String; 0]
    );
    let (said, cut) = heard.gmcp("Char.Status", &status(55, 100, 100));
    assert_eq!(
        (said.as_slice(), cut.len()),
        (["Vida 55 of 100".to_owned()].as_slice(), 0)
    );
    let (said, cut) = heard.gmcp("Char.Status", &status(25, 100, 100));
    assert_eq!(said, ["Vida 25 of 100"]);
    assert_eq!(cut, ["Vida 25 of 100"]);
    let (said, cut) = heard.gmcp("Char.Status", &status(25, 10, 10));
    assert_eq!(said, ["Energía 10 of 100", "Fe 10 of 100"]);
    assert_eq!(cut, [] as [String; 0]);
}

/// What was said before logging in is kept in its channel, not read.
#[test]
fn what_was_said_on_the_channels_before_is_not_read() {
    let mut heard = Heard::start();
    let before = r#"{"canal":"chat","mensaje":"Ana: ayer"}"#;
    let (said, _) = heard.gmcp("Comm.MensajeCanalHistorico", before);
    assert_eq!(said, [] as [String; 0]);
    let now = r#"{"canal":"chat","mensaje":"Ana: hoy"}"#;
    assert_eq!(heard.gmcp("Comm.MensajeCanal", now).0, ["chat: Ana: hoy"]);
    let channels = &heard.harness.state().tab().panels.channels;
    assert_eq!(channels[0].log.len(), 2);
}
