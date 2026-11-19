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
        if !self.send.trim().is_empty() {
            parts.push(format!("→ {}", self.send.trim()));
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
}

impl Rules {
    /// Reads the rules file; a missing one means no rules.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
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

    /// The rules as a Luau script. Disabled rules and those with nothing
    /// to match are left out.
    pub fn to_luau(&self) -> String {
        // `%1`... in a template become the match's groups.
        let mut code = String::from(
            "local function fill(template, m)\n\
             \treturn (template:gsub(\"%%(%d)\", function(n) return m[tonumber(n)] or \"\" end))\n\
             end\n",
        );
        let sends = |commands: &str| -> String {
            (commands
                .split(';')
                .map(str::trim)
                .filter(|command| !command.is_empty()))
            .map(|command| format!("\tsend(fill({}, m))\n", quoted(command)))
            .collect()
        };
        for trigger in self
            .triggers
            .iter()
            .filter(|trigger| trigger.enabled && !trigger.pattern.is_empty())
        {
            code.push_str(&format!(
                "trigger({}, function(m)\n",
                quoted(&trigger.regex_source())
            ));
            if !trigger.highlight.trim().is_empty() {
                code.push_str(&format!(
                    "\thighlight({})\n",
                    quoted(trigger.highlight.trim())
                ));
            }
            if trigger.gag {
                code.push_str("\tgag()\n");
            }
            code.push_str(&sends(&trigger.send));
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
        }
        for alias in self
            .aliases
            .iter()
            .filter(|alias| alias.enabled && !alias.pattern.trim().is_empty())
        {
            code.push_str(&format!(
                "alias({}, function(m)\n{}end)\n",
                quoted(&alias.regex_source()),
                sends(&alias.send)
            ));
        }
        for key in self
            .keys
            .iter()
            .filter(|key| key.enabled && !key.key.trim().is_empty())
        {
            code.push_str(&format!(
                "bind({}, {})\n",
                quoted(key.key.trim()),
                quoted(&key.send)
            ));
        }
        code
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
