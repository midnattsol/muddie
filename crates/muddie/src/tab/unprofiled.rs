//! A session not yet anyone's: no profile chosen, and not connected. The
//! layout and the rules it has are those of sessions without a profile,
//! which choosing one puts aside; the views of them say so first, rather
//! than show what this session may never have.

use super::*;
use crate::theme::Weight;
use crate::widgets;

/// Which view says so.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Unprofiled {
    Layout,
    Editor,
}

impl Tab {
    /// Whether the session is not yet anyone's: no profile, not started.
    pub(super) fn unprofiled(&self) -> bool {
        self.profile.is_none() && !self.started
    }

    /// In place of the layout or the editor of a session not yet anyone's:
    /// that they are each profile's own, the way to choose one, and the
    /// way to those of sessions without a profile, knowing they are.
    /// Returns whether it was shown, the view then not.
    pub(super) fn unprofiled_page(&mut self, ui: &mut egui::Ui, view: Unprofiled) -> bool {
        if !self.unprofiled() || self.unprofiled_seen {
            return false;
        }
        if widgets::take_escape(ui) {
            self.set_view(View::Game);
            return true;
        }
        let theme = theme::current();
        let (title, text, own) = match view {
            Unprofiled::Layout => (
                tr("No layout yet"),
                tr(
                    "Each profile has its own panels and where they go. Choose a profile, or connect, to lay its panels out.",
                ),
                tr("Edit the layout of sessions without a profile"),
            ),
            Unprofiled::Editor => (
                tr("No rules yet"),
                tr(
                    "Each profile has its own scripts, triggers and gauges. Choose a profile, or connect, to edit them.",
                ),
                tr("Edit the rules of sessions without a profile"),
            ),
        };
        let page = egui::Frame::new().fill(theme.surface).inner_margin(24);
        CentralPanel::default().frame(page).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.set_max_width(460.0);
                ui.add_space((ui.available_height() * 0.25).max(24.0));
                let heading = RichText::new(title)
                    .font(theme::font(Weight::Semibold, 17.0))
                    .color(theme.text);
                let shown = ui.add(egui::Label::new(heading).selectable(false));
                widgets::heading_role(ui, &shown, title);
                ui.add_space(8.0);
                let said = RichText::new(text)
                    .font(theme::font(Weight::Regular, 13.0))
                    .color(theme.text_dim);
                ui.add(egui::Label::new(said).selectable(false));
                ui.add_space(16.0);
                if widgets::primary_button(ui, tr("Choose a profile")).clicked() {
                    self.app_request = Some(commands::AppRequest::Profiles);
                }
                ui.add_space(6.0);
                if widgets::quiet_button(ui, own).clicked() {
                    self.unprofiled_seen = true;
                }
            });
        });
        true
    }

    /// Over the layout or the editor of a session not yet anyone's, once
    /// asked for: whose they are.
    pub(super) fn unprofiled_banner(&self, ui: &mut egui::Ui) {
        if !self.unprofiled() {
            return;
        }
        let theme = theme::current();
        egui::Frame::new()
            .fill(theme.accent.gamma_multiply(0.12))
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let said = RichText::new(tr(
                    "These are of sessions without a profile: choosing a profile puts them aside.",
                ))
                .font(theme::font(Weight::Regular, 12.5))
                .color(theme.text_dim);
                ui.add(egui::Label::new(said).selectable(false));
            });
        ui.add_space(8.0);
    }
}
