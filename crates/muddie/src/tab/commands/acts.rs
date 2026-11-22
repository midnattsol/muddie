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
        acts.extend(
            (self.scripts.map_tools().into_iter())
                .map(|label| item(&label, map, Act::MapTool(label.clone()))),
        );
        acts.push(item(tr("Undo the last room added"), map, Act::UndoAdded));
        // In the editor, what is changed in it is undone and redone.
        if self.editor.open {
            acts.push(item(tr("Undo"), tr("Editor"), Act::Undo(false)));
            acts.push(item(tr("Redo"), tr("Editor"), Act::Undo(true)));
        }
        let settings = tr("Settings");
        acts.extend(crate::app::Section::ALL.map(|section| {
            item(
                section.name(),
                settings,
                Act::App(AppRequest::Settings(section)),
            )
        }));
        acts.push(item(
            tr("Profiles"),
            settings,
            Act::App(AppRequest::Profiles),
        ));
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
        let mut acts = vec![item(tr("Reconnect"), session, Act::Reconnect)];
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
        acts.push(item(tr("Find"), session, Act::Find));
        acts.push(item(tr("Copy all output"), session, Act::CopyOutput));
        let log = if self.log.is_some() {
            tr("Stop logging this session")
        } else {
            tr("Log this session")
        };
        acts.push(item(log, session, Act::Log));
        let speech = if self.voice.enabled() {
            tr("Switch speech off")
        } else {
            tr("Switch speech on")
        };
        acts.push(item(speech, session, Act::Speech));
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
                match self.channel_quiet(name) {
                    true => item(
                        &trf("Read the {0} channel aloud", &[name]),
                        speech,
                        Act::ReadChannel(name.clone(), true),
                    ),
                    false => item(
                        &trf("Mute the {0} channel", &[name]),
                        speech,
                        Act::ReadChannel(name.clone(), false),
                    ),
                }
            })
            .collect()
    }

    /// Showing or hiding each panel, and adding one.
    fn panel_acts(&self) -> Vec<Item> {
        use super::layout::Kind;
        let panels = tr("Panels");
        let mut acts: Vec<Item> = (self.layout.panels.iter())
            .map(|panel| {
                let label = match panel.visible {
                    true => trf("Hide {0}", &[&panel.shown_name()]),
                    false => trf("Show {0}", &[&panel.shown_name()]),
                };
                item(&label, panels, Act::Panel(panel.id))
            })
            .collect();
        acts.push(item(
            tr("Add a text panel"),
            panels,
            Act::AddPanel(Kind::Text),
        ));
        acts.push(item(
            tr("Add a panel of gauges"),
            panels,
            Act::AddPanel(Kind::Gauges),
        ));
        if !self.layout.has(Kind::Map) {
            acts.push(item(tr("Add the map"), panels, Act::AddPanel(Kind::Map)));
        }
        let height = match self.layout.columns_full {
            true => tr("Side panels between the top and bottom ones"),
            false => tr("Side panels at full height"),
        };
        acts.push(item(height, panels, Act::FullHeight));
        for (side, out) in self.layout.sides() {
            acts.push(item(side.toggle_label(out), panels, Act::Side(side)));
        }
        acts
    }

    /// Connecting to each saved profile, and editing it.
    fn profile_acts(&self) -> Vec<Item> {
        let profiles = self
            .profiles
            .as_ref()
            .map(Profiles::names)
            .unwrap_or_default();
        (profiles.into_iter())
            .flat_map(|name| {
                let edit = AppRequest::Profile(crate::app::ProfileAction::Edit, name.clone());
                [
                    item(&name, tr("Connect to profile"), Act::Connect(name.clone())),
                    item(&name, tr("Edit profile"), Act::App(edit)),
                ]
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
                self.do_client_command("/log");
            }
            Act::ReadChannel(name, read) => {
                self.set_channel_quiet(&name, !read);
                self.note(&match read {
                    true => trf("The {0} channel is read aloud.", &[&name]),
                    false => trf("Channel {0} muted.", &[&name]),
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
            Act::Undo(again) => self.undo_step(again),
            Act::MapTool(label) => self.use_map_tool(&label),
            Act::UndoAdded => self.undo_added(),
            Act::CopyOutput => {
                ctx.copy_text(self.output.all_text());
                self.say(tr("Output copied."), false);
            }
            Act::Find => {
                self.find.open = true;
                self.find.wants_focus = true;
            }
            Act::AddPanel(kind) => {
                self.set_view(0);
                self.started = true;
                self.add_panel(kind);
            }
            Act::FullHeight => self.layout.columns_full = !self.layout.columns_full,
            Act::Side(side) => self.toggle_side(side),
            Act::Panel(id) => {
                if let Some(panel) = self.layout.panel_mut(id) {
                    panel.visible = !panel.visible;
                    let (side, shown) = (panel.side, panel.visible);
                    if shown {
                        self.layout.reveal(side);
                    }
                }
            }
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
