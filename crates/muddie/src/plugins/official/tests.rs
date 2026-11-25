use std::io::{Read, Write};
use std::net::TcpListener;

use super::*;
use crate::zipread::make::{Item, items, zip};

const MANIFEST: &str = "name = \"Pack\"\ndefault_on = true\n";

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("muddie-official-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A zip with one plugin whose script is `code`.
fn pack(code: &str) -> Vec<u8> {
    zip(&[
        ("pack/plugin.toml", MANIFEST.as_bytes()),
        ("pack/Pack.luau", code.as_bytes()),
        ("pack/rules.toml", b""),
    ])
}

/// One answer of the server: its status line, headers and body.
fn answer(status: &str, headers: &[&str], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\nConnection: close\r\n").into_bytes();
    for header in headers {
        out.extend(format!("{header}\r\n").as_bytes());
    }
    out.extend(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
    out.extend(body);
    out
}

/// A server on this machine that gives each of `answers` to one request,
/// in turn. Returns its address, and the requests it got once done.
fn serve(answers: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/plugins.zip", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for answer in answers {
            let (mut stream, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut byte = [0u8];
            while !got.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                got.push(byte[0]);
            }
            requests.push(String::from_utf8_lossy(&got).into_owned());
            stream.write_all(&answer).unwrap();
        }
        requests
    });
    (address, thread)
}

fn script(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("pack/Pack.luau")).unwrap()
}

