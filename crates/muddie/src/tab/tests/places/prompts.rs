//! Following Reinos de Leyenda whatever the player's prompt is, and what
//! others say: only a room is taken for one.

use super::*;

/// A prompt as a player made it, of all it can tell.
const PROMPT: &str = r"Pv:4141\4141 Pe:1300\1300 Xp:1964984 SL: [|so|,s,e,n,o] PL: Jgd:Indre> ";

/// A session on a map of a square, an arch and a road to its east, and a
/// wood of one word, with the player in the square.
struct Playing {
    harness: Harness<'static, App>,
    events: std::sync::mpsc::Sender<Event>,
}

impl Playing {
    fn start() -> Self {
        let mut playing = Self::lost();
        let app = playing.harness.state_mut().tab_mut();
        assert!(app.map.borrow_mut().set_current(2));
        playing.read(&["Anduar: Arco Este [o,e]"]);
        playing
    }

    /// The same, the map not knowing where the player is.
    fn lost() -> Self {
        let mut harness = window(None);
        let app = harness.state_mut().tab_mut();
        let (events, _sent) = fake_connection(app);
        desert(app);
        for (id, name) in [
            (1, "Anduar: Plaza Mayor"),
            (2, "Anduar: Arco Este"),
            (3, "Anduar: Camino"),
            (4, "Bosque"),
        ] {
            let mut map = app.map.borrow_mut();
            assert!(map.rename(id, name).unwrap());
        }
        reinos(app);
        Self { harness, events }
    }

    fn line(&self, text: &str) {
        let line = panels::styled_lines(text).remove(0);
        self.events.send(Event::Line(line)).unwrap();
    }

    fn moved(&mut self, way: &str) {
        let moved = Event::Gmcp {
            package: "Room.Movimiento".into(),
            data: format!("{way:?}"),
        };
        self.events.send(moved).unwrap();
        self.harness.run_steps(2);
    }

    /// Lines with no move after them, and what the plugin waits.
    fn read(&mut self, lines: &[&str]) {
        for line in lines {
            self.line(line);
        }
        self.harness.run_steps(1);
        std::thread::sleep(Duration::from_millis(300));
        self.harness.run_steps(2);
    }

    fn here(&self) -> Option<String> {
        let map = self.harness.state().tab().map.borrow();
        map.current().map(|room| room.name.clone())
    }

    fn told(&self, part: &str) -> usize {
        (notes(self.harness.state().tab()).iter())
            .filter(|note| note.contains(part))
            .count()
    }
}

#[test]
fn a_room_named_by_someone_does_not_move_the_player() {
    for said in [
        "Indre te dice: nos vemos en el Bosque",
        "Indre dice: ven a Anduar: Camino",
        "[Ciudad] Indre: estoy en Anduar: Camino",
        "Indre dice: estoy en Bosque [s]",
    ] {
        let mut playing = Playing::start();
        playing.read(&[said]);
        assert_eq!(
            playing.here().as_deref(),
            Some("Anduar: Arco Este"),
            "{said}"
        );
    }
}

#[test]
fn a_room_after_the_prompt_is_the_room() {
    for prompt in [PROMPT, "> ", "[PV 90] ", "Pv:90>"] {
        let mut playing = Playing::start();
        playing.line(&format!("{prompt}Anduar: Camino [o,n]"));
        playing.moved("este");
        assert_eq!(
            playing.here().as_deref(),
            Some("Anduar: Camino"),
            "{prompt}"
        );
        // Looked at, with no move, with its exits and by its name alone.
        for look in ["Anduar: Arco Este [o,e]", "Anduar: Camino"] {
            playing.read(&[&format!("{prompt}{look}")]);
            let name = look.split(" [").next();
            assert_eq!(playing.here().as_deref(), name, "{prompt}");
        }
    }
}

