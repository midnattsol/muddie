//! What is written of the API, against what there is of it: the definitions
//! for editors, `docs/muddie.d.luau`, and the manual, `docs/scripting.md`.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

const DEFINITIONS: &str = include_str!("../../../../docs/muddie.d.luau");
const MANUAL: &str = include_str!("../../../../docs/scripting.md");

/// What a type of the definitions has: each with whether it is a function,
/// and whether it is only read.
type Members = BTreeMap<String, (bool, bool)>;

/// The members of each type written as a table, by its name.
fn types() -> BTreeMap<String, Members> {
    let mut types: BTreeMap<String, Members> = BTreeMap::new();
    let mut inside: Option<String> = None;
    for line in DEFINITIONS.lines() {
        if let Some(rest) = line.strip_prefix("type ") {
            let (name, what) = rest.split_once(" = ").unwrap();
            let members = types.entry(name.to_owned()).or_default();
            match what {
                "{" => inside = Some(name.to_owned()),
                what => members.extend(listed(what)),
            }
        } else if line.starts_with('}') {
            inside = None;
        } else if let (Some(name), Some(member)) = (&inside, line.strip_prefix("    "))
            && !member.starts_with([' ', ')'])
        {
            types.get_mut(name).unwrap().extend(one(member));
        }
    }
    types
}

/// The members of a type in one line: `{ stop: (self: Sound) -> (), ... }`.
fn listed(what: &str) -> Members {
    let inner = what
        .strip_prefix("{ ")
        .and_then(|what| what.strip_suffix(" }"));
    let mut members = Members::new();
    let (mut deep, mut start) = (0, 0);
    let inner = inner.unwrap_or_default();
    for (at, ch) in inner.char_indices() {
        match ch {
            '(' | '{' => deep += 1,
            ')' | '}' => deep -= 1,
            ',' if deep == 0 => {
                members.extend(one(inner[start..at].trim()));
                start = at + 1;
            }
            _ => {}
        }
    }
    members.extend(one(inner[start..].trim()));
    members
}

/// `name: type`, if it is one.
fn one(member: &str) -> Option<(String, (bool, bool))> {
    let (read, member) = match member.strip_prefix("read ") {
        Some(member) => (true, member),
        None => (false, member),
    };
    let (name, what) = member.split_once(": ")?;
    let word = name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    let function = what.trim_end() == "(" || what.contains(") -> ");
    word.then(|| (name.to_owned(), (function, read)))
}

/// The globals the definitions declare, each with the type it is of, or none
/// if it is a function.
fn declared() -> BTreeMap<String, Option<String>> {
    (DEFINITIONS.lines())
        .filter_map(|line| line.strip_prefix("declare "))
        .map(|line| match line.strip_prefix("function ") {
            Some(function) => (function.split('(').next().unwrap().to_owned(), None),
            None => {
                let (name, what) = line.split_once(": ").unwrap();
                let table = (!what.starts_with('(')).then(|| what.to_owned());
                (name.to_owned(), table)
            }
        })
        .collect()
}

/// The names there are in the engine: `send`, `gauges.set`, but those that
/// only say what is in their place.
fn there_are(engine: &Engine) -> BTreeSet<String> {
    let retired: BTreeSet<&str> = crate::retired::names().collect();
    let plain = Lua::new();
    let globals = engine.lua.globals();
    let frozen: Table = globals.metatable().unwrap().get("__index").unwrap();
    let mut names = BTreeSet::new();
    for pair in frozen.pairs::<String, Value>() {
        let (name, value) = pair.unwrap();
        // Those of Luau itself, and Muddie's own `require`, which is as its.
        if plain.globals().contains_key(name.as_str()).unwrap() || name == "require" {
            continue;
        }
        match value {
            Value::Table(table) => {
                for pair in table.pairs::<String, Value>() {
                    names.insert(format!("{name}.{}", pair.unwrap().0));
                }
                names.insert(name);
            }
            _ => drop(names.insert(name)),
        }
    }
    names.retain(|name| !retired.contains(name.as_str()));
    names
}

fn errors(engine: &mut Engine) -> Vec<String> {
    (engine.take_actions().into_iter())
        .filter_map(|action| match action {
            Action::Error(error) => Some(error),
            _ => None,
        })
        .collect()
}

/// The members declared of a global that is a table: of a type with a name,
/// or of one written there.
fn members_of(types: &BTreeMap<String, Members>, what: &str) -> Members {
    types.get(what).cloned().unwrap_or_else(|| listed(what))
}

