//! The official plugins as sessions see them: updates taken by a profile
//! that is not connected and not by one that is, waiting for the first
//! download before connecting, and the profiles of an earlier Muddie, which
//! have the scripts of a plugin in them from before it was downloaded.

use super::*;
use crate::plugins::official::Download;
use crate::plugins::{fixture, store, unpack::Files};

fn temp(name: &str) -> Profiles {
    let dir = std::env::temp_dir().join(format!("muddie-sessions-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Profiles::at(dir)
}

/// The files of a plugin called Pack, on from the start, whose script is
/// `code`; the profile has them in a folder called Pack.
fn pack(code: &str) -> Files {
    Files::from([
        (
            "plugin.toml".to_owned(),
            b"name = \"Pack\"\ndefault_on = true\n".to_vec(),
        ),
        ("Pack.luau".to_owned(), code.as_bytes().to_vec()),
    ])
}

fn keep(profiles: &Profiles, id: &str, files: &Files) {
    store::put(&profiles.official_dir(), id, files).unwrap();
}

fn script_in(profiles: &Profiles) -> PathBuf {
    profiles.scripts_dir("P").join("Pack").join("Pack.luau")
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn playing(profiles: &Profiles) -> Harness<'static, App> {
    profiles.save("P", &Profile::default()).unwrap();
    let mut harness = window(Some(profiles.clone()));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("P".into())));
    harness
}

#[test]
fn an_update_is_taken_by_a_profile_not_connected_and_by_one_connected_when_opened_again() {
    let profiles = temp("update");
    keep(&profiles, "pack", &pack("-- one"));
    let mut harness = playing(&profiles);
    let script = script_in(&profiles);
    assert_eq!(read(&script), "-- one");

    // Not connected: at once, and said.
    keep(&profiles, "pack", &pack("-- two"));
    let app = harness.state_mut().tab_mut();
    app.official_changed();
    assert_eq!(read(&script), "-- two");
    assert!(
        notes(app)
            .iter()
            .any(|note| note == "Pack brought up to date.")
    );

    // Connected: its game is not touched.
    let (_events, _sent) = fake_connection(app);
    keep(&profiles, "pack", &pack("-- three"));
    app.official_changed();
    assert_eq!(read(&script), "-- two");
    app.load_scripts();
    assert_eq!(read(&script), "-- two");

    // Opened again, once it is not connected, it takes it.
    app.connection = None;
    assert!(app.select_profile(Some("P".into())));
    assert_eq!(read(&script), "-- three");
    std::fs::remove_dir_all(profiles.official_dir().parent().unwrap()).unwrap();
}

#[test]
fn what_the_player_changed_in_a_plugin_is_kept_when_it_is_updated() {
    let profiles = temp("kept");
    keep(&profiles, "pack", &pack("-- one"));
    let mut harness = playing(&profiles);
    let script = script_in(&profiles);
    std::fs::write(&script, "-- mine").unwrap();
    keep(&profiles, "pack", &pack("-- two"));
    harness.state_mut().tab_mut().official_changed();
    assert_eq!(read(&script), "-- two");
    let copy = script.with_file_name("Pack.luau.before-update");
    assert_eq!(read(&copy), "-- mine");
    std::fs::remove_dir_all(profiles.official_dir().parent().unwrap()).unwrap();
}

#[test]
fn a_plugin_that_arrives_while_playing_is_offered() {
    let profiles = temp("offer");
    fixture::forget();
    let mut harness = playing(&profiles);
    let app = harness.state_mut().tab_mut();
    app.host = "pack.example".into();
    let (_events, _sent) = fake_connection(app);
    let mut files = pack("-- one");
    files.insert(
        "plugin.toml".into(),
        b"name = \"Pack\"\nhosts = [\"pack.example\"]\n".to_vec(),
    );
    keep(&profiles, "pack", &files);
    app.official_changed();
    assert_eq!(
        app.plugin_offer.as_ref().map(|plugin| plugin.id.as_str()),
        Some("pack")
    );
    assert!(!app.plugin_on(&app.plugin_offer.clone().unwrap()));
    std::fs::remove_dir_all(profiles.official_dir().parent().unwrap()).unwrap();
}

/// A session that wants to connect to nothing that answers, with the
/// plugins being downloaded, and none downloaded before.
fn about_to_connect(name: &str) -> (Harness<'static, App>, Profiles, Download) {
    fixture::forget();
    let profiles = temp(name);
    let mut harness = playing(&profiles);
    let download = Download::default();
    download.set(true);
    let app = harness.state_mut().tab_mut();
    app.plugin_download = download.clone();
    app.host = "127.0.0.1".into();
    app.port = "9".into();
    (harness, profiles, download)
}

