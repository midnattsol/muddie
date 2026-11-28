//! Plugins: offered, switched on and off, installed in a profile, downloaded and removed.

use super::*;

#[test]
fn a_plugin_for_the_mud_is_offered_and_switched_on_without_the_rest_starting_over() {
    let dir = std::env::temp_dir().join(format!("muddie-included-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Reinos", &crate::profile::Profile::default())
        .unwrap();
    let mut harness = window(Some(profiles.clone()));
    harness.set_size(egui::vec2(1300.0, 800.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    app.host = "rlmud.org".into();
    let reinos = &crate::plugins::fixture::reinos();
    let common = &crate::plugins::included(crate::plugins::STANDARD).unwrap();
    // What many MUDs send the same way is on to begin with; the plugin
    // of one MUD is not.
    assert!(app.plugin_on(common) && !app.plugin_on(reinos));
    app.scripts.exec(
        "keeps",
        "kept = 7 alias(\"^kept$\", function() echo(\"kept \" .. kept) end)",
    );
    // Connected to that MUD, its plugin is offered over the command line.
    let (events, _sent) = fake_connection(app);
    events.send(Event::Connected).unwrap();
    harness.run_steps(3);
    harness.get_by_label_contains("There is a plugin for Reinos de Leyenda");
    // Switched on there, it works at once: what the MUD says on its
    // channels has a tab, and nothing else was run again.
    harness.get_by_label("Enable").click();
    harness.run_steps(3);
    let gmcp = |package: &str, data: &str| Event::Gmcp {
        package: package.into(),
        data: data.into(),
    };
    events
        .send(gmcp(
            "Comm.MensajeCanal",
            r#"{"canal": "chat", "mensaje": "[Chat] Ana: hola"}"#,
        ))
        .unwrap();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert!(app.plugin_on(reinos) && app.plugin_offer.is_none());
    assert_eq!(app.panels.channels[0].name, "chat");
    app.scripts.on_input("kept");
    app.run_actions();
    assert_eq!(output_lines(app).last().map(String::as_str), Some("kept 7"));
    // It is kept with the profile, and not offered again.
    let saved = profiles.load("Reinos").unwrap();
    assert_eq!(saved.plugins_on, ["reinos-de-leyenda"]);
    events.send(Event::Connected).unwrap();
    harness.run_steps(3);
    assert!(harness.state().tab().plugin_offer.is_none());
    // Switched off, what it made is gone: its channels' tabs too, and
    // what the MUD says on them is not taken up.
    let app = harness.state_mut().tab_mut();
    app.set_plugin(reinos, false);
    assert!(app.panels.channels.is_empty());
    app.scripts.on_gmcp(
        "Comm.MensajeCanal",
        r#"{"canal": "chat", "mensaje": "otra"}"#,
    );
    app.run_actions();
    assert!(app.panels.channels.is_empty());
    // Switched off, it is as declined: neither offered again nor switched
    // on by itself on connecting to its MUD.
    app.offer_plugin_for("rlmud.org", "");
    app.offer_plugin_for("", "Reinos de Leyenda");
    assert!(app.plugin_offer.is_none());
    app.switch_on_plugin_for("rlmud.org");
    assert!(!app.plugin_on(reinos));
    assert_eq!(
        profiles.load("Reinos").unwrap().plugins_declined,
        ["reinos-de-leyenda"]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn what_many_muds_send_alike_is_a_plugin_that_can_be_switched_off() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let common = &crate::plugins::included(crate::plugins::STANDARD).unwrap();
    let (events, _sent) = fake_connection(app);
    let vitals = || Event::Gmcp {
        package: "Char.Vitals".into(),
        data: r#"{"hp": 80, "maxhp": 100}"#.into(),
    };
    let said = || Event::Gmcp {
        package: "Comm.Channel.Text".into(),
        data: r#"{"channel": "ooc", "text": "Ana: hola"}"#.into(),
    };
    // On, as it is to begin with: gauges and channels by themselves.
    events.send(vitals()).unwrap();
    events.send(said()).unwrap();
    harness.run_steps(3);
    let app = harness.state_mut().tab_mut();
    assert!(app.gauges.summary().contains("80 of 100"));
    assert_eq!(app.panels.channels.len(), 1);
    // Off, the client makes nothing of them by itself; scripts still
    // get them.
    app.set_plugin(common, false);
    app.gauges.clear();
    app.panels.channels.clear();
    app.scripts.exec(
        "t",
        r#"on("gmcp", "Char.Vitals", function(data) echo("hp " .. data.hp) end)"#,
    );
    events.send(vitals()).unwrap();
    events.send(said()).unwrap();
    harness.run_steps(3);
    let app = harness.state().tab();
    assert!(app.gauges.summary().is_empty() && app.panels.channels.is_empty());
    assert_eq!(output_lines(app).last().map(String::as_str), Some("hp 80"));
}

#[test]
fn what_the_mud_is_asked_for_follows_the_plugins_on_and_off() {
    let dir = std::env::temp_dir().join(format!("muddie-asked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles.save("Reinos", &Profile::default()).unwrap();
    let mut harness = window(Some(profiles));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    let reinos = &crate::plugins::fixture::reinos();
    let common = &crate::plugins::included(crate::plugins::STANDARD).unwrap();
    let (_events, mut sent) = fake_connection(app);
    let mut told = || -> Vec<Vec<String>> {
        std::iter::from_fn(|| sent.try_recv().ok())
            .filter_map(|command| match command {
                Command::GmcpSupports(modules) => Some(modules),
                _ => None,
            })
            .collect()
    };
    let usual = muddie_core::Config::default().gmcp_supports;
    // What is asked for to begin with: the client's own, and what the
    // standard plugin asks for.
    app.sync_modules();
    told();
    let standard = ["Char 1", "Room 1", "Comm.Channel 1"]
        .map(String::from)
        .to_vec();
    let standard = [usual.clone(), standard].concat();
    app.set_plugin(reinos, true);
    let with_comm = [standard.clone(), vec!["Comm 1".to_owned()]].concat();
    assert_eq!(told(), [with_comm]);
    // Switched off, what it asked for is no longer wanted.
    app.set_plugin(reinos, false);
    assert_eq!(told(), [standard]);
    // Nothing is said when nothing changed.
    app.run_actions();
    assert_eq!(told(), Vec::<Vec<String>>::new());
    // Without the standard plugin, only what is the client's own.
    app.set_plugin(common, false);
    assert_eq!(told(), [usual]);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A plugin's gauges go in the panels they name, as it is installed: the
/// panel of gauges a profile comes with, empty, takes the first name, and
/// one more is made for the next.
#[test]
fn a_plugin_puts_its_gauges_in_the_panels_they_name() {
    let dir = std::env::temp_dir().join(format!("muddie-gauge-panels-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles.save("Reinos", &Profile::default()).unwrap();
    let mut harness = window(Some(profiles));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    let reinos = &crate::plugins::fixture::reinos();
    app.set_plugin(reinos, true);
    let panel_of = |app: &Tab, gauge: &str| -> String {
        let named = &(app.rules.gauges.iter())
            .find(|rule| rule.name == gauge)
            .unwrap()
            .panel;
        let id = app.gauges_panel_of(named).unwrap();
        app.layout.panel(id).unwrap().name.clone()
    };
    for (gauge, panel) in [("Vida", "Puntos"), ("Fe", "Puntos")] {
        assert_eq!(panel_of(app, gauge), panel, "{gauge}");
    }
    // The one of the states, made by its script once there are some.
    (app.scripts).on_gmcp("Char.Status.Estados.set", r#"{"sangrado": {}}"#);
    app.run_actions();
    let of_gauges: Vec<&str> = (app.layout.panels.iter())
        .filter(|panel| panel.kind == crate::tab::layout::Kind::Gauges)
        .map(|panel| panel.name.as_str())
        .collect();
    assert_eq!(of_gauges, ["Puntos", "Efectos"]);
    assert!(!app.rules.gauges.iter().any(|rule| rule.name == "Escudo"));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A plugin that comes with Muddie is installed in the profile when it is
/// switched on, in sight in the editor and the profile's to change: its
/// scripts in a folder named after it, its gauges among the profile's.
/// Switched off, they are; it is not installed again unless asked, and then
/// what it installed is put back, and what was added is kept.
#[test]
fn a_plugin_that_comes_with_muddie_is_installed_in_sight_and_the_profiles() {
    let dir = std::env::temp_dir().join(format!("muddie-install-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    // A profile for that MUD, which makes nothing of the standard GMCP.
    let reinos_only = Profile {
        plugins_off: vec![crate::plugins::STANDARD.into()],
        ..Profile::default()
    };
    profiles.save("Reinos", &reinos_only).unwrap();
    let mut harness = window(Some(profiles.clone()));
    harness.set_size(egui::vec2(1300.0, 860.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    let plugin = &crate::plugins::fixture::reinos();
    app.set_plugin(plugin, true);
    let script = (profiles.scripts_dir("Reinos"))
        .join("Reinos de Leyenda")
        .join("reinos.luau");
    assert_eq!(
        std::fs::read_to_string(&script).unwrap(),
        crate::plugins::fixture::SCRIPT
    );
    assert!(app.scripts.is_loaded(&script));
    // Each finds the others, as files and run as they come.
    assert_eq!(app.script_errors, [] as [String; 0]);
    let rules = || Rules::load(&profiles.rules_path(Some("Reinos"))).unwrap();
    let gauges = |rules: &Rules| -> Vec<(String, bool)> {
        (rules.gauges.iter())
            .map(|gauge| (gauge.name.clone(), gauge.enabled))
            .collect()
    };
    let all = |on: bool| -> Vec<(String, bool)> {
        ["Vida", "Energía", "Fe"]
            .map(|name| (name.to_owned(), on))
            .to_vec()
    };
    assert_eq!(gauges(&rules()), all(true));
    let saved = profiles.load("Reinos").unwrap();
    assert!(saved.plugins_installed[&plugin.id].starts_with(&format!("{}:", plugin.fingerprint())));
    // Changed by the player: theirs.
    let mine = "-- mine\n";
    std::fs::write(&script, mine).unwrap();
    let app = harness.state_mut().tab_mut();
    app.change_rules(|rules| {
        rules.gauges[0].colour = "#ffffff".into();
        rules.gauges[2].enabled = false;
    });
    // In its folder, as its scripts are.
    assert!(
        rules()
            .gauges
            .iter()
            .all(|gauge| gauge.folder == "Reinos de Leyenda")
    );
    // Switched off, its folders are; on again, they are, and what the
    // player switched off in them stays so, and nothing is installed again.
    let mine_off = |mut all: Vec<(String, bool)>| {
        all[2].1 = false;
        all
    };
    app.set_plugin(plugin, false);
    assert!(!app.scripts.is_loaded(&script));
    assert!(!rules().folder_on(Kind::Script, "Reinos de Leyenda"));
    assert!(!rules().folder_on(Kind::Gauge, "Reinos de Leyenda"));
    app.set_plugin(plugin, true);
    assert!(app.scripts.is_loaded(&script));
    assert!(rules().folder_on(Kind::Gauge, "Reinos de Leyenda"));
    assert_eq!(gauges(&rules()), mine_off(all(true)));
    assert_eq!(std::fs::read_to_string(&script).unwrap(), mine);
    assert_eq!(rules().gauges[0].colour, "#ffffff");

    // Installed with another version of it: brought up to date by
    // itself, once, and the script the player changed kept beside it.
    let older = |app: &mut Tab, mark: &str| {
        let mut saved = profiles.load("Reinos").unwrap();
        saved
            .plugins_installed
            .insert(plugin.id.clone(), mark.into());
        profiles.save("Reinos", &saved).unwrap();
        app.plugins_installed.insert(plugin.id.clone(), mark.into());
    };
    let app = harness.state_mut().tab_mut();
    older(app, "older");
    app.load_scripts();
    app.load_scripts();
    let updated = (notes(app).iter())
        .filter(|said| said.contains("brought up to date"))
        .count();
    assert_eq!(updated, 1);
    assert_eq!(
        std::fs::read_to_string(&script).unwrap(),
        crate::plugins::fixture::SCRIPT
    );
    let kept = script.with_file_name(format!(
        "{}.before-update",
        crate::plugins::fixture::SCRIPT_NAME
    ));
    assert_eq!(std::fs::read_to_string(&kept).unwrap(), mine);
    // Scripts as they were installed are not kept: there is nothing of
    // the player's in them.
    std::fs::remove_file(&kept).unwrap();
    let written = app.plugins_installed[&plugin.id]
        .split_once(':')
        .unwrap()
        .1
        .to_owned();
    older(app, &format!("older:{written}"));
    app.load_scripts();
    assert!(!kept.exists());
    // Installed again from its menu in the profile's plugins: what it
    // installed is as it came, what was added is kept.
    app.change_rules(|rules| rules.gauges.push(Default::default()));
    harness.state_mut().edit_profile("Reinos");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(egui::accesskit::Role::Tab, "Plugins")
        .click();
    harness.run_steps(3);
    harness
        .get_by_label("Reinos de Leyenda options")
        .click_accesskit();
    harness.run_steps(2);
    harness.get_by_label("Install again").click_accesskit();
    harness.run_steps(4);
    assert_eq!(
        std::fs::read_to_string(&script).unwrap(),
        crate::plugins::fixture::SCRIPT
    );
    let after = rules();
    assert_eq!(after.gauges.len(), 4);
    assert_eq!(after.gauges[0].colour, "#5fb85f");
    let saved = profiles.load("Reinos").unwrap();
    assert!(saved.plugins_installed[&plugin.id].starts_with(&format!("{}:", plugin.fingerprint())));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Without a profile there is nowhere to install it: it is run as it
/// comes, for the session, and goes when switched off.
#[test]
fn a_plugin_that_comes_with_muddie_runs_as_it_comes_without_a_profile() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let plugin = &crate::plugins::fixture::reinos();
    app.set_plugin(plugin, true);
    assert!(app.plugin_on(plugin));
    assert_eq!(app.script_errors, [] as [String; 0]);
    let theirs = |app: &Tab| {
        (app.rules.gauges.iter())
            .filter(|gauge| gauge.folder == "Reinos de Leyenda")
            .count()
    };
    assert_eq!(theirs(app), 3);
    assert!(app.scripts.supports().contains(&"Comm 1".to_owned()));
    app.set_plugin(plugin, false);
    assert_eq!(theirs(app), 0);
    assert!(!app.scripts.supports().contains(&"Comm 1".to_owned()));
}

/// A plugin downloaded from git is a plugin as those that come with
/// Muddie: switched on, it is installed in the profile, its scripts and its
/// gauges in folders of its name; removed, they go, and so does the
/// download, which no other profile has.
#[test]
fn a_plugin_downloaded_is_installed_and_removed_as_any_other() {
    let dir = std::env::temp_dir().join(format!("muddie-downloaded-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let source = dir.join("source");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("main.luau"), "gmcp.support(\"Combat 1\")").unwrap();
    std::fs::write(
        source.join("plugin.toml"),
        "about = { en = \"Fights.\", es = \"Peleas.\" }\nhosts = [\"combat.example.org\"]\n",
    )
    .unwrap();
    std::fs::write(
        source.join("rules.toml"),
        "[[gauge]]\nname = \"Rage\"\nvalue = \"Char.Rage\"\nmax = \"100\"\n\
         [[trigger]]\nname = \"Flee\"\npattern = \"hurts\"\nsend = \"flee\"\n",
    )
    .unwrap();
    let git = |arguments: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(["-c", "user.name=test", "-c", "user.email=test@example.org"])
            .args(arguments)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {arguments:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "first"]);
    let profiles = Profiles::at(dir.join("config"));
    profiles.save("Main", &Profile::default()).unwrap();
    let mut harness = window(Some(profiles.clone()));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Main".into())));
    // Typed, it is asked first; said yes to, it is downloaded.
    assert!(app.do_client_command(&format!("/plugin install {} combat", source.display())));
    assert!(matches!(
        app.take_app_request(),
        Some(AppRequest::InstallPlugin { .. })
    ));
    app.install_plugin(&format!("{} combat", source.display()));
    for _ in 0..300 {
        harness.run_steps(1);
        if harness.state().tab().plugin_jobs.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let app = harness.state_mut().tab_mut();
    let script = profiles
        .scripts_dir("Main")
        .join("combat")
        .join("main.luau");
    assert!(app.scripts.is_loaded(&script));
    assert!(app.scripts.supports().contains(&"Combat 1".to_owned()));
    // What it says of itself, as one that comes with Muddie does.
    let plugin = app.plugin("combat").unwrap();
    assert_eq!(
        (plugin.shown_name(), plugin.about()),
        ("combat".into(), "Fights.".into())
    );
    assert!(crate::plugins::for_mud(app.plugins(), "combat.example.org", "").is_some());
    let rage = app.rules.gauges.iter().find(|gauge| gauge.name == "Rage");
    assert_eq!(rage.map(|gauge| gauge.folder.as_str()), Some("combat"));
    // Every kind of rule it brings, as gauges are.
    let flee = app.rules.triggers.iter().find(|rule| rule.name == "Flee");
    assert_eq!(flee.map(|rule| rule.folder.as_str()), Some("combat"));
    assert!(
        profiles
            .load("Main")
            .unwrap()
            .plugins_on
            .contains(&"combat".to_owned())
    );
    // Switched off, its folders are, as any plugin's.
    assert!(app.do_client_command("/plugin disable combat"));
    assert!(!app.scripts.is_loaded(&script));
    assert!(!app.rules.folder_on(Kind::Gauge, "combat"));
    // Removed, all it installed goes, and the download with it.
    app.remove_plugin("combat");
    assert!(!script.exists() && !profiles.plugins_dir().join("combat").exists());
    assert!(!app.rules.gauges.iter().any(|gauge| gauge.name == "Rage"));
    assert!(app.rules.triggers.is_empty());
    assert!(app.plugin("combat").is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// What is done with a profile's plugins from the Profiles window is done
/// in that profile, whichever the session on screen plays, and what it
/// answers is what the client said of it.
#[test]
fn a_profile_s_plugins_are_its_own_whatever_is_played() {
    let dir = std::env::temp_dir().join(format!("muddie-edited-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles.save("Played", &Profile::default()).unwrap();
    profiles.save("Edited", &Profile::default()).unwrap();
    let mut harness = window(Some(profiles.clone()));
    assert!(
        harness
            .state_mut()
            .tab_mut()
            .select_profile(Some("Played".into()))
    );
    let app = harness.state_mut();
    let edited = app.session_of(Some("Edited"));
    let before = edited.said_count();
    assert!(edited.do_client_command("/plugin enable reinos-de-leyenda"));
    let said = edited.said_since(before);
    assert_eq!(
        said,
        [("Plugin Reinos de Leyenda enabled.".to_owned(), false)]
    );
    let on = |name: &str| profiles.load(name).unwrap().plugins_on;
    assert_eq!(on("Edited"), ["reinos-de-leyenda"]);
    assert!(on("Played").is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Connecting to a MUD that has a plugin switches it on first, in a
/// profile filled in by hand too: what it asks the MUD for is asked for
/// from the start, as some MUDs send what they send once only then.
#[test]
fn the_plugin_of_the_mud_is_on_before_connecting_to_it() {
    let dir = std::env::temp_dir().join(format!("muddie-before-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save("Mine", &crate::profile::Profile::default())
        .unwrap();
    let mut harness = window(Some(profiles.clone()));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Mine".into())));
    let reinos = &crate::plugins::fixture::reinos();
    app.switch_on_plugin_for("rlmud.org");
    assert!(app.plugin_on(reinos));
    assert!(
        app.wanted_modules()
            .iter()
            .any(|module| module.starts_with("Room"))
    );
    let said = app.messages().0.last().map(|message| message.text.clone());
    assert!(said.is_some_and(|said| said.starts_with("Reinos de Leyenda is on")));
    assert_eq!(
        profiles.load("Mine").unwrap().plugins_on,
        ["reinos-de-leyenda"]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A git plugin whose manifest names it after another download's
/// folder: removing it removes that other download, and leaves its own.
#[test]
fn removing_a_named_git_plugin_removes_its_own_download() {
    let dir = std::env::temp_dir().join(format!("muddie-plugin-rm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles.save("Main", &Profile::default()).unwrap();
    let downloads = profiles.plugins_dir();
    for (id, manifest) in [("pack", "name = \"other\"\n"), ("other", "")] {
        std::fs::create_dir_all(downloads.join(id)).unwrap();
        std::fs::write(downloads.join(id).join(format!("{id}.luau")), "-- x").unwrap();
        std::fs::write(downloads.join(id).join("plugin.toml"), manifest).unwrap();
    }
    let mut harness = window(Some(profiles.clone()));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Main".into())));
    assert!(app.do_client_command("/plugin enable pack"));
    app.remove_plugin("pack");
    assert!(
        downloads.join("other").exists(),
        "the download of another plugin, 'other', was deleted"
    );
    assert!(
        !downloads.join("pack").exists(),
        "the download of 'pack' was left"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A git plugin named as a folder of the player's own scripts:
/// removing the plugin deletes the player's scripts in that folder.
#[test]
fn removing_a_plugin_keeps_the_players_own_folder() {
    let dir = std::env::temp_dir().join(format!("muddie-plugin-own-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles.save("Main", &Profile::default()).unwrap();
    let mine = profiles
        .scripts_dir("Main")
        .join("Combat")
        .join("mine.luau");
    std::fs::create_dir_all(mine.parent().unwrap()).unwrap();
    std::fs::write(&mine, "-- written by the player").unwrap();
    let download = profiles.plugins_dir().join("pack");
    std::fs::create_dir_all(&download).unwrap();
    std::fs::write(download.join("pack.luau"), "-- x").unwrap();
    std::fs::write(download.join("plugin.toml"), "name = \"Combat\"\n").unwrap();
    let mut harness = window(Some(profiles.clone()));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Main".into())));
    assert!(app.do_client_command("/plugin enable pack"));
    app.remove_plugin("pack");
    let kept = mine.exists();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(kept, "the player's own script Combat/mine.luau was deleted");
}
