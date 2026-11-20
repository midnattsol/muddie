//! Small conversions between what scripts give and what the client uses.

use super::*;

/// Splits typed input into commands at `separator`; a doubled separator
/// stands for the separator itself. Without a separator the input is one
/// command.
pub(super) fn split_commands(input: &str, separator: Option<&str>) -> Vec<String> {
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

/// The name `bind` and [`Engine::on_key`] know a key by: lower case, with
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

/// Reads a colour given by a script: a palette number, `"#rrggbb"`, or a
/// name such as `"red"` or `"bright blue"`. `nil` gives `None`.
pub(super) fn color_from(value: &Value) -> mlua::Result<Option<Color>> {
    const NAMES: [&str; 8] = [
        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
    ];
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
        Value::String(text) => {
            let text = text.to_string_lossy().trim().to_lowercase();
            if let Some(hex) = text.strip_prefix('#') {
                let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
                return match (hex.len(), channel(0), channel(2), channel(4)) {
                    (6, Some(r), Some(g), Some(b)) => Ok(Some(Color::Rgb(r, g, b))),
                    _ => Err(unknown()),
                };
            }
            let (bright, name) = match text.strip_prefix("bright ") {
                Some(name) => (8, name),
                None => (0, text.as_str()),
            };
            let index = NAMES
                .iter()
                .position(|known| *known == name)
                .ok_or_else(unknown)?;
            Ok(Some(Color::Indexed(bright + index as u8)))
        }
        _ => Err(unknown()),
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
