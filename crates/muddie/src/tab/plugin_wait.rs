//! Connecting for the first time before the official plugins have been
//! downloaded: the session waits for them a few seconds, so that what a
//! plugin asks of the MUD is asked from the start, and connects either way.
//! The window stays alive while it waits, and the wait can be ended.

use super::*;

/// How long a session waits for the plugins at most.
pub(super) const WAIT: Duration = Duration::from_secs(5);

impl Tab {
    /// Whether to wait before connecting: the official plugins are being
    /// downloaded, and none was ever downloaded to go on with.
    fn should_wait_for_plugins(&self) -> bool {
        self.plugin_download.is_running()
            && !self
                .plugins
                .iter()
                .any(|plugin| plugin.origin == crate::plugins::Origin::Official)
    }

    /// Starts waiting for the plugins, if that is called for, saying so.
    /// Returns whether it is waiting, so that connecting is put off.
    pub(super) fn wait_for_plugins(&mut self, ctx: &egui::Context) -> bool {
        if self.plugin_wait.is_some() {
            return true;
        }
        // Waited already: connects either way.
        if std::mem::take(&mut self.waited) {
            return false;
        }
        self.find_plugins();
        if !self.should_wait_for_plugins() {
            return false;
        }
        self.user_disconnected = false;
        self.reconnect_at = None;
        self.started = true;
        self.plugin_wait = Some(Instant::now());
        self.note(tr("Downloading plugins…"));
        ctx.request_repaint_after(Duration::from_millis(100));
        true
    }

    /// Whether there is something to disconnect from: a connection, or
    /// connecting that waits for the plugins.
    pub fn can_disconnect(&self) -> bool {
        self.connection.is_some() || self.plugin_wait.is_some()
    }

    /// Connects once the plugins arrived, or the wait is over.
    pub(super) fn check_plugin_wait(&mut self, ctx: &egui::Context) {
        let Some(since) = self.plugin_wait else {
            return;
        };
        if self.plugin_download.is_running() && since.elapsed() < WAIT {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        self.plugin_wait = None;
        self.waited = true;
        self.find_plugins();
        self.connect(ctx);
    }

    /// Stops waiting to connect, if it was.
    pub(super) fn stop_waiting_for_plugins(&mut self) {
        self.plugin_wait = None;
    }
}
