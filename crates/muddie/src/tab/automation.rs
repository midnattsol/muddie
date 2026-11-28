//! The editor: each kind of rule, the variables and the scripts, in a
//! list on the left with the selected one's details beside it.

use super::*;

mod aliases;
mod detail;
mod folders;
mod gauges;
mod history;
mod list;
mod macros;
mod page;
mod scripts;
mod selection;
mod tags;
mod timers;
mod triggers;
mod undo;
mod values;
mod variables;

pub(super) use folders::{Dragged, Renaming};
pub(super) use list::{Spot, folder_label, name_of};
pub(super) use scripts::Checks;
pub(super) use triggers::Extra;
pub(super) use undo::Undo;
// What the sections share, and what one needs of another.
use crate::rules::{fill, tag_key};
use crate::theme::Weight;
use crate::widgets;
use detail::*;
pub(super) use page::section_name;
use scripts::EXAMPLES;
use tags::*;
use values::*;

/// The sections of the editor, in the order of the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Triggers,
    Aliases,
    Macros,
    Timers,
    Gauges,
    Variables,
    Scripts,
    /// What the MUD sends besides text: not rules, but what they are
    /// made from.
    Data,
}

pub(super) use Section::{
    Aliases as ALIASES, Data as DATA, Gauges as GAUGES, Macros as MACROS, Scripts as SCRIPTS,
    Timers as TIMERS, Triggers as TRIGGERS, Variables as VARIABLES,
};

impl Section {
    /// Every section, in the order of the sidebar.
    pub const ALL: [Section; 8] = [
        TRIGGERS, ALIASES, MACROS, TIMERS, GAUGES, VARIABLES, SCRIPTS, DATA,
    ];

    /// The kind of thing it holds, for those that are kept in folders and
    /// switched by their place.
    pub fn kind(self) -> Option<Kind> {
        match self {
            TRIGGERS => Some(Kind::Trigger),
            ALIASES => Some(Kind::Alias),
            MACROS => Some(Kind::Macro),
            TIMERS => Some(Kind::Timer),
            GAUGES => Some(Kind::Gauge),
            SCRIPTS => Some(Kind::Script),
            VARIABLES | DATA => None,
        }
    }
}

/// What is chosen in each section, by the section.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Selected([usize; 8]);

impl std::ops::Index<Section> for Selected {
    type Output = usize;
    fn index(&self, section: Section) -> &usize {
        &self.0[section as usize]
    }
}

impl std::ops::IndexMut<Section> for Selected {
    fn index_mut(&mut self, section: Section) -> &mut usize {
        &mut self.0[section as usize]
    }
}

/// The field of text being typed in, if one has the keyboard.
pub(super) fn typing_in(ctx: &egui::Context) -> Option<egui::Id> {
    let focused = ctx.memory(|memory| memory.focused())?;
    egui::TextEdit::load_state(ctx, focused).map(|_| focused)
}

impl Tab {
    /// Opens the automation window on one of its sections.
    pub fn open_automation(&mut self, section: Section) {
        self.layout_view.open = false;
        if !self.editor.open {
            self.editor.draft = self.rules.clone();
            self.editor.seen = self.rules.clone();
            self.editor.history.clear();
            self.editor.drafting = true;
        }
        self.editor.section = section;
        self.editor.folder = None;
        self.reveal_chosen();
        // Opened with the keyboard, the keyboard goes to the section.
        self.editor.focus_section |= !self.editor.open;
        self.editor.open = true;
        self.editor.marked.clear();
        self.list_scripts();
    }

    /// Chooses one of the rules of a section.
    pub(super) fn select_rule(&mut self, section: Section, index: usize) {
        self.editor.selected[section] = index;
        self.editor.marked.clear();
    }

    /// Opens the automation view where it was left.
    pub fn reopen_automation(&mut self) {
        self.open_automation(self.editor.section);
    }

