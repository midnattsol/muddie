//! Exploring the map with the keys, for whoever does not see it: the room
//! chosen moves along the exits and is said, and places are looked for,
//! named and noted, all without leaving the map or sending anything to the
//! MUD.

use super::*;
use crate::i18n::{trf, trn};
use muddie_mapper::{Found, FoundBy};

/// The keys while exploring the map, said on coming to it.
const EXPLORING_KEYS: &str =
    "Arrows follow the exits, Enter goes, Escape goes back. F1 says all the keys.";

/// Every key while exploring, said with F1.
const ALL_KEYS: &str = "Arrows, or the keypad, follow the exits; Page Up and Page Down go up and down, as minus and plus on the keypad, and slash and star go in and out. Home comes back to you, Backspace to the room before. Space says the room again, {0} all about it, {1} what is noted near. {2} searches the map. {3} gives the room a name, {4} writes a note on it. Enter goes there, Shift+Enter says the way. Escape, or typing, goes back to the command line.";

/// How many moves away from the room chosen what is noted is looked for.
const NEAR: usize = 10;
/// How many of the things near are said, at most.
const NEAR_SAID: usize = 6;

/// What the line over the map asks for while exploring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asked {
    /// What to look for on the map.
    Search,
    /// A name for the room chosen.
    Mark,
    /// A note on the room chosen.
    Note,
}

/// A line over the map that asks for some words while exploring, and what
/// a search found.
pub(super) struct Prompt {
    asked: Asked,
    /// The room it was asked on: the one named or noted.
    room: RoomId,
    text: String,
    /// What was looked for last, and what was found, with the one the
    /// arrows are on.
    searched: Option<String>,
    found: Vec<Found>,
    at: usize,
    /// Whether it is to take the keyboard when next drawn.
    focus: bool,
}

/// The keys with Ctrl (Cmd on a Mac) the map takes while exploring: all
/// about the room, what is near, search, name and note.
const COMMANDS: [egui::Key; 5] = [
    egui::Key::I,
    egui::Key::L,
    egui::Key::F,
    egui::Key::B,
    egui::Key::N,
];

/// A room as it is said: its name, without the exits some MUDs write
/// after it, and its region when that is not its name already.
pub fn place_words(map: &Map, room: &Room) -> String {
    let name = map.bare_name(&room.name);
    match map.region(room) {
        Some(region) if region != name && !name.starts_with(region) => format!("{name}, {region}"),
        _ => name.to_owned(),
    }
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
            let name = map.bare_name(&room.name);
            if let Some(region) = map.region(room).filter(|region| *region != name) {
                detail.push(region.to_owned());
            }
            name.to_owned()
        }
    };
    if let Some(moves) = found.moves.filter(|moves| *moves > 0) {
        let said = trn("{0} move", "{0} moves", moves as u64);
        let way = here.and_then(|here| toward(map, here, room.id));
        detail.push(match way {
            Some(way) => format!("{said} {way}"),
            None => said,
        });
    } else if found.moves == Some(0) {
        detail.push(tr("you are here").to_owned());
    }
    if found.alike > 1 {
        detail.push(trn("nearest of {0}", "nearest of {0}", found.alike as u64));
    }
    (title, detail.join(" · "))
}

impl MapView {
    /// Whether the map is being explored with the keys: it has the
    /// keyboard, or asks for some words over it. The keys it takes then
    /// are its own, not bound to anything else.
    pub fn exploring(&self) -> bool {
        self.had_focus || self.prompt.is_some()
    }

    /// Whether the map takes this key while exploring, before anything
    /// bound to it: the keypad, F1, and its keys with Ctrl.
    pub fn takes(&self, key: egui::Key, modifiers: egui::Modifiers) -> bool {
        self.exploring()
            && (key == egui::Key::F1
                || (modifiers.matches_exact(egui::Modifiers::COMMAND) && COMMANDS.contains(&key)))
    }

