//! What arrives from the connection, and the session's log.

use super::*;

impl Tab {
    /// Takes what arrived from the connection since the last frame.
    pub(super) fn handle_events(&mut self) {
        self.sync_trace();
        let recording = self.recording();
        let Some(connection) = &self.connection else {
            return;
        };
        let events: Vec<Event> = connection.events.try_iter().collect();
        for event in events {
            self.handle_event(event, recording);
        }
    }

    /// Does what one thing that arrived asks for. With `recording`, what
    /// the MUD sends besides text is kept to be looked at.
    fn handle_event(&mut self, event: Event, recording: bool) {
        match event {
            Event::Connected => self.on_connected(),
            Event::Line(line) | Event::Prompt(line) => self.on_line(line, recording),
            Event::Partial(line) => self.on_partial(line),
            Event::InputHidden(hidden) => {
                self.input_hidden = hidden;
                if hidden {
                    self.login_password();
                }
            }
            // Not spoken: the user has just typed it.
            Event::Sent(text) => self.show_local(&format!("> {text}")),
            Event::Gmcp { package, data } => self.on_gmcp(&package, &data, recording),
            Event::Msdp { variable, data } => self.on_msdp(&variable, &data, recording),
            Event::Mssp(status) => self.on_mssp(&status, recording),
            Event::Other {
                protocol,
                name,
                data,
            } => {
                if recording {
                    self.saw(
                        &protocol,
                        &name,
                        &serde_json::Value::String(data).to_string(),
                    );
                }
            }
            Event::Mxp(asked) => self.mxp(asked, recording),
            Event::Protocol { name, on } => {
                match self.protocols.iter_mut().find(|(known, _)| *known == name) {
                    Some(known) => known.1 = on,
                    None => self.protocols.push((name, on)),
                }
            }
            Event::Disconnected(error) => self.on_disconnected(error),
        }
    }

    /// The connection is made: start afresh, log if asked, tell the
    /// scripts, and get ready to log in.
    fn on_connected(&mut self) {
        self.offer_plugin_for(&self.host.clone(), "");
        self.protocols.clear();
        // A new connection passes over what the last was told to say.
        self.data_view.trace_sent = Some(false);
        self.reconnect_attempts = 0;
        self.last_activity = Instant::now();
        if self.prefs.log_sessions && self.log.is_none() {
            self.start_log();
        }
        self.note(tr("Connected."));
        // The server has yet to hear how big the panels are, and the text
        // of the game.
        self.panels_untold();
        self.sizes.window_reported = None;
        self.scripts.on_event("connect", "");
        self.run_actions();
        self.login = if self.character.trim().is_empty() {
            Login::Idle
        } else {
            Login::Name(Instant::now())
        };
    }

    /// A whole line, or a prompt: a sound asked for is played, anything
    /// else goes through the triggers and is shown as they leave it.
    fn on_line(&mut self, line: Line, recording: bool) {
        let text = line.text();
        if let Some(request) = sound::parse_msp(&text) {
            if recording {
                self.saw(
                    "MSP",
                    "sound",
                    &serde_json::Value::String(text.clone()).to_string(),
                );
            }
            self.output.clear_partial();
            self.pending_partial = None;
            return self.msp(request);
        }
        self.pending_partial = None;
        match self.triggered(&line, &text) {
            Some(shown) => {
                self.show_line(&shown);
                self.voice.line(&text);
            }
            None => {
                self.output.clear_partial();
                self.voice.skip_line(&text);
            }
        }
        self.last_activity = Instant::now();
        self.run_actions();
        self.login_name();
    }

    /// What the triggers make of a line: how it is to be shown, or nothing
    /// if hidden. Text whose triggers already ran while it was unterminated
    /// keeps that verdict instead of firing twice.
    fn triggered(&mut self, line: &Line, text: &str) -> Option<Line> {
        let settled = (self.settled_partial.take()).filter(|(fired, _)| fired == text);
        if let Some((_, shown)) = settled {
            return shown;
        }
        let shown = self.scripts.on_styled_line(line);
        self.dispatched = Some(shown.clone().unwrap_or_else(|| line.clone()));
        shown
    }

    /// A line not yet ended: shown as it is, and waited on in case it is a
    /// prompt that will not be ended.
    fn on_partial(&mut self, line: Line) {
        let text = line.text();
        let settled = (self.settled_partial.as_ref()).filter(|(fired, _)| *fired == text);
        match settled {
            Some((_, None)) => return,
            Some((_, Some(_))) => {}
            None => {
                self.settled_partial = None;
                let waiting = (self.pending_partial.as_ref())
                    .is_some_and(|(pending, _)| pending.text() == text);
                if !waiting {
                    self.pending_partial = Some((line.clone(), Instant::now()));
                }
            }
        }
        self.output.set_partial(&line);
        self.voice.partial(&text);
    }

