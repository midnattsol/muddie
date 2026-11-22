//! The commands of the client, in a palette that Ctrl+K opens over the
//! window: type to find one, the arrows choose, Enter runs it, and Escape
//! or a click anywhere else closes it, with the game in view all along.
//! Typed in the command line after a `/` they are carried out the same.
//! What a command answers is not put among the lines of the game: a line
//! of it is said over the corner, or in the palette if it was run from
//! there, and a list of things is shown in the palette, to choose from.

use super::*;
use crate::theme::Weight;

mod acts;
mod answers;
mod window;

/// Something the palette offers.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// What choosing it puts in the command line.
    pub insert: String,
    /// How it is shown: the command as it is typed.
    pub usage: String,
    /// The group it is shown under; none for what a command listed.
    pub group: String,
    /// What it is, or for, shown after it: what a command does, or where
    /// a place is and how far.
    pub about: String,
    /// Whether it takes something that has yet to be typed, so that
    /// choosing it completes it rather than runs it.
    pub more: bool,
    /// What choosing it does, if it is not a command to run.
    pub act: Option<Act>,
}

/// Something the palette can do that is not a command typed: go to a
/// view or a section, open the settings, connect to a profile.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// The game, the editor where it was left, or what the MUD sends.
    View(usize),
    /// A section of the editor.
    Section(usize),
    /// Something the window does rather than the session.
    App(AppRequest),
    Reconnect,
    Disconnect,
    Find,
    /// Copies all the output, for those who do not choose it with the
    /// pointer.
    CopyOutput,
    /// Starts or stops writing the session to a file.
    Log,
    /// Switches the voice on or off.
    Speech,
    /// Reads the channel of this name aloud, or keeps it quiet.
    ReadChannel(String, bool),
    /// Shows or hides the panel with this number.
    Panel(u32),
    /// Switches to the saved profile of this name and connects.
    Connect(String),
    /// Goes to the nearest room of this name, or mark; or, asked for the
    /// way, says it.
    Place(String),
    /// Does what a script offered to do with the map, chosen by this.
    MapTool(String),
    /// Undoes the last change the map made of itself.
    UndoChange,
    /// Carries out one of Muddie's commands, offered under a name of its
    /// own.
    Run(String),
    /// Asks, in the palette's field, what something is to be done with.
    Ask(Asking),
    /// What was asked, given: the answer chosen or typed.
    Answer(String),
    /// Tidies the area the player is in, asking first if it is large.
    TidyHere,
}

/// Something to do that needs a word more, asked in the palette's field:
/// what is asked, the command it completes, and what is offered to choose
/// from as it is typed.
#[derive(Debug, Clone, PartialEq)]
pub struct Asking {
    pub(super) title: String,
    pub(super) command: String,
    pub(super) offers: Offers,
}

/// What is offered as an answer, as it is typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offers {
    /// Nothing: what is typed is the answer.
    Nothing,
    /// The places of the map.
    Places,
    /// The rooms' marks.
    Marks,
    /// The exits of the room the player is in.
    Exits,
    /// The plugins installed.
    Plugins,
}

impl Act {
    /// Whether it is among the few the palette offers before anything is
    /// typed: what is done most, in the game.
    fn is_front(&self) -> bool {
        match self {
            Act::Run(command) => {
                ["/map where", "/map near", "/map stop", "/map explore"].contains(&command.as_str())
            }
            Act::Reconnect | Act::Speech | Act::MapTool(_) | Act::UndoChange => true,
            _ => false,
        }
    }
}

/// What a session asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppRequest {
    NewSession,
    CloseSession,
    /// The settings, at this page.
    Settings(crate::app::Section),
    /// The Profiles window.
    Profiles,
    Shortcuts,
    About,
    /// Something done with the saved profile of this name.
    Profile(crate::app::ProfileAction, String),
    /// Keeps this session, which has none, as a profile.
    SaveAsProfile,
    /// Asks whether to remove this plugin, then does.
    RemovePlugin(String),
}

/// What a command said: a line of text, and for a thing that can be
/// chosen, what it is and the command that choosing it runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub text: String,
    pub about: String,
    pub run: Option<String>,
}

impl Tab {
    /// Opens the palette of commands, or closes it.
    pub fn toggle_palette(&mut self) {
        self.palette.open = !self.palette.open;
        self.palette.query.clear();
        self.palette.chosen = 0;
        self.palette.results = None;
        self.palette.title = None;
        self.palette.message = None;
        self.palette.spoken_group = None;
        self.palette.focus = self.palette.open;
        // The press that opened it is not one elsewhere.
        self.palette.fresh = self.palette.open;
    }

    /// Keeps what was run from the palette, to offer it first with nothing
    /// typed: the last few, each once.
    fn remember_chosen(&mut self, mut item: Item) {
        const RECENT: usize = 5;
        item.group = tr("Recent").to_owned();
        let recent = &mut self.palette.recent;
        recent.retain(|known| known.usage != item.usage);
        recent.insert(0, item);
        recent.truncate(RECENT);
    }

