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

/// How each kind of rule is made in Luau.
pub trait Luau: When {
    /// What it is called in errors.
    const WORD: &'static str;
    /// The code of a rule of this kind, given its `options` table.
    fn code(rule: &Rule<Self>, options: &str) -> String;
}

impl<T: Luau> Rule<T> {
    /// The rule as Luau, switched by its place, and what to call it in
    /// errors.
    pub(super) fn chunk(&self, place: &str) -> (String, String) {
        let tags: Vec<String> = (self.tags.iter())
            .filter(|tag| !tag.trim().is_empty())
            .map(|tag| quoted(tag.trim()))
            .collect();
        let options = format!(
            "{{name = {}, tags = {{{}}}}}",
            quoted(place),
            tags.join(", ")
        );
        let title = titled(T::WORD, &self.name, &self.when.made_of());
        (title, T::code(self, &options))
    }

    /// What it does: its script, or its commands.
    fn action(&self) -> String {
        match self.lua {
            true => script(&self.script),
            false => sends(&self.send),
        }
    }
}

impl Luau for OnLine {
    const WORD: &'static str = "trigger";
    /// What it does with the line it matches, in order.
    fn code(rule: &Rule<Self>, options: &str) -> String {
        let when = &rule.when;
        let mut code = format!(
            "{PRELUDE}trigger({}, function(m, line)\n",
            quoted(&when.regex_source())
        );
        let call = |name: &str, given: &str| format!("\t{name}({})\n", quoted(given.trim()));
        if !when.highlight.trim().is_empty() {
            code.push_str(&call("highlight", &when.highlight));
        }
        if when.gag {
            code.push_str("\tgag()\n");
        }
        code.push_str(&rule.action());
        if !when.speak.trim().is_empty() {
            code.push_str(&format!(
                "\tspeak(fill({}, m))\n",
                quoted(when.speak.trim())
            ));
        }
        if !when.sound.trim().is_empty() {
            code.push_str(&call("play", &when.sound));
        }
        if !when.panel.trim().is_empty() {
            code.push_str(&call("to_panel", &when.panel));
        }
        code.push_str(&format!("end, {options})\n"));
        code
    }
}

impl Luau for OnTyped {
    const WORD: &'static str = "alias";
    /// What it sends, or its script, in place of what was typed.
    fn code(rule: &Rule<Self>, options: &str) -> String {
        format!(
            "{PRELUDE}alias({}, function(m, input)\n{}end, {options})\n",
            quoted(&rule.when.regex_source()),
            rule.action(),
        )
    }
}

impl Luau for OnKey {
    const WORD: &'static str = "macro";
    /// Its key bound to its script, or to what it sends, which goes
    /// through aliases as if typed.
    fn code(rule: &Rule<Self>, options: &str) -> String {
        let key = quoted(rule.when.key.trim());
        if rule.lua {
            format!(
                "bind({key}, function()\n{}end, {options})\n",
                script(&rule.script)
            )
        } else {
            let typed = commands(&rule.send).collect::<Vec<_>>().join(";");
            format!("bind({key}, {}, {options})\n", quoted(&typed))
        }
    }
}

impl Luau for OnTime {
    const WORD: &'static str = "timer";
    /// Once after a while, or again and again.
    fn code(rule: &Rule<Self>, options: &str) -> String {
        let action = if rule.lua {
            script(&rule.script)
        } else {
            commands(&rule.send)
                .map(|command| format!("\tsend(expand({}))\n", quoted(command)))
                .collect()
        };
        let how = if rule.when.repeat { "every" } else { "after" };
        let seconds = f64::from(rule.when.tenths) / 10.0;
        format!("{how}({seconds}, function()\n{action}end, {options})\n")
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
