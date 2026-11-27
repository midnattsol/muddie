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

/// The prompt of a session that was recorded, as the game says it.
const TEMPLATE: &str = r"Pv:$v\$V Pe:$g\$G Xp:$x SL:$s PL:$a Jgd:$j>";
/// How it came in it, with nobody in the room, and ending in each sign.
const CAME: &str = r"Pv:4141\4141 Pe:1300\1300 Xp:1964984 SL: [|so|,s,e,n,o] PL: Jgd:";

impl Playing {
    /// What the game says as the player makes their prompt.
    fn makes(&mut self, template: &str, fight: bool) {
        let which = if fight { "de combate " } else { "" };
        self.line(&format!(
            "Tu prompt {which}ha sido definido como '{template}'."
        ));
        self.harness.run_steps(2);
    }

    /// What is read as a line comes, or a prompt the MUD marks.
    fn heard(&mut self, text: &str, marked: bool) -> Vec<String> {
        self.harness.state_mut().tab_mut().voice.said.clear();
        let line = panels::styled_lines(text).remove(0);
        let event = if marked {
            Event::Prompt(line)
        } else {
            Event::Line(line)
        };
        self.events.send(event).unwrap();
        self.harness.run_steps(2);
        std::mem::take(&mut self.harness.state_mut().tab_mut().voice.said)
    }
}

const NOTHING: [&str; 0] = [];

/// Once the player made it, the prompt is not read, whatever sign ends it, and
/// what comes after it on its line is.
#[test]
fn the_prompt_made_is_not_read_and_what_comes_after_it_is() {
    let mut playing = Playing::start();
    assert_eq!(playing.heard(&format!("{CAME}> "), true).len(), 1);
    playing.makes(TEMPLATE, false);
    for end in [">", "]", "}"] {
        for marked in [true, false] {
            assert_eq!(playing.heard(&format!("{CAME}{end} "), marked), NOTHING);
            assert_eq!(
                playing.heard(&format!("{CAME}Indre, Furan{end}"), marked),
                NOTHING
            );
        }
        let more = "Pvs: 4141/4141 (+260) Pe: 1300/1300 (0)";
        assert_eq!(playing.heard(&format!("{CAME}{end} {more}"), false), [more]);
        // Two of them, when much comes.
        assert_eq!(
            playing.heard(&format!("{CAME}{end} {CAME}> {more}"), false),
            [more]
        );
    }
    for said in ["Un orco te ataca.", "Pv: mucho > poco"] {
        assert_eq!(playing.heard(said, false), [said]);
    }
    // What is said in the room is read short, as the plugin's modes do.
    assert_eq!(
        playing.heard("Indre dice: Pv:10 > ven", false),
        ["Indre: Pv:10 > ven"]
    );
}

/// That of a fight is its own, and one made again takes the place of the one
/// there was; what it says is whatever the player wants.
#[test]
fn the_prompts_are_those_the_player_makes() {
    let mut playing = Playing::start();
    playing.makes(TEMPLATE, false);
    playing.makes("[$v/$V] -- $AM.$AP$l(50%) $g>", true);
    assert_eq!(playing.heard("[90/100] -- 1.2", false), NOTHING);
    assert_eq!(playing.heard("(50%) 30} ", true), NOTHING);
    assert_eq!(playing.heard(&format!("{CAME}> "), true), NOTHING);
    playing.makes("Vida $v>", false);
    assert_eq!(playing.heard("Vida 90> ", true), NOTHING);
    assert_eq!(playing.heard("Vida 90> Un orco.", false), ["Un orco."]);
    assert_eq!(playing.heard(&format!("{CAME}> "), true).len(), 1);
    // So long that the game writes it in two lines.
    playing.line("Tu prompt ha sido definido como 'Vida actual: $v de");
    playing.line("$V y energía: $g>'.");
    playing.harness.run_steps(2);
    assert_eq!(
        playing.heard("Vida actual: 9 de 10 y energía: 3> ", true),
        NOTHING
    );
    // The one of who makes none.
    assert_eq!(playing.heard("> ", true), NOTHING);
    assert_eq!(playing.heard("> Un orco.", false), ["Un orco."]);
    // One with nothing to be told by takes nothing for it.
    playing.makes("$j>", false);
    assert_eq!(playing.heard("Indre > Furan", false), ["Indre > Furan"]);
}

/// The room comes after the prompt known, whatever ends it.
#[test]
fn a_room_after_the_prompt_known_is_the_room() {
    for (known, end) in [
        (true, ">"),
        (true, "]"),
        (true, "}"),
        (false, ">"),
        (false, "]"),
        (false, "}"),
    ] {
        let mut playing = Playing::start();
        if known {
            playing.makes(TEMPLATE, false);
        }
        let case = format!("{known} {end}");
        playing.line(&format!("{CAME}{end} Anduar: Camino [o,n]"));
        playing.moved("este");
        assert_eq!(playing.here().as_deref(), Some("Anduar: Camino"), "{case}");
        playing.read(&[&format!("{CAME}{end} ")]);
        assert_eq!(playing.here().as_deref(), Some("Anduar: Camino"), "{case}");
        playing.read(&[&format!("{CAME}{end} Anduar: Arco Este")]);
        assert_eq!(
            playing.here().as_deref(),
            Some("Anduar: Arco Este"),
            "{case}"
        );
    }
}

/// With the prompt known, what others say is not taken for a room.
#[test]
fn with_the_prompt_known_a_room_named_by_someone_does_not_move_the_player() {
    for said in [
        "Indre dice: > Anduar: Camino",
        "[Ciudad] Anduar: Camino",
        "Indre> Bosque [s]",
    ] {
        let mut playing = Playing::start();
        playing.makes(TEMPLATE, false);
        playing.read(&[&format!("{CAME}> {said}"), said]);
        assert_eq!(
            playing.here().as_deref(),
            Some("Anduar: Arco Este"),
            "{said}"
        );
    }
}

/// A prompt not known is told of once, in the game and not before, and never
/// to who has the usual one.
#[test]
fn a_prompt_not_known_is_told_of_once() {
    let mut playing = Playing::start();
    let told = "Your prompt is read";
    playing.heard("Introduce tu nombre: ", true);
    assert_eq!(playing.told(told), 0);
    let char = Event::Gmcp {
        package: "Char.Status".into(),
        data: "{}".into(),
    };
    playing.events.send(char).unwrap();
    playing.heard("> ", true);
    assert_eq!(playing.told(told), 0);
    for _ in 0..2 {
        playing.heard(&format!("{CAME}> "), true);
    }
    assert_eq!(playing.told(told), 1);
}
