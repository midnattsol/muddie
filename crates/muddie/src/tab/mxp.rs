//! What the MUD marks with MXP: links, values, gauges and frames.

use super::*;

impl Tab {
    /// Does what a link of the server's that was clicked asks for.
    pub(super) fn link_clicked(&mut self, clicked: crate::output::Clicked) {
        match clicked {
            crate::output::Clicked::Send(command) => self.send(command),
            // To be finished in the command line, with a space to go on.
            crate::output::Clicked::Prompt(command) => {
                self.input = format!("{command} ");
                self.kept_input = None;
                self.cursor_to_end = true;
            }
        }
    }

    /// Carries out what an MXP tag asked of the client besides text.
    pub(super) fn mxp(&mut self, asked: muddie_core::Mxp, recording: bool) {
        use muddie_core::Mxp;
        match asked {
            // A value by a name: kept, told to the scripts, and followed
            // by the gauges the server asked for.
            Mxp::Value { name, value } => {
                if recording {
                    self.saw(
                        "MXP",
                        &name,
                        &serde_json::Value::String(value.clone()).to_string(),
                    );
                }
                let told = serde_json::json!({"name": name, "value": value}).to_string();
                self.scripts.on_event("mxp", &told);
                self.run_actions();
                self.mxp_values.insert(name.to_lowercase(), value);
                self.mxp_gauges_follow();
            }
            Mxp::Gauge {
                entity,
                max,
                caption,
                colour,
                bar,
            } => {
                let called = if caption.is_empty() {
                    entity.clone()
                } else {
                    caption
                };
                self.mxp_gauges.retain(|gauge| gauge.0 != called);
                self.mxp_gauges.push((
                    called,
                    entity.to_lowercase(),
                    max.to_lowercase(),
                    colour,
                    bar,
                ));
                self.mxp_gauges_follow();
            }
            // A frame is a panel of text, by its name.
            Mxp::Frame {
                name,
                open,
                title: _,
                align,
            } => {
                if name.trim().is_empty() {
                    return;
                }
                let known = (self.layout.panels.iter())
                    .any(|panel| panel.name.eq_ignore_ascii_case(name.trim()));
                if !known && !open {
                    return;
                }
                let id = self.panel_called(&name);
                if let Some(panel) = self.layout.panel_mut(id) {
                    panel.visible = open;
                    if !known {
                        panel.side = match align.as_str() {
                            "left" => layout::Side::Left,
                            "top" => layout::Side::Top,
                            "bottom" => layout::Side::Bottom,
                            _ => layout::Side::Right,
                        };
                    }
                }
            }
            Mxp::Clear(name) => {
                if !name.trim().is_empty() {
                    let id = self.panel_called(&name);
                    self.text_of(id).log.clear();
                }
            }
            Mxp::Sound(request) => {
                if let Some(request) = sound::parse_msp(&request) {
                    self.msp(request);
                }
            }
            Mxp::Expire(name) => {
                self.output.expire(&name);
                let ids: Vec<u32> = self.layout.panels.iter().map(|panel| panel.id).collect();
                for id in ids {
                    self.text_of(id).log.expire(&name);
                }
            }
            // Said, not done: where to connect is the player's to choose.
            Mxp::Relocate { host, port } => {
                self.note(&trf(
                    "The MUD asks to go on at {0}, port {1}.",
                    &[&host, &port],
                ));
            }
            // The name and the password of the profile, as when logging
            // in by itself.
            Mxp::User => {
                let name = self.character.trim().to_owned();
                if !name.is_empty() {
                    self.send(name);
                    self.login = Login::Password(Instant::now());
                }
            }
            Mxp::Password => {
                if !self.character.trim().is_empty() {
                    self.login = Login::Password(Instant::now());
                    self.login_password();
                }
            }
        }
    }

    /// Sets the gauges the server asked for with MXP to the values they
    /// follow, as far as those are known and are numbers.
    pub(super) fn mxp_gauges_follow(&mut self) {
        for (called, entity, max, colour, bar) in self.mxp_gauges.clone() {
            let number = |name: &str| {
                let known = self
                    .mxp_values
                    .get(name)
                    .map(String::as_str)
                    .unwrap_or(name);
                known.trim().parse::<f64>().ok()
            };
            let Some(value) = number(&entity) else {
                continue;
            };
            let max = number(&max).unwrap_or(0.0);
            let look = muddie_script::GaugeLook {
                colour,
                kind: if bar && max > 0.0 { "bar" } else { "number" }.to_owned(),
                ..muddie_script::GaugeLook::default()
            };
            self.gauges
                .set_with(&called, Some((value, max)), Some(look));
        }
    }
}
