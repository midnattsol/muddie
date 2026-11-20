//! Turning each kind of rule into the Luau that does what it says.

use super::*;

/// Its commands, each sent with the groups of the match put in.
fn sends(send: &str) -> String {
    commands(send)
        .map(|command| format!("\tsend(fill({}, m))\n", quoted(command)))
        .collect()
}

/// A script of the user's, kept apart from what surrounds it.
fn script(code: &str) -> String {
    if code.trim().is_empty() {
        return String::new();
    }
    format!("\tdo\n{code}\n\tend\n")
}

/// What a rule is called in errors: its kind and its name, or else
/// what it is made of.
fn titled(kind: &str, name: &str, fallback: &str) -> String {
    let name = if name.trim().is_empty() {
        fallback
    } else {
        name
    };
    format!("{kind} {}", name.trim())
}

/// How a rule says which group it is in.
fn grouped(group: &str) -> String {
    if group.trim().is_empty() {
        return String::new();
    }
    format!(", {{group = {}}}", quoted(group.trim()))
}

impl Trigger {
    /// A trigger: what it does with the line it matches, in order.
    pub(super) fn chunk(&self) -> (String, String) {
        let mut code = format!(
            "{PRELUDE}trigger({}, function(m, line)\n",
            quoted(&self.regex_source())
        );
        let call = |name: &str, given: &str| format!("\t{name}({})\n", quoted(given.trim()));
        if !self.highlight.trim().is_empty() {
            code.push_str(&call("highlight", &self.highlight));
        }
        if self.gag {
            code.push_str("\tgag()\n");
        }
        code.push_str(&if self.lua {
            script(&self.script)
        } else {
            sends(&self.send)
        });
        if !self.speak.trim().is_empty() {
            code.push_str(&format!(
                "\tspeak(fill({}, m))\n",
                quoted(self.speak.trim())
            ));
        }
        if !self.sound.trim().is_empty() {
            code.push_str(&call("play", &self.sound));
        }
        if !self.panel.trim().is_empty() {
            code.push_str(&call("to_panel", &self.panel));
        }
        code.push_str(&format!("end{})\n", grouped(&self.group)));
        (titled("trigger", &self.name, &self.pattern), code)
    }
}

impl Alias {
    /// An alias: what it sends, or its script, in place of what was typed.
    pub(super) fn chunk(&self) -> (String, String) {
        let action = if self.lua {
            script(&self.script)
        } else {
            sends(&self.send)
        };
        let code = format!(
            "{PRELUDE}alias({}, function(m, input)\n{action}end{})\n",
            quoted(&self.regex_source()),
            grouped(&self.group)
        );
        (titled("alias", &self.name, &self.pattern), code)
    }
}

impl KeyBinding {
    /// A macro: its key bound to its script, or to what it sends, which
    /// goes through aliases as if typed.
    pub(super) fn chunk(&self) -> (String, String) {
        let name = quoted(self.key.trim());
        let code = if self.lua {
            format!(
                "bind({name}, function()\n{}end{})\n",
                script(&self.script),
                grouped(&self.group)
            )
        } else {
            let typed = commands(&self.send).collect::<Vec<_>>().join(";");
            format!("bind({name}, {}{})\n", quoted(&typed), grouped(&self.group))
        };
        (titled("macro", &self.name, &self.key), code)
    }
}

impl Timer {
    /// A timer: once after a while, or again and again.
    pub(super) fn chunk(&self) -> (String, String) {
        let action = if self.lua {
            script(&self.script)
        } else {
            commands(&self.send)
                .map(|command| format!("\tsend(expand({}))\n", quoted(command)))
                .collect()
        };
        let how = if self.repeat { "every" } else { "after" };
        let seconds = f64::from(self.tenths) / 10.0;
        let code = format!(
            "{how}({seconds}, function()\n{action}end{})\n",
            grouped(&self.group)
        );
        (titled("timer", &self.name, &self.interval()), code)
    }
}

impl Gauge {
    /// A gauge: shown again whenever what it follows is reported, over
    /// GMCP or MSDP.
    pub(super) fn chunk(&self) -> (String, String) {
        let name = if self.name.trim().is_empty() {
            self.value.trim()
        } else {
            self.name.trim()
        };
        let root = self.value.trim().split('.').next().unwrap_or_default();
        let code = format!(
            "{PRELUDE}local function update()\n{shown}end\n\
             on_gmcp({root}, update)\n\
             on_msdp({variable}, update)\n",
            shown = self.shown(name),
            root = quoted(root),
            variable = quoted(self.value.trim()),
        );
        (titled("gauge", name, ""), code)
    }

    /// How it is shown, by its kind: lit or not, as text, or as a value
    /// against its maximum.
    fn shown(&self, name: &str) -> String {
        let (name, value, look) = (quoted(name), quoted(self.value.trim()), self.look());
        match self.kind {
            GaugeKind::Light => {
                format!("\tgauge({name}, lit(raw({value})) and 1 or 0, 1, {{{look}}})\n")
            }
            GaugeKind::Text => format!(
                "\tlocal said = raw({value})\n\
                 \tif said ~= nil and type(said) ~= \"table\" then\n\
                 \t\tgauge({name}, 0, 0, {{{look}, text = tostring(said)}})\n\
                 \tend\n"
            ),
            _ => format!(
                "\tlocal value, max = dig({value}), dig({max})\n\
                 \tif value and max then\n\
                 \t\tgauge({name}, value, max, {{{look}}})\n\
                 \tend\n",
                max = quoted(self.maximum()),
            ),
        }
    }

    /// Its maximum: a number alone has none unless it is given one.
    fn maximum(&self) -> &str {
        match (self.max.trim(), self.kind) {
            ("", GaugeKind::Number) => "0",
            ("", _) => "100",
            (max, _) => max,
        }
    }

    /// How it looks: its colours, its style, and its levels.
    fn look(&self) -> String {
        let steps: Vec<String> = (self.steps.iter())
            .filter(|step| !step.colour.trim().is_empty())
            .map(|step| format!("{{{}, {}}}", step.under, quoted(step.colour.trim())))
            .collect();
        format!(
            "colour = {}, off = {}, style = {}, steps = {{{}}}",
            quoted(self.colour.trim()),
            quoted(self.off_colour.trim()),
            quoted(self.kind.style()),
            steps.join(", ")
        )
    }
}