    fn close_palette(&mut self) {
        self.palette.open = false;
        self.palette.asking = None;
        self.palette.results = None;
        self.palette.title = None;
        self.palette.message = None;
        self.palette.query.clear();
        self.palette.chosen = 0;
    }

    /// What the palette offers: what the last thing done listed, until
    /// something is typed; while it asks, what may answer it; else what can
    /// be done and the places of the map, as typed, or with nothing typed
    /// what was done last and the few most used.
    pub(super) fn palette_items(&mut self) -> Vec<Item> {
        if self.palette.query.is_empty()
            && let Some(results) = &self.palette.results
        {
            return results.clone();
        }
        // What was answered is read alone, till something is typed.
        if self.palette.query.is_empty() && self.palette.message.is_some() {
            return Vec::new();
        }
        let query = self.palette.query.trim().to_lowercase();
        if let Some(asking) = self.palette.asking.clone() {
            return self.answers(asking.offers, &query);
        }
        // Worked out when what is typed, the map or where the player is
        // changes, not every frame: the places listed say how far.
        let key = {
            let map = self.map.borrow();
            let here = map.current().map(|room| room.id);
            (self.palette.query.clone(), map.revision(), here)
        };
        if let Some((known, items)) = &self.palette.cache
            && *known == key
        {
            return items.clone();
        }
        let acts = self.palette_acts();
        let named = |item: &&Item| {
            let words = format!("{} {} {}", item.usage, item.about, item.group).to_lowercase();
            query.split_whitespace().all(|word| words.contains(word))
        };
        let mut items: Vec<Item> = match query.is_empty() {
            // With nothing typed, what was done last, and the few most used.
            true => (self.palette.recent.iter().cloned())
                .chain(
                    (acts.iter())
                        .filter(|item| item.act.as_ref().is_some_and(Act::is_front))
                        .cloned(),
                )
                .collect(),
            false => acts.iter().filter(named).cloned().collect(),
        };
        if !query.is_empty() {
            items.extend(self.palette_places(&query));
        }
        order_by_relevance(&mut items, &query);
        self.palette.cache = Some((key, items.clone()));
        items
    }

    /// What may answer what is asked, of what it offers, as typed.
    fn answers(&self, offers: Offers, query: &str) -> Vec<Item> {
        let answer = |name: String, about: String| Item {
            insert: name.clone(),
            usage: name.clone(),
            group: String::new(),
            about,
            more: false,
            act: Some(Act::Answer(name)),
        };
        let has = |name: &str| name.to_lowercase().contains(query);
        match offers {
            Offers::Nothing => Vec::new(),
            Offers::Places if query.is_empty() => Vec::new(),
            Offers::Places => (self.palette_places(query).into_iter())
                .filter_map(|item| match item.act {
                    Some(Act::Place(name)) => Some(answer(name, item.about)),
                    _ => None,
                })
                .collect(),
            Offers::Marks => (self.map.borrow().marks().iter())
                .filter(|(name, _)| has(name))
                .map(|(name, _)| answer(name.to_owned(), String::new()))
                .collect(),
            Offers::Exits => {
                let map = self.map.borrow();
                let ways: Vec<String> = (map.current().into_iter())
                    .flat_map(|room| room.exits.keys().cloned())
                    .collect();
                (ways.iter())
                    .map(|way| crate::mapview::direction_words(&map, way))
                    .filter(|word| has(word))
                    .map(|word| answer(word, String::new()))
                    .collect()
            }
            Offers::Plugins => (self.profiles.as_ref())
                .map(Profiles::packages)
                .unwrap_or_default()
                .into_iter()
                .filter(|name| has(name))
                .map(|name| answer(name, String::new()))
                .collect(),
        }
    }

    /// Asks, in the palette's field, what something is to be done with:
    /// the field is emptied, and named and said as what it asks.
    pub(super) fn ask(&mut self, asking: Asking) {
        self.voice.say_now(&asking.title);
        self.palette.query.clear();
        self.palette.chosen = 0;
        self.palette.cache = None;
        self.palette.focus = true;
        self.palette.asking = Some(asking);
    }

    /// Stops asking, back to looking for something to do.
    pub(super) fn stop_asking(&mut self) {
        if self.palette.asking.take().is_some() {
            self.palette.query.clear();
            self.palette.chosen = 0;
            self.palette.cache = None;
            self.voice.say_now(tr("Back to the search."));
        }
    }

    /// Does what was asked, with the answer given.
    pub(super) fn answer(&mut self, answer: &str) {
        let Some(asking) = self.palette.asking.take() else {
            return;
        };
        let answer = answer.trim();
        let done = format!("{} {answer}", asking.title.trim_end_matches('…'));
        let line = format!("{}{answer}", asking.command);
        self.remember_chosen(Item {
            insert: String::new(),
            usage: done,
            group: String::new(),
            about: String::new(),
            more: false,
            act: Some(Act::Run(line.clone())),
        });
        self.palette.query.clear();
        self.palette.doing = Some(asking.title.clone());
        // Taking a plugin away, files and all, is asked once more.
        if let Some(name) = line.strip_prefix("/plugin remove ") {
            self.close_palette();
            self.app_request = Some(AppRequest::RemovePlugin(name.to_owned()));
            return;
        }
        self.run_command_from(&line, true);
        // What it answered stays in sight; what answers nothing is done.
        if self.palette.message.is_none() && self.palette.results.is_none() {
            self.close_palette();
        }
    }