/// Closes the window and deletes what it kept: not while it is open, for
/// the map of a session that connected is a file in use, which Windows does
/// not let be deleted.
fn clean_up(harness: Harness<'_, App>, profiles: &Profiles) {
    drop(harness);
    std::fs::remove_dir_all(profiles.official_dir().parent().unwrap()).unwrap();
}

#[test]
fn the_first_connection_waits_for_the_plugins_and_goes_on_when_they_come() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _inside = runtime.enter();
    let (mut harness, profiles, download) = about_to_connect("wait");
    let ctx = harness.ctx.clone();
    let app = harness.state_mut().tab_mut();
    app.connect(&ctx);
    assert!(app.can_disconnect() && !app.is_connected());
    assert!(notes(app).iter().any(|note| note == "Downloading plugins…"));
    assert_eq!(app.status().1, "Downloading plugins…");
    // Still on its way: still waiting.
    app.check_plugin_wait(&ctx);
    assert!(!app.is_connected() && app.plugin_wait.is_some());
    // It came: connects, with it.
    keep(&profiles, "pack", &pack("-- one"));
    download.set(false);
    app.check_plugin_wait(&ctx);
    assert!(app.is_connected() && app.plugin_wait.is_none());
    assert!(app.plugins().iter().any(|plugin| plugin.id == "pack"));
    clean_up(harness, &profiles);
}

#[test]
fn the_first_connection_waits_no_longer_than_five_seconds() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _inside = runtime.enter();
    let (mut harness, profiles, _download) = about_to_connect("timeout");
    let ctx = harness.ctx.clone();
    let app = harness.state_mut().tab_mut();
    app.connect(&ctx);
    app.plugin_wait = Instant::now().checked_sub(Duration::from_secs(6));
    app.check_plugin_wait(&ctx);
    assert!(app.is_connected() && app.plugin_wait.is_none());
    clean_up(harness, &profiles);
}

#[test]
fn disconnecting_while_waiting_cancels_connecting() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _inside = runtime.enter();
    let (mut harness, profiles, download) = about_to_connect("cancel");
    let ctx = harness.ctx.clone();
    let app = harness.state_mut().tab_mut();
    app.connect(&ctx);
    app.disconnect();
    assert!(!app.can_disconnect());
    download.set(false);
    app.check_plugin_wait(&ctx);
    assert!(!app.is_connected());
    clean_up(harness, &profiles);
}

#[test]
fn what_arrives_while_waiting_is_taken_before_connecting() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _inside = runtime.enter();
    let (mut harness, profiles, download) = about_to_connect("taken");
    let ctx = harness.ctx.clone();
    let app = harness.state_mut().tab_mut();
    app.connect(&ctx);
    keep(&profiles, "pack", &pack("-- one"));
    app.official_changed();
    assert_eq!(read(&script_in(&profiles)), "-- one");
    assert!(app.plugin_wait.is_some());
    download.set(false);
    app.check_plugin_wait(&ctx);
    assert!(app.is_connected() && app.scripts.is_loaded(&script_in(&profiles)));
    clean_up(harness, &profiles);
}

#[test]
fn choosing_another_profile_while_waiting_does_not_connect_to_it() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _inside = runtime.enter();
    let (mut harness, profiles, download) = about_to_connect("another");
    profiles.save("Q", &Profile::default()).unwrap();
    let ctx = harness.ctx.clone();
    let app = harness.state_mut().tab_mut();
    app.connect(&ctx);
    assert!(app.select_profile(Some("Q".into())));
    download.set(false);
    app.check_plugin_wait(&ctx);
    assert!(!app.is_connected() && !app.can_disconnect());
    clean_up(harness, &profiles);
}

#[test]
fn with_plugins_on_disk_or_none_on_the_way_it_connects_at_once() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _inside = runtime.enter();
    // Something was downloaded before.
    let (mut first, profiles, _download) = about_to_connect("disk");
    keep(&profiles, "pack", &pack("-- one"));
    let ctx = first.ctx.clone();
    let app = first.state_mut().tab_mut();
    app.connect(&ctx);
    assert!(app.is_connected() && app.plugin_wait.is_none());
    // Nothing was, and nothing is on its way, as with the setting off.
    let (mut harness, profiles2, download) = about_to_connect("none");
    download.set(false);
    let app = harness.state_mut().tab_mut();
    app.connect(&ctx);
    assert!(app.is_connected() && app.plugin_wait.is_none());
    clean_up(first, &profiles);
    clean_up(harness, &profiles2);
}

