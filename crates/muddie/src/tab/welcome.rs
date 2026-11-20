//! The welcome screen of a new session: where to connect, typed or among
//! the saved profiles.

use super::*;
use crate::theme::Weight;
use crate::widgets;

/// The last light of the day, low behind everything.
fn sky(ui: &egui::Ui) {
    let theme = theme::current();
    let sky = ui.max_rect();
    let painter = ui.painter().with_clip_rect(sky);
    let strength = if theme.dark { 0.32 } else { 0.3 };
    let reach = egui::vec2(sky.width() * 0.62, sky.height() * 0.75);
    let low = sky.center_bottom() + egui::vec2(0.0, sky.height() * 0.2);
    let left = low - egui::vec2(sky.width() * 0.22, 0.0);
    widgets::glow(
        &painter,
        left,
        reach,
        theme.accent.gamma_multiply(strength * 0.35),
    );
    let right = low + egui::vec2(sky.width() * 0.1, 0.0);
    widgets::glow(
        &painter,
        right,
        reach * 0.8,
        theme.accent.gamma_multiply(strength * 0.5),
    );
}

/// The mark, the name, and the question.
fn heading(ui: &mut egui::Ui) {
    let theme = theme::current();
    ui.vertical_centered(|ui| {
        widgets::logo(ui, 104.0);
        ui.add_space(6.0);
        let name = RichText::new("Muddie")
            .font(theme::font(Weight::Semibold, 32.0))
            .color(theme.text);
        ui.add(Label::new(name).selectable(false));
        ui.add_space(6.0);
        let question = RichText::new(tr("Where do you want to play?"))
            .font(theme::font(Weight::Regular, 15.0))
            .color(theme.text_dim);
        ui.add(Label::new(question).selectable(false));
    });
}

/// Splits an address as people write them — `host:port`, `host port`, or
/// only the host, with or without `telnet://` — into host and port. The
/// port is the telnet one when none is given.
pub(super) fn parse_address(address: &str) -> Option<(String, String)> {
    let address = address.trim();
    let address = address.strip_prefix("telnet://").unwrap_or(address);
    let address = address.trim_end_matches('/');
    let is_port = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    let split = address
        .rsplit_once(|ch: char| ch == ':' || ch.is_whitespace())
        .filter(|(_, port)| is_port(port));
    let (host, port) = split.unwrap_or((address, "23"));
    let host = host.trim().trim_end_matches(':');
    (!host.is_empty()).then(|| (host.to_owned(), port.to_owned()))
}

