//! Saving what is edited once the edits stop for a moment, rather than at
//! every key: the one way the editor and the settings keep their work.

use std::time::{Duration, Instant};

use eframe::egui;

/// How long the edits are left alone before they are saved.
pub const SAVE_AFTER: Duration = Duration::from_millis(900);

/// When what is edited last changed, to tell when to save it.
#[derive(Debug, Default, Clone, Copy)]
pub struct Settle {
    since: Option<Instant>,
}

impl Settle {
    /// Something changed now: the wait starts again.
    pub fn changed(&mut self) {
        self.since = Some(Instant::now());
    }

    /// Nothing is waiting any more: it was saved, or let go.
    pub fn clear(&mut self) {
        self.since = None;
    }

    /// Whether what changed has been left alone long enough to be saved;
    /// it is then no longer waiting. If not yet, asks for a frame when it
    /// will be.
    pub fn due(&mut self, ctx: &egui::Context) -> bool {
        let Some(at) = self.since else {
            return false;
        };
        let waited = at.elapsed();
        if waited >= SAVE_AFTER {
            self.since = None;
            return true;
        }
        ctx.request_repaint_after(SAVE_AFTER - waited);
        false
    }
}