/// A profile of the version before plugins were downloaded: Reinos was on
/// and installed in it, as it was then.
fn from_before(profiles: &Profiles) {
    let mut harness = playing(profiles);
    let app = harness.state_mut().tab_mut();
    app.set_plugin(&fixture::reinos(), true);
    assert!(app.plugins_installed.contains_key("reinos-de-leyenda"));
}

fn reinos_script(profiles: &Profiles) -> PathBuf {
    (profiles.scripts_dir("P"))
        .join("Reinos de Leyenda")
        .join(fixture::SCRIPT_NAME)
}

#[test]
fn profiles_of_before_keep_working_until_the_plugin_is_downloaded_and_update_after() {
    let profiles = temp("migration");
    from_before(&profiles);
    // Muddie now has no such plugin of its own, and none downloaded.
    fixture::forget();
    let mut harness = window(Some(profiles.clone()));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("P".into())));
    let script = reinos_script(&profiles);
    assert!(
        app.plugins()
            .iter()
            .all(|plugin| plugin.id != "reinos-de-leyenda")
    );
    assert!(app.scripts.is_loaded(&script));
    // The gauges of the plugin, in the folder of its name.
    let ours = || {
        let rules = crate::rules::Rules::load(&profiles.rules_path(Some("P"))).unwrap();
        let ours: Vec<_> = (rules.gauges.into_iter())
            .filter(|gauge| gauge.folder == "Reinos de Leyenda")
            .collect();
        ours
    };
    assert_eq!(ours().len(), 3);
    assert!(ours().iter().all(|gauge| gauge.enabled));
    // Saved again, it lists them as it did.
    app.keep_plugins();
    let saved = profiles.load("P").unwrap();
    assert_eq!(saved.plugins_on, ["reinos-de-leyenda"]);
    assert!(saved.plugins_installed.contains_key("reinos-de-leyenda"));
    assert!(script.is_file());
    app.official_changed();
    assert!(app.scripts.is_loaded(&script) && script.is_file());
    assert_eq!(ours().len(), 3);

    // Downloaded, it is updated as any other, and what the player wrote
    // in its script is kept.
    std::fs::write(&script, "-- mine").unwrap();
    let mut files = Files::new();
    files.insert("plugin.toml".into(), fixture::MANIFEST.as_bytes().to_vec());
    files.insert("rules.toml".into(), fixture::RULES.as_bytes().to_vec());
    let newer = format!("{}\n-- newer", fixture::SCRIPT);
    files.insert(fixture::SCRIPT_NAME.into(), newer.as_bytes().to_vec());
    keep(&profiles, "reinos-de-leyenda", &files);
    let app = harness.state_mut().tab_mut();
    app.official_changed();
    assert_eq!(read(&script), newer);
    assert_eq!(
        read(&script.with_extension("luau.before-update")),
        "-- mine"
    );
    assert_eq!(ours().len(), 3);
    assert!(app.plugins_installed.contains_key("reinos-de-leyenda"));
    std::fs::remove_dir_all(profiles.official_dir().parent().unwrap()).unwrap();
}

#[test]
fn a_plugin_from_git_updated_keeps_the_scripts_the_player_edited() {
    let profiles = temp("git");
    let folder = profiles.plugins_dir().join("gitpack");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("plugin.toml"),
        "name = \"Pack\"\ndefault_on = true\n",
    )
    .unwrap();
    std::fs::write(folder.join("Pack.luau"), "-- one").unwrap();
    let mut harness = playing(&profiles);
    let ctx = harness.ctx.clone();
    let script = script_in(&profiles);
    assert_eq!(read(&script), "-- one");
    std::fs::write(&script, "-- mine").unwrap();
    // What `git pull` does, and the job that ends.
    std::fs::write(folder.join("Pack.luau"), "-- two").unwrap();
    let (done, outcome) = std::sync::mpsc::channel();
    done.send(Ok("gitpack".to_owned())).unwrap();
    let app = harness.state_mut().tab_mut();
    app.plugin_jobs.push((outcome, false));
    app.check_plugin_jobs(&ctx);
    assert_eq!(read(&script), "-- two");
    assert_eq!(
        read(&script.with_extension("luau.before-update")),
        "-- mine"
    );
    // Unedited, nothing is kept.
    std::fs::write(folder.join("Pack.luau"), "-- three").unwrap();
    std::fs::remove_file(script.with_extension("luau.before-update")).unwrap();
    let (done, outcome) = std::sync::mpsc::channel();
    done.send(Ok("gitpack".to_owned())).unwrap();
    let app = harness.state_mut().tab_mut();
    app.plugin_jobs.push((outcome, false));
    app.check_plugin_jobs(&ctx);
    assert_eq!(read(&script), "-- three");
    assert!(!script.with_extension("luau.before-update").exists());
    std::fs::remove_dir_all(profiles.plugins_dir().parent().unwrap()).unwrap();
}

