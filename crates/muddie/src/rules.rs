//! Triggers, aliases and keys made in the interface instead of in a
//! script. They are kept in `rules.toml` and turned into Luau for the
//! script engine, so they behave exactly like their scripted equivalents.

use std::path::Path;

use serde::{Deserialize, Serialize};

fn yes() -> bool {
    true
}

/// Reacts to a line of output.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Trigger {
    /// What the user calls it; shown in lists instead of the pattern.
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Text the line must contain, or a regular expression if `regex`.
    pub pattern: String,
    pub regex: bool,
    /// Commands to send, separated by `;`. `%1`... are the pattern's groups.
    pub send: String,
    /// Hide the line.
    pub gag: bool,
    /// Colour to give the matched text, as a name or `#rrggbb`; empty for
    /// none.
    pub highlight: String,
    /// Text to say; `%1`... are the pattern's groups.
    pub speak: String,
    /// Sound file to play.
    pub sound: String,
    /// A panel of text to show the line in, besides or instead of the
    /// output; empty for none.
    pub panel: String,
    /// Luau to run, with the match as `m` and the line as `line`.
    pub script: String,
    /// What it does is the script, not the commands.
    pub lua: bool,
    /// The group it is switched on and off with; empty for none.
    pub group: String,
}

/// Replaces a typed command.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Alias {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// The word that starts the command, or a regular expression matched
    /// against the whole command if `regex`.
    pub pattern: String,
    pub regex: bool,
    /// Commands to send, separated by `;`. For a word alias `%1` is the
    /// rest of what was typed; for a regular expression, its groups.
    pub send: String,
    /// Luau to run, with the match as `m` and what was typed as `input`.
    pub script: String,
    /// What it does is the script, not the commands.
    pub lua: bool,
    /// The group it is switched on and off with; empty for none.
    pub group: String,
}

/// Sends commands when a key is pressed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBinding {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// The key as `bind` names it, such as `f5` or `ctrl+k`.
    pub key: String,
    pub send: String,
    /// Luau to run when the key is pressed.
    pub script: String,
    /// What it does is the script, not the commands.
    pub lua: bool,
    /// The group it is switched on and off with; empty for none.
    pub group: String,
}

/// Does something after a while, once or again and again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Timer {
    pub name: String,
    pub enabled: bool,
    /// How long it waits, in tenths of a second.
    pub tenths: u32,
    /// Goes off every so long, instead of only once after starting.
    pub repeat: bool,
    pub send: String,
    pub script: String,
    /// What it does is the script, not the commands.
    pub lua: bool,
    /// The group it is switched on and off with; empty for none.
    pub group: String,
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            name: String::new(),
            enabled: true,
            tenths: 600,
            repeat: true,
            send: String::new(),
            script: String::new(),
            lua: false,
            group: String::new(),
        }
    }
}

impl Timer {
    /// How long it waits, as people say it: `1.5 s`, `2 min`.
    pub fn interval(&self) -> String {
        let seconds = f64::from(self.tenths) / 10.0;
        if self.tenths >= 600 && self.tenths.is_multiple_of(600) {
            format!("{} min", self.tenths / 600)
        } else if self.tenths >= 600 {
            format!(
                "{} min {} s",
                self.tenths / 600,
                f64::from(self.tenths % 600) / 10.0
            )
        } else {
            format!("{seconds} s")
        }
    }
}

/// Whether a group of rules starts a session switched on. A group that
/// has no entry does.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Group {
    pub name: String,
    pub enabled: bool,
}

fn is_zero(number: &u8) -> bool {
    *number == 0
}

/// What a gauge is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GaugeKind {
    /// A bar that fills.
    #[default]
    Bar,
    /// A row of ten cells.
    Cells,
    /// Its number, with no bar.
    Number,
    /// A light that is on or off: for what is so or is not.
    Light,
    /// What the MUD says, as text.
    Text,
}

impl GaugeKind {
    /// Whether it shows how full something is, and so has a maximum and
    /// can change colour by level.
    pub fn has_level(self) -> bool {
        matches!(self, Self::Bar | Self::Cells | Self::Number)
    }

    fn style(self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Cells => "cells",
            Self::Number => "number",
            Self::Light => "light",
            Self::Text => "text",
        }
    }
}

/// A colour a gauge takes while it is under a percentage of its maximum.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Step {
    pub under: u8,
    pub colour: String,
}

