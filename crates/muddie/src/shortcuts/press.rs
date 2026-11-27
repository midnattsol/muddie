//! A press of keys, as a shortcut is pressed with: what is held, and the
//! key; how it is written to be kept, read back from what a player or a
//! script writes, and shown.

use eframe::egui::{Key, Modifiers};

use super::MAC;
use crate::i18n::{tr, trf};

/// The keys held with another: Ctrl, Alt (Option on a Mac), Shift and Cmd,
/// as the keyboard has them, Cmd only on a Mac.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord)]
pub struct Held {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
}

impl Held {
    pub const NONE: Held = Held {
        ctrl: false,
        alt: false,
        shift: false,
        cmd: false,
    };
    pub const CTRL: Held = Held {
        ctrl: true,
        ..Held::NONE
    };
    pub const ALT: Held = Held {
        alt: true,
        ..Held::NONE
    };
    pub const SHIFT: Held = Held {
        shift: true,
        ..Held::NONE
    };
    pub const CMD: Held = Held {
        cmd: true,
        ..Held::NONE
    };

    /// These, and Shift too.
    pub const fn and_shift(self) -> Held {
        Held {
            shift: true,
            ..self
        }
    }

    /// What egui says is held, as the keyboard has it.
    pub fn of(modifiers: Modifiers) -> Held {
        Held {
            ctrl: modifiers.ctrl || (modifiers.command && !MAC),
            alt: modifiers.alt,
            shift: modifiers.shift,
            cmd: modifiers.mac_cmd || (modifiers.command && MAC),
        }
    }

    /// As egui says keys are held.
    pub fn modifiers(self) -> Modifiers {
        Modifiers {
            alt: self.alt,
            ctrl: self.ctrl,
            shift: self.shift,
            mac_cmd: self.cmd,
            command: (self.ctrl && !MAC) || (self.cmd && MAC),
        }
    }

    fn any_but_shift(self) -> bool {
        self.ctrl || self.alt || self.cmd
    }
}

/// The key of a press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressed {
    /// A key of the main block, as egui knows it.
    Key(Key),
    /// A key of the numeric keypad, by its name: `numpad8`, `numpadup`.
    Keypad(&'static str),
    /// Nothing but what is held, as Ctrl alone silences speech.
    Alone,
}

/// Keys pressed together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    pub held: Held,
    pub key: Pressed,
}

/// A key with these held.
pub const fn press(held: Held, key: Key) -> Press {
    Press {
        held,
        key: Pressed::Key(key),
    }
}

/// A key of the keypad, nothing held.
pub const fn keypad(name: &'static str) -> Press {
    Press {
        held: Held::NONE,
        key: Pressed::Keypad(name),
    }
}

/// What is wrong with keys as a shortcut is to be pressed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrong {
    /// They are not keys there are.
    Unknown,
    /// They would type, or move the cursor, as typing in the command line.
    Types,
    /// They go through the window (Tab, Escape, the menu), always.
    Kept,
    /// Held alone, but this shortcut is pressed with a key.
    Alone,
}

impl Wrong {
    /// What is wrong, in the language of the interface.
    pub fn said(self) -> &'static str {
        match self {
            Wrong::Unknown => tr("Those are not keys Muddie knows."),
            Wrong::Types => tr(
                "Those keys type: hold Ctrl or Alt with them, or use a function key or the keypad.",
            ),
            Wrong::Kept => tr("Tab, Escape and the menu keys always go through the window."),
            Wrong::Alone => tr("Hold a key with them."),
        }
    }

    /// What is wrong, as a script is told: in English, as its errors are.
    pub fn for_scripts(self) -> &'static str {
        match self {
            Wrong::Unknown => {
                "is not a key there is: a key is named as in macro, as ctrl+shift+k, f5, up or numpad8"
            }
            Wrong::Types => {
                "would type: here a key needs ctrl, alt or cmd held, or is f1 to f35 or of the keypad"
            }
            Wrong::Kept => "always goes through the window: tab, escape and the menu keys stay",
            Wrong::Alone => "is only held: this shortcut needs a key with it",
        }
    }
}

