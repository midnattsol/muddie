//! The commands of the client, in a palette that Ctrl+K opens over the
//! window: type to find one, the arrows choose, Enter runs it, and Escape
//! or a click anywhere else closes it, with the game in view all along.
//! Typed in the command line after a `/` they are carried out the same.
//! What a command answers is not put among the lines of the game: a line
//! of it is said inside the command line for a moment, and a list of
//! things is shown in the palette, to choose from.

use super::*;
use crate::theme::Weight;

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
    /// Starts or stops writing the session to a file.
    Log,
    /// Switches the voice on or off.
    Speech,
    /// Adds a panel of this kind.
    AddPanel(super::layout::Kind),
    /// Shows or hides the panel with this number.
    Panel(u32),
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
    Shortcuts,
    About,
    /// Something done with the saved profile of this name.
    Profile(crate::app::ProfileAction, String),
    /// Keeps this session, which has none, as a profile.
    SaveAsProfile,
}

/// The words of a command that are typed as they are, not the `<this>` or
/// `[that]` that stand for something to give it.
fn fixed(usage: &str) -> Vec<&str> {
    (usage.split_whitespace())
        .take_while(|word| !word.starts_with(['<', '[']))
        .collect()
}

/// What the palette offers for what has been typed: the commands that
/// begin so, and for those that take the name of something there is, the
/// names.
pub fn offered(typed: &str, groups: &[(String, bool)], plugins: &[String]) -> Vec<Item> {
    if !typed.starts_with('/') {
        return Vec::new();
    }
    let words: Vec<&str> = typed.split_whitespace().collect();
    // A word is still being typed unless a space follows it.
    let open = !typed.ends_with(char::is_whitespace);
    let item = |insert: String, usage: String, about: &str, more: bool| Item {
        insert,
        usage,
        about: about.to_owned(),
        more,
        act: None,
    };
    // The names of what there is, for the commands that take one.
    let named = |prefix: &[&str], names: Vec<(String, String)>| -> Option<Vec<Item>> {
        let given = words.len() == prefix.len() + usize::from(open);
        if words.len() < prefix.len()
            || words[..prefix.len()] != *prefix
            || !(given || (words.len() == prefix.len() && !open))
        {
            return None;
        }
        let start = if open {
            words.last().copied().unwrap_or_default().to_lowercase()
        } else {
            String::new()
        };
        let head = prefix.join(" ");
        let matching = names
            .into_iter()
            .filter(|(name, _)| name.to_lowercase().starts_with(&start));
        let items: Vec<Item> = matching
            .map(|(name, about)| {
                let whole = format!("{head} {name}");
                item(whole.clone(), whole, &about, false)
            })
            .collect();
        (!items.is_empty()).then_some(items)
    };
    let group_names = || {
        groups
            .iter()
            .map(|(name, on)| (name.clone(), tr(if *on { "on" } else { "off" }).to_owned()))
            .collect()
    };
    let plugin_names = || {
        plugins
            .iter()
            .map(|name| (name.clone(), String::new()))
            .collect()
    };
    let by_name = named(&["/group"], group_names())
        .filter(|_| words.len() > 1 || !open)
        .or_else(|| {
            ["enable", "disable", "update", "remove"]
                .iter()
                .find_map(|what| named(&["/plugin", what], plugin_names()))
        });
    if let Some(items) = by_name {
        return items;
    }
    let mut items = Vec::new();
    for (usage, about) in COMMANDS {
        let command = fixed(usage);
        // What was typed must be the start of the command, word by word;
        // past its fixed words it is being given what it takes.
        let within = words.len().min(command.len());
        let exact = if open && words.len() <= command.len() {
            within.saturating_sub(1)
        } else {
            within
        };
        let fits = words[..exact] == command[..exact]
            && (exact == within || command[exact].starts_with(words[exact]));
        let takes_more = usage.split_whitespace().count() > command.len();
        if !fits || (words.len() > command.len() && !takes_more) {
            continue;
        }
        // Only what it cannot do without keeps it from being run as it is.
        let more = usage.contains('<');
        let insert = format!("{}{}", command.join(" "), if more { " " } else { "" });
        items.push(item(insert, (*usage).to_owned(), tr(about), more));
    }
    items
}