/// A room the map has not is added by its name alone.
#[test]
fn a_room_added_after_the_prompt_has_nothing_of_it() {
    for prompt in [PROMPT, "> ", ""] {
        let mut playing = Playing::start();
        playing.line("Anduar: Camino [o,n]");
        playing.moved("este");
        playing.line("Bosque [s]");
        playing.moved("norte");
        assert_eq!(playing.here().as_deref(), Some("Bosque"), "{prompt}");
        playing.line(&format!("{prompt}Anduar: Puente Nuevo [s,e]"));
        playing.moved("norte");
        assert_eq!(
            playing.here().as_deref(),
            Some("Anduar: Puente Nuevo"),
            "{prompt}"
        );
    }
}

/// A prompt of several lines may have one of exits alone.
#[test]
fn exits_of_a_prompt_are_not_a_room() {
    for exits in ["SL: [o,e]", "Pv:10 SL: [o,e]", "Salidas [o,e]"] {
        let mut playing = Playing::start();
        for _ in 0..2 {
            playing.read(&[exits]);
            assert_eq!(
                playing.here().as_deref(),
                Some("Anduar: Arco Este"),
                "{exits}"
            );
        }
        // The room written goes first, whichever comes after it.
        playing.line("Anduar: Camino [o,n]");
        playing.line(exits);
        playing.moved("este");
        assert_eq!(playing.here().as_deref(), Some("Anduar: Camino"), "{exits}");
        // With no room written, the move alone is followed.
        playing.line(exits);
        playing.moved("norte");
        assert_eq!(playing.here().as_deref(), Some("Bosque"), "{exits}");
        assert!(
            playing
                .harness
                .state()
                .tab()
                .map
                .borrow()
                .added()
                .is_empty()
        );
    }
}

/// What the map could not follow is told once, until it follows again.
#[test]
fn a_move_the_map_does_not_follow_is_told_once() {
    let mut playing = Playing::start();
    for _ in 0..3 {
        playing.moved("sur");
    }
    assert_eq!(playing.told("has not followed you"), 1);
    playing.moved("este");
    playing.moved("sur");
    assert_eq!(playing.told("has not followed you"), 2);
}

#[test]
fn the_player_lost_is_told_once() {
    let mut playing = Playing::lost();
    for way in ["norte", "sur"] {
        playing.line("Sitio raro: Nada [s,e,ne]");
        playing.moved(way);
    }
    assert_eq!(playing.here(), None);
    assert_eq!(playing.told("does not know where you are"), 1);
}

/// Lost, a room looked at that the map names otherwise, as cut short, is
/// looked for by the names most like it.
#[test]
fn lost_a_room_the_map_has_cut_short_is_found_by_looking() {
    let mut playing = Playing::lost();
    playing.read(&["Anduar: Arco Este de la Muralla [o,e]"]);
    assert_eq!(playing.here().as_deref(), Some("Anduar: Arco Este"));
    // A prompt is not looked for.
    let mut playing = Playing::lost();
    playing.read(&["Este [o,e]"]);
    assert_eq!(playing.here(), None);
}

/// A room only looked at is still one to go to: named so once gone to.
#[test]
fn a_room_looked_at_is_added_when_gone_to() {
    let mut playing = Playing::start();
    playing.line("Anduar: Camino [o,n]");
    playing.moved("este");
    playing.line("Bosque [s]");
    playing.moved("norte");
    playing.read(&["Anduar: Puente Nuevo [s,e]"]);
    playing.line("Anduar: Puente Nuevo [s,e]");
    playing.moved("norte");
    assert_eq!(playing.here().as_deref(), Some("Anduar: Puente Nuevo"));
}

/// What is written in room after room is of the prompt, though it looks
/// like the name of a room.
#[test]
fn a_prompt_that_looks_like_a_room_is_known_by_coming_everywhere() {
    let mut playing = Playing::start();
    let prompt = "Vida: mucha [s]";
    playing.read(&[prompt]);
    playing.line("Anduar: Camino [o,n]");
    playing.moved("este");
    playing.read(&[prompt]);
    playing.line("Bosque [s]");
    playing.moved("norte");
    playing.line(prompt);
    playing.moved("norte");
    assert!(
        playing
            .harness
            .state()
            .tab()
            .map
            .borrow()
            .added()
            .is_empty()
    );
}