impl Press {
    /// Reads keys as a player or a script writes them, as macros name
    /// them: `ctrl+shift+k`, `alt+up`, `f5`, `numpad8`, `space`; `ctrl`
    /// alone. Letter case does not matter, and `option` is Alt.
    pub fn parse(text: &str) -> Option<Press> {
        let lower = text.trim().to_lowercase();
        let mut parts: Vec<&str> = lower.split('+').map(str::trim).collect();
        // A + alone is the key named so: "ctrl++".
        let mut name = parts.pop()?;
        if name.is_empty() && parts.last() == Some(&"") {
            parts.pop();
            name = "plus";
        }
        let mut held = Held::NONE;
        for part in parts {
            hold(&mut held, part)?;
        }
        let mut alone = Held::NONE;
        if hold(&mut alone, name).is_some() {
            if held != Held::NONE {
                return None;
            }
            return Some(Press {
                held: alone,
                key: Pressed::Alone,
            });
        }
        if let Some(found) = crate::keypad::names().find(|known| *known == name) {
            return Some(Press {
                held,
                key: Pressed::Keypad(found),
            });
        }
        Some(press(held, key_named(name)?))
    }

    /// How the keys are written to be kept, and given to scripts: as
    /// [`Press::parse`] reads them, in lower case.
    pub fn stored(&self) -> String {
        let mut text = String::new();
        for (down, name) in self.held_names() {
            if down {
                text.push_str(name);
                text.push('+');
            }
        }
        match self.key {
            Pressed::Key(key) => text + &key.name().to_lowercase(),
            Pressed::Keypad(name) => text + name,
            Pressed::Alone => {
                text.pop();
                text
            }
        }
    }

    /// How the keys are written to be read: `Ctrl+Alt+K`, `Alt+↑`, `Keypad
    /// 8`, with Option and Cmd on a Mac.
    pub fn written(&self) -> String {
        let alt = if MAC { "Option" } else { "Alt" };
        let held = self.held;
        let names = [
            (held.ctrl, "Ctrl"),
            (held.alt, alt),
            (held.shift, "Shift"),
            (held.cmd, "Cmd"),
        ];
        let mut parts: Vec<String> = (names.iter())
            .filter(|(down, _)| *down)
            .map(|(_, name)| (*name).to_owned())
            .collect();
        match self.key {
            Pressed::Key(key) => parts.push(key_written(key).to_owned()),
            Pressed::Keypad(name) => parts.push(keypad_written(name)),
            Pressed::Alone => {}
        }
        parts.join("+")
    }

    /// Whether, pressed, the keys type a character.
    pub fn types(&self) -> bool {
        match self.key {
            Pressed::Key(key) => !self.held.any_but_shift() && types_alone(key),
            _ => false,
        }
    }

    fn held_names(&self) -> [(bool, &'static str); 4] {
        let held = self.held;
        [
            (held.ctrl, "ctrl"),
            (held.alt, "alt"),
            (held.shift, "shift"),
            (held.cmd, "cmd"),
        ]
    }

    /// Whether the keys can be pressed for a shortcut that works where
    /// `typing` says: anywhere, over what is typed, they must not type nor
    /// move the cursor; in a place of its own, as the map, any key but
    /// those that go through the window will do.
    pub fn check(&self, typing: Typing, alone: bool) -> Result<(), Wrong> {
        let held = self.held;
        let key = match self.key {
            Pressed::Alone if alone => return Ok(()),
            Pressed::Alone => return Err(Wrong::Alone),
            Pressed::Keypad(_) => return Ok(()),
            Pressed::Key(key) => key,
        };
        let kept = matches!(key, Key::Tab | Key::Escape) && !held.any_but_shift()
            || (key == Key::F10 && held == Held::SHIFT);
        if kept {
            return Err(Wrong::Kept);
        }
        if held.any_but_shift() || is_function_key(key) {
            return Ok(());
        }
        let types = types_alone(key);
        match typing {
            Typing::Anywhere => Err(Wrong::Types),
            Typing::Line | Typing::List if types => Err(Wrong::Types),
            Typing::Line if key == Key::Enter => Err(Wrong::Kept),
            Typing::Line | Typing::List | Typing::Own => Ok(()),
        }
    }
}

/// Where a shortcut works, by what else its keys could do there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typing {
    /// Anywhere, the command line too: its keys must not type nor move
    /// the cursor.
    Anywhere,
    /// In the command line: keys that do not type, as the arrows.
    Line,
    /// In a list, as the editor's: keys that do not type.
    List,
    /// In a place that has the keyboard to itself, as the map explored:
    /// any key.
    Own,
}