/// A command as the palette shows it: without the slash it is typed
/// after in the command line.
fn shown_usage(usage: &str) -> &str {
    usage.strip_prefix('/').unwrap_or(usage)
}

/// What a command said: a line of text, and for a thing that can be
/// chosen, what it is and the command that choosing it runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub text: String,
    pub about: String,
    pub run: Option<String>,
}

/// A line shown inside the command line for a moment.
#[derive(Debug, Clone, PartialEq)]
pub struct BarNote {
    pub text: String,
    /// Something could not be done: it stays until something is typed.
    pub problem: bool,
    /// When it was first shown, in the time of the window.
    pub since: Option<f64>,
}

impl Tab {
    /// Says, as part of what a command answers, a thing that can be
    /// chosen: `command` is run by choosing it. Said outside a command, it
    /// is a line like any other.
    pub(super) fn offer(&mut self, text: &str, about: &str, command: &str) {
        if self.answering {
            self.voice.say(&format!("{text}: {about}"));
            self.answer.push(Said {
                text: text.to_owned(),
                about: about.to_owned(),
                run: Some(command.to_owned()),
            });
        } else {
            self.note(&format!("{text}: {about}"));
        }
    }

    /// Says that what was asked could not be done.
    pub(super) fn refuse(&mut self, text: &str) {
        self.answer_failed |= self.answering;
        self.note(text);
    }

    /// Says something inside the command line, if it is empty to say it
    /// in, and among the lines of the game if not: for what a command
    /// answers some time after it was given.
    pub(super) fn bar(&mut self, text: &str, problem: bool) {
        if self.input.is_empty() {
            self.remember_notice(text, problem);
            self.voice.say(text);
            self.bar_note = Some(BarNote {
                text: text.to_owned(),
                problem,
                since: None,
            });
        } else {
            self.note(text);
        }
    }

    /// Carries out a command of the client and puts what it answers where
    /// it is shown. Returns whether it was one.
    pub(super) fn run_command(&mut self, line: &str) -> bool {
        self.results = None;
        self.bar_note = None;
        self.palette_cache = None;
        self.palette_chosen = 0;
        self.answer.clear();
        (self.answering, self.answer_failed) = (true, false);
        let mut ours = self.do_client_command(line);
        // A line of Lua is a command of the client too, carried out by
        // the scripts: what it prints is its answer, and if it prints
        // nothing, that it was run is.
        if !ours && (line == "/lua" || line.starts_with("/lua ")) {
            self.scripts.on_input(line);
            self.run_actions();
            if self.answer.is_empty() {
                self.note(tr("Done."));
            }
            ours = true;
        }
        self.answering = false;
        let mut said = std::mem::take(&mut self.answer);
        match said.len() {
            0 => {}
            // A line of text is said in the command line itself.
            1 if said[0].run.is_none() => {
                self.bar_note = Some(BarNote {
                    text: said.remove(0).text,
                    problem: self.answer_failed,
                    since: None,
                });
            }
            // More than that is a list, to read and to choose from.
            _ => {
                let items = said.into_iter().map(|said| Item {
                    insert: said.run.unwrap_or_default(),
                    usage: said.text,
                    about: said.about,
                    more: false,
                    act: None,
                });
                self.results = Some(items.collect());
                self.palette_query.clear();
                self.palette_open = true;
            }
        }
        ours
    }

    /// Opens the palette of commands, or closes it.
    pub fn toggle_palette(&mut self) {
        self.palette_open = !self.palette_open;
        self.palette_query.clear();
        self.palette_chosen = 0;
        self.results = None;
        self.palette_focus = self.palette_open;
        // The press that opened it is not one elsewhere.
        self.palette_fresh = self.palette_open;
    }

    fn close_palette(&mut self) {
        self.palette_open = false;
        self.results = None;
        self.palette_query.clear();
        self.palette_chosen = 0;
    }