#[test]
fn the_web_is_asked_conditionally_and_what_it_gives_is_kept() {
    let base = temp("web");
    let (dir, etags) = (base.join("official"), base.join("versions"));
    let (address, server) = serve(vec![
        answer("200 OK", &["ETag: \"v1\""], &pack("-- one")),
        answer("304 Not Modified", &[], b""),
        answer("404 Not Found", &[], b"nope"),
        answer("200 OK", &["ETag: \"v2\""], b"this is not a zip"),
        answer("200 OK", &["ETag: \"v3\""], &pack("-- three")),
    ]);
    let first = update(&dir, &etags, &address).unwrap();
    assert!(first.fetched);
    assert_eq!(first.changed, [("pack".to_owned(), "Pack".to_owned())]);
    assert_eq!(script(&dir), "-- one");
    assert_eq!(
        store::known_version(&etags, &address).as_deref(),
        Some("\"v1\"")
    );
    // Not changed: nothing is taken, nothing touched.
    assert_eq!(update(&dir, &etags, &address).unwrap(), Report::default());
    // Not there: an error, and what was kept stays.
    assert!(update(&dir, &etags, &address).is_err());
    assert_eq!(script(&dir), "-- one");
    // Not a zip: the same.
    assert!(update(&dir, &etags, &address).is_err());
    assert_eq!(script(&dir), "-- one");
    assert_eq!(
        store::known_version(&etags, &address).as_deref(),
        Some("\"v1\"")
    );
    // A new one replaces it.
    let third = update(&dir, &etags, &address).unwrap();
    assert_eq!(third.changed.len(), 1);
    assert_eq!(script(&dir), "-- three");
    let requests = server.join().unwrap();
    assert!(!requests[0].to_lowercase().contains("if-none-match"));
    assert!(requests[0].to_lowercase().contains("user-agent: muddie"));
    for later in &requests[1..3] {
        assert!(
            later.to_lowercase().contains("if-none-match: \"v1\""),
            "{later}"
        );
    }
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn nothing_is_asked_to_be_unchanged_when_nothing_is_kept() {
    let base = temp("lost");
    let (dir, etags) = (base.join("official"), base.join("versions"));
    store::keep_version(&etags, "http://x/", "\"v1\"").unwrap();
    let (address, server) = serve(vec![answer("200 OK", &["ETag: \"v1\""], &pack("-- one"))]);
    store::keep_version(&etags, &address, "\"v1\"").unwrap();
    // The version is known, but the plugins are gone: all is asked for.
    assert!(update(&dir, &etags, &address).unwrap().fetched);
    assert!(
        !server.join().unwrap()[0]
            .to_lowercase()
            .contains("if-none-match")
    );
    // The version of another Muddie is not taken for this one's.
    let old = format!("{address}\t0.0.1\t\"v1\"\n");
    std::fs::write(&etags, old).unwrap();
    assert_eq!(store::known_version(&etags, &address), None);
    std::fs::remove_dir_all(&base).unwrap();
}

/// What a zip with `files` gives when taken, with `kept` already there.
fn taken(dir: &Path, etags: &Path, zip: Vec<u8>) -> Report {
    let (address, server) = serve(vec![answer("200 OK", &["ETag: \"x\""], &zip)]);
    let report = update(dir, etags, &address).unwrap();
    server.join().unwrap();
    report
}

#[test]
fn a_plugin_that_is_not_right_leaves_the_one_kept() {
    let base = temp("wrong");
    let (dir, etags) = (base.join("official"), base.join("versions"));
    taken(&dir, &etags, pack("-- one"));
    let wrong = |manifest: &str, rules: &str, script: &str| {
        let mut files: Vec<(&str, &[u8])> = vec![
            ("pack/plugin.toml", manifest.as_bytes()),
            ("pack/rules.toml", rules.as_bytes()),
        ];
        if !script.is_empty() {
            files.push(("pack/Pack.luau", script.as_bytes()));
        }
        zip(&files)
    };
    for (manifest, rules, script, why) in [
        ("name = [", "", "-- two", "plugin.toml"),
        ("unknown_field = 1\n", "", "-- two", "plugin.toml"),
        (MANIFEST, "gauges = [", "-- two", "rules.toml"),
        (MANIFEST, "", "", "no .luau script"),
    ] {
        let report = taken(&dir, &etags, wrong(manifest, rules, script));
        assert!(report.changed.is_empty(), "{why}");
        assert!(report.refused[0].contains(why), "{:?}", report.refused);
        assert_eq!(script_of(&dir), "-- one");
    }
    // The others in the zip are taken all the same.
    let mixed = items(&[
        Item::file("pack/plugin.toml", b"name = ["),
        Item::file("pack/Pack.luau", b"-- two"),
        Item::file("good/Good.luau", b"-- good"),
    ]);
    let report = taken(&dir, &etags, mixed);
    assert_eq!(report.changed.len(), 1);
    assert!(dir.join("good/Good.luau").is_file());
    assert_eq!(script_of(&dir), "-- one");
    std::fs::remove_dir_all(&base).unwrap();
}

fn script_of(dir: &Path) -> String {
    script(dir)
}

#[test]
fn a_zip_with_something_unsafe_is_taken_in_no_part() {
    let base = temp("unsafe");
    let (dir, etags) = (base.join("official"), base.join("versions"));
    let bad = items(&[
        Item::file("pack/plugin.toml", MANIFEST.as_bytes()),
        Item::file("pack/Pack.luau", b"-- one"),
        Item::file("../outside.luau", b"x"),
    ]);
    let (address, server) = serve(vec![answer("200 OK", &["ETag: \"x\""], &bad)]);
    assert!(update(&dir, &etags, &address).is_err());
    server.join().unwrap();
    assert!(!dir.exists() || !store::has_plugins(&dir));
    assert!(!base.join("outside.luau").exists());
    assert_eq!(store::known_version(&etags, &address), None);
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn a_plugin_for_a_newer_muddie_is_kept_as_it_is_and_said_once() {
    let base = temp("newer");
    let (dir, etags) = (base.join("official"), base.join("versions"));
    taken(&dir, &etags, pack("-- one"));
    // With fields this Muddie does not know, and so cannot read whole.
    let future = "name = \"Pack\"\nmuddie = \"99.0.0\"\nfrom_the_future = true\n";
    let zip = zip(&[
        ("pack/plugin.toml", future.as_bytes()),
        ("pack/Pack.luau", b"-- future"),
    ]);
    let report = taken(&dir, &etags, zip);
    assert_eq!(report.newer, [("Pack".to_owned(), "99.0.0".to_owned())]);
    assert!(report.changed.is_empty() && report.refused.is_empty());
    assert_eq!(script(&dir), "-- one");
    // The strict reading takes the field, for a Muddie that is new enough.
    let manifest: crate::plugins::Manifest = toml::from_str("muddie = \"0.1.0\"").unwrap();
    assert_eq!(manifest.muddie.as_deref(), Some("0.1.0"));
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn a_plugin_is_replaced_whole_or_not_at_all() {
    let base = temp("store");
    let files =
        |code: &str| unpack::Files::from([("Pack.luau".to_owned(), code.as_bytes().to_vec())]);
    assert!(store::put(&base, "pack", &files("-- one")).unwrap());
    assert!(!store::put(&base, "pack", &files("-- one")).unwrap());
    // A file that cannot be written: the plugin is as it was.
    let broken = unpack::Files::from([
        ("Pack.luau".to_owned(), b"-- two".to_vec()),
        ("no/such/folder.luau".to_owned(), b"x".to_vec()),
    ]);
    assert!(store::put(&base, "pack", &broken).is_err());
    assert_eq!(
        std::fs::read_to_string(base.join("pack/Pack.luau")).unwrap(),
        "-- one"
    );
    let left: Vec<_> = std::fs::read_dir(&base).unwrap().flatten().collect();
    assert_eq!(left.len(), 1, "what was being written was left behind");
    // A crash between putting one aside and putting the new one in place.
    std::fs::rename(base.join("pack"), base.join(".old-pack-1")).unwrap();
    std::fs::create_dir(base.join(".new-pack-1")).unwrap();
    std::fs::write(base.join(".new-pack-1/Pack.luau"), "-- half").unwrap();
    // Neither is taken for a plugin.
    assert_eq!(crate::plugins::folders(&base).count(), 0);
    assert!(!store::has_plugins(&base));
    store::recover(&base);
    assert_eq!(
        std::fs::read_to_string(base.join("pack/Pack.luau")).unwrap(),
        "-- one"
    );
    assert_eq!(std::fs::read_dir(&base).unwrap().count(), 1);
    std::fs::remove_dir_all(&base).unwrap();
}

/// The real zip, made by the repository of the plugins, if `MUDDIE_REAL_ZIP`
/// says where it is: taken through the web as it would be.
#[test]
fn the_real_zip_is_taken_through_the_web() {
    let Some(path) = std::env::var_os("MUDDIE_REAL_ZIP") else {
        return;
    };
    let base = temp("real");
    let (dir, etags) = (base.join("official"), base.join("versions"));
    let bytes = std::fs::read(path).unwrap();
    let report = taken(&dir, &etags, bytes);
    assert_eq!(report.changed.len() + report.newer.len(), 1, "{report:?}");
    assert!(report.refused.is_empty(), "{report:?}");
    std::fs::remove_dir_all(&base).unwrap();
}
