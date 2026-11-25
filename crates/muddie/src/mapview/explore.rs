//! Exploring the map with the keys, for whoever does not see it: the room
//! chosen moves along the exits and is said, and places are looked for,
//! named and noted, all without leaving the map or sending anything to the
//! MUD.

use super::*;
use crate::i18n::{trf, trn};
use muddie_mapper::Found;

/// What the line over the map asks for while exploring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Asked {
    /// What to look for on the map.
    Search,
    /// A mark for the room chosen.
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
    /// For how many frames more it asks for the keyboard: two, as it is
    /// opened, for a menu it was chosen in gives the keyboard back where
    /// it was as it closes, in the frame after.
    focus: u8,
}

/// The shortcuts the map takes while exploring: all about the room, what
/// is near, search, mark and note.
const COMMANDS: [crate::shortcuts::Shortcut; 5] = [
    crate::shortcuts::ROOM_INFO,
    crate::shortcuts::NEAR,
    crate::shortcuts::FIND,
    crate::shortcuts::MARK,
    crate::shortcuts::NOTE,
];

/// What the map's keys are, in short: said on starting, and with the map
/// when it is reached.
pub(super) fn keys_hint() -> &'static str {
    tr("Arrows follow the exits, Enter goes, Escape goes back. F1 says all the keys.")
}

impl MapView {
    /// Whether the map is being explored with the keys: it has the
    /// keyboard, or asks for some words over it. The keys it takes then
    /// are its own, not bound to anything else.
    pub fn exploring(&self) -> bool {
        self.had_focus || self.prompt.is_some()
    }

    /// Run as a frame begins: if the map was not drawn in the one before,
    /// it is no longer explored, as nothing would read its keys. Returns
    /// whether it was.
    pub fn left_unseen(&mut self) -> bool {
        if std::mem::take(&mut self.drawn) {
            return false;
        }
        let was = self.exploring();
        self.had_focus = false;
        self.prompt = None;
        self.keypad.clear();
        was
    }

    /// Whether the map takes this key while exploring, before anything
    /// bound to it: the keypad, F1, and its shortcuts.
    pub fn takes(&self, key: egui::Key, modifiers: egui::Modifiers) -> bool {
        let shortcut = |keys: egui::KeyboardShortcut| {
            keys.logical_key == key && modifiers.matches_logically(keys.modifiers)
        };
        self.exploring()
            && (key == egui::Key::F1 || COMMANDS.iter().any(|command| shortcut(command.keys())))
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
    ) -> Option<MapAsk> {
        let had_focus = std::mem::replace(&mut self.had_focus, response.has_focus());
        if let Some(ask) = self.arrived_or_escaped(ui, response, map, current, had_focus) {
            return Some(ask);
        }
        if !response.has_focus() {
            return None;
        }
        if let Some(ask) = self.typed_leaves(ui, response) {
            return Some(ask);
        }
        // The arrows are the map's while it has the keyboard, not to move
        // to another widget.
        lock_arrows(ui, response.id);
        let chosen = self.selected.unwrap_or(current.id);
        // The keypad first, by where its keys are: what egui made of them,
        // Home for 7 with Num Lock off, was taken away.
        for name in std::mem::take(&mut self.keypad) {
            if let Some(action) = self.keypad_key(map, chosen, name) {
                return Some(action);
            }
        }
        self.keys_pressed(ui, map, current, chosen)
    }