    /// Saves what is being edited once the edits stop for a moment, and
    /// at once when what they are made in closes.
    pub(super) fn rules_autosave(&mut self, ctx: &egui::Context, open: bool) {
        if self.editor.draft != self.editor.seen {
            let before = std::mem::replace(&mut self.editor.seen, self.editor.draft.clone());
            self.record_step(before, typing_in(ctx));
            self.editor.settle.changed();
        }
        // The script being edited is saved the same way.
        if self.editor.script_text != self.editor.script_seen {
            self.editor.script_seen = self.editor.script_text.clone();
            self.editor.script_settle.changed();
        }
        if self.editor.script_settle.due(ctx) {
            self.flush_script();
        }
        if !open {
            self.close_automation();
        } else if self.editor.settle.due(ctx) && self.editor.draft != self.rules {
            self.save_rules();
        }
    }

    /// Closes the automation window, saving what was still unsaved. One
    /// never opened has nothing of its own: what it holds is not the rules,
    /// and is never saved over them.
    pub fn close_automation(&mut self) {
        self.editor.open = false;
        self.editor.recording_key = false;
        self.editor.settle.clear();
        self.flush_script();
        if std::mem::take(&mut self.editor.drafting) && self.editor.draft != self.rules {
            self.save_rules();
        }
    }
}