/// A bar that follows a value the server reports.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Gauge {
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Where the value is: a GMCP package and the fields inside it, such
    /// as `Char.Vitals.health.current`, or the name of a MSDP variable.
    pub value: String,
    /// Where the maximum is, in the same way, or a number.
    pub max: String,
    /// A colour name or `#rrggbb`; empty to let the client choose.
    pub colour: String,
    /// What it is drawn as.
    pub kind: GaugeKind,
    /// For a light, its colour while it is off; empty leaves it unlit.
    pub off_colour: String,
    /// The colour it takes under each percentage of its maximum.
    #[serde(rename = "step")]
    pub steps: Vec<Step>,
    /// As earlier versions kept the two above; read, and brought up to
    /// date by [`Rules::migrate`].
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub segments: bool,
    #[serde(skip_serializing_if = "is_zero")]
    pub low: u8,
    /// The panel of gauges it is shown in, by the panel's id; none shows
    /// it in the first there is.
    pub panel: u32,
}

impl Gauge {
    /// A gauge as one starts: cells, red when low.
    pub fn starter() -> Self {
        Self {
            enabled: true,
            kind: GaugeKind::Cells,
            steps: vec![Step {
                under: 30,
                colour: "red".to_owned(),
            }],
            ..Self::default()
        }
    }

    /// What the gauge is called on screen: its name, or failing that
    /// where its value is.
    pub fn shown_name(&self) -> &str {
        if self.name.trim().is_empty() {
            self.value.trim()
        } else {
            self.name.trim()
        }
    }
}

impl Trigger {
    /// The regular expression the trigger matches lines with.
    pub fn regex_source(&self) -> String {
        if self.regex {
            self.pattern.clone()
        } else {
            regex_escape(&self.pattern)
        }
    }

    /// A line saying what the trigger does, for lists.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.lua {
            parts.push("Luau".to_owned());
        } else if !self.send.trim().is_empty() {
            parts.push(format!(
                "→ {}",
                commands(&self.send).collect::<Vec<_>>().join("; ")
            ));
        }
        if !self.speak.trim().is_empty() {
            parts.push(format!("“{}”", self.speak.trim()));
        }
        if !self.sound.trim().is_empty() {
            parts.push(format!("♪ {}", self.sound.trim()));
        }
        if !self.highlight.trim().is_empty() {
            parts.push(self.highlight.trim().to_owned());
        }
        if !self.panel.trim().is_empty() {
            parts.push(format!("▸ {}", self.panel.trim()));
        }
        if self.gag {
            parts.push("∅".to_owned());
        }

        parts.join("  ·  ")
    }
}

impl Alias {
    /// The regular expression the alias matches typed commands with.
    pub fn regex_source(&self) -> String {
        if self.regex {
            self.pattern.clone()
        } else {
            // The word alone, or followed by arguments, which are %1.
            format!("^{}(?:\\s+(.*))?$", regex_escape(self.pattern.trim()))
        }
    }
}

/// Puts a match's groups into a template: `%1`... become the groups.
pub fn fill(template: &str, captures: &regex::Captures) -> String {
    let mut filled = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match (ch, chars.peek().and_then(|next| next.to_digit(10))) {
            ('%', Some(group)) => {
                chars.next();
                filled.push_str(captures.get(group as usize).map_or("", |m| m.as_str()));
            }
            _ => filled.push(ch),
        }
    }
    filled
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    #[serde(rename = "trigger")]
    pub triggers: Vec<Trigger>,
    #[serde(rename = "alias")]
    pub aliases: Vec<Alias>,
    #[serde(rename = "key")]
    pub keys: Vec<KeyBinding>,
    #[serde(rename = "timer")]
    pub timers: Vec<Timer>,
    #[serde(rename = "group")]
    pub groups: Vec<Group>,
    #[serde(rename = "gauge")]
    pub gauges: Vec<Gauge>,
    /// The scripts that are switched off, by [`script_key`]: they are
    /// kept, and not run.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scripts_off: Vec<String>,
}

/// What a script file is known by among those switched off: its name, or
/// for the one of every session, which is not in a `scripts` folder,
/// its name after `every/`.
pub fn script_key(file: &std::path::Path) -> String {
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    match file.parent().is_some_and(|dir| dir.ends_with("scripts")) {
        true => name.into_owned(),
        false => format!("every/{name}"),
    }
}

/// The commands of a rule's `send`: one per line, or separated by `;`.
pub fn commands(send: &str) -> impl Iterator<Item = &str> {
    send.split([';', '\n'])
        .map(str::trim)
        .filter(|command| !command.is_empty())
}

