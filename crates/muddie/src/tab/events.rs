//! What arrives from the connection, and the session's log.

use super::*;
use muddie_script::{Aloud, Heard};

/// How long a frame spends on what arrived, at most: enough for hundreds
/// of lines, little enough for the window to keep moving.
const EVENTS_PER_FRAME: Duration = Duration::from_millis(8);

impl Tab {
    /// Takes what arrived from the connection since the last frame.
    /// Does what arrived from the connection, for as long as a frame can
    /// spare: a burst is taken over several frames rather than holding one
    /// up. Returns whether some was left for the next.
    pub(super) fn handle_events(&mut self) -> bool {
        self.sync_trace();
        let recording = self.recording();
        let started = Instant::now();
        loop {
            if started.elapsed() >= EVENTS_PER_FRAME {
                return true;
            }
            let next =
                (self.connection.as_ref()).and_then(|connection| connection.events.try_recv().ok());
            match next {
                Some(event) => self.handle_event(event, recording),
                None => return false,
            }
        }
    }

    /// Does what one thing that arrived asks for. With `recording`, what
    /// the MUD sends besides text is kept to be looked at.
    fn handle_event(&mut self, event: Event, recording: bool) {
        self.record(&event);
        // Asked for a password, what is typed is hidden until it is sent,
        // whatever comes meanwhile.
        if let Event::Line(line) | Event::Prompt(line) | Event::Partial(line) = &event
            && asks_for_password(&line.text())
        {
            self.asks_password = true;
        }
        match event {
            Event::Connected => self.on_connected(),
            Event::Line(line) => self.on_line(line, false, recording),
            Event::Prompt(line) => self.on_line(line, true, recording),
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
            // Recorded above, and nothing more.
            Event::Wire { .. } => {}
            Event::Protocol { name, on } => {
                match self.protocols.iter_mut().find(|(known, _)| *known == name) {
                    Some(known) => known.1 = on,
                    None => self.protocols.push((name, on)),
                }
            }
            Event::Disconnected(error) => self.on_disconnected(error),
        }
    }

