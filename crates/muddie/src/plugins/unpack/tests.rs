use super::*;
use crate::zipread::make::{Item, items, zip};

const PLUGIN: &str = "name = \"Pack\"\n";

fn good() -> Vec<u8> {
    zip(&[
        ("pack/plugin.toml", PLUGIN.as_bytes()),
        ("pack/Pack.luau", b"echo('hi')"),
        ("pack/rules.toml", b""),
    ])
}

#[test]
fn a_zip_of_plugins_is_opened_by_plugin() {
    let found = plugins(&good()).unwrap();
    assert_eq!(found.keys().collect::<Vec<_>>(), ["pack"]);
    assert_eq!(found["pack"]["Pack.luau"], b"echo('hi')");
    // Folders listed, and several plugins.
    let many = items(&[
        Item::folder("a/"),
        Item::file("a/x.luau", b"1"),
        Item::folder("b/"),
        Item::file("b/y y.luau", b"2"),
        Item::file("b/template.toml", b""),
    ]);
    let found = plugins(&many).unwrap();
    assert_eq!(found.len(), 2);
    assert!(found["b"].contains_key("y y.luau"));
}

fn refused(zip: Vec<u8>) -> String {
    plugins(&zip).unwrap_err()
}

#[test]
fn names_that_could_reach_outside_are_refused() {
    for name in [
        "/etc/x.luau",
        "pack/../x.luau",
        "../pack/x.luau",
        "pack/./x.luau",
        "pack\\x.luau",
        "pack//x.luau",
        "C:/x/x.luau",
        "C:x.luau",
        "pack/C:x.luau",
        "pack/x.luau\0",
    ] {
        let found = plugins(&zip(&[(name, b"1")]));
        assert!(found.is_err(), "{name:?} was taken");
    }
}

#[test]
fn what_this_muddie_does_not_know_is_left_out_and_the_rest_taken() {
    for name in [
        "pack/sub/x.luau",
        "pack/sub/",
        "bad name!/x.luau",
        "x.luau",
        "pack/readme.md",
        "pack/run.sh",
        "pack/x.exe",
        "pack/.luau",
        "pack/.hidden.luau",
        "pack/other.toml",
        "__MACOSX/pack/x.luau",
    ] {
        let found = plugins(&zip(&[(name, b"1"), ("pack/Pack.luau", b"2")])).unwrap();
        assert_eq!(found.keys().collect::<Vec<_>>(), ["pack"], "{name}");
        assert_eq!(found["pack"].keys().collect::<Vec<_>>(), ["Pack.luau"]);
    }
}

#[test]
fn links_and_odd_files_are_refused() {
    let link = items(&[Item::link("pack/x.luau", b"/etc/passwd")]);
    assert!(refused(link).contains("neither"));
    let device = Item {
        attributes: 0o020_644 << 16,
        ..Item::file("pack/x.luau", b"1")
    };
    assert!(plugins(&items(&[device])).is_err());
    let windows_link = Item {
        attributes: 0x400,
        made_by: 0,
        ..Item::file("pack/x.luau", b"1")
    };
    assert!(plugins(&items(&[windows_link])).is_err());
    let locked = Item {
        flags: 1,
        ..Item::file("pack/x.luau", b"1")
    };
    assert!(plugins(&items(&[locked])).is_err());
    // The same name twice.
    let twice = zip(&[("pack/x.luau", b"1"), ("pack/x.luau", b"2")]);
    assert!(refused(twice).contains("twice"));
}

#[test]
fn sizes_are_counted_as_the_zip_is_inflated() {
    // Too large when the zip says so.
    let big = vec![b'a'; MAX_FILE as usize + 1];
    assert!(plugins(&zip(&[("pack/x.luau", &big)])).is_err());
    // And when it lies about how small it is.
    let lie = Item {
        said_size: Some(10),
        ..Item::file("pack/x.luau", &big)
    };
    assert!(plugins(&items(&[lie])).is_err());
    // Many files that are each small, but together too much.
    let chunk = vec![b'a'; MAX_FILE as usize];
    let names: Vec<String> = (0..5).map(|n| format!("pack/{n}.luau")).collect();
    let files: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &chunk[..])).collect();
    assert!(plugins(&zip(&files)).is_err());
    // Too many entries.
    let names: Vec<String> = (0..=MAX_ENTRIES).map(|n| format!("p{n}/")).collect();
    let folders: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &b""[..])).collect();
    assert!(plugins(&zip(&folders)).is_err());
    // Packed too large to even look at.
    assert!(plugins(&vec![0; MAX_PACKED as usize + 1]).is_err());
}

#[test]
fn a_damaged_zip_is_refused() {
    let zip = good();
    assert!(plugins(&zip[..zip.len() / 2]).is_err());
    assert!(plugins(b"not a zip at all, not even close").is_err());
    assert!(plugins(b"").is_err());
}

/// The real zip, if `MUDDIE_REAL_ZIP` says where one is: made by the
/// repository of the plugins, not by these tests.
#[test]
fn the_real_zip_is_taken_when_there_is_one() {
    let Some(path) = std::env::var_os("MUDDIE_REAL_ZIP") else {
        return;
    };
    let found = plugins(&std::fs::read(path).unwrap()).unwrap();
    assert!(!found.is_empty());
    for (id, files) in &found {
        let verdict = crate::plugins::judge::judge(files);
        let newer = matches!(verdict, crate::plugins::judge::Verdict::Newer(_));
        assert!(
            verdict == crate::plugins::judge::Verdict::Ready || newer,
            "{id}: {verdict:?}"
        );
    }
}
