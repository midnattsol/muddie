//! What the palette can do besides commands: go to a place, open the
//! settings, switch something, connect to a profile.

use super::*;

impl Tab {
    /// What the palette can do besides commands, each with where it is:
    /// places to go, the session's own, the panels, the settings, the
    /// profiles, and help.
    pub(in crate::tab) fn palette_acts(&self) -> Vec<Item> {
        let mut acts = views();
        acts.extend(self.session_acts());
        acts.extend(self.panel_acts());
        acts.extend(self.channel_acts());
        let map = tr("Map");
        acts.extend(self.scripts.map_actions().into_iter().map(|tool| {
            let label = match tool.checked {
                None => tool.label.clone(),
                Some(on) => switched(&tool.label, on),
            };
            item(&label, map, Act::MapAction(tool.label))
        }));
        if self.map.borrow().last_change().is_some() {
            acts.push(item(
                tr("Undo the map's last change"),
                map,
                Act::Run("/map undo".into()),
            ));
        }
        acts.extend(self.map_acts());
        let settings = tr("Settings");
        acts.extend(crate::app::Section::ALL.map(|section| {
            item(
                section.name(),
                settings,
                Act::App(AppRequest::Settings(section)),
            )
        }));
        acts.extend(self.profile_acts());
        let help = tr("Help");
        acts.push(item(
            tr("Keyboard shortcuts"),
            help,
            Act::App(AppRequest::Shortcuts),
        ));
        acts.push(item(tr("About Muddie"), help, Act::App(AppRequest::About)));
        acts
    }

    /// What is done with the session: connecting, closing, finding, logging,
    /// speech and its profile.
    fn session_acts(&self) -> Vec<Item> {
        let session = tr("Session");
        let connect = match self.connection.is_some() {
            true => tr("Reconnect"),
            false => tr("Connect"),
        };
        let mut acts = vec![item(connect, session, Act::Reconnect)];
        if self.connection.is_some() {
            acts.push(item(tr("Disconnect"), session, Act::Disconnect));
        }
        acts.push(item(
            tr("New session"),
            session,
            Act::App(AppRequest::NewSession),
        ));
        acts.push(item(
            tr("Close session"),
            session,
            Act::App(AppRequest::CloseSession),
        ));
        acts.push(item(
            tr("Profiles"),
            session,
            Act::App(AppRequest::Profiles),
        ));
        acts.push(item(tr("Find"), session, Act::Find));
        acts.push(item(tr("Copy all output"), session, Act::CopyOutput));
        let log = switched(tr("Log this session"), self.log.is_some());
        acts.push(item(&log, session, Act::Log));
        let speech = switched(tr("Speech in this session"), self.voice.enabled());
        acts.push(item(&speech, session, Act::Speech));
        acts.push(match &self.profile {
            Some(name) => {
                let edit = AppRequest::Profile(crate::app::ProfileAction::Edit, name.clone());
                item(tr("Edit profile"), session, Act::App(edit))
            }
            None => item(
                tr("Save as profile"),
                session,
                Act::App(AppRequest::SaveAsProfile),
            ),
        });
        acts
    }

    /// Reading each of the MUD's channels aloud, or keeping it quiet.
    fn channel_acts(&self) -> Vec<Item> {
        let channels = tr("Channels");
        (self.panels.channels.iter())
            .map(|channel| {
                let name = &channel.name;
                let aloud = !self.channel_quiet(name);
                let label = switched(&trf("Read the {0} channel aloud", &[name]), aloud);
                item(&label, channels, Act::ReadChannel(name.clone(), !aloud))
            })
            .collect()
    }

    /// Showing or hiding each panel. How they are laid out, and adding
    /// one, is done in Layout.
    fn panel_acts(&self) -> Vec<Item> {
        let panels = tr("Panels");
        (self.layout.panels.iter())
            .map(|panel| {
                let label = switched(&trf("Show {0}", &[&panel.shown_name()]), panel.visible);
                item(&label, panels, Act::Panel(panel.id))
            })
            .collect()
    }

