//! The documents of scripting and of plugins, held to what Muddie is: each
//! example runs as it is written, each file of a plugin shown reads as Muddie
//! reads it, each link inside them leads somewhere, and the Spanish of each
//! has the same code as its English.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

/// The documents, by their names, in English and in Spanish where there is
/// a Spanish one.
const DOCS: [(&str, &str); 6] = [
    ("scripting.md", include_str!("../../../docs/scripting.md")),
    ("plugins.md", include_str!("../../../docs/plugins.md")),
    (
        "scripting.es.md",
        include_str!("../../../docs/scripting.es.md"),
    ),
    ("plugins.es.md", include_str!("../../../docs/plugins.es.md")),
    (
        "first-scripts.md",
        include_str!("../../../docs/first-scripts.md"),
    ),
    (
        "first-scripts.es.md",
        include_str!("../../../docs/first-scripts.es.md"),
    ),
];

/// The fenced blocks of a document, each with what they are written in.
fn blocks(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut open: Option<(String, String)> = None;
    for line in text.lines() {
        let fence = line.trim_start().strip_prefix("```");
        match (&mut open, fence) {
            (None, Some(language)) => open = Some((language.trim().to_owned(), String::new())),
            (Some(_), Some(_)) => found.extend(open.take()),
            (Some((_, code)), None) => {
                // As it is in the document, without its indent in a list.
                code.push_str(line.strip_prefix("  ").unwrap_or(line));
                code.push('\n');
            }
            (None, None) => {}
        }
    }
    found
}

/// The anchor GitHub gives a heading.
fn anchor(heading: &str) -> String {
    let kept = |ch: &char| ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_');
    (heading.to_lowercase().chars())
        .filter(kept)
        .map(|ch| if ch == ' ' { '-' } else { ch })
        .collect()
}

/// The headings of a document, outside its code.
fn headings(text: &str) -> Vec<String> {
    let mut inside = false;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            inside = !inside;
        } else if !inside && line.starts_with('#') {
            found.push(line.trim_start_matches('#').trim().to_owned());
        }
    }
    found
}

fn text_of(name: &str) -> &'static str {
    (DOCS.iter().find(|(known, _)| *known == name)).map_or("", |(_, text)| text)
}

#[test]
fn each_example_of_luau_runs_as_it_is_written() {
    let mut wrong = Vec::new();
    for (name, text) in DOCS {
        for (language, code) in blocks(text) {
            if language != "lua" {
                continue;
            }
            let mut engine = muddie_script::Engine::new(Rc::new(RefCell::new(Default::default())))
                .expect("the engine starts");
            engine.know_shortcuts(
                crate::shortcuts::for_scripts(),
                crate::shortcuts::check_for_scripts,
            );
            if let Some((line, problem)) = engine.check(&code) {
                wrong.push(format!("{name}, line {line}: {problem}\n{code}"));
                continue;
            }
            // What it needs that the document says is beside it.
            engine.offer_module(
                "example/util.luau",
                "return {percent = function() return 1 end}",
            );
            engine.exec_owned("example/example.luau", "example", &code);
            for action in engine.take_actions() {
                if let muddie_script::Action::Error(error) = action {
                    wrong.push(format!("{name}: {error}\n{code}"));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

#[test]
fn each_file_of_a_plugin_shown_reads_as_muddie_reads_it() {
    let mut wrong = Vec::new();
    for (name, text) in DOCS {
        for (language, code) in blocks(text) {
            if language != "toml" {
                continue;
            }
            let read = if code.contains("[[") {
                toml::from_str::<crate::rules::Rules>(&code).map(drop)
            } else if code.contains("port =") {
                toml::from_str::<crate::templates::Template>(&code).map(drop)
            } else {
                toml::from_str::<crate::plugins::Manifest>(&code).map(drop)
            };
            if let Err(error) = read {
                wrong.push(format!("{name}: {error}\n{code}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

#[test]
fn each_link_inside_the_documents_leads_to_a_heading() {
    let mut wrong = Vec::new();
    for (name, text) in DOCS {
        let mut rest = text;
        while let Some(at) = rest.find("](") {
            rest = &rest[at + 2..];
            let Some(end) = rest.find(')') else { break };
            let link = &rest[..end];
            if link.starts_with("http") {
                continue;
            }
            let (file, place) = link.split_once('#').unwrap_or((link, ""));
            let target = if file.is_empty() { name } else { file };
            let known =
                DOCS.iter().any(|(doc, _)| *doc == target) || ["muddie.d.luau"].contains(&target);
            let there = place.is_empty()
                || headings(text_of(target))
                    .iter()
                    .any(|heading| anchor(heading) == place);
            if !known || !there {
                wrong.push(format!("{name}: {link}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:?}");
}

/// The Spanish of a document has its code, and the headings of each function.
#[test]
fn each_spanish_document_has_the_code_of_its_english() {
    for (english, spanish) in [
        ("scripting.md", "scripting.es.md"),
        ("plugins.md", "plugins.es.md"),
        ("first-scripts.md", "first-scripts.es.md"),
    ] {
        let code = |name: &str| blocks(text_of(name));
        assert_eq!(code(english), code(spanish), "{spanish}");
        let of_code = |name: &str| -> BTreeSet<String> {
            (headings(text_of(name)).into_iter())
                .filter(|heading| heading.starts_with('`'))
                .collect()
        };
        assert_eq!(of_code(english), of_code(spanish), "{spanish}");
    }
}

/// Every shortcut is in the reference, in English and in Spanish, with its
/// name and its keys as they are at first. The reference writes them as
/// they are on Linux and Windows, and says how they are on a Mac.
#[test]
#[cfg(not(target_os = "macos"))]
fn the_reference_has_every_shortcut() {
    for name in ["scripting.md", "scripting.es.md"] {
        let text = text_of(name);
        for shortcut in crate::shortcuts::ALL {
            let keys: Vec<String> = (shortcut.defaults().iter())
                .map(|keys| format!("`{}`", keys.stored()))
                .collect();
            let row = format!(
                "| `{}` | `{}` | {} |",
                shortcut.id,
                shortcut.place.id(),
                keys.join(", ")
            );
            assert!(text.contains(&row), "{name}: {row}");
        }
    }
}

#[test]
fn an_anchor_is_as_github_makes_it() {
    assert_eq!(anchor("`gauges.set(name, gauge)`"), "gaugessetname-gauge");
    assert_eq!(anchor("Listening: on"), "listening-on");
    assert_eq!(anchor("Qué se oye"), "qué-se-oye");
}
