//! What is near a room and what a search found, worked out and put in words.

use super::*;
use crate::i18n::{trf, trn};
use muddie_mapper::{Found, FoundBy};

/// How many moves away what is near is looked for, unless told.
pub const NEAR: usize = 10;
/// How many of the things near are said while exploring, at most.
const NEAR_SAID: usize = 6;

/// What is near a room, up to `moves` moves away, the nearest first: what
/// is noted, marks, notes and the ways into other regions, then the other
/// places, each name once at its nearest room. Each with what it is and the
/// way there.
pub fn near(
    map: &Map,
    from: RoomId,
    moves: usize,
) -> Vec<(RoomId, String, Vec<muddie_mapper::Step>)> {
    let Some(start) = map.room(from) else {
        return Vec::new();
    };
    let mut regions = vec![map.region(start).map(str::to_owned)];
    let mut named = std::collections::HashSet::from([place_words(map, start).to_lowercase()]);
    let (mut noted, mut places) = (Vec::new(), Vec::new());
    for (id, steps) in map.around(from, moves) {
        let Some(room) = map.room(id) else { continue };
        let marks = map.marks_of(id);
        let region = map.region(room).map(str::to_owned);
        let place = place_words(map, room);
        if !marks.is_empty() {
            noted.push((id, marks.join(", "), steps));
        } else if !room.note.is_empty() {
            noted.push((
                id,
                trf("{0}, note: {1}", &[map.short_name(room), &room.note]),
                steps,
            ));
        } else if region.is_some() && !regions.contains(&region) {
            let into = trf("into {0}", &[region.as_deref().unwrap_or_default()]);
            regions.push(region);
            noted.push((id, into, steps));
        } else if named.insert(place.to_lowercase()) {
            places.push((id, place, steps));
        }
    }
    noted.extend(places);
    noted
}

/// A place found, as it is listed and said: what it was found by, and
/// where it is, how far from `here` and which way, and, found by a name
/// many rooms have, that it is the nearest of them.
pub fn found_words(map: &Map, here: Option<RoomId>, found: &Found) -> (String, String) {
    let Some(room) = map.room(found.room) else {
        return (String::new(), String::new());
    };
    let mut detail: Vec<String> = Vec::new();
    let title = match &found.by {
        FoundBy::Mark(mark) => {
            detail.push(trf("mark, {0}", &[&place_words(map, room)]));
            mark.clone()
        }
        FoundBy::Note => {
            detail.push(trf("note, {0}", &[&place_words(map, room)]));
            room.note.clone()
        }
        FoundBy::Name => {
            let (name, region) = place_parts(map, room);
            detail.extend(region.map(str::to_owned));
            name.to_owned()
        }
    };
    if let (Some(moves), Some(here)) = (found.moves, here) {
        detail.push(moves_toward(map, here, room.id, moves));
    }
    if found.alike > 1 {
        detail.push(trn("nearest of {0}", "nearest of {0}", found.alike as u64));
    }
    (title, detail.join(" · "))
}

/// What is near a room, the nearest first: the rooms the user named
/// or wrote a note on, and the way into each other region, with how far
/// and which way each is.
pub(super) fn near_words(map: &Map, from: RoomId) -> String {
    let near: Vec<String> = (near(map, from, NEAR).into_iter().take(NEAR_SAID))
        .map(|(id, what, steps)| {
            trf(
                "{0}, {1}",
                &[&what, &moves_toward(map, from, id, steps.len())],
            )
        })
        .collect();
    match near.is_empty() {
        true => tr("Nothing else is that near on the map.").to_owned(),
        false => trf("Near: {0}.", &[&near.join("; ")]),
    }
}

/// Every key while exploring, as said.
pub(super) fn all_keys() -> String {
    let mut said: Vec<String> = (crate::shortcuts::ALL.iter())
        .filter(|shortcut| shortcut.place == crate::shortcuts::Place::Map)
        .filter(|shortcut| !shortcut.keys().is_empty())
        .map(|shortcut| trf("{0}: {1}.", &[&shortcut.all_written(), shortcut.about()]))
        .collect();
    said.push(tr("Shift+F10 opens the room's menu, with all that is done with it. Escape, or typing, goes back to the command line.").to_owned());
    said.join(" ")
}