    /// The keys, while the map has the keyboard: those that move the room
    /// chosen along its exits, from where the player is if none is chosen,
    /// and back; Enter, to walk there; Shift and Enter, to say the way;
    /// those that say the room, what is near and the keys; those that
    /// search, name and note; Escape, to leave.
    pub(super) fn keys(
        &mut self,
        ui: &Ui,
        response: &Response,
        map: &Map,
        current: &Room,
    ) -> Option<MapAction> {
        let had_focus = std::mem::replace(&mut self.had_focus, response.has_focus());
        // Escape, or anything typed but the digits that move: back to the
        // command line, with what was typed.
        if had_focus && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            return Some(self.leave(ui, response, String::new()));
        }
        if !response.has_focus() {
            return None;
        }
        let typed: String = ui.input(|input| {
            (input.events.iter())
                .filter_map(|event| match event {
                    egui::Event::Text(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect()
        });
        if typed
            .chars()
            .any(|c| !c.is_ascii_digit() && !c.is_whitespace())
        {
            return Some(self.leave(ui, response, typed));
        }
        // The arrows are the map's while it has the keyboard, not to move
        // to another widget.
        let filter = egui::EventFilter {
            horizontal_arrows: true,
            vertical_arrows: true,
            ..Default::default()
        };
        ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, filter));
        let pressed = |modifiers, key| ui.input_mut(|input| input.consume_key(modifiers, key));
        let command = |key| pressed(egui::Modifiers::COMMAND, key);
        let chosen = self.selected.unwrap_or(current.id);
        // The keypad first, by where its keys are: what egui made of them,
        // Home for 7 with Num Lock off, was taken away.
        for name in std::mem::take(&mut self.keypad) {
            if let Some(action) = self.keypad_key(map, chosen, name) {
                return Some(action);
            }
        }
        if pressed(egui::Modifiers::NONE, egui::Key::F1) {
            return Some(MapAction::Say(all_keys()));
        }
        if command(egui::Key::I) {
            return Some(MapAction::Info(chosen));
        }
        if command(egui::Key::L) {
            return Some(MapAction::Say(near_words(map, chosen)));
        }
        let asked = [
            (egui::Key::F, Asked::Search),
            (egui::Key::B, Asked::Mark),
            (egui::Key::N, Asked::Note),
        ];
        if let Some((_, asked)) = asked.into_iter().find(|(key, _)| command(*key)) {
            let note = map.room(chosen).map(|room| room.note.clone());
            self.prompt = Some(Prompt {
                asked,
                room: chosen,
                text: note.filter(|_| asked == Asked::Note).unwrap_or_default(),
                searched: None,
                found: Vec::new(),
                at: 0,
                focus: true,
            });
            return None;
        }
        if pressed(egui::Modifiers::NONE, egui::Key::Space) {
            return Some(MapAction::Selected(chosen, None));
        }
        if pressed(egui::Modifiers::NONE, egui::Key::Backspace) {
            let Some(back) = self.trail.pop() else {
                return Some(MapAction::Say(
                    tr("This is where you began exploring.").to_owned(),
                ));
            };
            self.selected = Some(back);
            self.reveal = Some(back);
            return Some(MapAction::Selected(back, None));
        }
        if let Some(id) = self.selected {
            if pressed(egui::Modifiers::SHIFT, egui::Key::Enter) {
                return Some(MapAction::Way(id));
            }
            if pressed(egui::Modifiers::NONE, egui::Key::Enter) {
                return Some(MapAction::Walk(id));
            }
        }
        if pressed(egui::Modifiers::NONE, egui::Key::Home) {
            // A jump, not a move: nothing is said of the way back.
            self.choose(current.id);
            return Some(MapAction::Selected(current.id, None));
        }
        let (_, way) = MOVES
            .iter()
            .find(|(key, _)| pressed(egui::Modifiers::NONE, *key))?;
        Some(self.step(map, chosen, way))
    }