/// The commands of a rule's `send` as the Luau that sends them: `%1`...
/// become the match's groups if the rule has a match.
pub fn commands_as_lua(send: &str, matched: bool) -> String {
    let mut code = String::new();
    for command in commands(send) {
        let mut pieces: Vec<String> = Vec::new();
        let mut text = String::new();
        let mut chars = command.chars().peekable();
        while let Some(ch) = chars.next() {
            match (ch, chars.peek().and_then(|next| next.to_digit(10))) {
                ('%', Some(group)) if matched => {
                    chars.next();
                    if !text.is_empty() {
                        pieces.push(quoted(&std::mem::take(&mut text)));
                    }
                    pieces.push(format!("(m[{group}] or \"\")"));
                }
                _ => text.push(ch),
            }
        }
        if !text.is_empty() || pieces.is_empty() {
            pieces.push(quoted(&text));
        }
        code.push_str(&format!("send({})\n", pieces.join(" .. ")));
    }
    code
}

/// What every chunk of [`Rules::chunks`] starts with.
const PRELUDE: &str = r#"local function fill(template, m)
	return expand((template:gsub("%%(%d)", function(n) return m[tonumber(n)] or "" end)))
end
local function raw(path)
	local parts = string.split(path, ".")
	for n = #parts, 1, -1 do
		local data = gmcp(table.concat(parts, ".", 1, n))
		if data ~= nil then
			for i = n + 1, #parts do
				if type(data) ~= "table" then return nil end
				data = data[parts[i]]
			end
			return data
		end
	end
	return msdp(path)
end
local function dig(path)
	return tonumber(path) or tonumber(raw(path))
end
local function lit(value)
	if type(value) == "string" then
		local word = string.lower(value)
		return tonumber(value) ~= 0 and word ~= "" and word ~= "false" and word ~= "no" and word ~= "off"
	end
	return value == true or (type(value) == "number" and value ~= 0) or type(value) == "table"
end
"#;

impl Rules {
    /// Reads the rules file; a missing one means no rules.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let loaded: Result<Self, _> = toml::from_str(&text);
                let mut rules = loaded.map_err(|error| format!("{}: {error}", path.display()))?;
                rules.migrate();
                Ok(rules)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("cannot read {}: {error}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|error| error.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        }
        std::fs::write(path, text)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    }

    /// Brings rules written by earlier versions up to date: a rule does
    /// its commands or its script, so one that had both becomes a script
    /// that starts by sending the commands.
    pub fn migrate(&mut self) {
        let merge = |send: &mut String, script: &mut String, lua: &mut bool, matched: bool| {
            if !*lua && !script.trim().is_empty() {
                *script = format!("{}{script}", commands_as_lua(send, matched));
                send.clear();
                *lua = true;
            }
        };
        for rule in &mut self.triggers {
            merge(&mut rule.send, &mut rule.script, &mut rule.lua, true);
        }
        for rule in &mut self.aliases {
            merge(&mut rule.send, &mut rule.script, &mut rule.lua, true);
        }
        for rule in &mut self.keys {
            merge(&mut rule.send, &mut rule.script, &mut rule.lua, false);
        }
        // Gauges had a switch for cells and one level to warn under.
        for gauge in &mut self.gauges {
            if std::mem::take(&mut gauge.segments) && gauge.kind == GaugeKind::Bar {
                gauge.kind = GaugeKind::Cells;
            }
            let low = std::mem::take(&mut gauge.low);
            if low > 0 && gauge.steps.is_empty() {
                gauge.steps.push(Step {
                    under: low,
                    colour: "red".to_owned(),
                });
            }
        }
    }

    /// The groups there are, in alphabetical order: those a rule is in
    /// and those with a state of their own.
    pub fn group_names(&self) -> Vec<String> {
        let rules = (self.triggers.iter().map(|rule| &rule.group))
            .chain(self.aliases.iter().map(|rule| &rule.group))
            .chain(self.keys.iter().map(|rule| &rule.group))
            .chain(self.timers.iter().map(|rule| &rule.group));
        let mut names: Vec<String> = rules
            .chain(self.groups.iter().map(|group| &group.name))
            .cloned()
            .collect();
        names.retain(|name| !name.trim().is_empty());
        names.sort_by_key(|name| name.to_lowercase());
        names.dedup();
        names
    }

    /// Whether a group starts a session switched on.
    pub fn group_enabled(&self, name: &str) -> bool {
        self.groups
            .iter()
            .find(|group| group.name == name)
            .is_none_or(|group| group.enabled)
    }

    /// Sets whether a group starts a session switched on.
    pub fn set_group_enabled(&mut self, name: &str, enabled: bool) {
        self.groups.retain(|group| group.name != name);
        // Only what differs from the usual is worth keeping.
        if !enabled {
            self.groups.push(Group {
                name: name.to_owned(),
                enabled,
            });
        }
    }

    /// The rules as one Luau script.
    #[cfg(test)]
    pub fn to_luau(&self) -> String {
        let chunks = self
            .chunks()
            .into_iter()
            .map(|(_, code)| format!("do\n{code}end\n"));
        chunks.collect()
    }

