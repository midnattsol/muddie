//! What the map says in words about rooms, exits and ways, for the speech and the lines under the map.

use super::*;

/// A room as it is shown when chosen, but for its exits, shown apart:
/// its name, its region if the name does not say it, its number, how many
/// moves away it is from `from`, and its note.
pub(super) fn selection_shown(map: &Map, from: Option<RoomId>, room: &Room) -> String {
    let (name, region) = place_parts(map, room);
    let mut parts: Vec<String> = [Some(name), region]
        .into_iter()
        .flatten()
        .map(str::to_owned)
        .collect();
    parts.push(format!("#{}", room.id));
    parts.push(how_far(map, from, room));
    if !room.note.is_empty() {
        parts.push(room.note.clone());
    }
    parts.join(" · ")
}

/// A room's exits, in the order of the compass, then up and down, in and
/// out, and the rest: the one order they are shown and said in.
pub fn ordered_exits(room: &Room) -> Vec<&str> {
    let mut ways: Vec<&str> = room.exits.keys().map(String::as_str).collect();
    ways.sort_by_key(|way| muddie_mapper::direction_rank(way));
    ways
}

/// A direction in words: the MUD's, if the map was taught them, else as
/// people say it.
pub fn direction_words(map: &Map, direction: &str) -> String {
    let taught = map.command_for(direction);
    if taught != direction {
        return taught.to_owned();
    }
    match muddie_mapper::known_direction(direction) {
        Some(known) => tr(known.english).to_owned(),
        None => direction.to_owned(),
    }
}

/// Which way `to` lies from `from`, said: "to the north-east". None if
/// the map cannot tell, being in another area or level.
pub fn toward(map: &Map, from: RoomId, to: RoomId) -> Option<String> {
    let direction = map.bearing(from, to)?;
    Some(crate::i18n::trf(
        "to the {0}",
        &[&direction_words(map, direction)],
    ))
}

/// How far a room is from `from`, said: here, or how many moves and which
/// way, or that there is no known way.
pub fn how_far(map: &Map, from: Option<RoomId>, room: &Room) -> String {
    let Some(from) = from else {
        return tr("the map does not know where you are").to_owned();
    };
    if from == room.id {
        return tr("you are here").to_owned();
    }
    match map.moves_between(from, room.id) {
        Some(moves) => moves_toward(map, from, room.id, moves),
        None => tr("no known way").to_owned(),
    }
}

/// How many moves away a room is from `from`, and which way, said: "3
/// moves to the north", or that it is where the player is.
pub fn moves_toward(map: &Map, from: RoomId, to: RoomId, moves: usize) -> String {
    if moves == 0 {
        return tr("you are here").to_owned();
    }
    let said = crate::i18n::trn("{0} move", "{0} moves", moves as u64);
    match toward(map, from, to) {
        Some(way) => trf("{0} {1}", &[&said, &way]),
        None => said,
    }
}

/// A room's exits, said in the MUD's words, in the order of the compass,
/// then up and down, in and out, and the rest.
pub fn exits_words(map: &Map, room: &Room) -> String {
    let exits: Vec<String> = ordered_exits(room)
        .into_iter()
        .map(|way| direction_words(map, way))
        .collect();
    match exits.is_empty() {
        true => tr("No exits").to_owned(),
        false => crate::i18n::trf("Exits: {0}", &[&exits.join(", ")]),
    }
}

/// Parts said one after another as sentences, each ending in one full
/// stop, whether it came with one or not: "Plaza. Exits: north."
pub fn sentences(parts: impl IntoIterator<Item = String>) -> String {
    let parts: Vec<String> = (parts.into_iter())
        .map(|part| part.trim().trim_end_matches('.').to_owned())
        .filter(|part| !part.is_empty())
        .collect();
    match parts.is_empty() {
        true => String::new(),
        false => format!("{}.", parts.join(". ")),
    }
}

