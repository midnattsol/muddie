//! Profiles and plugins.

use super::*;

#[test]
fn a_session_is_saved_as_a_profile_that_brings_its_scripts() {
    let dir = std::env::temp_dir().join(format!("muddie-app-profile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut harness = window(Some(Profiles::at(dir.clone())));
    let app = harness.state_mut().tab_mut();
    app.host = "mud.example.org".into();
    app.port = "4000".into();
    app.tls = true;
    app.verify_certificate = false;
    // Asked for a name, with one made from the MUD's address.
    app.app_request = Some(AppRequest::SaveAsProfile);
    harness.run_steps(3);
    harness.get_by_label("Save").click();
    harness.run_steps(3);
    let saved = Profiles::at(dir.clone()).load("Mud").unwrap();
    assert_eq!((saved.host.as_str(), saved.port), ("mud.example.org", 4000));
    assert!(saved.tls && !saved.verify_certificate);
    assert_eq!(harness.state().tab().profile.as_deref(), Some("Mud"));

    // Scripts dropped in the profile's folder run when it is selected.
    std::fs::write(
        dir.join("profiles/Mud/scripts/hello.luau"),
        "echo('hola desde el perfil')",
    )
    .unwrap();
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(None));
    app.host.clear();
    assert!(app.select_profile(Some("Mud".into())));
    assert_eq!(app.host, "mud.example.org");
    assert!(output_lines(app).contains(&"hola desde el perfil".to_owned()));
    assert!(!app.select_profile(Some("Missing".into())));
    std::fs::remove_dir_all(&dir).unwrap();
}

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
    let reinos = &crate::plugins::included("reinos-de-leyenda").unwrap();
    let common = &crate::plugins::included(crate::plugins::COMMON).unwrap();
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
    // Switched off, what it made is gone.
    let app = harness.state_mut().tab_mut();
    app.set_plugin(reinos, false);
    let before = app.panels.channels[0].log.len();
    app.scripts.on_gmcp(
        "Comm.MensajeCanal",
        r#"{"canal": "chat", "mensaje": "otra"}"#,
    );
    app.run_actions();
    assert_eq!(app.panels.channels[0].log.len(), before);
    // Declined instead, it is not offered again in this profile either.
    app.offer_plugin_for("rlmud.org", "");
    assert!(app.plugin_offer.is_some());
    app.decline_plugin();
    app.offer_plugin_for("", "Reinos de Leyenda");
    assert!(app.plugin_offer.is_none());
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
    let common = &crate::plugins::included(crate::plugins::COMMON).unwrap();
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
        r#"on_gmcp("Char.Vitals", function(data) echo("hp " .. data.hp) end)"#,
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
    let reinos = &crate::plugins::included("reinos-de-leyenda").unwrap();
    let common = &crate::plugins::included(crate::plugins::COMMON).unwrap();
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
    let reinos = &crate::plugins::included("reinos-de-leyenda").unwrap();
    app.set_plugin(reinos, true);
    let panel_of = |app: &Tab, gauge: &str| -> String {
        let id = (app.rules.gauges.iter())
            .find(|rule| rule.name == gauge)
            .unwrap()
            .panel;
        app.layout
            .panels
            .iter()
            .find(|panel| panel.id == id)
            .unwrap()
            .name
            .clone()
    };
    for (gauge, panel) in [("Vida", "Puntos"), ("Fe", "Puntos"), ("Efectos", "Efectos")] {
        assert_eq!(panel_of(app, gauge), panel, "{gauge}");
    }
    let of_gauges: Vec<&str> = (app.layout.panels.iter())
        .filter(|panel| panel.kind == crate::tab::layout::Kind::Gauges)
        .map(|panel| panel.name.as_str())
        .collect();
    assert_eq!(of_gauges, ["Puntos", "Efectos"]);
    assert!(!app.rules.gauges.iter().any(|rule| rule.name == "Escudo"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_profile_keeps_what_is_changed_in_the_settings_as_it_is_changed() {
    let dir = std::env::temp_dir().join(format!("muddie-autosave-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    profiles
        .save(
            "Reinos",
            &Profile {
                host: "rlmud.org".into(),
                port: 5001,
                ..Profile::default()
            },
        )
        .unwrap();
    let mut harness = window(Some(profiles));
    harness.set_size(egui::vec2(1300.0, 860.0));
    assert!(
        harness
            .state_mut()
            .tab_mut()
            .select_profile(Some("Reinos".into()))
    );
    harness.state_mut().edit_profile("Reinos");
    harness.run_steps(3);
    let character = || (egui::accesskit::Role::TextInput, "Character");
    harness
        .get_by_role_and_label(character().0, character().1)
        .focus();
    harness.run_steps(2);
    (harness.get_by_role_and_label(character().0, character().1)).type_text("Naylhena");
    harness.run_steps(2);
    // What depends on the MUD is the profile's, not every session's.
    harness
        .get_by_role_and_label(egui::accesskit::Role::Tab, "Session")
        .click();
    harness.run_steps(2);
    (harness.get_by_role_and_label(egui::accesskit::Role::Switch, "Auto-reconnect"))
        .click_accesskit();
    harness.run_steps(2);
    // Closing the window keeps what was changed, without a button, and
    // the session playing with the profile takes it.
    harness.get_by_label("Close").click();
    harness.run_steps(3);
    assert_eq!(harness.state().tab().character, "Naylhena");
    let saved = Profiles::at(dir.clone()).load("Reinos").unwrap();
    assert_eq!(saved.character, "Naylhena");
    // It was on, as it is to begin with: off in this profile only.
    assert_eq!(saved.session.map(|session| session.reconnect), Some(false));
    assert!(crate::profile::SessionPrefs::default().reconnect);
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
        plugins_off: vec![crate::plugins::COMMON.into()],
        ..Profile::default()
    };
    profiles.save("Reinos", &reinos_only).unwrap();
    let mut harness = window(Some(profiles.clone()));
    harness.set_size(egui::vec2(1300.0, 860.0));
    let app = harness.state_mut().tab_mut();
    assert!(app.select_profile(Some("Reinos".into())));
    let plugin = &crate::plugins::included("reinos-de-leyenda").unwrap();
    app.set_plugin(plugin, true);
    let script = (profiles.scripts_dir("Reinos"))
        .join("Reinos de Leyenda")
        .join("Reinos de Leyenda.luau");
    assert_eq!(
        std::fs::read_to_string(&script).unwrap(),
        plugin.scripts[0].1
    );
    assert!(app.scripts.is_loaded(&script));
    let rules = || Rules::load(&profiles.rules_path(Some("Reinos"))).unwrap();
    let gauges = |rules: &Rules| -> Vec<(String, bool)> {
        (rules.gauges.iter())
            .map(|gauge| (gauge.name.clone(), gauge.enabled))
            .collect()
    };
    let all = |on: bool| -> Vec<(String, bool)> {
        ["Vida", "Energía", "Fe", "Efectos"]
            .map(|name| (name.to_owned(), on))
            .to_vec()
    };
    assert_eq!(gauges(&rules()), all(true));
    let saved = profiles.load("Reinos").unwrap();
    assert_eq!(saved.plugins_installed[&plugin.id], plugin.fingerprint());
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

    // Installed with another version of it: said, once.
    let mut saved = profiles.load("Reinos").unwrap();
    saved
        .plugins_installed
        .insert(plugin.id.clone(), "older".into());
    profiles.save("Reinos", &saved).unwrap();
    let app = harness.state_mut().tab_mut();
    app.plugins_installed
        .insert(plugin.id.clone(), "older".into());
    app.load_scripts();
    app.load_scripts();
    let changed = (notes(app).iter())
        .filter(|said| said.contains("has changed since it was installed"))
        .count();
    assert_eq!(changed, 1);
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
        plugin.scripts[0].1
    );
    let after = rules();
    assert_eq!(after.gauges.len(), 5);
    assert_eq!(after.gauges[0].colour, "#5fb85f");
    let saved = profiles.load("Reinos").unwrap();
    assert_eq!(saved.plugins_installed[&plugin.id], plugin.fingerprint());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Without a profile there is nowhere to install it: it is run as it
/// comes, for the session, and goes when switched off.
#[test]
fn a_plugin_that_comes_with_muddie_runs_as_it_comes_without_a_profile() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let plugin = &crate::plugins::included("reinos-de-leyenda").unwrap();
    app.set_plugin(plugin, true);
    assert!(app.plugin_on(plugin));
    let theirs = |app: &Tab| {
        (app.rules.gauges.iter())
            .filter(|gauge| gauge.folder == "Reinos de Leyenda")
            .count()
    };
    assert_eq!(theirs(app), 4);
    assert!(app.scripts.supports().contains(&"Comm 1".to_owned()));
    app.set_plugin(plugin, false);
    assert_eq!(theirs(app), 0);
    assert!(!app.scripts.supports().contains(&"Comm 1".to_owned()));
}

#[test]
fn welcome_screen_lists_the_profiles_and_gives_way_once_connecting() {
    let dir = std::env::temp_dir().join(format!("muddie-app-welcome-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    let profile = Profile {
        host: "reinos.example.org".into(),
        port: 4000,
        ..Profile::default()
    };
    profiles.save("Reinos", &profile).unwrap();
    let mut harness = window(Some(profiles));
    harness.state_mut().tab_mut().started = false;
    harness.run_steps(3);
    assert!(harness.query_by_label("Reinos").is_some());
    assert!(
        harness
            .query_by_label("Where do you want to play?")
            .is_some()
    );
    // The session's own controls are not there yet.
    assert!(harness.query_by_label("Command").is_none());
    harness.state_mut().tab_mut().started = true;
    harness.run_steps(3);
    assert!(harness.query_by_label("Command").is_some());
    assert!(
        harness
            .query_by_label("Where do you want to play?")
            .is_none()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A new profile can be made for a game Muddie has a plugin for: with its
/// address and its plugin on, named after it.
#[test]
fn a_new_profile_is_made_for_a_game_with_its_address_and_plugin() {
    let dir = std::env::temp_dir().join(format!("muddie-game-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    let mut harness = window(Some(profiles.clone()));
    harness.set_size(egui::vec2(1300.0, 860.0));
    harness.state_mut().open_profiles(None);
    harness.run_steps(2);
    // In the list and on the page, with no profile yet: either.
    harness
        .get_all_by_label("New profile")
        .next()
        .unwrap()
        .click();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(egui::accesskit::Role::ComboBox, "Game")
        .click();
    harness.run_steps(2);
    harness.get_by_label("Reinos de Leyenda").click();
    harness.run_steps(2);
    harness.get_by_label_contains("Ready to play Reinos de Leyenda");
    harness.get_by_label("Create").click();
    harness.run_steps(3);
    let saved = profiles.load("Reinos de Leyenda").unwrap();
    assert_eq!(
        (saved.host.as_str(), saved.port, saved.tls),
        ("reinosdeleyenda.es", 5001, false)
    );
    assert_eq!(saved.plugins_on, ["reinos-de-leyenda"]);
    std::fs::remove_dir_all(&dir).unwrap();
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
    std::fs::write(source.join("main.luau"), "support_gmcp(\"Combat 1\")").unwrap();
    std::fs::write(
        source.join("rules.toml"),
        "[[gauge]]\nname = \"Rage\"\nvalue = \"Char.Rage\"\nmax = \"100\"\n",
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
    app.plugin_command(&format!("/plugin install {} combat", source.display()));
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
    let rage = app.rules.gauges.iter().find(|gauge| gauge.name == "Rage");
    assert_eq!(rage.map(|gauge| gauge.folder.as_str()), Some("combat"));
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
    assert!(app.plugin("combat").is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}
