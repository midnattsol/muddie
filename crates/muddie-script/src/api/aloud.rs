//! How what a script says is heard, said the same way everywhere.

use super::*;

/// How it is heard, as `options` say with `aloud` and `always`; `of_gauge`,
/// it may be by its lowest step.
pub(crate) fn aloud_of(options: Option<&Table>, of_gauge: bool) -> mlua::Result<Aloud> {
    let Some(options) = options else {
        return Ok(Aloud::default());
    };
    let key: Option<String> = options.get("aloud")?;
    let how = match key.as_deref().map(str::trim) {
        None => How::Usual,
        Some(key) => How::from_key(key)
            .filter(|how| of_gauge || *how != How::Lowest)
            .ok_or_else(|| {
                let there = match of_gauge {
                    true => "usual, never, wait, lowest or cut",
                    false => "usual, never, wait or cut",
                };
                mlua::Error::runtime(format!("aloud is {there}, not {key}"))
            })?,
    };
    Ok(Aloud {
        how,
        always: options.get::<Option<bool>>("always")?.unwrap_or_default(),
    })
}

/// The options given after a text. A number there is none of them but how
/// many `text:gsub` replaced, which comes after the text it returns.
pub(crate) struct AfterText(pub(crate) Option<Table>);

impl mlua::FromLua for AfterText {
    fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::Integer(_) | Value::Number(_) => Ok(Self(None)),
            value => Option::from_lua(value, lua).map(Self),
        }
    }
}