    /// What is done with the map, and with the scripts, plugins and tags:
    /// each by what it does, asking in the field for what it needs more.
    fn map_acts(&self) -> Vec<Item> {
        let run = |line: &str| Act::Run(line.to_owned());
        let ask = |title: String, command: &str, offers: Offers| {
            Act::Ask(Asking {
                title,
                command: command.to_owned(),
                offers,
            })
        };
        let map = tr("Map");
        let mut acts = vec![
            item(tr("Where am I"), map, run("/map where")),
            item(tr("Exits of this room"), map, run("/map exits")),
            item(tr("What is near"), map, run("/map near")),
            item(tr("Explore the map"), map, run("/map explore")),
            item(
                tr("Go to…"),
                map,
                ask(tr("Go to").into(), "/map go ", Offers::Places),
            ),
            item(
                tr("The way to…"),
                map,
                ask(tr("The way to").into(), "/map way ", Offers::Places),
            ),
            item(
                tr("Find rooms…"),
                map,
                ask(tr("Find rooms with").into(), "/map find ", Offers::Nothing),
            ),
            item(
                tr("What the map knows of…"),
                map,
                ask(
                    tr("What the map knows of").into(),
                    "/map info ",
                    Offers::Places,
                ),
            ),
            item(tr("Areas of the map"), map, run("/map areas")),
            item(tr("Check the map"), map, run("/map check")),
            item(tr("Tidy this area"), map, run("/map tidy")),
            item(
                tr("Remove a room…"),
                map,
                ask(tr("Remove the room").into(), "/map remove ", Offers::Places),
            ),
            item(
                tr("Import a Mudlet map…"),
                map,
                ask(
                    tr("Import the Mudlet map at").into(),
                    "/map import ",
                    Offers::Nothing,
                ),
            ),
        ];
        acts.push(item(
            tr("I am in…"),
            map,
            ask(
                tr("The room you are in").into(),
                "/map here ",
                Offers::Places,
            ),
        ));
        let state = self.map.borrow();
        if self.map_view.area_count(&state) > 1 {
            acts.push(item(
                tr("Show an area…"),
                map,
                Act::Ask(Asking::area_to_show()),
            ));
        }
        if !state.candidates().is_empty() {
            acts.push(item(tr("Which room am I in"), map, run("/map here")));
        }
        acts.push(item(tr("Map appearance…"), map, Act::Look(None)));
        let all = switched(tr("Show all areas"), self.map_view.all_areas);
        acts.push(item(&all, map, Act::AllAreas));
        if let Some(room) = state.current() {
            let name = state.short_name(room).to_owned();
            acts.push(item(
                &trf("Mark {0}…", &[&name]),
                map,
                ask(trf("Mark for {0}", &[&name]), "/map mark ", Offers::Nothing),
            ));
            acts.push(item(
                &trf("Note on {0}…", &[&name]),
                map,
                ask(trf("Note on {0}", &[&name]), "/map note ", Offers::Nothing),
            ));
            if !room.note.is_empty() {
                acts.push(item(
                    &trf("Take the note off {0}", &[&name]),
                    map,
                    run("/map unnote"),
                ));
            }
            acts.push(item(
                &trf("Appearance of {0}…", &[&name]),
                map,
                Act::Look(Some(room.id)),
            ));
            let avoid = switched(&trf("Keep routes off {0}", &[&name]), room.avoid);
            acts.push(item(&avoid, map, run("/map avoid")));
            if !room.exits.is_empty() {
                acts.push(item(
                    tr("Keep routes off an exit…"),
                    map,
                    ask(
                        tr("Keep routes off the exit").into(),
                        "/map block ",
                        Offers::Exits,
                    ),
                ));
            }
        }
        if !state.marks().is_empty() {
            acts.push(item(
                tr("Forget a mark…"),
                map,
                ask(tr("Forget the mark").into(), "/map unmark ", Offers::Marks),
            ));
        }
        if !state.added().is_empty() {
            acts.push(item(tr("Rooms the map added"), map, run("/map added")));
        }
        drop(state);
        if self.walk.is_some() {
            acts.push(item(tr("Stop going"), map, run("/map stop")));
        }
        // The scripts, plugins and tags.
        let scripts = tr("Scripts");
        acts.push(item(
            tr("Run Luau…"),
            scripts,
            ask(tr("Luau to run").into(), "/luau ", Offers::Nothing),
        ));
        acts.push(item(tr("Restart scripts"), scripts, run("/reload")));
        let switches = self.scripts.switches();
        for tag in self.scripts.tags() {
            let on = switches.tag_on(&tag);
            let line = format!("/tag {tag} {}", if on { "off" } else { "on" });
            acts.push(item(
                &switched(&trf("Tag {0}", &[&tag]), on),
                scripts,
                Act::Run(line),
            ));
        }
        let plugins = tr("Plugins");
        for plugin in &self.plugins {
            let on = self.plugin_on(plugin);
            let what = if on { "disable" } else { "enable" };
            let line = format!("/plugin {what} {}", plugin.id);
            acts.push(item(
                &switched(&trf("Plugin {0}", &[&plugin.shown_name()]), on),
                plugins,
                Act::Run(line),
            ));
        }
        // Asked in its window, as from the profile's plugins.
        acts.push(item(
            tr("Install a plugin…"),
            plugins,
            run("/plugin install"),
        ));
        if self.plugins.iter().any(|plugin| plugin.downloaded) {
            acts.push(item(
                tr("Update a plugin…"),
                plugins,
                ask(
                    tr("Plugin to update").into(),
                    "/plugin update ",
                    Offers::Plugins,
                ),
            ));
            acts.push(item(
                tr("Remove a plugin…"),
                plugins,
                ask(
                    tr("Plugin to remove").into(),
                    "/plugin remove ",
                    Offers::Plugins,
                ),
            ));
        }
        acts
    }

