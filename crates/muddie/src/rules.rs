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
    /// Luau to run, with the match as `m` and the line as `line`.
    pub script: String,
    /// What it does is the script, not the commands.
    pub lua: bool,
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
    /// Drawn as a row of cells instead of a continuous bar.
    pub segments: bool,
    /// The percentage under which the numbers turn to a warning colour.
    pub low: u8,
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
    #[serde(rename = "gauge")]
    pub gauges: Vec<Gauge>,
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
	return (template:gsub("%%(%d)", function(n) return m[tonumber(n)] or "" end))
end
local function dig(path)
	local number = tonumber(path)
	if number then return number end
	local parts = string.split(path, ".")
	for n = #parts, 1, -1 do
		local data = gmcp(table.concat(parts, ".", 1, n))
		if data ~= nil then
			for i = n + 1, #parts do
				if type(data) ~= "table" then return nil end
				data = data[parts[i]]
			end
			return tonumber(data)
		end
	end
	return tonumber(msdp(path))
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
        let mut chunks = Vec::new();
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
            code.push_str("end)\n");
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
                "{PRELUDE}alias({}, function(m, input)\n{action}end)\n",
                quoted(&alias.regex_source())
            );
            chunks.push((titled("alias", &alias.name, &alias.pattern), code));
        }
        for key in (self.keys.iter()).filter(|rule| rule.enabled && !rule.key.trim().is_empty()) {
            let name = quoted(key.key.trim());
            let code = if key.lua {
                format!("bind({name}, function()\n{}end)\n", script(&key.script))
            } else {
                // What it sends goes through aliases, as if typed.
                let typed = commands(&key.send).collect::<Vec<_>>().join(";");
                format!("bind({name}, {})\n", quoted(&typed))
            };
            chunks.push((titled("macro", &key.name, &key.key), code));
        }
        for gauge in
            (self.gauges.iter()).filter(|rule| rule.enabled && !rule.value.trim().is_empty())
        {
            let name = if gauge.name.trim().is_empty() {
                gauge.value.trim()
            } else {
                gauge.name.trim()
            };
            let max = if gauge.max.trim().is_empty() {
                "100"
            } else {
                gauge.max.trim()
            };
            let style = if gauge.segments { "segments" } else { "bar" };
            let root = gauge.value.trim().split('.').next().unwrap_or_default();
            let code = format!(
                "{PRELUDE}local function update()\n\
                 \tlocal value, max = dig({value}), dig({max})\n\
                 \tif value and max then\n\
                 \t\tgauge({name}, value, max, {{colour = {colour}, style = {style}, low = {low}}})\n\
                 \tend\n\
                 end\n\
                 on_gmcp({root}, update)\n\
                 on_msdp({variable}, update)\n",
                value = quoted(gauge.value.trim()),
                max = quoted(max),
                name = quoted(name),
                colour = quoted(gauge.colour.trim()),
                style = quoted(style),
                low = gauge.low,
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
            enabled: true,
            value: value.into(),
            max: max.into(),
            colour: "red".into(),
            segments: true,
            low: 30,
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
            colour: "red".into(),
            segments: true,
            low: 30.0,
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
            gauges: vec![Gauge {
                name: "Salud".into(),
                enabled: true,
                value: "Char.Vitals.health.current".into(),
                max: "Char.Vitals.health.max".into(),
                colour: "red".into(),
                segments: true,
                low: 30,
            }],
        };
        rules.save(&path).unwrap();
        assert_eq!(Rules::load(&path).unwrap(), rules);
        // A rule written by hand without `enabled` is on.
        std::fs::write(&path, "[[trigger]]\npattern = \"x\"\n").unwrap();
        assert!(Rules::load(&path).unwrap().triggers[0].enabled);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