/// Notes a held key named `name`; none if it is not one.
fn hold(held: &mut Held, name: &str) -> Option<()> {
    match name {
        "ctrl" | "control" => held.ctrl = true,
        "alt" | "option" => held.alt = true,
        "shift" => held.shift = true,
        "cmd" | "command" | "super" | "meta" => held.cmd = true,
        _ => return None,
    }
    Some(())
}

/// The key named so, in any letter case: as egui names it (`up`,
/// `pageup`, `f5`, `a`, `1`), or by a name players use.
fn key_named(name: &str) -> Option<Key> {
    let name = match name {
        "esc" => "escape",
        "del" => "delete",
        "ins" => "insert",
        "return" => "enter",
        "pgup" => "pageup",
        "pgdn" | "pgdown" => "pagedown",
        "arrowup" => "up",
        "arrowdown" => "down",
        "arrowleft" => "left",
        "arrowright" => "right",
        "," => "comma",
        "." => "period",
        "-" => "minus",
        "/" => "slash",
        ";" => "semicolon",
        "=" => "equals",
        other => other,
    };
    (Key::ALL.iter().copied()).find(|key| key.name().eq_ignore_ascii_case(name))
}

fn is_function_key(key: Key) -> bool {
    let name = key.name();
    name.len() > 1 && name.starts_with('F') && name[1..].parse::<u8>().is_ok()
}

/// Whether the key, pressed alone or with Shift, types a character.
fn types_alone(key: Key) -> bool {
    let name = key.name();
    name.chars().count() == 1
        || key == Key::Space
        || matches!(
            key,
            Key::Colon
                | Key::Comma
                | Key::Minus
                | Key::Period
                | Key::Plus
                | Key::Equals
                | Key::Semicolon
                | Key::Backslash
                | Key::Slash
                | Key::Pipe
                | Key::Questionmark
                | Key::Exclamationmark
                | Key::OpenBracket
                | Key::CloseBracket
                | Key::OpenCurlyBracket
                | Key::CloseCurlyBracket
                | Key::Backtick
                | Key::Quote
        )
}

/// How a key of the main block is written: the arrows as arrows.
fn key_written(key: Key) -> &'static str {
    match key {
        Key::ArrowUp => "↑",
        Key::ArrowDown => "↓",
        Key::ArrowLeft => "←",
        Key::ArrowRight => "→",
        Key::PageUp => tr("Page Up"),
        Key::PageDown => tr("Page Down"),
        Key::Space => tr("Space"),
        Key::Enter => tr("Enter"),
        Key::Backspace => tr("Backspace"),
        Key::Delete => tr("Delete"),
        Key::Home => tr("Home"),
        Key::End => tr("End"),
        other => other.symbol_or_name(),
    }
}

/// How a key of the keypad is written: "Keypad 8", "Keypad ↑".
fn keypad_written(name: &str) -> String {
    let key = name.strip_prefix("numpad").unwrap_or(name);
    let shown = match key {
        "add" => "+",
        "subtract" => "−",
        "multiply" => "*",
        "divide" => "/",
        "decimal" => ".",
        "up" => "↑",
        "down" => "↓",
        "left" => "←",
        "right" => "→",
        "enter" => tr("Enter"),
        "home" => tr("Home"),
        "end" => tr("End"),
        "pageup" => tr("Page Up"),
        "pagedown" => tr("Page Down"),
        "clear" => tr("Clear"),
        "insert" => tr("Insert"),
        "delete" => tr("Delete"),
        digit => digit,
    };
    trf("Keypad {0}", &[shown])
}
