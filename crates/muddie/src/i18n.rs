//! Translation of the interface, the gettext way. Text is written in
//! English in the code and looked up in the catalogue of the language in
//! use, a `.po` file, so an untranslated string simply shows in English.
//!
//! The catalogues that come with the program are built in. Any `.po` file
//! in the `locale` folder of the configuration adds a language, or
//! replaces a built-in one, with no need to build the program again.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};

/// The catalogues built into the program: language code and `.po` text.
const BUILT_IN: [(&str, &str); 1] = [("es", include_str!("../po/es.po"))];

/// The texts of one language.
#[derive(Default)]
struct Catalogue {
    /// What the language calls itself, from the `X-Language-Name` header.
    name: String,
    texts: HashMap<String, &'static str>,
    /// For texts that depend on a number: each form the language has.
    plurals: HashMap<String, Vec<&'static str>>,
    /// Which form goes with a number.
    form: Option<fn(u64) -> usize>,
}

/// The catalogue in use; none for English.
static CURRENT: RwLock<Option<&'static Catalogue>> = RwLock::new(None);
/// Catalogues read so far, by language code. They are never dropped: the
/// interface holds on to their text.
static LOADED: Mutex<Vec<(String, &'static Catalogue)>> = Mutex::new(Vec::new());
/// Where the user's own catalogues are.
static USER_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Says where the user's own `.po` files are looked for.
pub fn set_user_dir(dir: PathBuf) {
    *USER_DIR.write().expect("locale lock") = Some(dir);
}

fn user_file(code: &str) -> Option<PathBuf> {
    let dir = USER_DIR.read().expect("locale lock").clone()?;
    let file = dir.join(format!("{code}.po"));
    file.is_file().then_some(file)
}

/// The languages there are besides English, as code and name, in the
/// order of their codes.
pub fn languages() -> Vec<(String, String)> {
    let mut codes: Vec<String> = BUILT_IN
        .iter()
        .map(|(code, _)| (*code).to_owned())
        .collect();
    if let Some(dir) = USER_DIR.read().expect("locale lock").clone() {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let stem = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned());
            if let (Some(stem), true) = (stem, path.extension().is_some_and(|ext| ext == "po")) {
                codes.push(stem);
            }
        }
    }
    codes.sort();
    codes.dedup();
    let named = codes.into_iter().filter_map(|code| {
        let name = catalogue(&code)?.name.clone();
        Some((code.clone(), if name.is_empty() { code } else { name }))
    });
    named.collect()
}

/// The catalogue of a language, read once.
fn catalogue(code: &str) -> Option<&'static Catalogue> {
    let mut loaded = LOADED.lock().expect("locale lock");
    if let Some((_, catalogue)) = loaded.iter().find(|(known, _)| known == code) {
        return Some(catalogue);
    }
    let text = match user_file(code) {
        Some(file) => std::fs::read_to_string(file).ok()?,
        None => BUILT_IN
            .iter()
            .find(|(known, _)| *known == code)?
            .1
            .to_owned(),
    };
    let catalogue: &'static Catalogue = Box::leak(Box::new(parse(&text, code)));
    loaded.push((code.to_owned(), catalogue));
    Some(catalogue)
}

/// Chooses the language used from now on: a code such as `es` or
/// `pt_BR`, `en` for none, or nothing to follow the system.
pub fn set_language(code: &str) {
    let code = match code.trim() {
        "" => system_language(),
        code => code.to_owned(),
    };
    // `pt_BR` falls back to `pt` if only that is there.
    let base = code.split(['_', '-']).next().unwrap_or_default().to_owned();
    let found = match catalogue(&code) {
        Some(found) => Some((found, code)),
        None => catalogue(&base).map(|found| (found, base)),
    };
    *CURRENT.write().expect("locale lock") = found.as_ref().map(|(found, _)| *found);
    // Scripts show their own text in it too, if they have it.
    let code = found.map_or_else(|| "en".to_owned(), |(_, code)| code);
    muddie_script::set_interface_language(&code);
}