    /// What the keys that are not the keypad's ask for, with `chosen` the
    /// room they work from: the keys that say, search, name and note, go
    /// back or leave, and the ones that move along the exits.
    fn keys_pressed(
        &mut self,
        ui: &Ui,
        map: &Map,
        current: &Room,
        chosen: RoomId,
    ) -> Option<MapAsk> {
        let pressed = |modifiers, key| ui.input_mut(|input| input.consume_key(modifiers, key));
        if pressed(egui::Modifiers::NONE, egui::Key::F1) {
            return Some(MapAsk::Say(all_keys()));
        }
        let shortcut =
            |shortcut: crate::shortcuts::Shortcut| ui.input_mut(|input| shortcut.pressed(input));
        if shortcut(crate::shortcuts::ROOM_INFO) {
            return Some(MapAsk::Info(chosen));
        }
        if shortcut(crate::shortcuts::NEAR) {
            return Some(MapAsk::Say(near_words(map, chosen)));
        }
        let asked = [
            (crate::shortcuts::FIND, Asked::Search),
            (crate::shortcuts::MARK, Asked::Mark),
            (crate::shortcuts::NOTE, Asked::Note),
        ];
        if let Some((_, asked)) = asked.into_iter().find(|(keys, _)| shortcut(*keys)) {
            self.ask(map, asked, chosen);
            return None;
        }
        if pressed(egui::Modifiers::NONE, egui::Key::Space) {
            return Some(MapAsk::Selected(chosen, None));
        }
        if pressed(egui::Modifiers::NONE, egui::Key::Backspace) {
            let Some(back) = self.trail.pop() else {
                return Some(MapAsk::Say(
                    tr("This is where you began exploring.").to_owned(),
                ));
            };
            self.selected = Some(back);
            self.reveal = Some(back);
            return Some(MapAsk::Selected(back, None));
        }
        if let Some(id) = self.selected {
            if pressed(egui::Modifiers::SHIFT, egui::Key::Enter) {
                return Some(MapAsk::Way(id));
            }
            if pressed(egui::Modifiers::NONE, egui::Key::Enter) {
                return Some(MapAsk::Walk(id));
            }
        }
        if pressed(egui::Modifiers::NONE, egui::Key::Home) {
            // A jump, not a move: nothing is said of the way back.
            self.choose(current.id);
            return Some(MapAsk::Selected(current.id, None));
        }
        let (_, way) = MOVES
            .iter()
            .find(|(key, _)| pressed(egui::Modifiers::NONE, *key))?;
        Some(self.step(map, chosen, way))
    }

    /// What arriving on the map with Tab, or Escape pressed on it, asks
    /// for; none if it is neither.
    fn arrived_or_escaped(
        &mut self,
        ui: &Ui,
        response: &Response,
        map: &Map,
        current: &Room,
        had_focus: bool,
    ) -> Option<MapAsk> {
        // Reached with Tab rather than started: started all the same,
        // from the room chosen or the player's. Given back by a menu that
        // closed, it goes on as it was.
        let arrived = response.has_focus() && !had_focus;
        let started = arrived && std::mem::take(&mut self.explore_started);
        if arrived && !started && ui.input(|input| input.key_pressed(egui::Key::Tab)) {
            let from = self.selected.and_then(|id| map.room(id)).unwrap_or(current);
            return Some(MapAsk::Say(self.explore(map, from)));
        }
        // Escape, or anything typed but the digits that move: back to the
        // command line, with what was typed.
        // While walking, Escape stops the walk first, as it says; then it
        // leaves.
        if had_focus && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            if self.walking {
                // egui took the keyboard away on Escape: it stays here.
                response.request_focus();
                self.had_focus = true;
                return Some(MapAsk::Command("/map stop"));
            }
            return Some(self.leave(ui, response, String::new()));
        }
        None
    }

    /// Leaves the map for the command line, with what was typed, if
    /// anything but the digits that move was.
    fn typed_leaves(&mut self, ui: &Ui, response: &Response) -> Option<MapAsk> {
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
        None
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
    ) -> Option<MapAsk> {
        let prompt = self.prompt.as_mut()?;
        let id = ui.make_persistent_id("map prompt");
        let focused = ui.memory(|memory| memory.has_focus(id));
        let mut said = None;
        let searched = prompt.searched.as_deref() == Some(prompt.text.trim());
        if focused && searched && !prompt.found.is_empty() {
            said = found_arrows(ui, map, here, prompt);
        }
        let (field, label) = prompt_field(ui, map, prompt, id);
        // Opened, what it asks is said for whoever has no screen reader to
        // say where the keyboard went.
        if prompt.focus == 2 {
            said = said.or(Some(MapAsk::Asking(label.clone())));
        }
        // What was found, shown as it is said, for whoever sees it.
        if searched {
            show_found(ui, map, here, prompt);
        }
        if prompt.focus > 0 {
            prompt.focus -= 1;
            field.request_focus();
        }
        // The arrows go through what was found, not to another control.
        lock_arrows(ui, id);
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
                Some(MapAsk::Selected(to, None))
            }
            Asked::Search => Some(search_for(map, here, prompt, &text, &field)),
            asked => {
                self.prompt = None;
                self.focus_wanted = true;
                Some(match asked {
                    Asked::Mark => MapAsk::Mark(room, text),
                    _ => MapAsk::Note(room, text),
                })
            }
        }
    }

    /// Asks, in a line under the map, what is to be searched, or a room's
    /// mark or note: from the keys, or the room's menu.
    pub(super) fn ask(&mut self, map: &Map, asked: Asked, room: RoomId) {
        let note = map.room(room).map(|room| room.note.clone());
        self.prompt = Some(Prompt {
            asked,
            room,
            text: note.filter(|_| asked == Asked::Note).unwrap_or_default(),
            searched: None,
            found: Vec::new(),
            at: 0,
            focus: 2,
        });
    }
}