    /// What the palette offers: what the last command listed, until
    /// something is typed, or the commands that begin as what is typed.
    pub(super) fn palette_items(&mut self) -> Vec<Item> {
        if self.palette_query.is_empty()
            && let Some(results) = &self.results
        {
            return results.clone();
        }
        // Worked out when what is typed changes, not every frame.
        if let Some((typed, items)) = &self.palette_cache
            && *typed == self.palette_query
        {
            return items.clone();
        }
        // As it would be typed in the command line: after a slash.
        let typed = format!("/{}", self.palette_query.trim_start_matches('/'));
        // The groups and the plugins there are, for the commands that
        // take the name of one; the plugins are read from the disk.
        let groups: Vec<(String, bool)> = if typed.starts_with("/group") {
            (self.scripts.groups().into_iter())
                .map(|name| {
                    let on = self.scripts.group_enabled(&name);
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
        let query = self.palette_query.trim().to_lowercase();
        let acts = self.palette_acts();
        let named = |item: &&Item| {
            let words = format!("{} {}", item.usage, item.about).to_lowercase();
            query.split_whitespace().all(|word| words.contains(word))
        };
        let mut items: Vec<Item> = acts.iter().filter(named).cloned().collect();
        let commands = offered(&typed, &groups, &plugins);
        // One being typed as a command goes first, to run with Enter.
        if self.palette_query.starts_with('/') || items.is_empty() {
            items.splice(0..0, commands);
        } else {
            items.extend(commands);
        }
        self.palette_cache = Some((self.palette_query.clone(), items.clone()));
        items
    }

    /// What the palette can do besides commands, each with where it is.
    fn palette_acts(&self) -> Vec<Item> {
        let act = |label: &str, place: &str, act: Act| Item {
            insert: String::new(),
            usage: label.to_owned(),
            about: place.to_owned(),
            more: false,
            act: Some(act),
        };
        let (go, session, panels, settings, help) = (
            tr("Go to"),
            tr("Session"),
            tr("Panels"),
            tr("Settings"),
            tr("Help"),
        );
        let mut acts = vec![
            act(tr("Game"), go, Act::View(0)),
            act(tr("Editor"), go, Act::View(1)),
            act(tr("Incoming data"), go, Act::View(2)),
        ];
        let sections = [
            (tr("Triggers"), automation::TRIGGERS),
            (tr("Aliases"), automation::ALIASES),
            (tr("Macros"), automation::MACROS),
            (tr("Timers"), automation::TIMERS),
            (tr("Gauges"), automation::GAUGES),
            (tr("Variables"), automation::VARIABLES),
            (tr("Scripts"), automation::SCRIPTS),
        ];
        for (name, section) in sections {
            acts.push(act(name, tr("Editor"), Act::Section(section)));
        }
        acts.push(act(tr("Reconnect"), session, Act::Reconnect));
        if self.connection.is_some() {
            acts.push(act(tr("Disconnect"), session, Act::Disconnect));
        }
        acts.push(act(
            tr("New session"),
            session,
            Act::App(AppRequest::NewSession),
        ));
        acts.push(act(
            tr("Close session"),
            session,
            Act::App(AppRequest::CloseSession),
        ));
        acts.push(act(tr("Find"), session, Act::Find));
        let log = if self.log.is_some() {
            tr("Stop logging this session")
        } else {
            tr("Log this session")
        };
        acts.push(act(log, session, Act::Log));
        let speech = if self.voice.enabled() {
            tr("Switch speech off")
        } else {
            tr("Switch speech on")
        };
        acts.push(act(speech, session, Act::Speech));
        match &self.profile {
            Some(name) => acts.push(act(
                tr("Edit profile"),
                session,
                Act::App(AppRequest::Profile(
                    crate::app::ProfileAction::Edit,
                    name.clone(),
                )),
            )),
            None => acts.push(act(
                tr("Save as profile"),
                session,
                Act::App(AppRequest::SaveAsProfile),
            )),
        }
        for panel in &self.layout.panels {
            let label = match panel.visible {
                true => trf("Hide {0}", &[&panel.name.clone()]),
                false => trf("Show {0}", &[&panel.name.clone()]),
            };
            acts.push(act(&label, panels, Act::Panel(panel.id)));
        }
        acts.push(act(
            tr("Add a text panel"),
            panels,
            Act::AddPanel(super::layout::Kind::Text),
        ));
        acts.push(act(
            tr("Add a panel of gauges"),
            panels,
            Act::AddPanel(super::layout::Kind::Gauges),
        ));
        if !self.layout.has(super::layout::Kind::Map) {
            acts.push(act(
                tr("Add the map"),
                panels,
                Act::AddPanel(super::layout::Kind::Map),
            ));
        }
        for section in crate::app::Section::ALL {
            acts.push(act(
                section.name(),
                settings,
                Act::App(AppRequest::Settings(section)),
            ));
        }
        let profiles = self
            .profiles
            .as_ref()
            .map(Profiles::names)
            .unwrap_or_default();
        for name in profiles {
            acts.push(act(
                &name,
                tr("Connect to profile"),
                Act::Connect(name.clone()),
            ));
            let edit = AppRequest::Profile(crate::app::ProfileAction::Edit, name.clone());
            acts.push(act(&name, tr("Edit profile"), Act::App(edit)));
        }
        acts.push(act(
            tr("Keyboard shortcuts"),
            help,
            Act::App(AppRequest::Shortcuts),
        ));
        acts.push(act(tr("About Muddie"), help, Act::App(AppRequest::About)));
        acts
    }

    /// Does what an act of the palette means.
    fn act(&mut self, act: Act, ctx: &egui::Context) {
        match act {
            Act::View(view) => self.set_view(view),
            Act::Section(section) => self.open_automation(section),
            Act::App(request) => self.app_request = Some(request),
            Act::Reconnect => self.reconnect(ctx),
            Act::Disconnect => self.disconnect(),
            Act::Log => {
                self.do_client_command("/log");
            }
            Act::Speech => {
                let on = !self.voice.enabled();
                self.voice.set_enabled(on);
            }
            Act::Find => {
                self.find_open = true;
                self.find_wants_focus = true;
            }
            Act::AddPanel(kind) => {
                self.set_view(0);
                self.started = true;
                self.add_panel(kind);
            }
            Act::Panel(id) => {
                if let Some(panel) = self.layout.panel_mut(id) {
                    panel.visible = !panel.visible;
                }
            }
            Act::Connect(name) => {
                if self.select_profile(Some(name)) {
                    self.connect(ctx);
                }
            }
        }
    }

    /// What this session asked the window to do, once.
    pub fn take_app_request(&mut self) -> Option<AppRequest> {
        self.app_request.take()
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
        let typed = format!("/{}", self.palette_query.trim_start_matches('/'));
        if item.more && !typed_more {
            self.palette_query = item.insert.trim_start_matches('/').to_owned();
            self.palette_chosen = 0;
            self.palette_cursor_end = true;
            return;
        }
        let line = if item.more {
            typed
        } else {
            item.insert.clone()
        };
        self.close_palette();
        self.history.add(&line);
        self.run_command(&line);
    }

    /// The palette of commands, over the window while it is open.
    pub(super) fn palette(&mut self, ctx: &egui::Context) {
        let theme = theme::current();
        let shown =
            ctx.animate_bool_with_time(egui::Id::unique("palette shown"), self.palette_open, 0.1);
        if !self.palette_open {
            return;
        }
        let items = self.palette_items();
        self.palette_chosen = self.palette_chosen.min(items.len().saturating_sub(1));
        // Its keys, before the field in it gets them.
        let (up, down, tab, escape, enter) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
                input.consume_key(Modifiers::NONE, Key::Tab),
                input.consume_key(Modifiers::NONE, Key::Escape),
                input.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if escape {
            return self.close_palette();
        }
        if (up || down) && !items.is_empty() {
            let last = items.len() - 1;
            self.palette_chosen = match (up, self.palette_chosen) {
                (true, 0) => last,
                (true, chosen) => chosen - 1,
                (false, chosen) if chosen == last => 0,
                (false, chosen) => chosen + 1,
            };
            // Said aloud, for those who do not see the list.
            let chosen = &items[self.palette_chosen];
            self.voice.stop();
            self.voice
                .say(&format!("{}. {}", shown_usage(&chosen.usage), chosen.about));
        }
        let listed = self.results.is_some() && self.palette_query.is_empty();
        if let Some(chosen) = items.get(self.palette_chosen).cloned() {
            let typed = format!("/{}", self.palette_query.trim_start_matches('/'));
            let typed_more = typed.trim_end().len() > chosen.insert.trim_end().len();
            if enter {
                return self.choose(&chosen, typed_more, ctx);
            }
            // Tab completes, and there is nothing to complete in a list.
            if tab && !listed && !typed_more {
                self.palette_query = chosen.insert.trim_start_matches('/').to_owned();
                self.palette_chosen = 0;
                self.palette_cursor_end = true;
            }
        }

        const ROWS: usize = 8;
        const ROW: f32 = 32.0;
        let screen = ctx.content_rect();
        let width = (screen.width() - 32.0).min(560.0);
        let place = egui::pos2(screen.center().x, screen.top() + 84.0 - (1.0 - shown) * 8.0);
        let card = egui::Frame::new()
            .fill(theme.raised)
            .stroke(egui::Stroke::new(1.0, theme.border))
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(egui::Margin::same(6))
            .shadow(egui::Shadow {
                offset: [0, 10],
                blur: 30,
                spread: 0,
                color: egui::Color32::from_black_alpha(if theme.dark { 110 } else { 36 }),
            });
        let mut clicked = None;
        let field = egui::Id::unique("palette field");
        let area = egui::Area::new(egui::Id::unique("palette"))
            .order(egui::Order::Foreground)
            .fixed_pos(place)
            .pivot(egui::Align2::CENTER_TOP)
            .show(ctx, |ui| {
                ui.set_opacity(shown);
                card.show(ui, |ui| {
                    ui.set_width(width - 12.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // What to look for.
                    let hint = if listed {
                        tr("Choose one, or type a command")
                    } else {
                        tr("Go to, open or run anything")
                    };
                    let hint = RichText::new(hint)
                        .font(theme::font(Weight::Regular, 14.5))
                        .color(theme.text_faint);
                    let edit = egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            TextEdit::singleline(&mut self.palette_query)
                                .id(field)
                                .frame(egui::Frame::NONE)
                                .font(theme::font(Weight::Regular, 14.5))
                                .hint_text(hint)
                                .lock_focus(true)
                                .desired_width(f32::INFINITY)
                                .show(ui)
                        })
                        .inner;
                    if edit.response.changed() {
                        self.palette_chosen = 0;
                        self.results = None;
                    }
                    if std::mem::take(&mut self.palette_cursor_end) {
                        let end = egui::text::CCursor::new(self.palette_query.chars().count());
                        let mut state = edit.state.clone();
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(end)));
                        state.store(ui.ctx(), field);
                    }
                    // The keyboard is its own while it is open.
                    if std::mem::take(&mut self.palette_focus) || !edit.response.has_focus() {
                        edit.response.request_focus();
                    }
                    if items.is_empty() {
                        return;
                    }
                    let line = ui.available_rect_before_wrap();
                    ui.painter().hline(
                        line.x_range(),
                        line.top(),
                        egui::Stroke::new(1.0, theme.border),
                    );
                    ui.add_space(6.0);
                    // A window of the list around the one chosen.
                    let first = (self.palette_chosen.saturating_sub(ROWS - 1))
                        .min(items.len().saturating_sub(ROWS));
                    for (index, item) in items.iter().enumerate().skip(first).take(ROWS) {
                        let chosen = index == self.palette_chosen;
                        let (rect, row) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), ROW),
                            egui::Sense::click(),
                        );
                        let usage = shown_usage(&item.usage);
                        row.widget_info(|| {
                            let said = match item.about.is_empty() {
                                true => usage.to_owned(),
                                false => format!("{usage}. {}", item.about),
                            };
                            egui::WidgetInfo::selected(egui::Role::ListItem, true, chosen, said)
                        });
                        if chosen || row.hovered() {
                            let fill = if chosen {
                                theme.hover
                            } else {
                                theme.hover.gamma_multiply(0.5)
                            };
                            ui.painter()
                                .rect_filled(rect, egui::CornerRadius::same(8), fill);
                        }
                        let inside = rect.shrink2(egui::vec2(10.0, 0.0));
                        // What it is, at the right, and the thing itself
                        // up to it.
                        let about = ui.painter().with_clip_rect(inside).text(
                            inside.right_center(),
                            egui::Align2::RIGHT_CENTER,
                            &item.about,
                            theme::font(Weight::Regular, 12.5),
                            theme.text_faint,
                        );
                        let room = egui::Rect::from_min_max(
                            inside.min,
                            egui::pos2(about.left() - 14.0, inside.max.y),
                        );
                        ui.painter().with_clip_rect(room).text(
                            inside.left_center(),
                            egui::Align2::LEFT_CENTER,
                            usage,
                            theme::font(Weight::Regular, 13.5),
                            theme.text,
                        );
                        if row.clicked() {
                            clicked = Some(index);
                        }
                    }
                });
            });
        if let Some(index) = clicked {
            let item = items[index].clone();
            self.palette_focus = true;
            return self.choose(&item, false, ctx);
        }
        // A click anywhere else closes it; not the one that opened it,
        // from the field at the top, in the frame it opened.
        let fresh = std::mem::take(&mut self.palette_fresh);
        if !fresh && area.response.clicked_elsewhere() {
            self.close_palette();
        }
    }

    /// What the last command said in a line, while it is to be shown,
    /// and the colour to show it in: for a moment, fading at the end.
    pub(super) fn bar_said(&mut self, ui: &egui::Ui) -> Option<(String, egui::Color32)> {
        let theme = theme::current();
        let now = ui.input(|input| input.time);
        let note = self.bar_note.as_mut()?;
        let since = *note.since.get_or_insert(now);
        // A problem stays to be read; the rest goes by itself, sooner the
        // less it says.
        let mut fading = 1.0;
        if !note.problem {
            let lasts = (2.0 + 0.05 * note.text.chars().count() as f64).min(7.0);
            let left = since + lasts - now;
            if left <= 0.0 {
                self.bar_note = None;
                return None;
            }
            if left < 0.4 {
                fading = (left / 0.4) as f32;
                ui.ctx().request_repaint();
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(left - 0.4));
            }
        }
        let colour = if note.problem {
            theme.danger
        } else {
            theme.text_dim
        };
        Some((note.text.clone(), colour.gamma_multiply(fading)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usages(typed: &str) -> Vec<String> {
        crate::i18n::set_language("en");
        let groups = [("Combat".to_owned(), true), ("Social".to_owned(), false)];
        let plugins = ["mapper".to_owned()];
        offered(typed, &groups, &plugins)
            .into_iter()
            .map(|item| item.usage)
            .collect()
    }

    #[test]
    fn commands_are_offered_as_they_are_typed() {
        assert!(usages("north").is_empty());
        assert_eq!(usages("/").len(), COMMANDS.len());
        assert!(usages("/ma").iter().all(|usage| usage.starts_with("/map")));
        assert_eq!(usages("/map w"), ["/map where", "/map walk <room>"]);
        assert_eq!(usages("/map walk Temple of"), ["/map walk <room>"]);
        assert_eq!(usages("/re"), ["/reload"]);
        assert!(usages("/dance").is_empty());
        // One that takes nothing is not offered once more follows it.
        assert!(usages("/map where now").is_empty());
        // Those that take the name of something there is offer the names.
        assert_eq!(usages("/group "), ["/group Combat", "/group Social"]);
        assert_eq!(usages("/group so"), ["/group Social"]);
        assert_eq!(usages("/plugin enable "), ["/plugin enable mapper"]);
        // What choosing one puts in the line leaves room for what it
        // cannot do without, and only then is it not run as it is.
        let walk = offered("/map wa", &[], &[]).remove(0);
        assert_eq!((walk.insert.as_str(), walk.more), ("/map walk ", true));
        let stop = offered("/map st", &[], &[]).remove(0);
        assert_eq!((stop.insert.as_str(), stop.more), ("/map stop", false));
        let avoid = offered("/map av", &[], &[]).remove(0);
        assert_eq!((avoid.insert.as_str(), avoid.more), ("/map avoid", false));
        let combat = offered("/group Com", &[("Combat".to_owned(), true)], &[]).remove(0);
        assert_eq!(
            (combat.insert.as_str(), combat.more),
            ("/group Combat", false)
        );
    }
}
