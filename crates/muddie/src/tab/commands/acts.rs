//! What the palette can do besides commands: go to a place, open the
//! settings, switch something, connect to a profile.

use super::*;

impl Tab {
    /// What the palette can do besides commands, each with where it is.
    pub(in crate::tab) fn palette_acts(&self) -> Vec<Item> {
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
            Act::Speech => {
                let on = !self.voice.enabled();
                self.voice.set_enabled(on);
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
}
