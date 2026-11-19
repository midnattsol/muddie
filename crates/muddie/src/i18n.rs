//! Translation of the interface. Text is written in English in the code
//! and looked up here, so an untranslated string simply shows in English.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::settings::Language;

static SPANISH: AtomicBool = AtomicBool::new(false);

/// Chooses the language used from now on.
pub fn set_language(language: Language) {
    let spanish = match language {
        Language::Spanish => true,
        Language::English => false,
        Language::Auto => system_language_is_spanish(),
    };
    SPANISH.store(spanish, Ordering::Relaxed);
}

fn system_language_is_spanish() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .is_some_and(|locale| locale.to_lowercase().starts_with("es"))
}

/// The text in the current language.
pub fn tr(text: &'static str) -> &'static str {
    if !SPANISH.load(Ordering::Relaxed) {
        return text;
    }
    static TABLE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    let table = TABLE.get_or_init(|| crate::i18n_es::SPANISH.iter().copied().collect());
    table.get(text).copied().unwrap_or(text)
}

/// Like [`tr`] for text with values in it: `{0}`, `{1}`... in the
/// translated text are replaced by `values`.
pub fn trf(text: &'static str, values: &[&str]) -> String {
    let mut result = tr(text).to_owned();
    for (index, value) in values.iter().enumerate() {
        result = result.replace(&format!("{{{index}}}"), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_translation_keeps_its_placeholders() {
        for (english, spanish) in crate::i18n_es::SPANISH {
            for index in 0..4 {
                let placeholder = format!("{{{index}}}");
                assert_eq!(
                    english.contains(&placeholder),
                    spanish.contains(&placeholder),
                    "placeholder {placeholder} differs in {english:?}"
                );
            }
        }
    }

    #[test]
    fn values_are_put_in_place() {
        assert_eq!(trf("{1} before {0}", &["a", "b"]), "b before a");
    }
}