    /// Moves the room chosen along its exit `way`, or says there is none.
    fn step(&mut self, map: &Map, from: RoomId, way: &'static str) -> MapAction {
        let to = (map.room(from))
            .and_then(|room| room.exits.get(way))
            .map(|exit| exit.to)
            .filter(|to| map.room(*to).is_some());
        match to {
            Some(to) => {
                self.choose(to);
                MapAction::Selected(to, Some(from))
            }
            None => MapAction::NoExit(way),
        }
    }

    /// Whether the map has the keyboard, to be given the keypad's keys.
    pub fn has_keyboard(&self) -> bool {
        self.had_focus
    }

    /// Keys of the keypad pressed, by name, taken from what egui makes of
    /// them, for the map to take when next drawn.
    pub fn keypad_pressed(&mut self, names: impl IntoIterator<Item = &'static str>) {
        self.keypad.extend(names);
    }

    /// A key of the keypad, whether Num Lock is on or off: by where it is,
    /// the digits are the compass, minus and plus up and down, slash and
    /// star in and out; 5 says the room again, and Enter goes there.
    fn keypad_key(&mut self, map: &Map, chosen: RoomId, name: &str) -> Option<MapAction> {
        let way = match name {
            "numpad8" | "numpadup" => "n",
            "numpad2" | "numpaddown" => "s",
            "numpad4" | "numpadleft" => "w",
            "numpad6" | "numpadright" => "e",
            "numpad7" | "numpadhome" => "nw",
            "numpad9" | "numpadpageup" => "ne",
            "numpad1" | "numpadend" => "sw",
            "numpad3" | "numpadpagedown" => "se",
            "numpadsubtract" => "u",
            "numpadadd" => "d",
            "numpaddivide" => "in",
            "numpadmultiply" => "out",
            "numpad5" | "numpadclear" => return Some(MapAction::Selected(chosen, None)),
            "numpadenter" => return self.selected.map(MapAction::Walk),
            _ => return None,
        };
        Some(self.step(map, chosen, way))
    }

    /// Chooses a room with the keys, the one chosen before kept to go
    /// back to, and brings it into sight.
    fn choose(&mut self, room: RoomId) {
        if let Some(before) = self.selected.filter(|before| *before != room) {
            self.trail.push(before);
        }
        self.selected = Some(room);
        self.reveal = Some(room);
    }

    /// Leaves the map for the command line, choosing no room; the view
    /// goes back to following the player.
    fn leave(&mut self, ui: &Ui, response: &Response, typed: String) -> MapAction {
        match self.browsing.take() {
            Some(_) => (self.pan, self.glide) = (Vec2::ZERO, None),
            None => self.shift(ui, -self.pan),
        }
        self.selected = None;
        self.had_focus = false;
        self.trail.clear();
        self.prompt = None;
        ui.memory_mut(|memory| memory.surrender_focus(response.id));
        MapAction::Leave(typed)
    }

    /// Starts exploring the map from `from`: it is chosen, and the map
    /// takes the keyboard when next drawn. Returns what is said on coming
    /// to it.
    pub fn explore(&mut self, map: &Map, from: &Room) -> String {
        self.selected = Some(from.id);
        self.reveal = Some(from.id);
        self.focus_wanted = true;
        self.trail.clear();
        self.prompt = None;
        self.said_region = map.region(from).map(str::to_owned);
        let name = map.short_name(from);
        let here = match map.region(from) {
            Some(region) if region != name => format!("{name}, {region}"),
            _ => name.to_owned(),
        };
        trf(
            "Exploring the map from {0}. {1}. {2}",
            &[&here, &exits_words(map, from), tr(EXPLORING_KEYS)],
        )
    }