impl Tab {
    /// The welcome screen: the name, the field for an address, and the
    /// saved profiles as cards.
    pub(super) fn welcome(&mut self, ui: &mut egui::Ui) {
        let profiles = self.listed_profiles();
        let (mut chosen, mut connect) = (None, false);
        let frame = egui::Frame::new().fill(theme::current().surface);
        CentralPanel::default().frame(frame).show(ui, |ui| {
            sky(ui);
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    let width = 520.0_f32.min(ui.available_width() - 32.0);
                    let margin = ((ui.available_width() - width) / 2.0).max(0.0);
                    let rows = profiles.len().div_ceil(2).min(4) as f32;
                    let height = 290.0
                        + if profiles.is_empty() {
                            0.0
                        } else {
                            60.0 + rows * 78.0
                        };
                    ui.add_space(((ui.available_height() - height) * 0.4).max(24.0));
                    ui.horizontal(|ui| {
                        ui.add_space(margin);
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            heading(ui);
                            ui.add_space(30.0);
                            connect = self.address_field(ui);
                            ui.add_space(14.0);
                            self.options_row(ui);
                            if !profiles.is_empty() {
                                chosen = self.profile_cards(ui, &profiles, width);
                            }
                            ui.add_space(24.0);
                        });
                    });
                });
        });
        let ctx = ui.ctx().clone();
        if let Some(name) = chosen {
            self.play(&name, &ctx);
        } else if connect && self.apply_address() {
            self.connect(&ctx);
        }
    }

    /// The saved profiles, each with where it leads, read once and kept
    /// until they change.
    fn listed_profiles(&mut self) -> Vec<(String, String)> {
        if let Some(listed) = &self.welcome_profiles {
            return listed.clone();
        }
        let listed: Vec<(String, String)> =
            self.profiles.as_ref().map_or_else(Vec::new, |profiles| {
                (profiles.names().into_iter())
                    .map(|name| {
                        let place = profiles
                            .load(&name)
                            .map(|profile| format!("{}:{}", profile.host, profile.port));
                        (name, place.unwrap_or_default())
                    })
                    .collect()
            });
        self.welcome_profiles = Some(listed.clone());
        listed
    }

    /// One field takes the whole address, in a pill with the button that
    /// connects. Returns whether connecting was asked, by the button or
    /// with Enter.
    fn address_field(&mut self, ui: &mut egui::Ui) -> bool {
        let theme = theme::current();
        let id = ui.make_persistent_id("address");
        let focused = ui.memory(|memory| memory.has_focus(id));
        let edge = if focused { theme.accent } else { theme.border };
        let pill = egui::Frame::new()
            .fill(theme.field)
            .stroke(egui::Stroke::new(1.0, edge))
            .corner_radius(egui::CornerRadius::same(26))
            .inner_margin(egui::Margin {
                left: 22,
                right: 7,
                top: 7,
                bottom: 7,
            });
        let mut connect = false;
        pill.show(ui, |ui| {
            let row = egui::vec2(ui.available_width(), 38.0);
            let centred = egui::Layout::right_to_left(egui::Align::Center);
            ui.allocate_ui_with_layout(row, centred, |ui| {
                let pressed = widgets::go_button(ui, tr("Connect"), 38.0).clicked();
                let field = ui.add(
                    TextEdit::singleline(&mut self.address)
                        .id(id)
                        .frame(egui::Frame::NONE)
                        .hint_text(RichText::new("mud.example.org:4000").color(theme.text_faint))
                        .font(theme::font(Weight::Regular, 16.0))
                        .align(egui::Align2::LEFT_CENTER)
                        .desired_width(f32::INFINITY),
                );
                ui.ctx()
                    .accesskit_node_builder(field.id, |node| node.set_label(tr("Address")));
                let entered = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
                connect = pressed || entered;
            });
        });
        connect
    }

    /// Whether to connect securely, and more options, which make the
    /// address a profile: more than an address is a profile's.
    fn options_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 9.0;
            ui.add_space(12.0);
            widgets::switch(ui, &mut self.tls, tr("TLS encryption"));
            let label = RichText::new(tr("TLS encryption"))
                .font(theme::font(Weight::Regular, 13.0))
                .color(theme::current().text_dim);
            ui.add(Label::new(label).selectable(false));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                if widgets::quiet_button(ui, tr("More options")).clicked() {
                    self.apply_address();
                    self.app_request = Some(AppRequest::SaveAsProfile);
                }
            });
        });
    }

    /// The saved profiles, two to a row, each with its menu on a right
    /// click. Returns the one chosen to play with.
    fn profile_cards(
        &mut self,
        ui: &mut egui::Ui,
        profiles: &[(String, String)],
        width: f32,
    ) -> Option<String> {
        ui.add_space(38.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            widgets::caption(ui, tr("Your profiles"));
        });
        ui.add_space(10.0);
        let gap = 10.0;
        let card = (width - gap) / 2.0;
        let mut chosen = None;
        for pair in profiles.chunks(2) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (name, place) in pair {
                    let pressed = widgets::profile_card(ui, card, name, place);
                    if pressed.clicked() {
                        chosen = Some(name.clone());
                    }
                    // What can be done with it, as anywhere else.
                    pressed.context_menu(|ui| {
                        if let Some(action) = crate::app::profile_menu(ui) {
                            self.app_request = Some(AppRequest::Profile(action, name.clone()));
                        }
                    });
                }
            });
            ui.add_space(gap);
        }
        chosen
    }

    /// Takes the host and port from the address typed on the welcome
    /// screen. Says whether there is somewhere to connect to.
    pub(super) fn apply_address(&mut self) -> bool {
        if let Some((host, port)) = parse_address(&self.address) {
            self.host = host;
            self.port = port;
        }
        !self.host.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::parse_address;

    #[test]
    fn addresses_are_read_as_people_write_them() {
        let parsed = |address| parse_address(address).map(|(host, port)| format!("{host} {port}"));
        assert_eq!(
            parsed("mud.example.org:4000").unwrap(),
            "mud.example.org 4000"
        );
        assert_eq!(
            parsed("  mud.example.org   4000 ").unwrap(),
            "mud.example.org 4000"
        );
        assert_eq!(
            parsed("telnet://mud.example.org:4000/").unwrap(),
            "mud.example.org 4000"
        );
        assert_eq!(parsed("mud.example.org").unwrap(), "mud.example.org 23");
        assert_eq!(parsed("mud.example.org:").unwrap(), "mud.example.org 23");
        assert_eq!(parsed("   "), None);
    }
}
