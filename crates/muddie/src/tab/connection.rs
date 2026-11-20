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
    pub(super) fn connect(&mut self, ctx: &egui::Context) {
        let host = self.host.trim().to_owned();
        let Ok(port) = self.port.trim().parse::<u16>() else {
            self.note(tr("The port must be a number from 1 to 65535."));
            return;
        };
        if host.is_empty() {
            self.note(tr("Enter a host to connect to."));
            return;
        }
        if muddie_core::decode::Decoder::for_label(&self.encoding).is_none() {
            let unknown = self.encoding.clone();
            self.note(&trf(
                "Unknown text encoding {0}; using the automatic one.",
                &[&unknown],
            ));
        }
        self.user_disconnected = false;
        self.reconnect_at = None;
        self.started = true;
        self.connected_with = Some(self.connection_key());
        self.note(&trf(
            "Connecting to {0}:{1}...",
            &[&host.to_string(), &port.to_string()],
        ));
        self.walk = None;
        // A script may have chosen to keep the map in memory only.
        let stored = if self.scripts.persist_map() {
            // A profile keeps its map in its own folder.
            let path = match (&self.profiles, &self.profile) {
                (Some(profiles), Some(name)) => Some(profiles.map_path(name)),
                _ => map_path(&host, port),
            };
            path.map(|path| Map::open(&path))
        } else {
            Some(Ok(Map::in_memory()))
        };
        let map = match stored {
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
        };
        *self.map.borrow_mut() = map;
        let target = Target {
            host,
            port,
            tls: self.tls,
            verify_certificate: self.verify_certificate,
        };
        let Session {
            commands,
            mut events,
        } = connect(target, {
            let mut config = Config {
                screen_reader: self.voice.enabled(),
                encoding: self.encoding.clone(),
                ..Config::default()
            };
            // What many MUDs send the same way is asked for only while
            // the plugin that makes something of it is on.
            if !self.conventions() {
                let ours =
                    |module: &String| module.starts_with("Core ") || module.starts_with("Client.");
                config.gmcp_supports.retain(ours);
                config.msdp_report.clear();
            }
            config.gmcp_supports.extend(self.supports.iter().cloned());
            config
        });

        // Forward events to the UI thread and wake it up for each one.
        let (sender, receiver) = channel();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                if sender.send(event).is_err() {
                    break;
                }
                // A moment later rather than at once: asked for at once,
                // egui draws a second frame after the first, and what
                // arrives together is drawn together this way.
                ctx.request_repaint_after(Duration::from_millis(30));
            }
        });
        self.connection = Some(Connection {
            commands,
            events: receiver,
        });
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
        if self.sound.has_loops() || !self.media_jobs.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}