/// Keeps the arrows for the control `id`, not to move to another one.
fn lock_arrows(ui: &Ui, id: egui::Id) {
    let filter = egui::EventFilter {
        horizontal_arrows: true,
        vertical_arrows: true,
        ..Default::default()
    };
    ui.memory_mut(|memory| memory.set_focus_lock_filter(id, filter));
}

/// Moves the arrows' place among what was found by the arrow keys
/// pressed. Gives what to say of the new place, if it moved.
fn found_arrows(ui: &Ui, map: &Map, here: Option<RoomId>, prompt: &mut Prompt) -> Option<MapAsk> {
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
        return Some(MapAsk::Say(found_line(map, here, prompt)));
    }
    None
}

/// The text field of the line over the map, with its label beside it.
/// Gives the field and the label.
fn prompt_field(ui: &mut Ui, map: &Map, prompt: &mut Prompt, id: egui::Id) -> (Response, String) {
    let room =
        (map.room(prompt.room)).map_or_else(String::new, |room| map.short_name(room).to_owned());
    let label = match prompt.asked {
        Asked::Search => tr("Find on the map").to_owned(),
        Asked::Mark => trf("Mark for {0}", &[&room]),
        Asked::Note => trf("Note on {0}", &[&room]),
    };
    let field = ui
        .horizontal(|ui| {
            let words = ui.label(&label);
            let edit = egui::TextEdit::singleline(&mut prompt.text)
                .id(id)
                .desired_width(f32::INFINITY);
            let field = ui.add(edit);
            // Named by its words, which are not read again on their own.
            widgets::named_by(ui, &field, &words, &label);
            field
        })
        .inner;
    (field, label)
}

/// Shows what was found by the search, as it is said.
fn show_found(ui: &mut Ui, map: &Map, here: Option<RoomId>, prompt: &Prompt) {
    if let Some(searched) = &prompt.searched {
        let shown = match prompt.found.is_empty() {
            true => trf("Nothing on the map has {0}.", &[searched]),
            false => found_line(map, here, prompt),
        };
        ui.label(RichText::new(shown).color(theme::current().text_dim));
    }
}

/// Looks for `text`, keeping the keyboard in the field, and says
/// what was found.
fn search_for(
    map: &Map,
    here: Option<RoomId>,
    prompt: &mut Prompt,
    text: &str,
    field: &Response,
) -> MapAsk {
    prompt.found = map.search(text, here);
    prompt.at = 0;
    prompt.searched = Some(text.to_owned());
    // The keyboard stays, for the arrows and Enter.
    field.request_focus();
    MapAsk::Say(match prompt.found.is_empty() {
        true => trf("Nothing on the map has {0}.", &[text]),
        false => {
            let count = trn("{0} found", "{0} found", prompt.found.len() as u64);
            sentences([count, found_line(map, here, prompt)])
        }
    })
}

/// The place the arrows are on among those found, said with where it is
/// in the list: "2 of 5: …".
fn found_line(map: &Map, here: Option<RoomId>, prompt: &Prompt) -> String {
    let (title, detail) = found_words(map, here, &prompt.found[prompt.at]);
    let place = match detail.is_empty() {
        true => title,
        false => trf("{0}, {1}", &[&title, &detail]),
    };
    let at = [(prompt.at + 1).to_string(), prompt.found.len().to_string()];
    trf("{0} of {1}: {2}", &[&at[0], &at[1], &place])
}