    /// Each rule as a Luau script of its own, with a name to report its
    /// errors by. Disabled rules and those with nothing to match are left
    /// out.
    pub fn chunks(&self) -> Vec<(String, String)> {
        let sends = |send: &str| -> String {
            commands(send)
                .map(|command| format!("\tsend(fill({}, m))\n", quoted(command)))
                .collect()
        };
        // A script of the user's, kept apart from what surrounds it.
        let script = |code: &str| -> String {
            if code.trim().is_empty() {
                String::new()
            } else {
                format!("\tdo\n{code}\n\tend\n")
            }
        };
        let titled = |kind: &str, name: &str, fallback: &str| {
            let name = if name.trim().is_empty() {
                fallback
            } else {
                name
            };
            format!("{kind} {}", name.trim())
        };
        // How a rule says which group it is in.
        let grouped = |group: &str| -> String {
            if group.trim().is_empty() {
                String::new()
            } else {
                format!(", {{group = {}}}", quoted(group.trim()))
            }
        };
        let mut chunks = Vec::new();
        // The groups that start switched off, before anything is in them.
        let off: String = (self.groups.iter())
            .filter(|group| !group.enabled)
            .map(|group| format!("group.disable({})\n", quoted(group.name.trim())))
            .collect();
        if !off.is_empty() {
            chunks.push(("groups".to_owned(), off));
        }
        for trigger in
            (self.triggers.iter()).filter(|rule| rule.enabled && !rule.pattern.is_empty())
        {
            let mut code = format!(
                "{PRELUDE}trigger({}, function(m, line)\n",
                quoted(&trigger.regex_source())
            );
            if !trigger.highlight.trim().is_empty() {
                code.push_str(&format!(
                    "\thighlight({})\n",
                    quoted(trigger.highlight.trim())
                ));
            }
            if trigger.gag {
                code.push_str("\tgag()\n");
            }
            if trigger.lua {
                code.push_str(&script(&trigger.script));
            } else {
                code.push_str(&sends(&trigger.send));
            }
            if !trigger.speak.trim().is_empty() {
                code.push_str(&format!(
                    "\tspeak(fill({}, m))\n",
                    quoted(trigger.speak.trim())
                ));
            }
            if !trigger.sound.trim().is_empty() {
                code.push_str(&format!("\tplay({})\n", quoted(trigger.sound.trim())));
            }
            if !trigger.panel.trim().is_empty() {
                code.push_str(&format!("\tto_panel({})\n", quoted(trigger.panel.trim())));
            }
            code.push_str(&format!("end{})\n", grouped(&trigger.group)));
            chunks.push((titled("trigger", &trigger.name, &trigger.pattern), code));
        }
        for alias in
            (self.aliases.iter()).filter(|rule| rule.enabled && !rule.pattern.trim().is_empty())
        {
            let action = if alias.lua {
                script(&alias.script)
            } else {
                sends(&alias.send)
            };
            let code = format!(
                "{PRELUDE}alias({}, function(m, input)\n{action}end{})\n",
                quoted(&alias.regex_source()),
                grouped(&alias.group)
            );
            chunks.push((titled("alias", &alias.name, &alias.pattern), code));
        }
        for key in (self.keys.iter()).filter(|rule| rule.enabled && !rule.key.trim().is_empty()) {
            let name = quoted(key.key.trim());
            let code = if key.lua {
                format!(
                    "bind({name}, function()\n{}end{})\n",
                    script(&key.script),
                    grouped(&key.group)
                )
            } else {
                // What it sends goes through aliases, as if typed.
                let typed = commands(&key.send).collect::<Vec<_>>().join(";");
                format!("bind({name}, {}{})\n", quoted(&typed), grouped(&key.group))
            };
            chunks.push((titled("macro", &key.name, &key.key), code));
        }
        for timer in (self.timers.iter()).filter(|rule| rule.enabled && rule.tenths > 0) {
            let action = if timer.lua {
                script(&timer.script)
            } else {
                commands(&timer.send)
                    .map(|command| format!("\tsend(expand({}))\n", quoted(command)))
                    .collect()
            };
            let how = if timer.repeat { "every" } else { "after" };
            let seconds = f64::from(timer.tenths) / 10.0;
            let code = format!(
                "{how}({seconds}, function()\n{action}end{})\n",
                grouped(&timer.group)
            );
            chunks.push((titled("timer", &timer.name, &timer.interval()), code));
        }
        for gauge in
            (self.gauges.iter()).filter(|rule| rule.enabled && !rule.value.trim().is_empty())
        {
            let name = if gauge.name.trim().is_empty() {
                gauge.value.trim()
            } else {
                gauge.name.trim()
            };
            // A number alone has no maximum unless it is given one.
            let max = match (gauge.max.trim(), gauge.kind) {
                ("", GaugeKind::Number) => "0",
                ("", _) => "100",
                (max, _) => max,
            };
            let steps: Vec<String> = (gauge.steps.iter())
                .filter(|step| !step.colour.trim().is_empty())
                .map(|step| format!("{{{}, {}}}", step.under, quoted(step.colour.trim())))
                .collect();
            let look = format!(
                "colour = {}, off = {}, style = {}, steps = {{{}}}",
                quoted(gauge.colour.trim()),
                quoted(gauge.off_colour.trim()),
                quoted(gauge.kind.style()),
                steps.join(", ")
            );
            let shown = match gauge.kind {
                GaugeKind::Light => format!(
                    "\tgauge({name}, lit(raw({value})) and 1 or 0, 1, {{{look}}})\n",
                    name = quoted(name),
                    value = quoted(gauge.value.trim()),
                ),
                GaugeKind::Text => format!(
                    "\tlocal said = raw({value})\n\
                     \tif said ~= nil and type(said) ~= \"table\" then\n\
                     \t\tgauge({name}, 0, 0, {{{look}, text = tostring(said)}})\n\
                     \tend\n",
                    name = quoted(name),
                    value = quoted(gauge.value.trim()),
                ),
                _ => format!(
                    "\tlocal value, max = dig({value}), dig({max})\n\
                     \tif value and max then\n\
                     \t\tgauge({name}, value, max, {{{look}}})\n\
                     \tend\n",
                    name = quoted(name),
                    value = quoted(gauge.value.trim()),
                    max = quoted(max),
                ),
            };
            let root = gauge.value.trim().split('.').next().unwrap_or_default();
            let code = format!(
                "{PRELUDE}local function update()\n{shown}end\n\
                 on_gmcp({root}, update)\n\
                 on_msdp({variable}, update)\n",
                root = quoted(root),
                variable = quoted(gauge.value.trim()),
            );
            chunks.push((titled("gauge", name, ""), code));
        }
        chunks
    }
}

