//! The ways of playing of the plugin of Reinos de Leyenda, as a player
//! finds them: tells kept and replied to, a target warned of, what each
//! mode reads, the game quiet, and keys that are the player's until asked.

use super::*;

struct Playing {
    harness: Harness<'static, App>,
    events: std::sync::mpsc::Sender<Event>,
    sent: tokio::sync::mpsc::UnboundedReceiver<Command>,
}

impl Playing {
    fn start() -> Self {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        let (events, sent) = fake_connection(app);
        app.voice.away = false;
        reinos(app);
        app.voice.said.clear();
        Self {
            harness,
            events,
            sent,
        }
    }

    /// What is said as the game writes `text`.
    fn heard(&mut self, text: &str) -> Vec<String> {
        let line = panels::styled_lines(text).remove(0);
        self.events.send(Event::Line(line)).unwrap();
        self.harness.run_steps(2);
        std::mem::take(&mut self.harness.state_mut().tab_mut().voice.said)
    }

    /// What is said and sent as the player types `text`.
    fn typed(&mut self, text: &str) -> (Vec<String>, Vec<String>) {
        let app = self.harness.state_mut().tab_mut();
        app.input = text.into();
        app.submit();
        app.run_actions();
        self.harness.run_steps(2);
        let said = std::mem::take(&mut self.harness.state_mut().tab_mut().voice.said);
        let sent = std::iter::from_fn(|| self.sent.try_recv().ok())
            .filter_map(|command| match command {
                Command::Send(text) => Some(text),
                _ => None,
            })
            .collect();
        (said, sent)
    }
}

#[test]
fn a_tell_is_read_short_kept_and_replied_to() {
    let mut playing = Playing::start();
    assert_eq!(
        playing.heard("Ana te dice: hola!!!"),
        ["Tell from Ana: hola!"]
    );
    let (_, sent) = playing.typed("r qué tal");
    assert_eq!(sent, ["telepatia Ana qué tal"]);
    // What is said in the room, short, and not taken for a tell.
    assert_eq!(playing.heard("Bob dice: buenas"), ["Bob: buenas"]);
    let (_, sent) = playing.typed("r adiós");
    assert_eq!(sent, ["telepatia Ana adiós"]);
    // Kept, to read again from a list.
    playing.typed("htels");
    let items = playing.harness.state_mut().tab_mut().palette_items();
    let listed: Vec<&str> = items.iter().map(|item| item.usage.as_str()).collect();
    assert_eq!(listed, ["Ana: hola!"]);
}

#[test]
fn the_target_is_warned_of_as_it_comes() {
    let mut playing = Playing::start();
    let (said, sent) = playing.typed("obj Orome");
    assert_eq!(
        (said.as_slice(), sent.len()),
        (["Target: Orome."].map(String::from).as_slice(), 0)
    );
    assert_eq!(
        playing.heard("Un lobo llega desde el sur."),
        ["Un lobo arrives, sur"]
    );
    let said = playing.heard("Orome (Hum) llega desde el norte.");
    assert_eq!(said, ["Target here: Orome, norte!"]);
    assert_eq!(
        playing
            .harness
            .state()
            .tab()
            .voice
            .cut
            .last()
            .map(String::as_str),
        Some("Target here: Orome, norte!")
    );
}

#[test]
fn each_mode_reads_what_it_reads_and_the_game_can_be_quiet() {
    let mut playing = Playing::start();
    let (said, _) = playing.typed("modo xp");
    assert_eq!(said, ["Mode: Experience."]);
    // Gaining experience, the kills are not read.
    assert_eq!(
        playing.heard("Propinas el golpe mortal a un lobo."),
        [] as [String; 0]
    );
    let (said, _) = playing.typed("modo original");
    assert_eq!(said, ["Mode: Original."]);
    assert_eq!(playing.heard("Bob dice: buenas"), ["Bob dice: buenas"]);
    // Quiet: the game is not read, what the plugin says of itself is.
    let (said, _) = playing.typed("silencio");
    assert_eq!(said, ["Quiet: the game is not read."]);
    assert!(!playing.harness.state().tab().voice.reads_game());
    playing.typed("silencio");
    assert!(playing.harness.state().tab().voice.reads_game());
}

#[test]
fn the_keys_of_reinos_are_the_players_until_asked_for() {
    let mut playing = Playing::start();
    // Off: Muddie's keys are as they were.
    assert_eq!(
        crate::shortcuts::READ_RECENT_1.keys(),
        crate::shortcuts::READ_RECENT_1.defaults()
    );
    let (said, _) = playing.typed("teclasrl");
    assert_eq!(said, ["Keys of Reinos on."]);
    playing.harness.run_steps(2);
    let ctrl_one = crate::shortcuts::Press::parse("ctrl+1").unwrap();
    assert_eq!(crate::shortcuts::READ_RECENT_1.keys(), [ctrl_one]);
    // Alt and the letters walk.
    playing.harness.key_press_modifiers(Modifiers::ALT, Key::K);
    playing.harness.run_steps(2);
    let sent: Vec<Command> = std::iter::from_fn(|| playing.sent.try_recv().ok()).collect();
    assert!(sent.contains(&Command::Send("s".into())), "{sent:?}");
    playing.typed("teclasrl");
    playing.harness.run_steps(2);
    assert_eq!(
        crate::shortcuts::READ_RECENT_1.keys(),
        crate::shortcuts::READ_RECENT_1.defaults()
    );
}
