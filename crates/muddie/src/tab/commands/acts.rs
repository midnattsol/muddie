//! What the palette can do besides commands: go to a place, open the
//! settings, switch something, connect to a profile.

use super::*;

impl Tab {
    /// What the palette can do besides commands, each with where it is:
    /// places to go, the session's own, the panels, the settings, the
    /// profiles, and help.
    pub(in crate::tab) fn palette_acts(&self) -> Vec<Item> {
        let mut acts = places();
        acts.extend(self.session_acts());
        acts.extend(self.panel_acts());
        acts.extend(self.channel_acts());
        let map = tr("Map");
        acts.extend(self.scripts.map_tools().into_iter().map(|tool| {
            let label = match tool.checked {
                None => tool.label.clone(),
                Some(on) => switched(&tool.label, on),
            };
            item(&label, map, Act::MapTool(tool.label))
        }));
        if self.map.borrow().last_change().is_some() {
            acts.push(item(tr("Undo the map's last change"), map, Act::UndoChange));
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
        let speech = switched(tr("Speech"), self.voice.enabled());
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
        let speech = tr("Speech");
        (self.panels.channels.iter())
            .map(|channel| {
                let name = &channel.name;
                let aloud = !self.channel_quiet(name);
                let label = switched(&trf("Read the {0} channel aloud", &[name]), aloud);
                item(&label, speech, Act::ReadChannel(name.clone(), !aloud))
            })
            .collect()
    }

    /// Showing or hiding each panel. How they are laid out, and adding
    /// one, is done in Layout.
    fn panel_acts(&self) -> Vec<Item> {
        let panels = tr("Panels");
        (self.layout.panels.iter())
            .map(|panel| {
                let label = match panel.visible {
                    true => trf("Hide {0}", &[&panel.shown_name()]),
                    false => trf("Show {0}", &[&panel.shown_name()]),
                };
                item(&label, panels, Act::Panel(panel.id))
            })
            .collect()
    }

    /// What is asked of the map, and of the scripts and plugins, that
    /// takes nothing: Muddie's commands, by what they do.
    fn map_acts(&self) -> Vec<Item> {
        let map = tr("Map");
        let mut acts = vec![
            item(tr("Where am I"), map, Act::Run("/map where")),
            item(tr("Exits of this room"), map, Act::Run("/map exits")),
            item(tr("What is near"), map, Act::Run("/map near")),
            item(tr("Explore the map"), map, Act::Run("/map explore")),
            item(tr("Areas of the map"), map, Act::Run("/map areas")),
            item(tr("Check the map"), map, Act::Run("/map check")),
        ];
        if self.walk.is_some() {
            acts.push(item(tr("Stop going"), map, Act::Run("/map stop")));
        }
        if !self.map.borrow().added().is_empty() {
            acts.push(item(tr("Rooms the map added"), map, Act::Run("/map added")));
        }
        let scripts = tr("Scripts");
        acts.push(item(tr("Restart scripts"), scripts, Act::Run("/reload")));
        acts.push(item(
            tr("Installed plugins"),
            scripts,
            Act::Run("/plugin list"),
        ));
        acts.push(item(tr("All commands"), tr("Help"), Act::Run("/help")));
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
            .map(|name| item(&name, tr("Connect to profile"), Act::Connect(name.clone())))
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
                self.do_client_command("/log");
            }
            Act::ReadChannel(name, read) => {
                self.set_channel_quiet(&name, !read);
                self.note(&match read {
                    true => trf("The {0} channel is read aloud.", &[&name]),
                    false => trf("The {0} channel is no longer read aloud.", &[&name]),
                });
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
            Act::MapTool(label) => self.use_map_tool(&label),
            Act::UndoChange => self.undo_change(),
            Act::CopyOutput => {
                ctx.copy_text(self.output.all_text());
                self.say(tr("Output copied."), false);
            }
            Act::Find => {
                self.find.open = true;
                self.find.wants_focus = true;
            }
            Act::Panel(id) => {
                if let Some(panel) = self.layout.panel_mut(id) {
                    panel.visible = !panel.visible;
                    let (side, shown) = (panel.side, panel.visible);
                    if shown {
                        self.layout.reveal(side);
                    }
                }
            }
            Act::Run(command) => drop(self.run_command(command)),
            Act::Place(name) => self.go_to(&name),
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
}

/// Something the palette does, called `label`, said to be of `place`.
fn item(label: &str, place: &str, act: Act) -> Item {
    Item {
        insert: String::new(),
        usage: label.to_owned(),
        group: place.to_owned(),
        about: String::new(),
        more: false,
        act: Some(act),
    }
}

/// The places to go: the views, and each section of the editor.
fn places() -> Vec<Item> {
    let go = tr("Go to");
    let mut acts = vec![
        item(tr("Game"), go, Act::View(0)),
        item(tr("Editor"), go, Act::View(1)),
        item(tr("Layout"), go, Act::View(3)),
        item(tr("Incoming data"), go, Act::View(2)),
    ];
    acts.extend((automation::TRIGGERS..=automation::SCRIPTS).map(|section| {
        item(
            automation::section_name(section),
            tr("Editor"),
            Act::Section(section),
        )
    }));
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