/// A room as it is said: its name, without its region or the exits some
/// MUDs write after it, and its region when it has one of its own.
pub fn place_words(map: &Map, room: &Room) -> String {
    match place_parts(map, room) {
        (name, Some(region)) => format!("{name}, {region}"),
        (name, None) => name.to_owned(),
    }
}

/// A room's name, short of its region, and its region when that is not
/// what the name says: the one way a place is named, apart or together.
pub fn place_parts<'a>(map: &'a Map, room: &'a Room) -> (&'a str, Option<&'a str>) {
    let name = map.short_name(room);
    (name, map.region(room).filter(|region| *region != name))
}

/// A room of the map said by its place, or by its number if the map has
/// it no more.
pub fn room_words(map: &Map, id: RoomId) -> String {
    map.room(id)
        .map_or_else(|| format!("#{id}"), |room| place_words(map, room))
}

/// A room as it is listed where its number tells it from others of its
/// name: "Plaza, Anduar (#12)".
pub fn numbered(map: &Map, room: &Room) -> String {
    format!("{} (#{})", place_words(map, room), room.id)
}

/// What is so of a room, said only when it is: exits closed to routes,
/// that routes avoid it, its names and its note.
pub fn room_facts(map: &Map, room: &Room) -> Vec<String> {
    let mut facts = Vec::new();
    // The doors, as they are drawn: open, shut or locked.
    let doors: Vec<String> = (ordered_exits(room).into_iter())
        .filter_map(|way| Some((way, room.exits[way].door?)))
        .map(|(way, door)| door_words(map, way, door))
        .collect();
    if !doors.is_empty() {
        facts.push(trf("Doors: {0}", &[&doors.join("; ")]));
    }
    let closed: Vec<String> = (ordered_exits(room).into_iter())
        .filter(|way| room.exits[*way].blocked)
        .map(|way| direction_words(map, way))
        .collect();
    if !closed.is_empty() {
        facts.push(crate::i18n::trf(
            "Kept off routes: {0}",
            &[&closed.join(", ")],
        ));
    }
    let unexplored: Vec<String> = (ordered_exits(room).into_iter())
        .filter(|way| map.room(room.exits[*way].to).is_none())
        .map(|way| direction_words(map, way))
        .collect();
    if !unexplored.is_empty() {
        facts.push(crate::i18n::trf(
            "Not explored: {0}",
            &[&unexplored.join(", ")],
        ));
    }
    if room.added {
        facts.push(
            tr("The map added it where you went: the map imported does not have it.").to_owned(),
        );
    }
    if room.avoid {
        facts.push(tr("Kept off routes.").to_owned());
    }
    let marks = map.marks_of(room.id);
    if !marks.is_empty() {
        facts.push(crate::i18n::trf("Marked {0}.", &[&marks.join(", ")]));
    }
    if !room.note.is_empty() {
        facts.push(crate::i18n::trf("Note: {0}", &[&room.note]));
    }
    facts
}

/// A door on an exit, said: "east, locked".
pub fn door_words(map: &Map, way: &str, door: muddie_mapper::Door) -> String {
    let state = match door {
        muddie_mapper::Door::Open => tr("open"),
        muddie_mapper::Door::Closed => tr("shut"),
        muddie_mapper::Door::Locked => tr("locked"),
    };
    trf("{0}, {1}", &[&direction_words(map, way), state])
}
/// Why the map cannot say where the player is, and what to do.
pub fn unknown_whereabouts(map: &Map) -> String {
    match map.candidates().len() {
        0 => {
            tr("The map does not know where you are: tell it with I am in…, in the palette. Looking at the room, or connecting again, may tell it too.")
                .to_owned()
        }
        several => crate::i18n::trn(
            "The map is not sure where you are: in one of {0} rooms alike. Moving tells them apart; Which room am I in, in the palette, lists them.",
            "The map is not sure where you are: in one of {0} rooms alike. Moving tells them apart; Which room am I in, in the palette, lists them.",
            several as u64,
        ),
    }
}