#[test]
fn the_definitions_say_what_there_is() {
    let engine = engine("");
    let there = there_are(&engine);
    let types = types();
    let mut said = BTreeSet::new();
    for (name, what) in declared() {
        if let Some(what) = &what {
            for (member, (function, _)) in members_of(&types, what) {
                // A field that changes is not in the table itself.
                if function {
                    said.insert(format!("{name}.{member}"));
                }
            }
        }
        said.insert(name);
    }
    let missing: Vec<&String> = there.difference(&said).collect();
    let gone: Vec<&String> = said.difference(&there).collect();
    assert!(
        missing.is_empty() && gone.is_empty(),
        "not said: {missing:?}; not there: {gone:?}"
    );
    assert!(there.len() > 100, "{there:?}");
}

/// The fields of `session` and `map` there are, and that those are the only
/// tables with any: what is not a function is read as it is each time.
#[test]
fn the_fields_that_change_are_those_the_definitions_say() {
    let types = types();
    for (name, what) in declared() {
        let Some(what) = what else { continue };
        let fields: Vec<String> = (members_of(&types, &what).into_iter())
            .filter(|(_, (function, _))| !function)
            .map(|(field, _)| field)
            .collect();
        let changing = CHANGING.contains(&name.as_str());
        assert_eq!(
            changing,
            !fields.is_empty() || name == "vars",
            "{name}: {fields:?}"
        );
        for field in fields {
            let mut engine = engine("");
            engine.exec("t", &format!("local _ = {name}.{field}"));
            assert_eq!(errors(&mut engine), [] as [String; 0], "{name}.{field}");
        }
    }
    let mut engine = engine("");
    for table in ["Session", "MapTable"] {
        let settable = (types[table].iter()).filter(|(_, (function, read))| !function && !read);
        for (field, _) in settable {
            let global = if table == "Session" { "session" } else { "map" };
            engine.exec("t", &format!("{global}.{field} = nil"));
            assert_eq!(errors(&mut engine), [] as [String; 0], "{global}.{field}");
        }
    }
}

/// What a line, what a script makes and a sound have is there.
#[test]
fn the_things_scripts_are_given_are_as_the_definitions_say() {
    let types = types();
    let mut engine = engine("");
    engine.exec(
        "t",
        r#"trigger("", function(m, l) line = l end)
        made = after(9, function() end)
        sound = sounds.play("x")"#,
    );
    engine.on_line("x");
    engine.take_actions();
    for (what, name) in [("Line", "line"), ("Made", "made"), ("Sound", "sound")] {
        for (member, (function, _)) in &types[what] {
            if member == "spans" {
                continue;
            }
            engine.exec("t", &format!("send(type({name}.{member}))"));
            let actions = engine.take_actions();
            let [Action::Send(kind)] = actions.as_slice() else {
                panic!("{what}.{member}: {actions:?}");
            };
            assert_eq!(kind == "function", *function, "{what}.{member}");
        }
    }
    // And there is nothing more of a line.
    engine.exec("t", "local _ = line.nothing");
    let actions = engine.take_actions();
    let (_, there) = error_text(&actions).split_once("there are ").unwrap();
    let there: BTreeSet<&str> = there
        .split([',', ' '])
        .filter(|word| !["", "and"].contains(word))
        .collect();
    let said: BTreeSet<&str> = types["Line"].keys().map(String::as_str).collect();
    assert_eq!(there, said);
}

/// What a gauge is given, as the definitions say it.
#[test]
fn the_options_of_a_gauge_are_as_the_definitions_say() {
    let said: BTreeSet<String> = types()["GaugeSet"].keys().cloned().collect();
    let there: BTreeSet<String> = crate::api::GAUGE
        .iter()
        .map(|key| (*key).to_owned())
        .collect();
    assert_eq!(said, there);
}

/// Everything there is is in the manual.
#[test]
fn the_manual_tells_of_everything_there_is() {
    let engine = engine("");
    let mut untold = Vec::new();
    for name in there_are(&engine) {
        // Those that switch a kind are told of together.
        let switch = name.split_once('.').is_some_and(|(_, member)| {
            ["enable", "disable", "toggle", "enabled", "list"].contains(&member)
        });
        let told = ["(", "{", "`", " ", "."]
            .iter()
            .any(|after| MANUAL.contains(&format!("`{name}{after}")))
            || MANUAL.contains(&format!(", `{name}"));
        if !told && !switch {
            untold.push(name);
        }
    }
    for (what, name) in [("Line", "line"), ("Made", "made")] {
        for member in types()[what].keys() {
            let told = [":", "."]
                .iter()
                .any(|dot| MANUAL.contains(&format!("{name}{dot}{member}")));
            if !told {
                untold.push(format!("{name}:{member}"));
            }
        }
    }
    for (table, name) in [("Session", "session"), ("MapTable", "map")] {
        for member in types()[table].keys() {
            if !MANUAL.contains(&format!("`{name}.{member}")) {
                untold.push(format!("{name}.{member}"));
            }
        }
    }
    assert!(untold.is_empty(), "{untold:?}");
}
