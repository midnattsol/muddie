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
mod offered;
mod window;

pub use offered::offered;
use offered::shown_usage;

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
    /// Undoes the editor's last change, or redoes it.
    Undo(bool),
    /// Starts or stops writing the session to a file.
    Log,
    /// Switches the voice on or off.
    Speech,
    /// Reads the channel of this name aloud, or keeps it quiet.
    ReadChannel(String, bool),
    /// Adds a panel of this kind.
    AddPanel(super::layout::Kind),
    /// Shows or hides the panel with this number.
    Panel(u32),
    /// Names again the panel with this number, in its header.
    RenamePanel(u32),
    /// Lets the side panels reach the top and the bottom, or not.
    FullHeight,
    /// Puts a side away with its panels, or brings it back.
    Side(super::layout::Side),
    /// Switches to the saved profile of this name and connects.
    Connect(String),
    /// Goes to the nearest room of this name, or mark; or, asked for the
    /// way, says it.
    Place(String),
    /// Does what a script offered to do with the map, chosen by this.
    MapTool(String),
    /// Undoes the last room the map added.
    UndoAdded,
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
        self.palette.results = None;
        self.palette.title = None;
        self.palette.message = None;
        self.palette.query.clear();
        self.palette.chosen = 0;
    }

    /// What the palette offers: what the last command listed, until
    /// something is typed, or the commands that begin as what is typed.
    pub(super) fn palette_items(&mut self) -> Vec<Item> {
        if self.palette.query.is_empty()
            && let Some(results) = &self.palette.results
        {
            return results.clone();
        }
        // What a command answered is read alone, till something is typed.
        if self.palette.query.is_empty() && self.palette.message.is_some() {
            return Vec::new();
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
        // As it would be typed in the command line: after a slash.
        let typed = format!("/{}", self.palette.query.trim_start_matches('/'));
        // The tags and the plugins there are, for the commands that take
        // the name of one; the plugins are read from the disk.
        let tags: Vec<(String, bool)> = if typed.starts_with("/tag") {
            let switches = self.scripts.switches();
            (self.scripts.tags().into_iter())
                .map(|name| {
                    let on = switches.tag_on(&name);
                    (name, on)
                })
                .collect()
        } else {
            Vec::new()
        };
        let plugins = match &self.profiles {
            Some(profiles) if typed.starts_with("/plugin ") => profiles.packages(),
            _ => Vec::new(),
        };
        // What can be done, by what it is called, then the commands that
        // begin as what is typed; with nothing typed, the first of both.
        let query = self.palette.query.trim().to_lowercase();
        let acts = self.palette_acts();
        let named = |item: &&Item| {
            let words = format!("{} {} {}", item.usage, item.about, item.group).to_lowercase();
            query.split_whitespace().all(|word| words.contains(word))
        };
        let mut items: Vec<Item> = acts.iter().filter(named).cloned().collect();
        // The places of the map whose name has what is typed.
        if !self.palette.query.starts_with('/') {
            items.extend(self.palette_places(&query));
        }
        let commands = offered(&typed, &tags, &plugins);
        // One being typed as a command goes first, to run with Enter.
        if self.palette.query.starts_with('/') || items.is_empty() {
            items.splice(0..0, commands);
        } else {
            items.extend(commands);
        }
        // With nothing typed, what was run last comes first.
        if query.is_empty() {
            items.splice(0..0, self.palette.recent.iter().cloned());
        }
        order_by_relevance(&mut items, &query);
        self.palette.cache = Some((key, items.clone()));
        items
    }

    /// Does what choosing `item` means: runs it, or, if it takes
    /// something yet to be typed, completes it to type that after. With
    /// `typed_more`, what it takes is already there, and it is run as
    /// typed.
    fn choose(&mut self, item: &Item, typed_more: bool, ctx: &egui::Context) {
        if let Some(act) = item.act.clone() {
            self.remember_chosen(item.clone());
            self.close_palette();
            return self.act(act, ctx);
        }
        // Something only to be read: done with the list.
        if item.insert.is_empty() {
            return self.close_palette();
        }
        let typed = format!("/{}", self.palette.query.trim_start_matches('/'));
        if item.more && !typed_more {
            self.palette.query = item.insert.trim_start_matches('/').to_owned();
            self.palette.chosen = 0;
            self.palette.cursor_end = true;
            return;
        }
        let line = if item.more {
            typed
        } else {
            item.insert.clone()
        };
        self.history.add(&line);
        self.remember_chosen(Item {
            insert: line.clone(),
            usage: line.clone(),
            about: String::new(),
            more: false,
            ..item.clone()
        });
        self.palette.query.clear();
        self.run_command_from(&line, true);
        // What it answered stays in sight; a command that answers nothing
        // is done with.
        if self.palette.message.is_none() && self.palette.results.is_none() {
            self.close_palette();
        }
    }
}

/// How well `item` answers what is typed: its name beginning so first,
/// then one of its words, then having it anywhere, then only what it says
/// of itself having it.
fn relevance(item: &Item, typed: &str) -> usize {
    let name = shown_usage(&item.usage).to_lowercase();
    let typed = typed.trim_start_matches('/');
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
    /// When what is typed last changed, until what it finds is said.
    pub(super) typed_at: Option<Instant>,
}
