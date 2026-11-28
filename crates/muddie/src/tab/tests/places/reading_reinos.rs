//! The plugin of Reinos de Leyenda against text as the game writes it.

use super::*;

/// The prompt of a recorded session, as the game says it and as it comes.
const TEMPLATE: &str = r"Pv:$v\$V Pe:$g\$G Xp:$x SL:$s PL:$a Jgd:$j>";
const CAME: &str = r"Pv:4141\4141 Pe:1300\1300 Xp:1964984 SL: [|so|,s,e,n,o] PL: Jgd:> ";

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
        let mut playing = Self {
            harness,
            events,
            sent,
        };
        playing.heard(&format!("Tu prompt ha sido definido como '{TEMPLATE}'."));
        playing
    }

    fn heard(&mut self, text: &str) -> Vec<String> {
        let line = panels::styled_lines(text).remove(0);
        self.events.send(Event::Line(line)).unwrap();
        self.harness.run_steps(2);
        std::mem::take(&mut self.harness.state_mut().tab_mut().voice.said)
    }

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

    fn listed(&mut self, alias: &str) -> Vec<String> {
        self.typed(alias);
        let items = self.harness.state_mut().tab_mut().palette_items();
        items.iter().map(|item| item.usage.clone()).collect()
    }
}

/// A tell that comes after the prompt, on its line, is a tell all the same.
#[test]
fn a_tell_after_the_prompt_is_replied_to() {
    let mut playing = Playing::start();
    let said = playing.heard(&format!("{CAME}Ana te dice: hola"));
    assert_eq!(said, ["Tell from Ana: hola"]);
    let (_, sent) = playing.typed("r qué tal");
    assert_eq!(sent, ["telepatia Ana qué tal"]);
}

/// Being attacked, after the prompt, leaves idle.
#[test]
fn attacked_after_the_prompt_leaves_idle() {
    let mut playing = Playing::start();
    playing.typed("modo inactivo");
    playing.heard(&format!("{CAME}Estás siendo atacado por un orco."));
    let (said, _) = playing.typed("modo?");
    assert_eq!(said, ["Mode: Combat."]);
}

/// The target, coming after the prompt, cuts in.
#[test]
fn the_target_after_the_prompt_is_warned_of() {
    let mut playing = Playing::start();
    playing.typed("obj Orome");
    let said = playing.heard(&format!("{CAME}Orome llega desde el norte."));
    assert_eq!(said, ["Target here: Orome, norte!"]);
}

/// A whisper to the player, in the room, is kept somewhere.
#[test]
fn a_whisper_to_you_is_kept() {
    let mut playing = Playing::start();
    playing.heard("Ana te susurra: ven al norte");
    let room = playing.listed("hsala");
    let tells = playing.listed("htels");
    assert!(
        room.iter()
            .chain(tells.iter())
            .any(|said| said.contains("ven al norte")),
        "room {room:?} tells {tells:?}"
    );
}

/// Lines of a recorded session: a tell, the player's own to someone, and
/// what the game writes after its prompt on the same line.
#[test]
fn a_recorded_session_is_read_as_it_was_meant() {
    let mut playing = Playing::start();
    assert_eq!(
        playing.heard("Suka te dice: algo"),
        ["Tell from Suka: algo"]
    );
    let own = "Dices a Suka: < dime argo primoh >";
    assert_eq!(playing.heard(own), [own]);
    let after = "Pvs: 4141/4141 (+260) Pe: 1299/1300 (0)";
    assert_eq!(playing.heard(&format!("> {after}")), [after]);
}

/// The target, written with an accent as the game writes it, in lower case.
#[test]
fn an_accented_target_is_known() {
    let mut playing = Playing::start();
    playing.typed("obj ángel");
    let said = playing.heard("Ángel llega desde el norte.");
    assert_eq!(said, ["Target here: Ángel, norte!"]);
}
