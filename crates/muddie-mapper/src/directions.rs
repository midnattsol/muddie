//! The directions the map knows, once: their keys, their names in English
//! and the step each takes on the grid, in the order they are listed; and
//! the words a MUD says them with, those of its language under those a
//! script teaches.

use super::*;

/// One of the directions the map knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Direction {
    /// How the map keeps it: `n`, `ne`... `u`, `d`, `in`, `out`.
    pub key: &'static str,
    /// Its name in English, which the map understands of any MUD.
    pub english: &'static str,
    /// The step it takes on the grid, x east, y north and z up; none for
    /// in and out, which go nowhere on it.
    pub step: Option<(i32, i32, i32)>,
}

const fn direction(
    key: &'static str,
    english: &'static str,
    step: Option<(i32, i32, i32)>,
) -> Direction {
    Direction { key, english, step }
}

/// Every direction the map knows: the compass, from the north round by
/// the east, then up and down, in and out. Exits are listed and said in
/// this order.
pub const DIRECTIONS: [Direction; 12] = [
    direction("n", "north", Some((0, 1, 0))),
    direction("ne", "northeast", Some((1, 1, 0))),
    direction("e", "east", Some((1, 0, 0))),
    direction("se", "southeast", Some((1, -1, 0))),
    direction("s", "south", Some((0, -1, 0))),
    direction("sw", "southwest", Some((-1, -1, 0))),
    direction("w", "west", Some((-1, 0, 0))),
    direction("nw", "northwest", Some((-1, 1, 0))),
    direction("u", "up", Some((0, 0, 1))),
    direction("d", "down", Some((0, 0, -1))),
    direction("in", "in", None),
    direction("out", "out", None),
];

/// The direction kept as `key`, if the map knows it.
pub fn known_direction(key: &str) -> Option<&'static Direction> {
    DIRECTIONS.iter().find(|known| known.key == key)
}

/// Where a direction goes in the order exits are listed in: those the map
/// does not know after all those it does.
pub fn direction_rank(key: &str) -> usize {
    (DIRECTIONS.iter())
        .position(|known| known.key == key)
        .unwrap_or(DIRECTIONS.len())
}

/// Lower-cases a direction and makes the English names of those the map
/// knows their keys (`north` to `n`), so exits compare equal however the
/// server spells them.
pub fn canonical_direction(direction: &str) -> String {
    let lower = direction.trim().to_lowercase();
    match DIRECTIONS.iter().find(|known| known.english == lower) {
        Some(known) => known.key.to_owned(),
        None => lower,
    }
}

/// The grid step a direction stands for, if it is one of the compass, up
/// or down.
pub fn direction_offset(direction: &str) -> Option<(i32, i32, i32)> {
    known_direction(direction)?.step
}

/// Words for directions: each word with the direction it says, and the
/// word a walk sends for each.
#[derive(Debug, Clone, Default)]
pub(crate) struct Words {
    said: HashMap<String, String>,
    sent: HashMap<String, String>,
}

impl Words {
    /// Adds the words for the directions given, the first of each being
    /// what is sent for it from now on.
    fn add(&mut self, words: &[(String, Vec<String>)]) {
        for (direction, said) in words {
            let direction = canonical_direction(direction);
            for word in said {
                (self.said).insert(word.trim().to_lowercase(), direction.clone());
            }
            if let Some(first) = said.first() {
                self.sent.insert(direction, first.trim().to_owned());
            }
        }
    }
}

impl Map {
    /// Gives the map the words of the MUD's language for directions: for
    /// each (`n`, `ne`... `u`, `d`, `in`, `out`), the words that mean it,
    /// the first being what a walk sends. In place of the language's
    /// before; what scripts taught stays over them.
    pub fn set_language(&mut self, words: &[(String, Vec<String>)]) {
        self.language = Words::default();
        self.language.add(words);
        self.respell_exits();
    }

    /// Teaches the map words of the MUD's for directions, over those of
    /// its language: added to them, and the first of each is sent from now
    /// on. A MUD's own abbreviations, as its plugin knows them.
    pub fn teach_directions(&mut self, words: &[(String, Vec<String>)]) {
        self.taught.add(words);
        self.respell_exits();
    }

    /// Keys every room's exits by direction as the map keeps them: a map
    /// taken from elsewhere may have exits kept in the MUD's words, as
    /// "fu" for out, which mean a direction only once the map knows them.
    pub(crate) fn respell_exits(&mut self) {
        let respelled: Vec<(RoomId, BTreeMap<String, Exit>)> = (self.rooms.values())
            .filter_map(|room| Some((room.id, self.respelled(&room.exits)?)))
            .collect();
        let ids: Vec<RoomId> = respelled.iter().map(|(id, _)| *id).collect();
        for (id, exits) in respelled {
            if let Some(room) = self.rooms.get_mut(&id) {
                room.exits = exits;
            }
        }
        self.keep(ids);
    }

    /// Exits keyed by direction as the map keeps them, if any is not: one
    /// whose direction another exit of the room already has stays as it
    /// was, not to lose either.
    pub(crate) fn respelled(
        &self,
        exits: &BTreeMap<String, Exit>,
    ) -> Option<BTreeMap<String, Exit>> {
        let wrong = |key: &String| self.direction(key) != *key;
        if !exits.keys().any(wrong) {
            return None;
        }
        let mut kept: BTreeMap<String, Exit> = (exits.iter())
            .filter(|(key, _)| !wrong(key))
            .map(|(key, exit)| (key.clone(), exit.clone()))
            .collect();
        for (key, exit) in exits.iter().filter(|(key, _)| wrong(key)) {
            let direction = self.direction(key);
            let key = match kept.contains_key(&direction) {
                true => key.clone(),
                false => direction,
            };
            kept.insert(key, exit.clone());
        }
        (kept != *exits).then_some(kept)
    }

    /// Forgets what scripts taught of directions, as they are run again:
    /// the words of the language are left.
    pub fn forget_taught_directions(&mut self) {
        self.taught = Words::default();
    }

    /// A direction as the map keeps it, however the server or the user
    /// said it: in the server's words, in English, long or short.
    pub fn direction(&self, word: &str) -> String {
        let lower = word.trim().to_lowercase();
        (self.taught.said.get(&lower))
            .or_else(|| self.language.said.get(&lower))
            .cloned()
            .unwrap_or_else(|| canonical_direction(&lower))
    }

    /// What a walk sends to go `direction`: the server's word for it, if
    /// the map was given one.
    pub fn command_for<'a>(&'a self, direction: &'a str) -> &'a str {
        (self.taught.sent.get(direction))
            .or_else(|| self.language.sent.get(direction))
            .map_or(direction, String::as_str)
    }
}
