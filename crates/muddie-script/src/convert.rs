//! Small conversions between what scripts give and what the client uses.

use super::*;

/// Splits typed input into commands at `separator`; a doubled separator
/// stands for the separator itself. Without a separator the input is one
/// command.
pub fn split_commands(input: &str, separator: Option<&str>) -> Vec<String> {
    let Some(separator) = separator.filter(|separator| !separator.is_empty()) else {
        return vec![input.to_owned()];
    };
    let mut commands = vec![String::new()];
    let mut rest = input;
    while let Some(at) = rest.find(separator) {
        let current = commands.last_mut().expect("never empty");
        current.push_str(&rest[..at]);
        rest = &rest[at + separator.len()..];
        if let Some(after) = rest.strip_prefix(separator) {
            current.push_str(separator);
            rest = after;
        } else {
            commands.push(String::new());
        }
    }
    commands.last_mut().expect("never empty").push_str(rest);
    commands
        .iter()
        .map(|command| command.trim().to_owned())
        .collect()
}

/// Whether `name` is safe to look up as a module: a relative path of plain
/// names, so it cannot reach outside the script folders.
pub(super) fn valid_module_name(name: &str) -> bool {
    let plain = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    };
    name.split('/').all(plain)
}

/// The name `macro` and [`Engine::on_key`] know a key by: lower case, with
/// its modifiers in the order ctrl, alt, shift, as in `ctrl+shift+f5`.
pub fn key_name(key: &str) -> String {
    let lower = key.to_lowercase();
    let mut parts: Vec<&str> = lower.split('+').map(str::trim).collect();
    let key = parts.pop().unwrap_or_default();
    let mut name = String::new();
    for modifier in ["ctrl", "alt", "shift"] {
        if parts.contains(&modifier) {
            name.push_str(modifier);
            name.push('+');
        }
    }
    name + key
}

/// The keys of the main block that have a name of several letters, as the
/// frontend gives it, in lower case.
const NAMED_KEYS: &str = "down left right up escape tab backspace enter insert delete home end pageup \
    pagedown copy cut paste space colon comma minus period plus equals semicolon \
    backslash slash pipe questionmark exclamationmark openbracket closebracket \
    opencurlybracket closecurlybracket backtick quote browserback browserforward \
    browserrefresh browsersearch browserhome browserfavorites browserstop \
    mediaplaypause mediatracknext mediatrackprevious mediastop audiovolumemute \
    audiovolumedown audiovolumeup launchmail launchapp1 launchapp2 shiftleft \
    shiftright controlleft controlright altleft altright superleft superright \
    intlbackslash capslock numlock scrolllock printscreen pause menu fn eject help \
    power sleep clear";

/// The keys of the numeric keypad, after `numpad`.
const KEYPAD: &str = "0 1 2 3 4 5 6 7 8 9 add subtract multiply divide decimal enter up down left \
    right home end pageup pagedown clear insert delete";

/// Whether a key, named as in `macro`, can do something when it is pressed:
/// what is wrong with it if not.
pub fn bindable(key: &str) -> Result<(), String> {
    let lower = key.to_lowercase();
    let mut parts: Vec<&str> = lower.split('+').map(str::trim).collect();
    let base = parts.pop().unwrap_or_default();
    if let Some(wrong) = (parts.iter()).find(|part| !["ctrl", "alt", "shift"].contains(part)) {
        return Err(format!(
            "{key}: there is no {wrong} to hold; there are ctrl, alt and shift"
        ));
    }
    let among = |names: &str, name: &str| names.split_whitespace().any(|known| known == name);
    let function = (base.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()))
        .is_some_and(|n| (1..=35).contains(&n));
    let keypad = (base.strip_prefix("numpad")).is_some_and(|name| among(KEYPAD, name));
    if function || keypad {
        return Ok(());
    }
    let letter = base.chars().count() == 1;
    if !letter && !among(NAMED_KEYS, base) {
        return Err(format!("there is no key {base}, in {key}"));
    }
    match parts.contains(&"ctrl") || parts.contains(&"alt") {
        true => Ok(()),
        false => Err(format!(
            "{key} would be typed: but for f1 to f35 and the keypad, a key needs ctrl or alt"
        )),
    }
}

/// The names of the sixteen colours of the palette, each also `bright`.
pub const COLOUR_NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

/// Reads a colour given as text, as scripts and rules give them: a name
/// such as `"red"` or `"bright blue"`, `"#rrggbb"`, or a palette number.
pub fn colour_named(text: &str) -> Option<Color> {
    let text = text.trim().to_lowercase();
    if let Some(hex) = text.strip_prefix('#') {
        let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
        return match (hex.len(), channel(0), channel(2), channel(4)) {
            (6, Some(r), Some(g), Some(b)) => Some(Color::Rgb(r, g, b)),
            _ => None,
        };
    }
    if let Ok(index) = text.parse::<u8>() {
        return Some(Color::Indexed(index));
    }
    let (bright, name) = match text.strip_prefix("bright ") {
        Some(name) => (8, name),
        None => (0, text.as_str()),
    };
    let index = COLOUR_NAMES.iter().position(|known| *known == name)?;
    Some(Color::Indexed(bright + index as u8))
}

/// Reads a colour given by a script, as [`colour_named`] does, or as a
/// palette number. `nil` gives `None`.
pub(super) fn color_from(value: &Value) -> mlua::Result<Option<Color>> {
    let unknown =
        || mlua::Error::runtime("a colour is a name, \"#rrggbb\" or a number from 0 to 255");
    match value {
        Value::Nil => Ok(None),
        Value::Integer(index) => Ok(Some(Color::Indexed(
            u8::try_from(*index).map_err(|_| unknown())?,
        ))),
        Value::Number(index) if index.fract() == 0.0 && (0.0..=255.0).contains(index) => {
            Ok(Some(Color::Indexed(*index as u8)))
        }
        Value::String(text) => (colour_named(&text.to_string_lossy()))
            .map(Some)
            .ok_or_else(unknown),
        _ => Err(unknown()),
    }
}

/// The red, green and blue of a number of the 256-colour palette, the
/// first sixteen in their standard colours.
pub(super) fn indexed_rgb(index: u8) -> muddie_mapper::Colour {
    match Color::xterm_rgb(index) {
        Some((red, green, blue)) => [red, green, blue],
        None => muddie_mapper::STANDARD_COLOURS[usize::from(index)],
    }
}

/// A colour as scripts see it: a palette number, `"#rrggbb"`, or nil.
pub(super) fn color_value(lua: &Lua, color: Color) -> mlua::Result<Value> {
    match color {
        Color::Default => Ok(Value::Nil),
        Color::Indexed(index) => Ok(Value::Integer(index.into())),
        Color::Rgb(r, g, b) => Ok(Value::String(
            lua.create_string(format!("#{r:02x}{g:02x}{b:02x}"))?,
        )),
    }
}

/// The error as a script author wants to read it: the message with its
/// `file:line`, without mlua's wrapping or the stack traceback.
pub(super) fn describe(error: &mlua::Error) -> String {
    match error {
        mlua::Error::CallbackError { cause, .. } => describe(cause),
        mlua::Error::RuntimeError(message) => {
            let message = message.split("\nstack traceback:").next();
            message.unwrap_or_default().to_owned()
        }
        other => other.to_string(),
    }
}
