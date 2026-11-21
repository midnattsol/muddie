//! Notices over the bottom right corner of the window, which move nothing:
//! what the client says while the lines of the game are not in sight, what
//! it says of work done aside wherever the user is, and what waits for an
//! answer, such as a plugin offered for the MUD. What is
//! said is kept under the bell too, and the screen reader is told of it as
//! it is said.

use super::*;

/// How long a notice stays: one of something that could not be done,
/// longer.
const SAID_FOR: std::time::Duration = std::time::Duration::from_secs(6);
const PROBLEM_FOR: std::time::Duration = std::time::Duration::from_secs(10);

/// How many notices are shown at once, at most.
const AT_ONCE: usize = 3;

impl App {
    /// The notices of the session on screen.
    pub(super) fn toasts(&mut self, ctx: &egui::Context) {
        let tab = &self.tabs[self.active];
        // In sight, the lines of the game say it: notices are for what is
        // said while they are not.
        let covered = tab.view() != 0
            || self.show_settings
            || self.show_profiles
            || self.show_shortcuts
            || self.show_about
            || self.asking.is_some();
        if !covered {
            self.toasts_after = tab.notices_said();
        }
        let (said, _) = tab.notices();
        let fresh: Vec<&crate::tab::Notice> = (said.iter())
            .filter(|notice| notice.aside || notice.serial > self.toasts_after)
            .filter(|notice| {
                let lasts = if notice.problem {
                    PROBLEM_FOR
                } else {
                    SAID_FOR
                };
                notice.at.elapsed() < lasts
            })
            .collect();
        let fresh: Vec<(String, bool)> = (fresh.iter().rev().take(AT_ONCE).rev())
            .map(|notice| (notice.text.clone(), notice.problem))
            .collect();
        let offered = tab.plugin_offered();
        if fresh.is_empty() && offered.is_none() {
            return;
        }
        // Gone when its time is up, even with nothing else moving.
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        let mut answer = None;
        egui::Area::new(egui::Id::unique("toasts"))
            .order(egui::Order::Foreground)
            // Above the command line and the bar under it, clear of both.
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -104.0))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.spacing_mut().item_spacing.y = size::GAP_S;
                for (text, problem) in &fresh {
                    widgets::toast(ui, text, *problem, None);
                }
                if let Some((what, about)) = &offered {
                    let text = format!("{what} {about}");
                    widgets::toast(
                        ui,
                        &text,
                        false,
                        Some(&mut |ui: &mut Ui| {
                            if widgets::primary_button(ui, tr("Enable")).clicked() {
                                answer = Some(true);
                            }
                            if widgets::quiet_button(ui, tr("Not now")).clicked() {
                                answer = Some(false);
                            }
                        }),
                    );
                }
            });
        if let Some(enable) = answer {
            self.tabs[self.active].answer_plugin_offer(enable);
        }
    }
}