/// The editor: whether it is open, the rules and the script being edited,
/// what is chosen and filtered in it, and what it is in the middle of.
pub(super) struct Editor {
    /// The copy being edited in the rules window.
    pub(super) draft: Rules,
    /// The draft as last seen, and when it last changed: edits are saved
    /// once they stop for a moment.
    pub(super) seen: Rules,
    /// Whether the copy being edited was taken from the rules, as the
    /// editor or a gauge's details open: only then is it saved over them.
    pub(super) drafting: bool,
    pub(super) open: bool,
    /// The rule selected in each section of the automation window.
    pub(super) selected: Selected,
    /// The rows of the current section chosen together, when several are;
    /// the one selected is among them.
    pub(super) marked: Vec<usize>,
    /// The lines of the current section's list in the order they are
    /// shown, folders and things, for the keys and for choosing a range.
    pub(super) visible: Vec<Spot>,
    /// The folder chosen in the current section's list, if a folder is
    /// what is chosen.
    pub(super) folder: Option<String>,
    /// What is chosen was gone to by the keys, and is to be brought into
    /// sight.
    pub(super) reveal: bool,
    /// The closed folder what is dragged is held over, and since when: it
    /// opens after a moment.
    pub(super) drag_hover: Option<(String, Instant)>,
    /// The line a menu was opened on, while the menu is open, if it is not
    /// what is chosen: what the menu acts on.
    pub(super) menu_target: Option<Spot>,
    /// The folders among the profile's scripts, as they were last looked
    /// for on the disk.
    pub(super) script_folders: Vec<String>,
    /// What the automation window's search box holds.
    pub(super) filter: String,
    /// When the rules being edited last changed, to save them after.
    pub(super) settle: crate::autosave::Settle,
    /// A sample line or command to try the selected rule on.
    pub(super) sample: String,
    /// Which of triggers, aliases and keys the rules window shows.
    pub(super) section: Section,
    /// The script files that can be edited, and the one being edited:
    /// its file, its name as typed, its text, and that text as saved.
    pub(super) script_files: Vec<PathBuf>,
    pub(super) script_loaded: Option<PathBuf>,
    pub(super) script_name: String,
    pub(super) script_text: String,
    pub(super) script_saved: String,
    pub(super) script_seen: String,
    /// When the script being edited last changed, to save it after.
    pub(super) script_settle: crate::autosave::Settle,
    /// When the scripts were last run from the editor, and what kept
    /// them from running, if anything did.
    pub(super) script_ran: Option<(String, Option<String>)>,
    /// The next key pressed becomes the selected macro's key.
    pub(super) recording_key: bool,
    /// The palette of commands: which one is chosen, and the text it was
    /// put away over, so that it stays away until that changes.
    /// What is being dragged to another place in the editor's list.
    pub(super) dragged: Option<Dragged>,
    /// The folder whose name is being typed.
    pub(super) renaming: Option<Renaming>,
    /// The name in the details is to be given the keys, to type a new one.
    pub(super) rename: bool,
    /// Whether a field had the keys at the end of the last frame.
    pub(super) was_typing: bool,
    /// The keyboard is to go to the section chosen, as the editor opens.
    pub(super) focus_section: bool,
    /// The section chosen, as the keyboard goes to it with F6.
    pub(super) zone_section: Option<egui::Id>,
    /// When the scripts were last restarted, and the scripts' files (or
    /// the rules) that failed then.
    pub(super) restarted: Option<String>,
    pub(super) failed: Vec<String>,
    /// The trigger whose details are shown, and what it was asked to do
    /// besides its commands though that is still empty.
    pub(super) extras_added: (usize, Vec<automation::Extra>),
    /// The gauge whose details are open over the game.
    pub(super) gauge_here: Option<usize>,
    /// What was last deleted in the editor, to have it back.
    pub(super) undo: Option<automation::Undo>,
    /// What checking the scripts being edited said.
    pub(super) checks: automation::Checks,
    /// The pattern being tried, as it was last compiled.
    pub(super) patterns: automation::Patterns,
    /// The variable being edited: which, its name as typed, its value as
    /// typed.
    pub(super) variable_loaded: Option<String>,
    pub(super) variable_name: String,
    pub(super) variable_text: String,
    pub(super) variable_kind: usize,
    pub(super) variable_number: f64,
    pub(super) variable_flag: bool,
    pub(super) variable_items: Vec<(String, String)>,
    /// The tag the editor's lists are narrowed to.
    pub(super) tag_filter: Option<String>,
    /// The folders open: the others are closed, as they are at first, so
    /// that the lists are short to go through. Kept for each profile, as
    /// `opened_for` says, in the settings.
    pub(super) opened: Vec<(Kind, String)>,
    pub(super) opened_for: Option<String>,
    /// The widths of the automation view's first two panes, and how tall
    /// its details are.
    pub(super) panes: [f32; 2],
    pub(super) detail_height: f32,
    /// How tall the editors of commands and of scripts were made, and
    /// whether the one of the rule shown is open over the window.
    pub(super) code: widgets::CodeSizes,
    /// The steps to undo and to redo.
    pub(super) history: history::History,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            draft: Rules::default(),
            seen: Rules::default(),
            drafting: false,
            open: false,
            selected: Selected::default(),
            marked: Vec::new(),
            visible: Vec::new(),
            folder: None,
            reveal: false,
            drag_hover: None,
            menu_target: None,
            script_folders: Vec::new(),
            filter: String::new(),
            settle: Default::default(),
            sample: String::new(),
            section: TRIGGERS,
            script_files: Vec::new(),
            script_loaded: None,
            script_name: String::new(),
            script_text: String::new(),
            script_saved: String::new(),
            script_seen: String::new(),
            script_settle: Default::default(),
            script_ran: None,
            recording_key: false,
            dragged: None,
            renaming: None,
            rename: false,
            was_typing: false,
            focus_section: false,
            zone_section: None,
            restarted: None,
            failed: Vec::new(),
            extras_added: (usize::MAX, Vec::new()),
            gauge_here: None,
            undo: None,
            checks: automation::Checks::default(),
            patterns: automation::Patterns::default(),
            variable_loaded: None,
            variable_name: String::new(),
            variable_text: String::new(),
            variable_kind: 0,
            variable_number: 0.0,
            variable_flag: false,
            variable_items: Vec::new(),
            tag_filter: None,
            opened: Vec::new(),
            opened_for: None,
            panes: Settings::default().automation_panes,
            detail_height: 600.0,
            code: widgets::CodeSizes::default(),
            history: history::History::default(),
        }
    }
}