    /// A GMCP package: what many MUDs send the same way is made something
    /// of first, if its plugin is on, so that scripts can override it.
    fn on_gmcp(&mut self, package: &str, data: &str, recording: bool) {
        let common = self.conventions();
        let is = |name: &str| package.eq_ignore_ascii_case(name);
        if common && is("Char.Vitals") {
            for (name, value, max) in panels::vitals(data) {
                self.gauges.set(name, Some((value, max)));
            }
        } else if let Some(message) = crate::media::parse(package, data) {
            self.media(message);
        } else if common && is("Comm.Channel.Text") {
            let name = panels::channel_name(data);
            self.channel_said(&name, "", panels::channel_text(data));
        } else if common && is("Room.Info") && self.scripts.native_map() {
            self.entered_room(data);
        }
        if recording {
            self.saw_data(package, false, data);
        }
        self.scripts.on_gmcp(package, data);
        self.run_actions();
    }

    /// An MSDP variable, made something of as GMCP's are.
    fn on_msdp(&mut self, variable: &str, data: &str, recording: bool) {
        let common = self.conventions();
        if common && let Some((name, value, max)) = self.msdp_vitals.update(variable, data) {
            self.gauges.set(name, Some((value, max)));
        }
        if common && variable == "ROOM" && self.scripts.native_map() {
            self.entered_room(data);
        }
        if recording {
            self.saw_data(variable, true, data);
        }
        self.scripts.on_msdp(variable, data);
        self.run_actions();
    }

    /// What the MUD says of itself: its name may have a plugin to offer.
    fn on_mssp(&mut self, status: &[(String, String)], recording: bool) {
        let named = status
            .iter()
            .find(|(field, _)| field.eq_ignore_ascii_case("NAME"));
        if let Some((_, name)) = named {
            self.offer_plugin_for("", &name.clone());
        }
        if recording {
            let fields: serde_json::Map<String, serde_json::Value> = (status.iter())
                .map(|(name, value)| (name.clone(), value.clone().into()))
                .collect();
            self.saw(
                "MSSP",
                "MSSP",
                &serde_json::Value::Object(fields).to_string(),
            );
        }
        self.scripts.on_mssp(status);
    }

    /// The connection is closed: what lasted with it goes, and one lost to
    /// an error is opened again if asked.
    fn on_disconnected(&mut self, error: Option<String>) {
        self.scripts.on_event("disconnect", "");
        self.run_actions();
        self.forget_connection();
        // A connection lost to an error is worth retrying; one the server
        // or the user closed is not.
        let lost = error.is_some() && !self.user_disconnected;
        self.note(&match &error {
            Some(error) => trf("Disconnected: {0}", &[error]),
            None => tr("Disconnected.").to_owned(),
        });
        if lost && self.prefs.reconnect {
            self.retry_later();
        }
        // The log ends with what was said of it.
        self.log = None;
        // Closed to be opened again: as soon as can be.
        if std::mem::take(&mut self.reconnect_wanted) {
            self.reconnect_at = Some(Instant::now());
        }
    }

    /// Lets go of what lasted only as long as the connection.
    fn forget_connection(&mut self) {
        self.walk = None;
        self.pending_partial = None;
        self.settled_partial = None;
        self.gauges.clear();
        self.msdp_vitals.clear();
        self.sound.stop_all();
        self.fetching.base = None;
        self.fetching.jobs.clear();
        self.mxp_state.values.clear();
        self.mxp_state.gauges.clear();
        self.input_hidden = false;
        self.connection = None;
    }

    /// Connects again after a wait that grows with each attempt.
    fn retry_later(&mut self) {
        let wait = Duration::from_secs(2u64.pow(self.reconnect_attempts.min(5)).min(60));
        self.reconnect_attempts += 1;
        self.reconnect_at = Some(Instant::now() + wait);
        self.note(&trf(
            "Reconnecting in {0} seconds.",
            &[&wait.as_secs().to_string()],
        ));
    }

    pub(super) fn show_line(&mut self, line: &Line) {
        // What the server sent to a frame goes to the panel of its name.
        if let Some(frame) = &line.dest {
            return self.add_to_panel(&frame.clone(), line);
        }
        self.output.push(line);
        let time = self.output.time(self.output.len() - 1).to_owned();
        self.logged(|log| log.line(&time, line));
    }

    /// Writes to the log, if one is open; a failure closes it with a note.
    pub(super) fn logged(&mut self, write: impl FnOnce(&mut SessionLog) -> std::io::Result<()>) {
        let Some(log) = &mut self.log else {
            return;
        };
        if let Err(error) = write(log) {
            self.log = None;
            self.note(&trf("Logging stopped: {0}", &[&error.to_string()]));
        }
    }

    /// Starts writing the session to a new log file.
    pub(super) fn start_log(&mut self) {
        let Some(profiles) = &self.profiles else {
            return self.note(tr("Cannot log: no configuration folder was found."));
        };
        match SessionLog::start(&profiles.logs_dir(), &self.title(), self.prefs.log_format) {
            Ok(log) => {
                let path = log.path().display().to_string();
                self.log = Some(log);
                self.note(&trf("Logging to {0}", &[&path]));
            }
            Err(error) => self.note(&trf("Cannot log: {0}", &[&error.to_string()])),
        }
    }
}