/// The language the system is in, as `LANG` and its like say it.
fn system_language() -> String {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()));
    // `es_ES.UTF-8@euro` is the language `es_ES`.
    let locale = locale.unwrap_or_default();
    locale
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// The text in the current language.
pub fn tr(text: &'static str) -> &'static str {
    let current = *CURRENT.read().expect("locale lock");
    current
        .and_then(|catalogue| catalogue.texts.get(text).copied())
        .unwrap_or(text)
}

/// Like [`tr`] for text with values in it: `{0}`, `{1}`... in the
/// translated text are replaced by `values`.
pub fn trf(text: &'static str, values: &[&str]) -> String {
    fill(tr(text), values)
}

/// Like [`trf`] for text that depends on a number, which is its `{0}`:
/// `singular` is for one, `plural` for any other number, and a language
/// may have more forms than those two, or only one.
pub fn trn(singular: &'static str, plural: &'static str, number: u64) -> String {
    let current = *CURRENT.read().expect("locale lock");
    let translated = current.and_then(|catalogue| {
        let forms = catalogue.plurals.get(singular)?;
        let form = catalogue
            .form
            .map_or(usize::from(number != 1), |form| form(number));
        forms.get(form).or(forms.last()).copied()
    });
    let text = translated.unwrap_or(if number == 1 { singular } else { plural });
    fill(text, &[&number.to_string()])
}

fn fill(text: &str, values: &[&str]) -> String {
    let mut result = text.to_owned();
    for (index, value) in values.iter().enumerate() {
        result = result.replace(&format!("{{{index}}}"), value);
    }
    result
}

/// Which plural form goes with a number in the language `code`, for the
/// families of languages whose rule is not "one, or any other number".
fn plural_form(code: &str) -> Option<fn(u64) -> usize> {
    let language = code.split(['_', '-']).next().unwrap_or(code);
    let slavic = |n: u64| match (n % 10, n % 100) {
        (1, form) if form != 11 => 0,
        (2..=4, form) if !(12..=14).contains(&form) => 1,
        _ => 2,
    };
    Some(match language {
        // No plural at all.
        "ja" | "zh" | "ko" | "vi" | "th" | "id" | "tr" => |_| 0,
        // Zero is singular too.
        "fr" => |n| usize::from(n > 1),
        "ru" | "uk" | "sr" | "hr" | "be" | "bs" => slavic,
        "pl" => |n| match (n, n % 10, n % 100) {
            (1, ..) => 0,
            (_, 2..=4, form) if !(12..=14).contains(&form) => 1,
            _ => 2,
        },
        "cs" | "sk" => |n| match n {
            1 => 0,
            2..=4 => 1,
            _ => 2,
        },
        _ => return None,
    })
}

/// Reads a `.po` file.
fn parse(text: &str, code: &str) -> Catalogue {
    /// Which part of an entry the lines being read belong to.
    #[derive(Clone, Copy, PartialEq)]
    enum Part {
        None,
        Id,
        Plural,
        Text(usize),
        Ignored,
    }
    let unquote = |quoted: &str| -> String {
        let inner = quoted.trim().trim_start_matches('"');
        let inner = inner.strip_suffix('"').unwrap_or(inner);
        let mut text = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(ch) = chars.next() {
            match (ch, ch == '\\').1.then(|| chars.next()).flatten() {
                Some('n') => text.push('\n'),
                Some('t') => text.push('\t'),
                Some(escaped) => text.push(escaped),
                None => text.push(ch),
            }
        }
        text
    };
    let leak = |text: &str| -> &'static str { Box::leak(text.to_owned().into_boxed_str()) };
    let mut catalogue = Catalogue {
        form: plural_form(code),
        ..Catalogue::default()
    };
    let (mut id, mut texts): (String, Vec<String>) = (String::new(), Vec::new());
    let mut has_plural = false;
    let mut part = Part::None;
    let mut finish = |id: &mut String, texts: &mut Vec<String>, has_plural: &mut bool| {
        let translated = texts.first().is_some_and(|text| !text.is_empty());
        if id.is_empty() {
            // The header: one `Name: value` per line.
            let header = texts.first().map(String::as_str).unwrap_or_default();
            let named = header
                .lines()
                .find_map(|line| line.strip_prefix("X-Language-Name:"));
            catalogue.name = named.unwrap_or_default().trim().to_owned();
        } else if *has_plural && translated {
            let forms = texts.iter().map(|text| leak(text)).collect();
            catalogue.plurals.insert(id.clone(), forms);
        } else if translated {
            catalogue.texts.insert(id.clone(), leak(&texts[0]));
        }
        id.clear();
        texts.clear();
        *has_plural = false;
    };
    let mut started = false;
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("msgid ") {
            if started {
                finish(&mut id, &mut texts, &mut has_plural);
            }
            started = true;
            id = unquote(rest);
            part = Part::Id;
        } else if line.strip_prefix("msgid_plural ").is_some() {
            has_plural = true;
            part = Part::Plural;
        } else if let Some(rest) = line.strip_prefix("msgstr[") {
            let (index, rest) = rest.split_once(']').unwrap_or(("0", rest));
            let index = index.parse().unwrap_or(0);
            texts.resize(texts.len().max(index + 1), String::new());
            texts[index] = unquote(rest);
            part = Part::Text(index);
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            texts = vec![unquote(rest)];
            part = Part::Text(0);
        } else if line.starts_with("msgctxt ") {
            part = Part::Ignored;
        } else if line.starts_with('"') {
            // A string carried over from the line before.
            match part {
                Part::Id => id.push_str(&unquote(line)),
                Part::Text(index) => texts[index].push_str(&unquote(line)),
                Part::Plural | Part::Ignored | Part::None => {}
            }
        }
    }
    if started {
        finish(&mut id, &mut texts, &mut has_plural);
    }
    catalogue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogues_are_read_with_their_escapes_plurals_and_header() {
        let po = r#"
# A comment.
msgid ""
msgstr ""
"Language: xx\n"
"X-Language-Name: Test\n"

msgid "Save"
msgstr "Guardar"

#, python-brace-format
msgid ""
"Two\n"
"lines of \"{0}\""
msgstr ""
"Dos\n"
"líneas de \"{0}\""

msgid "Not translated"
msgstr ""

msgid "{0} room"
msgid_plural "{0} rooms"
msgstr[0] "{0} sala"
msgstr[1] "{0} salas"
"#;
        let catalogue = parse(po, "xx");
        assert_eq!(catalogue.name, "Test");
        assert_eq!(catalogue.texts["Save"], "Guardar");
        assert_eq!(
            catalogue.texts["Two\nlines of \"{0}\""],
            "Dos\nlíneas de \"{0}\""
        );
        // An empty translation is no translation: English shows instead.
        assert!(!catalogue.texts.contains_key("Not translated"));
        assert_eq!(catalogue.plurals["{0} room"], ["{0} sala", "{0} salas"]);
    }

    #[test]
    fn numbers_take_the_form_their_language_gives_them() {
        let russian = plural_form("ru_RU").unwrap();
        let forms: Vec<usize> = [1, 2, 5, 11, 21, 22, 25, 111].map(russian).to_vec();
        assert_eq!(forms, [0, 1, 2, 2, 0, 1, 2, 2]);
        assert_eq!([0, 1, 2].map(plural_form("fr").unwrap()), [0, 0, 1]);
        assert_eq!(plural_form("ja").unwrap()(7), 0);
        assert!(plural_form("es").is_none());
        // English, with no catalogue: one or many.
        set_language("en");
        assert_eq!(trn("{0} room", "{0} rooms", 1), "1 room");
        assert_eq!(trn("{0} room", "{0} rooms", 3), "3 rooms");
    }

    #[test]
    fn values_are_put_in_place() {
        assert_eq!(fill("{1} before {0}", &["a", "b"]), "b before a");
    }

    /// The texts of the interface: every string given to `tr`, `trf` or
    /// `trn` in the sources, outside their tests.
    fn texts_in_the_sources() -> Vec<String> {
        fn files(dir: &std::path::Path, found: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files(&path, found);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    found.push(path);
                }
            }
        }
        let mut sources = Vec::new();
        files(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut sources,
        );
        let call = regex::Regex::new(r#"\btr[fn]?\(\s*"((?:[^"\\]|\\.)*)""#).unwrap();
        let mut texts = Vec::new();
        for file in sources {
            let source = std::fs::read_to_string(&file).unwrap();
            let code = source.split("#[cfg(test)]\nmod tests").next().unwrap();
            for found in call.captures_iter(code) {
                // As the compiler reads the literal.
                let text = found[1]
                    .replace("\\n", "\n")
                    .replace("\\\"", "\"")
                    .replace("\\\\", "\\");
                if !texts.contains(&text) {
                    texts.push(text);
                }
            }
        }
        texts
    }

    #[test]
    fn built_in_catalogues_are_complete_and_keep_their_placeholders() {
        let texts = texts_in_the_sources();
        assert!(texts.len() > 200, "the sources were not found");
        for (code, po) in BUILT_IN {
            let catalogue = parse(po, code);
            assert!(!catalogue.name.is_empty(), "{code} does not say its name");
            let known = |text: &String| {
                catalogue.texts.contains_key(text) || catalogue.plurals.contains_key(text)
            };
            let missing: Vec<&String> = texts.iter().filter(|text| !known(text)).collect();
            assert!(
                missing.is_empty(),
                "not translated into {code}: {missing:#?}"
            );
            for (english, translated) in &catalogue.texts {
                for index in 0..4 {
                    let placeholder = format!("{{{index}}}");
                    assert_eq!(
                        english.contains(&placeholder),
                        translated.contains(&placeholder),
                        "placeholder {placeholder} differs in {english:?}"
                    );
                }
            }
        }
    }

    /// Writes `po/muddie.pot`, the template translators start a language
    /// from: `cargo test -p muddie template -- --ignored`.
    #[test]
    #[ignore = "writes the translation template"]
    fn template_is_written() {
        let quoted = |text: &str| {
            let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
            let lines: Vec<&str> = escaped.split('\n').collect();
            if lines.len() == 1 {
                return format!("\"{escaped}\"");
            }
            let mut out = String::from("\"\"");
            for (index, line) in lines.iter().enumerate() {
                let end = if index + 1 < lines.len() { "\\n" } else { "" };
                if !(line.is_empty() && end.is_empty()) {
                    out.push_str(&format!("\n\"{line}{end}\""));
                }
            }
            out
        };
        let mut pot = String::from(
            "# Translation template of muddie.\n\
             # Copy it to <language code>.po, translate each msgstr, and put the file in\n\
             # the `locale` folder of muddie's configuration, or send it to the project.\n\
             #\n\
             msgid \"\"\n\
             msgstr \"\"\n\
             \"Project-Id-Version: muddie\\n\"\n\
             \"Language: \\n\"\n\
             \"X-Language-Name: \\n\"\n\
             \"MIME-Version: 1.0\\n\"\n\
             \"Content-Type: text/plain; charset=UTF-8\\n\"\n\
             \"Content-Transfer-Encoding: 8bit\\n\"\n\n",
        );
        for text in texts_in_the_sources() {
            if text.contains("{0}") {
                pot.push_str("#, python-brace-format\n");
            }
            pot.push_str(&format!("msgid {}\nmsgstr \"\"\n\n", quoted(&text)));
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("po/muddie.pot");
        std::fs::write(path, pot).unwrap();
    }
}