    /// What is said of a room the keys or a click chose, `came` from the
    /// one before: its name, and its region on coming to another; how far
    /// it is and which way; its exits; and, only if so, that it has none
    /// back, that some are closed, that routes avoid it, its names and its
    /// note.
    pub fn said_of(&mut self, map: &Map, room: &Room, came: Option<RoomId>) -> String {
        let name = map.short_name(room);
        let region = map.region(room).map(str::to_owned);
        let mut said = match &region {
            Some(region) if self.said_region.as_ref() != Some(region) && region != name => {
                trf("Into {0}: {1}", &[region, name])
            }
            _ => name.to_owned(),
        };
        self.said_region = region;
        let here = map.current().map(|here| here.id);
        let mut parts = vec![how_far(map, here, room), exits_words(map, room)];
        if let Some(came) = came.filter(|came| *came != room.id)
            && !room.exits.values().any(|exit| exit.to == came)
        {
            parts.push(tr("no way straight back").to_owned());
        }
        let closed: Vec<String> = (room.exits.iter())
            .filter(|(_, exit)| exit.locked)
            .map(|(way, _)| direction_words(map, way))
            .collect();
        if !closed.is_empty() {
            parts.push(trf("closed: {0}", &[&closed.join(", ")]));
        }
        if room.avoid {
            parts.push(tr("routes avoid it").to_owned());
        }
        let marks = map.marks_of(room.id);
        if !marks.is_empty() {
            parts.push(trf("called {0}", &[&marks.join(", ")]));
        }
        if !room.note.is_empty() {
            parts.push(trf("note: {0}", &[&room.note]));
        }
        for part in parts {
            said = format!("{said}. {part}");
        }
        said
    }

    /// The line over the map while it asks for some words: what to look
    /// for, with the arrows through what was found and Enter to go to one;
    /// a name for the room chosen; or a note on it. Enter takes the words,
    /// Escape goes back to the map without them. Returns what to say or
    /// do.
    pub(super) fn prompt_line(
        &mut self,
        ui: &mut Ui,
        map: &Map,
        here: Option<RoomId>,
    ) -> Option<MapAction> {
        let prompt = self.prompt.as_mut()?;
        let id = ui.make_persistent_id("map prompt");
        let focused = ui.memory(|memory| memory.has_focus(id));
        let mut said = None;
        let searched = prompt.searched.as_deref() == Some(prompt.text.trim());
        if focused && searched && !prompt.found.is_empty() {
            let (up, down) = ui.input_mut(|input| {
                (
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                )
            });
            let last = prompt.found.len() - 1;
            if up || down {
                prompt.at = match down {
                    true => (prompt.at + 1).min(last),
                    false => prompt.at.saturating_sub(1),
                };
                said = Some(MapAction::Say(found_line(map, here, prompt)));
            }
        }
        let room = (map.room(prompt.room))
            .map_or_else(String::new, |room| map.short_name(room).to_owned());
        let label = match prompt.asked {
            Asked::Search => tr("Search the map").to_owned(),
            Asked::Mark => trf("Name for {0}", &[&room]),
            Asked::Note => trf("Note on {0}", &[&room]),
        };
        let field = ui
            .horizontal(|ui| {
                ui.label(&label);
                let edit = egui::TextEdit::singleline(&mut prompt.text)
                    .id(id)
                    .desired_width(f32::INFINITY);
                ui.add(edit)
            })
            .inner;
        (ui.ctx()).accesskit_node_builder(field.id, |node| node.set_label(label));
        // What was found, shown as it is said, for whoever sees it.
        if searched && let Some(searched) = &prompt.searched {
            let shown = match prompt.found.is_empty() {
                true => trf("Nothing on the map has {0}.", &[searched]),
                false => found_line(map, here, prompt),
            };
            ui.label(RichText::new(shown).color(theme::current().text_dim));
        }
        if std::mem::take(&mut prompt.focus) {
            field.request_focus();
        }
        // The arrows go through what was found, not to another control.
        let filter = egui::EventFilter {
            horizontal_arrows: true,
            vertical_arrows: true,
            ..Default::default()
        };
        ui.memory_mut(|memory| memory.set_focus_lock_filter(id, filter));
        if !field.lost_focus() {
            return said;
        }
        // Enter and Escape are the line's: the map, given the keyboard back
        // this frame, does not take them too.
        let key = |key| ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, key));
        let (enter, escape) = (key(egui::Key::Enter), key(egui::Key::Escape));
        let text = prompt.text.trim().to_owned();
        if !enter || (text.is_empty() && prompt.asked != Asked::Note) {
            // Escape, or a click elsewhere: no words; to the map, but on
            // a click, which takes the keyboard where it was made.
            if escape || enter {
                self.focus_wanted = true;
            }
            self.prompt = None;
            return said;
        }
        let room = prompt.room;
        match prompt.asked {
            Asked::Search if searched && !prompt.found.is_empty() => {
                let to = prompt.found[prompt.at].room;
                self.prompt = None;
                self.focus_wanted = true;
                self.choose(to);
                Some(MapAction::Selected(to, None))
            }
            Asked::Search => {
                prompt.found = map.search(&text, here);
                prompt.at = 0;
                prompt.searched = Some(text.clone());
                // The keyboard stays, for the arrows and Enter.
                field.request_focus();
                Some(MapAction::Say(match prompt.found.is_empty() {
                    true => trf("Nothing on the map has {0}.", &[&text]),
                    false => {
                        let count = trn("{0} found", "{0} found", prompt.found.len() as u64);
                        format!("{count}. {}", found_line(map, here, prompt))
                    }
                }))
            }
            asked => {
                self.prompt = None;
                self.focus_wanted = true;
                Some(match asked {
                    Asked::Mark => MapAction::Mark(room, text),
                    _ => MapAction::Note(room, text),
                })
            }
        }
    }
}