/// `text` as a Luau string literal.
fn quoted(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('"');
    for ch in text.chars() {
        match ch {
            '"' => literal.push_str("\\\""),
            '\\' => literal.push_str("\\\\"),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            ch if ch.is_control() => literal.push_str(&format!("\\u{{{:x}}}", ch as u32)),
            ch => literal.push(ch),
        }
    }
    literal.push('"');
    literal
}

/// `text` as a regular expression matching it literally.
fn regex_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use muddie_script::{Action, Engine};

    use super::*;

    fn run(rules: &Rules) -> Engine {
        let mut engine = Engine::new(Rc::default()).unwrap();
        engine.exec("rules", &rules.to_luau());
        assert_eq!(engine.take_actions(), [], "the generated script failed");
        engine
    }

    fn send(text: &str) -> Action {
        Action::Send(text.into())
    }

    #[test]
    fn triggers_do_what_their_fields_say() {
        let rules = Rules {
            triggers: vec![
                Trigger {
                    enabled: true,
                    pattern: "tells you (what?)".into(),
                    send: "smile; say \"hi\" \\o/".into(),
                    speak: "a tell".into(),
                    sound: "tell.ogg".into(),
                    ..Trigger::default()
                },
                Trigger {
                    enabled: true,
                    pattern: r"^(\w+) attacks (\w+)".into(),
                    regex: true,
                    send: "kill %1".into(),
                    speak: "%1 hits %2%3".into(),
                    highlight: "red".into(),
                    ..Trigger::default()
                },
                Trigger {
                    enabled: true,
                    pattern: "spam".into(),
                    gag: true,
                    ..Trigger::default()
                },
                Trigger {
                    enabled: false,
                    pattern: "spam".into(),
                    send: "never".into(),
                    ..Trigger::default()
                },
                Trigger::default(),
            ],
            ..Rules::default()
        };
        let mut engine = run(&rules);
        // A plain pattern is literal: "(what?)" is not a group.
        assert!(engine.on_line("Ana tells you (what?) now"));
        assert!(engine.on_line("Ana tells you what now"));
        assert_eq!(
            engine.take_actions(),
            [
                send("smile"),
                send("say \"hi\" \\o/"),
                Action::Speak {
                    text: "a tell".into(),
                    interrupt: false
                },
                Action::PlaySound {
                    file: "tell.ogg".into(),
                    volume: 1.0,
                    looped: false
                },
            ]
        );
        assert!(engine.on_line("Orc attacks Ana!"));
        assert_eq!(
            engine.take_actions(),
            [
                send("kill Orc"),
                Action::Speak {
                    text: "Orc hits Ana".into(),
                    interrupt: false
                }
            ]
        );
        assert!(!engine.on_line("buy spam today"));
        assert_eq!(engine.take_actions(), []);
    }

    #[test]
    fn aliases_and_keys() {
        let rules = Rules {
            aliases: vec![
                Alias {
                    enabled: true,
                    pattern: "k".into(),
                    send: "kill %1; loot".into(),
                    ..Alias::default()
                },
                Alias {
                    enabled: true,
                    pattern: r"^g (\w+) (\w+)$".into(),
                    regex: true,
                    send: "give %1 to %2".into(),
                    ..Alias::default()
                },
            ],
            keys: vec![
                KeyBinding {
                    enabled: true,
                    key: "F5".into(),
                    send: "look".into(),
                    ..KeyBinding::default()
                },
                KeyBinding {
                    enabled: false,
                    key: "f6".into(),
                    send: "never".into(),
                    ..KeyBinding::default()
                },
            ],
            ..Rules::default()
        };
        let mut engine = run(&rules);
        for input in ["k orc chief", "k", "kick", "g sword ana"] {
            engine.on_input(input);
        }
        assert_eq!(
            engine.take_actions(),
            [
                send("kill orc chief"),
                send("loot"),
                send("kill "),
                send("loot"),
                send("kick"),
                send("give sword to ana"),
            ]
        );
        assert!(engine.on_key("f5"));
        assert!(!engine.on_key("f6"));
        assert_eq!(engine.take_actions(), [send("look")]);
    }

    #[test]
    fn templates_are_filled_from_a_match() {
        let alias = Alias {
            pattern: "k".into(),
            ..Alias::default()
        };
        let regex = regex::Regex::new(&alias.regex_source()).unwrap();
        let captures = regex.captures("k orc chief").unwrap();
        assert_eq!(
            fill("kill %1; say 100%% of %1 %9", &captures),
            "kill orc chief; say 100%% of orc chief "
        );
        assert!(regex.captures("kick").is_none());
        let trigger = Trigger {
            pattern: "a (b)?".into(),
            send: "x".into(),
            gag: true,
            ..Trigger::default()
        };
        assert_eq!(trigger.regex_source(), "a \\(b\\)\\?");
        assert_eq!(trigger.summary(), "→ x  ·  ∅");
    }

    #[test]
    fn gauges_follow_values_the_server_reports() {
        let gauge = |value: &str, max: &str| Gauge {
            name: "Salud".into(),
            value: value.into(),
            max: max.into(),
            colour: "green".into(),
            ..Gauge::starter()
        };
        let rules = Rules {
            gauges: vec![
                gauge("Char.Vitals.health.current", "Char.Vitals.health.max"),
                Gauge {
                    name: "Oro".into(),
                    ..gauge("Player.Status.gold", "1000")
                },
            ],
            ..Rules::default()
        };
        let mut engine = run(&rules);
        engine.on_gmcp(
            "Char.Vitals",
            r#"{"health": {"current": 25, "max": "100"}}"#,
        );
        let look = muddie_script::GaugeLook {
            colour: "green".into(),
            kind: "cells".into(),
            steps: vec![(30.0, "red".into())],
            ..muddie_script::GaugeLook::default()
        };
        assert_eq!(
            engine.take_actions(),
            [Action::Gauge {
                name: "Salud".into(),
                level: Some((25.0, 100.0)),
                look: look.clone(),
            }]
        );
        // A maximum can be a plain number; a value not there yet shows nothing.
        engine.on_gmcp("Player.Status", r#"{"level": 3}"#);
        assert_eq!(engine.take_actions(), []);
        engine.on_gmcp("Player.Status", r#"{"gold": 40}"#);
        assert_eq!(
            engine.take_actions(),
            [Action::Gauge {
                name: "Oro".into(),
                level: Some((40.0, 1000.0)),
                look,
            }]
        );
    }

    #[test]
    fn rules_run_their_scripts_and_take_commands_on_several_lines() {
        let rules = Rules {
            triggers: vec![Trigger {
                enabled: true,
                pattern: r"^(\w+) attacks".into(),
                regex: true,
                send: "kill %1\nshield; parry".into(),
                script: "if m[1] == \"Orc\" then\n\tsend(\"flee from \" .. line)\nend".into(),
                ..Trigger::default()
            }],
            keys: vec![KeyBinding {
                enabled: true,
                key: "f5".into(),
                send: "look".into(),
                script: "send(\"score\")".into(),
                ..KeyBinding::default()
            }],
            ..Rules::default()
        };
        // Written by an earlier version, with commands and a script both:
        // the commands become the start of the script.
        let mut rules = rules;
        rules.migrate();
        let trigger = &rules.triggers[0];
        assert!(trigger.lua && trigger.send.is_empty());
        assert!(
            trigger
                .script
                .starts_with("send(\"kill \" .. (m[1] or \"\"))\nsend(\"shield\")\n")
        );
        let mut engine = run(&rules);
        engine.on_line("Orc attacks");
        let expected = ["kill Orc", "shield", "parry", "flee from Orc attacks"].map(send);
        assert_eq!(engine.take_actions(), expected);
        assert!(engine.on_key("f5"));
        assert_eq!(engine.take_actions(), ["look", "score"].map(send));
        // A rule does one or the other: with the script chosen, commands
        // left in the other form are not sent.
        let mut either = rules.clone();
        either.triggers[0].send = "never sent".into();
        let mut engine = run(&either);
        engine.on_line("Orc attacks");
        assert_eq!(engine.take_actions(), expected);
        // A rule whose script is wrong is reported by name, and alone.
        let broken = Rules {
            triggers: vec![Trigger {
                name: "Broken".into(),
                enabled: true,
                pattern: "x".into(),
                script: "this is not Luau".into(),
                lua: true,
                ..Trigger::default()
            }],
            ..rules
        };
        let chunks = broken.chunks();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, "trigger Broken");
    }

    #[test]
    fn timers_go_off_once_or_again_and_again() {
        use std::time::{Duration, Instant};
        let rules = Rules {
            timers: vec![
                Timer {
                    tenths: 20,
                    send: "save\nscore".into(),
                    ..Timer::default()
                },
                Timer {
                    tenths: 30,
                    repeat: false,
                    lua: true,
                    script: "send(\"once\")".into(),
                    ..Timer::default()
                },
                Timer {
                    enabled: false,
                    send: "never".into(),
                    ..Timer::default()
                },
            ],
            ..Rules::default()
        };
        let mut engine = run(&rules);
        let start = Instant::now();
        engine.tick(start + Duration::from_millis(2100));
        assert_eq!(engine.take_actions(), ["save", "score"].map(send));
        engine.tick(start + Duration::from_millis(4200));
        let mut sent = engine.take_actions();
        sent.sort_by_key(|action| format!("{action:?}"));
        assert_eq!(sent, ["once", "save", "score"].map(send));
        engine.tick(start + Duration::from_millis(6300));
        assert_eq!(engine.take_actions(), ["save", "score"].map(send));
        assert_eq!(rules.timers[0].interval(), "2 s");
        assert_eq!(Timer::default().interval(), "1 min");
        let long = Timer {
            tenths: 905,
            ..Timer::default()
        };
        assert_eq!(long.interval(), "1 min 30.5 s");
    }

    #[test]
    fn rules_of_a_group_are_switched_together_whatever_their_kind() {
        let mut rules = Rules {
            triggers: vec![Trigger {
                enabled: true,
                pattern: "attacks".into(),
                send: "parry".into(),
                group: "Combat".into(),
                ..Trigger::default()
            }],
            aliases: vec![Alias {
                enabled: true,
                pattern: "k".into(),
                send: "kill %1".into(),
                group: "Combat".into(),
                ..Alias::default()
            }],
            keys: vec![
                KeyBinding {
                    enabled: true,
                    key: "f1".into(),
                    send: "kick".into(),
                    group: "Combat".into(),
                    ..KeyBinding::default()
                },
                KeyBinding {
                    enabled: true,
                    key: "f1".into(),
                    send: "look".into(),
                    group: "Explore".into(),
                    ..KeyBinding::default()
                },
            ],
            ..Rules::default()
        };
        assert_eq!(rules.group_names(), ["Combat", "Explore"]);
        assert!(rules.group_enabled("Combat"));
        // Saved as off, it starts off: nothing of the group takes part.
        rules.set_group_enabled("Combat", false);
        let mut engine = run(&rules);
        engine.on_line("An orc attacks");
        engine.on_input("k orc");
        engine.on_key("f1");
        assert_eq!(engine.take_actions(), ["k orc", "look"].map(send));
        // Switched on for the session, all of it does.
        engine.set_group("Combat", true);
        engine.on_line("An orc attacks");
        engine.on_input("k orc");
        engine.on_key("f1");
        assert_eq!(
            engine.take_actions(),
            ["parry", "kill orc", "kick"].map(send)
        );
        rules.set_group_enabled("Combat", true);
        assert!(
            rules.groups.is_empty(),
            "the usual state is not written down"
        );
    }

    #[test]
    fn commands_of_rules_take_the_values_of_variables() {
        let rules = Rules {
            triggers: vec![Trigger {
                enabled: true,
                pattern: r"^(\w+) arrives".into(),
                regex: true,
                send: "tell $friend %1 is here; $5 stays".into(),
                ..Trigger::default()
            }],
            timers: vec![Timer {
                tenths: 10,
                send: "cast $spell".into(),
                ..Timer::default()
            }],
            ..Rules::default()
        };
        let mut engine = run(&rules);
        engine
            .set_variable("friend", Some(serde_json::json!("Ana")))
            .unwrap();
        engine
            .set_variable("spell", Some(serde_json::json!("shield")))
            .unwrap();
        engine.on_line("Orc arrives");
        assert_eq!(
            engine.take_actions(),
            ["tell Ana Orc is here", "$5 stays"].map(send)
        );
        engine.tick(std::time::Instant::now() + std::time::Duration::from_millis(1100));
        assert_eq!(engine.take_actions(), [send("cast shield")]);
    }

    #[test]
    fn gauges_come_in_kinds_and_change_colour_by_level() {
        let gauge = |name: &str, kind, value: &str| Gauge {
            name: name.into(),
            enabled: true,
            kind,
            value: value.into(),
            ..Gauge::default()
        };
        let rules = Rules {
            gauges: vec![
                gauge("Poisoned", GaugeKind::Light, "Char.Status.poisoned"),
                gauge("Stance", GaugeKind::Text, "Char.Status.stance"),
                gauge("Gold", GaugeKind::Number, "Char.Status.gold"),
            ],
            ..Rules::default()
        };
        let mut engine = run(&rules);
        engine.on_gmcp(
            "Char.Status",
            r#"{"poisoned": "yes", "stance": "defensive", "gold": 40}"#,
        );
        let actions = engine.take_actions();
        type Seen<'a> = (&'a str, Option<(f64, f64)>, &'a str, &'a str);
        let seen: Vec<Seen> = (actions.iter())
            .map(|action| match action {
                Action::Gauge { name, level, look } => (
                    name.as_str(),
                    *level,
                    look.kind.as_str(),
                    look.text.as_str(),
                ),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            seen,
            [
                ("Poisoned", Some((1.0, 1.0)), "light", ""),
                ("Stance", Some((0.0, 0.0)), "text", "defensive"),
                ("Gold", Some((40.0, 0.0)), "number", ""),
            ]
        );
        // What is not so puts the light out, whichever way it is said.
        for off in ["false", "0", "\"no\"", "\"\""] {
            engine.on_gmcp("Char.Status", &format!(r#"{{"poisoned": {off}}}"#));
            let Some(Action::Gauge { level, .. }) = engine.take_actions().into_iter().next() else {
                panic!("no gauge for {off}");
            };
            assert_eq!(level, Some((0.0, 1.0)), "{off}");
        }
        // As earlier versions kept them, they are read and brought up to date.
        let old: Rules =
            toml::from_str("[[gauge]]\nname = \"HP\"\nvalue = \"x\"\nsegments = true\nlow = 25\n")
                .unwrap();
        let mut old = old;
        old.migrate();
        let hp = &old.gauges[0];
        assert_eq!(
            (hp.kind, hp.steps[0].under, hp.steps[0].colour.as_str()),
            (GaugeKind::Cells, 25, "red")
        );
        assert!(!toml::to_string(&old).unwrap().contains("segments"));
    }

    #[test]
    fn rules_round_trip_through_their_file() {
        let path =
            std::env::temp_dir().join(format!("muddie-rules-{}/rules.toml", std::process::id()));
        assert_eq!(Rules::load(&path).unwrap(), Rules::default());
        let rules = Rules {
            triggers: vec![Trigger {
                enabled: true,
                pattern: "a \"quoted\" [pattern]".into(),
                gag: true,
                ..Trigger::default()
            }],
            aliases: vec![Alias {
                enabled: false,
                pattern: "k".into(),
                send: "kill %1".into(),
                ..Alias::default()
            }],
            keys: vec![KeyBinding {
                name: "Kick".into(),
                enabled: true,
                key: "ctrl+k".into(),
                send: "kick".into(),
                ..KeyBinding::default()
            }],
            groups: vec![Group {
                name: "Combat".into(),
                enabled: false,
            }],
            timers: vec![Timer {
                name: "Save".into(),
                tenths: 3000,
                repeat: false,
                send: "save".into(),
                ..Timer::default()
            }],
            gauges: vec![Gauge {
                name: "Salud".into(),
                value: "Char.Vitals.health.current".into(),
                max: "Char.Vitals.health.max".into(),
                colour: "red".into(),
                ..Gauge::starter()
            }],
            scripts_off: vec!["keys.luau".into()],
        };
        rules.save(&path).unwrap();
        assert_eq!(Rules::load(&path).unwrap(), rules);
        // A rule written by hand without `enabled` is on.
        std::fs::write(&path, "[[trigger]]\npattern = \"x\"\n").unwrap();
        assert!(Rules::load(&path).unwrap().triggers[0].enabled);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