#[test]
fn a_plugin_from_git_wins_over_an_official_one_of_the_same_id() {
    let profiles = temp("wins");
    keep(&profiles, "pack", &pack("-- official"));
    let folder = profiles.plugins_dir().join("pack");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("Pack.luau"), "-- git").unwrap();
    let found = crate::plugins::all(Some(&profiles));
    let packs: Vec<_> = found.iter().filter(|plugin| plugin.id == "pack").collect();
    assert_eq!(packs.len(), 1);
    assert!(packs[0].is_git() && packs[0].scripts[0].1 == "-- git");
    std::fs::remove_dir_all(profiles.official_dir().parent().unwrap()).unwrap();
}

#[test]
fn what_the_window_is_told_of_an_update_is_said_and_applied() {
    use crate::plugins::official::Report;
    let profiles = temp("report");
    keep(&profiles, "pack", &pack("-- one"));
    let mut harness = playing(&profiles);
    keep(&profiles, "pack", &pack("-- two"));
    let report = Report {
        changed: vec![("pack".into(), "Pack".into())],
        newer: vec![("Other".into(), "9.0.0".into())],
        ..Report::default()
    };
    harness.state_mut().apply_plugin_report(&report, false);
    harness.state_mut().apply_plugin_report(&report, false);
    let said = notes(harness.state().tab());
    let count = |text: &str| said.iter().filter(|note| note.as_str() == text).count();
    assert_eq!(count("Plugins updated: Pack."), 2);
    // Once, not twice.
    assert_eq!(
        count("Other needs Muddie 9.0.0 or newer, so it is not updated."),
        1
    );
    assert_eq!(read(&script_in(&profiles)), "-- two");
    std::fs::remove_dir_all(profiles.official_dir().parent().unwrap()).unwrap();
}

/// The files of the plugin called Pack with these scripts, and gauges of
/// these names.
fn pack_of(scripts: &[(&str, &str)], gauges: &[&str]) -> Files {
    let mut files = pack("");
    files.remove("Pack.luau");
    for (name, code) in scripts {
        files.insert((*name).to_owned(), code.as_bytes().to_vec());
    }
    let rules: Vec<String> = (gauges.iter())
        .map(|name| format!("[[gauge]]\nname = \"{name}\"\nvalue = \"X.{name}\"\n"))
        .collect();
    files.insert("rules.toml".into(), rules.concat().into_bytes());
    files
}

/// Updated, a plugin is as it comes: what it installed before and no longer
/// has goes, scripts and gauges; what the player changed of a script that
/// goes is kept beside, and what they added, where it is.
#[test]
fn an_update_takes_away_what_the_plugin_no_longer_has() {
    let profiles = temp("clean");
    let before = [
        ("a.luau", "vars.a = 1"),
        ("b.luau", "vars.b = 1"),
        ("c.luau", "vars.c = 1"),
    ];
    keep(&profiles, "pack", &pack_of(&before, &["Life", "Mana"]));
    let mut harness = playing(&profiles);
    let file = |name: &str| script_in(&profiles).with_file_name(name);
    std::fs::write(file("b.luau"), "vars.b = 2").unwrap();
    std::fs::write(file("mine.luau"), "vars.mine = 1").unwrap();
    let app = harness.state_mut().tab_mut();
    app.change_rules(|rules| {
        let own = crate::rules::Gauge {
            name: "Own".into(),
            folder: "Pack".into(),
            ..Default::default()
        };
        rules.gauges.push(own);
    });
    keep(
        &profiles,
        "pack",
        &pack_of(
            &[("c.luau", "vars.c = 2"), ("d.luau", "vars.d = 1")],
            &["Life", "Moves"],
        ),
    );
    app.official_changed();
    let there = |app: &Tab| -> Vec<&str> {
        ["a.luau", "b.luau", "c.luau", "d.luau", "mine.luau"]
            .into_iter()
            .filter(|name| file(name).exists() && app.scripts.is_loaded(&file(name)))
            .collect()
    };
    assert_eq!(there(app), ["c.luau", "d.luau", "mine.luau"]);
    assert!(!file("a.luau.before-update").exists());
    assert_eq!(read(&file("b.luau.before-update")), "vars.b = 2");
    let gauges = |app: &Tab| -> Vec<String> {
        (app.rules.gauges.iter())
            .filter(|gauge| gauge.folder == "Pack")
            .map(|gauge| gauge.name.clone())
            .collect()
    };
    assert_eq!(gauges(app), ["Life", "Own", "Moves"]);
    // And so once more, with what was installed then.
    keep(
        &profiles,
        "pack",
        &pack_of(&[("d.luau", "vars.d = 1")], &[]),
    );
    app.official_changed();
    assert_eq!(there(app), ["d.luau", "mine.luau"]);
    assert_eq!(gauges(app), ["Own"]);
    clean_up(harness, &profiles);
}

