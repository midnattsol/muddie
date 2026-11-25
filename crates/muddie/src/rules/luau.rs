//! Turning each kind of rule into the Luau that does what it says.

use super::*;

/// Its commands, each sent with the groups of the match put in if it
/// has one, as [`commands_as_luau`] makes them.
fn sends(send: &str, matched: bool) -> String {
    (commands_as_luau(send, matched).lines())
        .map(|line| format!("\t{line}\n"))
        .collect()
}

/// A script of the user's, kept apart from what surrounds it.
fn script(code: &str) -> String {
    if code.trim().is_empty() {
        return String::new();
    }
    format!("\tdo\n{code}\n\tend\n")
}

/// The options that say what a rule is switched by: its place and its
/// tags, as a Luau table's fields.
fn belonging(place: &str, tags: &[String]) -> String {
    let tags: Vec<String> = (tags.iter())
        .filter(|tag| !tag.trim().is_empty())
        .map(|tag| quoted(tag.trim()))
        .collect();
    format!("place = {}, tags = {{{}}}", quoted(place), tags.join(", "))
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
    /// Whether it goes off on a match, whose groups its commands use.
    const MATCHED: bool;
    /// The code of a rule of this kind, given its `options` table.
    fn code(rule: &Rule<Self>, options: &str) -> String;
}

impl<T: Luau> Rule<T> {
    /// What it is called in its errors.
    pub fn reported_as(&self) -> String {
        titled(T::WORD, &self.name, &self.when.made_of())
    }

    /// The rule as Luau, switched by its place, and what to call it in
    /// errors.
    pub(super) fn chunk(&self, place: &str) -> (String, String) {
        let options = format!("{{{}}}", belonging(place, &self.tags));
        let title = self.reported_as();
        (title, T::code(self, &options))
    }

    /// What it does: its script, or its commands.
    fn action(&self) -> String {
        match self.luau {
            true => script(&self.code),
            false => sends(&self.send, T::MATCHED),
        }
    }
}

impl Luau for OnLine {
    const WORD: &'static str = "trigger";
    const MATCHED: bool = true;
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
        if when.silence {
            code.push_str("\tsilence()\n");
        }
        code.push_str(&rule.action());
        if !when.speak.trim().is_empty() {
            code.push_str(&format!(
                "\tspeak({})\n",
                filled_luau(when.speak.trim(), true)
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
    const MATCHED: bool = true;
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
    const MATCHED: bool = false;
    /// Its key bound to what it does.
    fn code(rule: &Rule<Self>, options: &str) -> String {
        let key = quoted(rule.when.key.trim());
        format!(
            "macro({key}, function()\n{}end, {options})\n",
            rule.action()
        )
    }
}

impl Luau for OnTime {
    const WORD: &'static str = "timer";
    const MATCHED: bool = false;
    /// Once after a while, or again and again.
    fn code(rule: &Rule<Self>, options: &str) -> String {
        let action = rule.action();
        let how = if rule.when.repeat { "every" } else { "after" };
        let seconds = f64::from(rule.when.tenths) / 10.0;
        format!("{how}({seconds}, function()\n{action}end, {options})\n")
    }
}

impl Gauge {
    /// What it is called in its errors.
    pub fn reported_as(&self) -> String {
        titled("gauge", self.shown_name(), "")
    }

    /// A gauge: shown again whenever what it follows is reported, over
    /// GMCP or MSDP, or changes, as a variable, and at once, from what is
    /// there: as it is saved, not when the MUD next says something.
    pub(super) fn chunk(&self, place: &str) -> (String, String) {
        let name = self.shown_name();
        let root = self.value.trim().split('.').next().unwrap_or_default();
        let code = format!(
            "{PRELUDE}local shown = false\nlocal function update()\n{shown}end\n\
             on(\"gmcp\", {root}, update)\n\
             on(\"msdp\", {variable}, update)\n\
             on(\"var\", {root}, update)\n\
             update()\n",
            shown = self.shown(name, &self.belonging(place)),
            root = quoted(root),
            variable = quoted(self.value.trim()),
        );
        (self.reported_as(), code)
    }

    /// How it is shown, by its kind: lit or not, as text, or as a value
    /// against its maximum.
    fn shown(&self, name: &str, belonging: &str) -> String {
        let (name, value) = (quoted(name), quoted(self.value.trim()));
        let look = format!("{}, {belonging}", self.look());
        match self.kind {
            GaugeKind::Light => {
                format!(
                    "\tlocal said = raw({value})\n\
                     \tif said ~= nil then\n\
                     \t\tgauge({name}, lit(said) and 1 or 0, 1, {{{look}}})\n\
                     \tend\n"
                )
            }
            // What is on, from a list or a table by name, once there is
            // one; none, once it is gone.
            GaugeKind::Effects => format!(
                "\tlocal said = raw({value})\n\
                 \tif type(said) == \"table\" or shown then\n\
                 \t\tgauge({name}, 0, 0, {{{look}, items = type(said) == \"table\" and said or {{}}}})\n\
                 \t\tshown = true\n\
                 \tend\n"
            ),
            GaugeKind::Text => format!(
                "\tlocal said = raw({value})\n\
                 \tif said ~= nil and type(said) ~= \"table\" then\n\
                 \t\tgauge({name}, 0, 0, {{{look}, text = tostring(said)}})\n\
                 \tend\n"
            ),
            // A bar of nothing, as the shield of one who has none, is not
            // shown; a number is, as it has no maximum.
            GaugeKind::Bar | GaugeKind::Cells => format!(
                "\tlocal value, max = dig({value}), dig({max})\n\
                 \tif value and max and max > 0 then\n\
                 \t\tgauge({name}, value, max, {{{look}}})\n\
                 \telseif value and max then\n\
                 \t\tgauge({name}, nil, nil, {{{belonging}}})\n\
                 \tend\n",
                max = quoted(self.maximum()),
            ),
            GaugeKind::Number => format!(
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

    /// What it is switched by, its place and its tags, and the panel it
    /// is shown in, if it was put in one.
    fn belonging(&self, place: &str) -> String {
        let mut said = belonging(place, &self.tags);
        if !self.panel.trim().is_empty() {
            said.push_str(&format!(", panel = {}", quoted(self.panel.trim())));
        }
        said
    }

    /// How it looks and is announced, named as the rule names it: its
    /// kind, its colours, its steps, and how its changes are said.
    fn look(&self) -> String {
        let steps: Vec<String> = (self.steps.iter())
            .filter(|step| !step.colour.trim().is_empty())
            .map(|step| {
                let colour = quoted(step.colour.trim());
                format!("{{under = {}, colour = {colour}}}", step.under)
            })
            .collect();
        format!(
            "kind = {}, colour = {}, off_colour = {}, step = {{{}}}, announce = {}",
            quoted(self.kind.key()),
            quoted(self.colour.trim()),
            quoted(self.off_colour.trim()),
            steps.join(", "),
            quoted(self.announce.key())
        )
    }
}
