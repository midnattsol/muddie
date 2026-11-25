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
pub fn in_interface_language(texts: &[(String, String)]) -> Option<String> {
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

/// A text a script gives, as one string or by language code, as
/// `{en = "Map", es = "Mapa"}`: each language's, in order of code; a
/// string alone is under no code.
pub(crate) fn texts_of(value: Value) -> mlua::Result<Vec<(String, String)>> {
    let mut texts: Vec<(String, String)> = match value {
        Value::String(text) => vec![(String::new(), text.to_str()?.trim().to_owned())],
        Value::Table(texts) => (texts.pairs::<String, String>())
            .map(|pair| pair.map(|(code, text)| (code, text.trim().to_owned())))
            .collect::<mlua::Result<_>>()?,
        other => {
            return Err(mlua::Error::runtime(format!(
                "a text is a string, or the texts by language, as {{en = \"Hello\", es = \"Hola\"}}, not {}",
                other.type_name()
            )));
        }
    };
    texts.retain(|(_, text)| !text.is_empty());
    texts.sort();
    Ok(texts)
}

/// A text a script tells the user: one, a line, or the texts by language,
/// which is the one in the language of the interface.
pub(super) struct Told(pub(super) String);

impl mlua::FromLua for Told {
    fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::Table(_) => in_interface_language(&texts_of(value)?)
                .map(Self)
                .ok_or_else(|| mlua::Error::runtime("there is no text in any language")),
            value => Text::from_lua(value, lua).map(|text| Self(text.0)),
        }
    }
}

pub(crate) fn install(lua: &Lua) -> mlua::Result<()> {
    lua.globals().set(
        "tr",
        lua.create_function(|_, texts: Value| {
            in_interface_language(&texts_of(texts)?)
                .ok_or_else(|| mlua::Error::runtime("tr needs a text in some language"))
        })?,
    )
}