    /// Connecting to each saved profile; they are edited from Profiles,
    /// with the session's.
    fn profile_acts(&self) -> Vec<Item> {
        let profiles = self
            .profiles
            .as_ref()
            .map(Profiles::names)
            .unwrap_or_default();
        (profiles.into_iter())
            .map(|name| {
                item(
                    &name,
                    tr("Connect to profile"),
                    Act::App(AppRequest::Play(name.clone())),
                )
            })
            .collect()
    }

    /// Does what an act of the palette means.
    pub(in crate::tab) fn act(&mut self, act: Act, ctx: &egui::Context) {
        match act {
            Act::View(view) => self.set_view(view),
            Act::Section(section) => self.open_automation(section),
            Act::App(request) => self.app_request = Some(request),
            Act::Reconnect => self.reconnect(ctx),
            Act::Disconnect => self.disconnect(),
            Act::Log => {
                self.toggle_log();
            }
            Act::ReadChannel(name, read) => {
                self.set_channel_quiet(&name, !read);
            }
            Act::Speech => match self.voice.enabled() {
                true => self.voice.set_muted(true),
                false => {
                    self.voice.set_muted(false);
                    // Asked to speak with nothing to speak through: the
                    // system's voice does.
                    if !self.voice.enabled() {
                        self.voice.force_voice();
                    }
                }
            },
            Act::MapAction(label) => self.use_map_action(&label),
            Act::CopyOutput => {
                self.output.copy_all(ctx);
                self.say(tr("Copied."), false);
            }
            Act::Find => self.open_find(),
            Act::Panel(id) => {
                let shown = self.layout.panel(id).is_some_and(|panel| panel.visible);
                self.layout.set_shown(id, !shown);
            }
            Act::Run(line) => drop(self.run_command(&line)),
            Act::Ask(asking) => self.ask(asking),
            Act::Answer(answer) => self.answer(&answer),
            Act::Look(Some(room)) => self.open_look(room),
            Act::Look(None) => self.open_map_appearance(),
            Act::AllAreas => self.map_view.all_areas = !self.map_view.all_areas,
            Act::Place(name) => self.go_to(&name),
        }
    }

    /// What this session asked the window to do, once.
    pub fn take_app_request(&mut self) -> Option<AppRequest> {
        self.app_request.take()
    }
}

/// Something the palette does, called `label`, said to be of `place`.
fn item(label: &str, place: &str, act: Act) -> Item {
    Item {
        insert: String::new(),
        usage: label.to_owned(),
        group: place.to_owned(),
        about: String::new(),
        act: Some(act),
    }
}

/// The views, and each section of the editor.
fn views() -> Vec<Item> {
    let go = tr("Views");
    let mut acts = vec![
        item(tr("Game"), go, Act::View(View::Game)),
        item(tr("Editor"), go, Act::View(View::Editor)),
        item(tr("Layout"), go, Act::View(View::Layout)),
        item(tr("Incoming data"), go, Act::View(View::Data)),
    ];
    acts.extend(
        (automation::Section::ALL.into_iter())
            .filter(|section| *section != automation::DATA)
            .map(|section| {
                item(
                    automation::section_name(section),
                    tr("Editor"),
                    Act::Section(section),
                )
            }),
    );
    acts
}

/// A switch, as the palette shows one: what it switches, and whether it
/// is on.
fn switched(label: &str, on: bool) -> String {
    match on {
        true => trf("{0} (on)", &[label]),
        false => trf("{0} (off)", &[label]),
    }
}
