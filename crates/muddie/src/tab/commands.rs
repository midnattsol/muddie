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
    /// What it is for.
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
    /// Lets the side panels reach the top and the bottom, or not.
    FullHeight,
    /// Puts a side away with its panels, or brings it back.
    Side(super::layout::Side),
    /// Switches to the saved profile of this name and connects.
    Connect(String),
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
        self.palette.focus = self.palette.open;
        // The press that opened it is not one elsewhere.
        self.palette.fresh = self.palette.open;
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
        // Worked out when what is typed changes, not every frame.
        if let Some((typed, items)) = &self.palette.cache
            && *typed == self.palette.query
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
            let words = format!("{} {}", item.usage, item.about).to_lowercase();
            query.split_whitespace().all(|word| words.contains(word))
        };
        let mut items: Vec<Item> = acts.iter().filter(named).cloned().collect();
        let commands = offered(&typed, &tags, &plugins);
        // One being typed as a command goes first, to run with Enter.
        if self.palette.query.starts_with('/') || items.is_empty() {
            items.splice(0..0, commands);
        } else {
            items.extend(commands);
        }
        // Each group together, in the order they first come.
        let mut groups: Vec<String> = Vec::new();
        for item in &items {
            let group = group_of(item);
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
        items.sort_by_key(|item| groups.iter().position(|group| *group == group_of(item)));
        self.palette.cache = Some((self.palette.query.clone(), items.clone()));
        items
    }

    /// Does what choosing `item` means: runs it, or, if it takes
    /// something yet to be typed, completes it to type that after. With
    /// `typed_more`, what it takes is already there, and it is run as
    /// typed.
    fn choose(&mut self, item: &Item, typed_more: bool, ctx: &egui::Context) {
        if let Some(act) = item.act.clone() {
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
        self.palette.query.clear();
        self.run_command_from(&line, true);
        // What it answered stays in sight; a command that answers nothing
        // is done with.
        if self.palette.message.is_none() && self.palette.results.is_none() {
            self.close_palette();
        }
    }
}

/// The group something offered is shown under: what it does, for what
/// the palette does; the commands, for the commands.
pub(super) fn group_of(item: &Item) -> String {
    match item.act {
        Some(_) => item.about.clone(),
        None => tr("Commands").to_owned(),
    }
}

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
    pub(super) cache: Option<(String, Vec<commands::Item>)>,
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
    pub(super) message: Option<(String, bool)>,
    /// When what is typed last changed, until what it finds is said.
    pub(super) typed_at: Option<Instant>,
}