    /// Shows what arrived, with `--debug`.
    fn record(&self, event: &Event) {
        use crate::debug::{on, protocol, show as debug};
        if !on() {
            return;
        }
        let session = self.title();
        match event {
            Event::Gmcp { package, data } => protocol(
                &session,
                "GMCP",
                true,
                format!("{package} {data}").trim_end(),
            ),
            Event::Msdp { variable, data } => {
                protocol(&session, "MSDP", true, &format!("{variable} {data}"))
            }
            Event::Mssp(status) => {
                let status: Vec<String> = status
                    .iter()
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect();
                protocol(&session, "MSSP", true, &status.join(" "))
            }
            Event::Other {
                protocol: name,
                data,
                ..
            } => protocol(&session, name, true, data),
            Event::Wire {
                outgoing,
                protocol: name,
                text,
            } => protocol(&session, name, !outgoing, text),
            Event::Protocol { name, on } => protocol(
                &session,
                "TELNET",
                true,
                &format!("{name} {}", if *on { "in use" } else { "not in use" }),
            ),
            Event::Line(line) => debug(&session, "TEXT", &line.text()),
            Event::Prompt(line) => debug(&session, "PROMPT", &line.text()),
            Event::Partial(line) => debug(&session, "PARTIAL", &line.text()),
            Event::Sent(text) => debug(&session, "SENT", text),
            Event::Connected => debug(&session, "CONNECTED", &self.host),
            Event::Disconnected(error) => {
                debug(&session, "DISCONNECTED", error.as_deref().unwrap_or(""))
            }
            _ => {}
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
        self.tell_session(true);
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
    pub(super) fn on_line(&mut self, line: Line, prompt: bool, recording: bool) {
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
        // Text whose triggers already ran while it was unterminated keeps
        // that verdict instead of firing twice.
        let settled = (self.settled_partial.take()).filter(|(fired, _)| *fired == text);
        // Read then, if it was to be.
        let heard = settled.is_some();
        let shown = match settled {
            Some((_, shown)) => shown,
            None => self
                .run_triggers(line, prompt)
                .then(|| self.dispatched.take())
                .flatten(),
        };
        match shown {
            Some(shown) => {
                self.show_line(&shown);
                // Read unless a trigger asked not to.
                match self.scripts.line_heard() {
                    Heard::AsIs if prompt => self.voice.prompt(&text),
                    Heard::AsIs => self.voice.line(&text),
                    Heard::Instead(said) if !heard => {
                        self.voice.skip_line(&text);
                        self.voice.instead(&said, prompt);
                    }
                    Heard::Not | Heard::Instead(_) => self.voice.skip_line(&text),
                }
                self.dispatched = Some(shown);
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

    /// Runs the triggers on a line. What they leave of it, or the line as
    /// it came if they hid it, is kept for what they asked to be done with
    /// it. Returns whether it is to be shown.
    pub(super) fn run_triggers(&mut self, line: Line, prompt: bool) -> bool {
        let shown = self.scripts.on_styled_line(&line, prompt);
        let visible = shown.is_some();
        self.dispatched = Some(shown.unwrap_or(line));
        visible
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

    /// A GMCP package: media are played; the rest is the scripts', and
    /// the plugins'.
    fn on_gmcp(&mut self, package: &str, data: &str, recording: bool) {
        if let Some(message) = crate::media::parse(package, data) {
            self.media(message);
        }
        if recording {
            self.saw_data(package, false, data);
        }
        self.scripts.on_gmcp(package, data);
        self.run_actions();
    }

    /// Sets a gauge, and says it when it falls under one of its warnings.
    pub(super) fn set_gauge(
        &mut self,
        name: &str,
        level: Option<(f64, f64)>,
        look: Option<muddie_script::GaugeLook>,
        owner: Option<&str>,
        belongs: muddie_script::Belongs,
        panel: String,
    ) {
        // A panel named that there is not is made, as a channel's is.
        if !panel.trim().is_empty() {
            self.gauges_panel_named(&panel);
        }
        // What the client makes, scripts are told of; what they made, they
        // know.
        if owner.is_none()
            && let (Some(level), Some(look)) = (level, &look)
        {
            self.scripts.know_gauge(name, level, look.clone());
        }
        // Switched off, it is not shown, nor heard.
        let on = (self.scripts.switches()).active(muddie_script::Kind::Gauge, &belongs);
        let said = (self.gauges).set_by(name, level, look, owner, belongs, panel);
        if let Some(said) = said.filter(|_| on) {
            self.voice.say_of_game(&said);
        }
    }

    /// An MSDP variable: the scripts', and the plugins'.
    fn on_msdp(&mut self, variable: &str, data: &str, recording: bool) {
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
        self.announce_language(status);
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
        self.tell_session(false);
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
    pub(super) fn forget_connection(&mut self) {
        self.walk = None;
        self.pending_partial = None;
        self.settled_partial = None;
        self.gauges.clear();
        self.scripts.forget_gauges();
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
            return self.add_to_panel(&frame.clone(), line, Aloud::default());
        }
        self.output.push(line);
        self.scripts.know_line(&line.text());
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
            self.refuse(&trf("Logging stopped: {0}", &[&error.to_string()]));
        }
    }

    /// Writes out what the log and the map have waiting, once a frame:
    /// line by line or room by room, a burst would wait on the disk.
    pub(super) fn write_down(&mut self) {
        self.logged(SessionLog::flush);
        let written = self.map.borrow_mut().flush();
        if let Err(error) = written {
            self.map.borrow_mut().stop_storing();
            self.note(&trf(
                "The map will not be saved: {0}",
                &[&error.to_string()],
            ));
        }
    }

    /// Starts writing the session to a new log file.
    pub(super) fn start_log(&mut self) {
        let Some(profiles) = &self.profiles else {
            return self.refuse(tr(
                "Could not start the log: no configuration folder was found.",
            ));
        };
        match SessionLog::start(&profiles.logs_dir(), &self.title(), self.prefs.log_format) {
            Ok(log) => {
                let path = log.path().display().to_string();
                self.log = Some(log);
                self.note(&trf("Logging to {0}", &[&path]));
            }
            Err(error) => self.refuse(&trf("Could not start the log: {0}", &[&error.to_string()])),
        }
    }
}

/// Whether something the server said asks for a password: it ends in a
/// colon, with the word for one in it.
pub(super) fn asks_for_password(text: &str) -> bool {
    let text = text.trim().to_lowercase();
    let words = [
        "password",
        "passwd",
        "contraseña",
        "clave",
        "senha",
        "mot de passe",
        "passwort",
    ];
    text.ends_with(':') && words.iter().any(|word| text.contains(word))
}
