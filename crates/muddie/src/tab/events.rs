//! What arrives from the connection, and the session's log.

use super::*;

impl Tab {
    pub(super) fn handle_events(&mut self) {
        self.sync_trace();
        let recording = self.recording();
        let Some(connection) = &self.connection else {
            return;
        };
        let events: Vec<Event> = connection.events.try_iter().collect();
        for event in events {
            match event {
                Event::Connected => {
                    self.offer_plugin_for(&self.host.clone(), "");
                    self.protocols.clear();
                    // A new connection passes over what the last was
                    // told to say.
                    self.trace_sent = Some(false);
                    self.reconnect_attempts = 0;
                    self.last_activity = Instant::now();
                    if self.prefs.log_sessions && self.log.is_none() {
                        self.start_log();
                    }
                    self.note(tr("Connected."));
                    // The server has yet to hear how big the panels are,
                    // and the text of the game.
                    self.panels_untold();
                    self.window_reported = None;
                    self.scripts.on_event("connect", "");
                    self.run_actions();
                    self.login = if self.character.trim().is_empty() {
                        Login::Idle
                    } else {
                        Login::Name(Instant::now())
                    };
                }
                Event::Line(line) | Event::Prompt(line) => {
                    let text = line.text();
                    // A sound request is carried out, not shown.
                    if let Some(request) = sound::parse_msp(&text) {
                        if recording {
                            let said = serde_json::Value::String(text.clone()).to_string();
                            self.saw("MSP", "sound", &said);
                        }
                        self.output.clear_partial();
                        self.pending_partial = None;
                        self.msp(request);
                        continue;
                    }
                    self.pending_partial = None;
                    // Text whose triggers already ran while it was
                    // unterminated keeps that verdict instead of firing twice.
                    let settled = self
                        .settled_partial
                        .take()
                        .filter(|(fired, _)| *fired == text);
                    let shown = match settled {
                        Some((_, shown)) => shown,
                        None => {
                            let shown = self.scripts.on_styled_line(&line);
                            self.dispatched = Some(shown.clone().unwrap_or_else(|| line.clone()));
                            shown
                        }
                    };
                    match shown {
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
                Event::Partial(line) => {
                    let text = line.text();
                    let settled =
                        (self.settled_partial.as_ref()).filter(|(fired, _)| *fired == text);
                    match settled {
                        Some((_, None)) => continue,
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
                Event::InputHidden(hidden) => {
                    self.input_hidden = hidden;
                    if hidden {
                        self.login_password();
                    }
                }
                // Not spoken: the user has just typed it.
                Event::Sent(text) => self.show_local(&format!("> {text}")),
                Event::Gmcp { package, data } => {
                    // What many MUDs send the same way first, if its plugin
                    // is on, so that scripts can override it.
                    let common = self.conventions();
                    if common && package.eq_ignore_ascii_case("Char.Vitals") {
                        for (name, value, max) in panels::vitals(&data) {
                            self.gauges.set(name, Some((value, max)));
                        }
                    } else if let Some(message) = crate::media::parse(&package, &data) {
                        self.media(message);
                    } else if common && package.eq_ignore_ascii_case("Comm.Channel.Text") {
                        let name = panels::channel_name(&data);
                        self.channel_said(&name, "", panels::channel_text(&data));
                    } else if common
                        && package.eq_ignore_ascii_case("Room.Info")
                        && self.scripts.native_map()
                    {
                        self.entered_room(&data);
                    }
                    if recording {
                        self.saw_data(&package, false, &data);
                    }
                    self.scripts.on_gmcp(&package, &data);
                    self.run_actions();
                }
                Event::Msdp { variable, data } => {
                    let common = self.conventions();
                    if common
                        && let Some((name, value, max)) = self.msdp_vitals.update(&variable, &data)
                    {
                        self.gauges.set(name, Some((value, max)));
                    }
                    if common && variable == "ROOM" && self.scripts.native_map() {
                        self.entered_room(&data);
                    }
                    if recording {
                        self.saw_data(&variable, true, &data);
                    }
                    self.scripts.on_msdp(&variable, &data);
                    self.run_actions();
                }
                Event::Mssp(status) => {
                    let named = status
                        .iter()
                        .find(|(field, _)| field.eq_ignore_ascii_case("NAME"));
                    if let Some((_, name)) = named {
                        self.offer_plugin_for("", &name.clone());
                    }
                    // Shown with the rest of what the MUD sends, if looked at.
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
                    self.scripts.on_mssp(&status);
                }
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
                Event::Disconnected(error) => {
                    self.scripts.on_event("disconnect", "");
                    self.run_actions();
                    self.walk = None;
                    self.pending_partial = None;
                    self.settled_partial = None;
                    self.gauges.clear();
                    self.msdp_vitals.clear();
                    self.sound.stop_all();
                    self.media_base = None;
                    self.media_jobs.clear();
                    self.mxp_values.clear();
                    self.mxp_gauges.clear();
                    self.input_hidden = false;
                    self.connection = None;
                    // A connection lost to an error is worth retrying; one
                    // the server or the user closed is not.
                    let lost = error.is_some() && !self.user_disconnected;
                    self.note(&match &error {
                        Some(error) => trf("Disconnected: {0}", &[error]),
                        None => tr("Disconnected.").to_owned(),
                    });
                    if lost && self.prefs.reconnect {
                        let wait =
                            Duration::from_secs(2u64.pow(self.reconnect_attempts.min(5)).min(60));
                        self.reconnect_attempts += 1;
                        self.reconnect_at = Some(Instant::now() + wait);
                        self.note(&trf(
                            "Reconnecting in {0} seconds.",
                            &[&wait.as_secs().to_string()],
                        ));
                    }
                    self.log = None;
                    // Closed to be opened again: as soon as can be.
                    if std::mem::take(&mut self.reconnect_wanted) {
                        self.reconnect_at = Some(Instant::now());
                    }
                }
            }
        }
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