/// The place the arrows are on among those found, said with where it is
/// in the list: "2 of 5: …".
fn found_line(map: &Map, here: Option<RoomId>, prompt: &Prompt) -> String {
    let (title, detail) = found_words(map, here, &prompt.found[prompt.at]);
    let place = match detail.is_empty() {
        true => title,
        false => format!("{title}, {detail}"),
    };
    let at = [(prompt.at + 1).to_string(), prompt.found.len().to_string()];
    trf("{0} of {1}: {2}", &[&at[0], &at[1], &place])
}

/// What is noted near a room, the nearest first: the rooms the user named
/// or wrote a note on, and the way into each other region, with how far
/// and which way each is.
fn near_words(map: &Map, from: RoomId) -> String {
    let Some(start) = map.room(from) else {
        return String::new();
    };
    let mut regions = vec![map.region(start).map(str::to_owned)];
    let mut near: Vec<String> = Vec::new();
    for (id, steps) in map.around(from, NEAR) {
        if near.len() == NEAR_SAID {
            break;
        }
        let Some(room) = map.room(id) else { continue };
        let marks = map.marks_of(id);
        let region = map.region(room).map(str::to_owned);
        let what = if !marks.is_empty() {
            marks.join(", ")
        } else if !room.note.is_empty() {
            trf("{0}, note: {1}", &[map.short_name(room), &room.note])
        } else if region.is_some() && !regions.contains(&region) {
            let into = trf("into {0}", &[region.as_deref().unwrap_or_default()]);
            regions.push(region);
            into
        } else {
            continue;
        };
        let moves = trn("{0} move", "{0} moves", steps.len() as u64);
        let way = match toward(map, from, id) {
            Some(way) => format!("{moves} {way}"),
            None => moves,
        };
        near.push(format!("{what}, {way}"));
    }
    match near.is_empty() {
        true => trn(
            "Nothing noted within {0} move.",
            "Nothing noted within {0} moves.",
            NEAR as u64,
        ),
        false => trf("Near: {0}.", &[&near.join("; ")]),
    }
}

/// Every key while exploring, as said.
fn all_keys() -> String {
    trf(
        ALL_KEYS,
        &[
            crate::shortcuts::command!("I"),
            crate::shortcuts::command!("L"),
            crate::shortcuts::command!("F"),
            crate::shortcuts::command!("B"),
            crate::shortcuts::command!("N"),
        ],
    )
}
