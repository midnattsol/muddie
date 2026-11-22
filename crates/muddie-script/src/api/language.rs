//! Text in the language of the interface: a script gives what it shows
//! in each language it has, and the one the user reads is chosen.

use super::*;
use std::sync::RwLock;

/// The language the interface is in, as a code such as `es` or `pt_BR`;
/// empty for English.
static INTERFACE: RwLock<String> = RwLock::new(String::new());

/// Says which language the interface is in, for scripts to show their
/// text in: a code such as `es` or `pt_BR`, `en` or nothing for English.
pub fn set_interface_language(code: &str) {
    *INTERFACE.write().expect("language lock") = code.trim().to_owned();
}

/// Of the texts given by language code, the one in the language of the
/// interface, or the one of its language without the country, or the
/// English one, or else the first by code.
pub(crate) fn in_interface_language(texts: &[(String, String)]) -> Option<String> {
    let code = INTERFACE.read().expect("language lock").clone();
    let base = code.split(['_', '-']).next().unwrap_or_default();
    let found = |wanted: &str| {
        (texts.iter())
            .find(|(known, _)| !wanted.is_empty() && known.eq_ignore_ascii_case(wanted))
            .map(|(_, text)| text.clone())
    };
    (found(&code).or_else(|| found(base)).or_else(|| found("en"))).or_else(|| {
        texts
            .iter()
            .min_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, text)| text.clone())
    })
}

pub(crate) fn install(lua: &Lua) -> mlua::Result<()> {
    lua.globals().set(
        "tr",
        lua.create_function(|_, texts: Value| match texts {
            Value::String(text) => Ok(text.to_str()?.to_owned()),
            Value::Table(texts) => {
                let texts: Vec<(String, String)> = texts
                    .pairs::<String, String>()
                    .collect::<mlua::Result<_>>()?;
                in_interface_language(&texts)
                    .ok_or_else(|| mlua::Error::runtime("tr needs a text in some language"))
            }
            other => Err(mlua::Error::runtime(format!(
                "tr takes the texts by language, as tr{{en = \"Hello\", es = \"Hola\"}}, not {}",
                other.type_name()
            ))),
        })?,
    )
}
