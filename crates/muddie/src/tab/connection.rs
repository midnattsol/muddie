//! Connecting, logging in, sending, and connecting again.

use super::*;

/// Where the map of the MUD at `host:port` is stored, creating the folder.
pub(super) fn map_path(host: &str, port: u16) -> Option<PathBuf> {
    let maps = crate::profile::user_dir(crate::profile::UserDir::Data)?.join("maps");
    std::fs::create_dir_all(&maps).ok()?;
    let safe_host: String = (host.chars())
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    Some(maps.join(format!("{safe_host}_{port}.sqlite")))
}

/// How long an automatic login waits for the prompt it expects.
pub(super) const LOGIN_TIMEOUT: Duration = Duration::from_secs(60);

impl Tab {
    /// Connects to the host and port in the fields, with the map of where
    /// it leads and what the client asks the MUD for.
    pub(super) fn connect(&mut self, ctx: &egui::Context) {
        let Some((host, port)) = self.address_to_connect() else {
            return;
        };
        self.user_disconnected = false;
        self.reconnect_at = None;
        self.started = true;
        self.connected_with = Some(self.connection_key());
        self.note(&trf(
            "Connecting to {0}:{1}...",
            &[&host, &port.to_string()],
        ));
        self.walk = None;
        *self.map.borrow_mut() = self.map_for(&host, port);
        let target = Target {
            host,
            port,
            tls: self.tls,
            verify_certificate: self.verify_certificate,
        };
        let Session { commands, events } = connect(target, self.connection_config());
        let events = forward(events, ctx.clone());
        self.connection = Some(Connection { commands, events });
    }

    /// The host and port to connect to, or nothing, said why, if they will
    /// not do. An encoding not known is said, and the automatic one used.
    fn address_to_connect(&mut self) -> Option<(String, u16)> {
        let host = self.host.trim().to_owned();
        let Ok(port) = self.port.trim().parse::<u16>() else {
            self.note(tr("The port must be a number from 1 to 65535."));
            return None;
        };
        if host.is_empty() {
            self.note(tr("Enter a host to connect to."));
            return None;
        }
        if muddie_core::decode::Decoder::for_label(&self.encoding).is_none() {
            let unknown = self.encoding.clone();
            self.note(&trf(
                "Unknown text encoding {0}; using the automatic one.",
                &[&unknown],
            ));
        }
        Some((host, port))
    }

    /// The map of where the session leads: its profile's, one by its
    /// address, or one kept in memory only if a script chose so or it
    /// cannot be saved, which is said.
    fn map_for(&mut self, host: &str, port: u16) -> Map {
        if !self.scripts.persist_map() {
            return Map::in_memory();
        }
        let path = match (&self.profiles, &self.profile) {
            (Some(profiles), Some(name)) => Some(profiles.map_path(name)),
            _ => map_path(host, port),
        };
        match path.map(|path| Map::open(&path)) {
            Some(Ok(map)) => map,
            Some(Err(error)) => {
                self.note(&trf(
                    "The map will not be saved: {0}",
                    &[&error.to_string()],
                ));
                Map::in_memory()
            }
            None => {
                self.note(tr("The map will not be saved: no data folder was found."));
                Map::in_memory()
            }
        }
    }

    /// What the client says of itself and asks the MUD for. What many MUDs
    /// send the same way is asked for only while the plugin that makes
    /// something of it is on.
    fn connection_config(&self) -> Config {
        let mut config = Config {
            screen_reader: self.voice.enabled(),
            encoding: self.encoding.clone(),
            ..Config::default()
        };
        if !self.conventions() {
            let ours =
                |module: &String| module.starts_with("Core ") || module.starts_with("Client.");
            config.gmcp_supports.retain(ours);
            config.msdp_report.clear();
        }
        config.gmcp_supports.extend(self.supports.iter().cloned());
        config
    }

    pub fn disconnect(&mut self) {
        self.user_disconnected = true;
        self.reconnect_at = None;
        if let Some(connection) = &self.connection {
            let _ = connection.commands.send(Command::Disconnect);
        }
    }

    /// Connects again: at once if there is no connection, and if there
    /// is one, once it has been closed.
    pub fn reconnect(&mut self, ctx: &egui::Context) {
        if self.host.trim().is_empty() {
            return;
        }
        if self.connection.is_some() {
            self.disconnect();
            self.reconnect_wanted = true;
        } else {
            self.connect(ctx);
        }
    }

    pub(super) fn send(&mut self, line: String) {
        match &self.connection {
            Some(connection) => {
                let _ = connection.commands.send(Command::Send(line));
                self.last_activity = Instant::now();
            }
            None => self.note(tr("Not connected.")),
        }
    }

    /// Answers the first prompt after connecting with the profile's
    /// character name, if it has one.
    pub(super) fn login_name(&mut self) {
        let Login::Name(since) = self.login else {
            return;
        };
        if since.elapsed() > LOGIN_TIMEOUT {
            self.login = Login::Idle;
            return;
        }
        // Only a prompt counts: text the server left unterminated.
        if self.settled_partial.is_none() {
            return;
        }
        self.login = Login::Password(Instant::now());
        self.send(self.character.trim().to_owned());
    }

    /// Answers a hidden prompt after the name with the password kept in
    /// the keyring for the profile.
    pub(super) fn login_password(&mut self) {
        let Login::Password(since) = self.login else {
            return;
        };
        self.login = Login::Idle;
        let (true, Some(profile)) = (since.elapsed() <= LOGIN_TIMEOUT, self.profile.clone()) else {
            return;
        };
        match secrets::password(&profile) {
            Ok(Some(password)) => self.send(password),
            Ok(None) => {}
            Err(error) => self.note(&trf(
                "The password could not be read from the keyring: {0}",
                &[&error],
            )),
        }
    }

    /// Reconnects, keeps the connection alive and keeps looped sounds
    /// going, each when its time comes.
    pub(super) fn check_timers(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if let Some(at) = self.reconnect_at {
            if at <= now {
                self.reconnect_at = None;
                self.connect(ctx);
            } else {
                ctx.request_repaint_after(at - now);
            }
        }
        let keepalive = Duration::from_secs(self.prefs.keepalive_seconds.into());
        if let (Some(connection), false) = (&self.connection, keepalive.is_zero()) {
            let idle = now.saturating_duration_since(self.last_activity);
            if idle >= keepalive {
                let _ = connection.commands.send(Command::KeepAlive);
                self.last_activity = now;
                ctx.request_repaint_after(keepalive);
            } else {
                ctx.request_repaint_after(keepalive - idle);
            }
        }
        self.media_fetched();
        self.sound.poll();
        if self.sound.has_loops() || !self.fetching.jobs.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

/// Passes what the connection says to the window's thread, and wakes it
/// for each: a moment later rather than at once, as asked for at once
/// egui draws a second frame after the first, and what arrives together
/// is drawn together this way.
fn forward(
    mut events: tokio::sync::mpsc::UnboundedReceiver<Event>,
    ctx: egui::Context,
) -> Receiver<Event> {
    let (sender, receiver) = channel();
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if sender.send(event).is_err() {
                break;
            }
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    });
    receiver
}