/// What was installed with nothing noted of what it was is taken as the
/// plugin's: what it no longer has is put aside.
#[test]
fn an_update_puts_aside_what_is_not_known_to_be_the_players() {
    let profiles = temp("unknown");
    keep(&profiles, "pack", &pack("-- one"));
    let mut harness = playing(&profiles);
    let old = script_in(&profiles);
    let app = harness.state_mut().tab_mut();
    app.plugins_installed.insert("pack".into(), "older".into());
    app.keep_plugins();
    keep(&profiles, "pack", &pack_of(&[("new.luau", "")], &[]));
    app.official_changed();
    assert!(!old.exists() && !app.scripts.is_loaded(&old));
    assert!(app.scripts.is_loaded(&old.with_file_name("new.luau")));
    assert_eq!(read(&old.with_extension("luau.before-update")), "-- one");
    clean_up(harness, &profiles);
}

/// One from git, updated while it runs: what it no longer has stops, and
/// what it has anew runs.
#[test]
fn a_plugin_from_git_updated_runs_as_it_now_is() {
    let profiles = temp("git-clean");
    let folder = profiles.plugins_dir().join("gitpack");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("plugin.toml"),
        "name = \"Pack\"\ndefault_on = true\n",
    )
    .unwrap();
    std::fs::write(folder.join("old.luau"), "").unwrap();
    let mut harness = playing(&profiles);
    let app = harness.state_mut().tab_mut();
    let (old, new) = (
        script_in(&profiles).with_file_name("old.luau"),
        script_in(&profiles).with_file_name("new.luau"),
    );
    assert!(app.scripts.is_loaded(&old));
    std::fs::remove_file(folder.join("old.luau")).unwrap();
    std::fs::write(folder.join("new.luau"), "").unwrap();
    app.find_plugins();
    let plugin = app.plugin("gitpack").unwrap();
    app.git_plugin_updated(&plugin);
    assert!(!old.exists() && !app.scripts.is_loaded(&old));
    assert!(app.scripts.is_loaded(&new));
    clean_up(harness, &profiles);
}

/// A plugin brought up to date by a session whose rules are older than the
/// profile's, as one opened in Profiles before the player added a macro in
/// the game's: the macro is kept.
#[test]
fn an_update_keeps_the_rules_another_session_saved_since() {
    let profiles = temp("stale");
    keep(&profiles, "pack", &pack("-- one"));
    let mut playing_one = playing(&profiles);
    let mut other = window(Some(profiles.clone()));
    assert!(other.state_mut().tab_mut().select_profile(Some("P".into())));
    // The player adds a macro in the session they play in.
    let mine = Macro {
        send: "look".into(),
        ..Macro::default()
    };
    (playing_one.state_mut().tab_mut()).change_rules(|rules| rules.macros.push(mine.clone()));
    let saved = || crate::rules::Rules::load(&profiles.rules_path(Some("P"))).unwrap();
    assert_eq!(saved().macros.len(), 1);
    // The plugin comes anew, and the other session takes it.
    keep(&profiles, "pack", &pack("-- two"));
    other.state_mut().tab_mut().official_changed();
    let left = saved().macros.len();
    std::fs::remove_dir_all(profiles.official_dir().parent().unwrap()).unwrap();
    assert_eq!(left, 1, "the macro saved by the other session was lost");
}