    /// Does what choosing `item` means.
    fn choose(&mut self, item: &Item, ctx: &egui::Context) {
        match item.act.clone() {
            Some(Act::Ask(asking)) => self.ask(asking),
            Some(Act::Answer(answer)) => self.answer(&answer),
            Some(Act::Run(line)) => {
                self.remember_chosen(item.clone());
                self.palette.query.clear();
                self.palette.doing = Some(item.usage.clone());
                self.run_command_from(&line, true);
                if self.palette.message.is_none() && self.palette.results.is_none() {
                    self.close_palette();
                }
            }
            Some(act) => {
                self.remember_chosen(item.clone());
                self.close_palette();
                self.act(act, ctx);
            }
            // Something only to be read: done with the list.
            None if item.insert.is_empty() => self.close_palette(),
            // What something done listed, to choose: it is done.
            None => {
                let line = item.insert.clone();
                self.palette.query.clear();
                self.run_command_from(&line, true);
                if self.palette.message.is_none() && self.palette.results.is_none() {
                    self.close_palette();
                }
            }
        }
    }
}

/// How well `item` answers what is typed: its name beginning so first,
/// then one of its words, then having it anywhere, then only what it says
/// of itself having it.
fn relevance(item: &Item, typed: &str) -> usize {
    let name = item.usage.to_lowercase();
    if typed.is_empty() || name.starts_with(typed) {
        0
    } else if (name.split(|c: char| !c.is_alphanumeric())).any(|word| word.starts_with(typed)) {
        1
    } else if name.contains(typed) {
        2
    } else {
        3
    }
}

/// Puts each group together, the groups by how well the best of each
/// answers what is typed and then in the order they first come, and in
/// each group the best first, as they came for the rest.
fn order_by_relevance(items: &mut Vec<Item>, typed: &str) {
    let mut groups: Vec<(String, usize)> = Vec::new();
    for item in items.iter() {
        let (group, score) = (group_of(item), relevance(item, typed));
        match groups.iter_mut().find(|(known, _)| *known == group) {
            Some((_, best)) => *best = (*best).min(score),
            None => groups.push((group, score)),
        }
    }
    let rank = |item: &Item| {
        let group = group_of(item);
        let at = groups
            .iter()
            .position(|(known, _)| *known == group)
            .unwrap_or(0);
        (groups[at].1, at, relevance(item, typed))
    };
    let mut ranked: Vec<(usize, Item)> = items.drain(..).enumerate().collect();
    ranked.sort_by_key(|(index, item)| (rank(item), *index));
    items.extend(ranked.into_iter().map(|(_, item)| item));
}

/// The group something offered is shown under: what it does, for what
/// the palette does; the commands, for the commands.
pub(super) fn group_of(item: &Item) -> String {
    item.group.clone()
}

/// What the palette's items are worked out from: what is typed, the
/// map's revision and the room the player is in.
type PaletteKey = (String, u64, Option<RoomId>);

/// The palette of commands: whether it is open, what is typed in it, what
/// it lists, and what the last command answered.
#[derive(Default)]
pub(super) struct Palette {
    pub(super) open: bool,
    /// The palette opened in this frame, by a press that is not to close it.
    pub(super) fresh: bool,
    pub(super) query: String,
    /// The palette's field is to take the keyboard, or its cursor to go
    /// to the end of what was put in it.
    pub(super) focus: bool,
    pub(super) cursor_end: bool,
    pub(super) cache: Option<(PaletteKey, Vec<commands::Item>)>,
    pub(super) chosen: usize,
    /// The one chosen was chosen with the keys: the list is to be scrolled
    /// to it.
    pub(super) reveal: bool,
    /// What the command being carried out has said so far, and whether
    /// it said it could not do what was asked.
    pub(super) answer: Vec<commands::Said>,
    pub(super) answering: bool,
    pub(super) answer_failed: bool,
    /// What the last command listed, to choose from, under the command;
    /// or the line it answered, when it was run from the palette, and
    /// whether that line is of something that could not be done.
    pub(super) results: Option<Vec<commands::Item>>,
    pub(super) title: Option<String>,
    /// The group said last, to say the next when it is another.
    pub(super) spoken_group: Option<String>,
    /// What was run from it last, newest first, to offer with nothing
    /// typed.
    pub(super) recent: Vec<commands::Item>,
    pub(super) message: Option<(String, bool)>,
    /// What is being asked in its field, while it asks.
    pub(super) asking: Option<Asking>,
    /// What was chosen, by its name, while it is done: what it lists is
    /// shown under it.
    pub(super) doing: Option<String>,
    /// When what is typed last changed, until what it finds is said.
    pub(super) typed_at: Option<Instant>,
}
