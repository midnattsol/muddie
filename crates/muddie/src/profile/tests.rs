//! Tests of profiles kept in their folders.

use super::*;

fn scratch(test: &str) -> Profiles {
    let dir = std::env::temp_dir().join(format!("muddie-profile-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Profiles::at(dir)
}

#[test]
fn a_profile_is_renamed_copied_and_deleted_with_all_it_has() {
    let profiles = scratch("folders");
    profiles
        .save(
            "Reinos",
            &Profile {
                port: 5001,
                ..Profile::default()
            },
        )
        .unwrap();
    std::fs::write(profiles.scripts_dir("Reinos").join("a.luau"), "x = 1").unwrap();
    std::fs::write(profiles.map_path("Reinos"), "map").unwrap();
    profiles.duplicate("Reinos", "Reinos 2").unwrap();
    assert_eq!(profiles.load("Reinos 2").unwrap().port, 5001);
    assert!(profiles.scripts_dir("Reinos 2").join("a.luau").is_file());
    assert!(profiles.map_path("Reinos 2").is_file());
    // Not over another, nor under a name that cannot be a folder's.
    assert!(profiles.duplicate("Reinos", "Reinos 2").is_err());
    assert!(profiles.rename("Reinos", "a/b").is_err());
    profiles.rename("Reinos 2", "Naylhena").unwrap();
    assert_eq!(profiles.names(), ["Naylhena", "Reinos"]);
    assert_eq!(profiles.unused_name("Reinos"), "Reinos 2");
    assert_eq!(profiles.unused_name("Nuevo"), "Nuevo");
    profiles.delete("Naylhena").unwrap();
    assert_eq!(profiles.names(), ["Reinos"]);
    std::fs::remove_dir_all(profiles.dir).unwrap();
}

#[test]
fn profile_round_trips_and_lists() {
    let profiles = scratch("roundtrip");
    assert_eq!(profiles.names(), Vec::<String>::new());
    let profile = Profile {
        host: "mud.example.org".into(),
        port: 4000,
        tls: true,
        verify_certificate: false,
        character: "Gandalf".into(),
        encoding: "koi8-r".into(),
        language: "es".into(),
        plugins_on: vec!["reinos-de-leyenda".into()],
        plugins_off: vec!["standard".into()],
        plugins_declined: Vec::new(),
        plugins_installed: Default::default(),
        session: Some(SessionPrefs {
            reconnect: false,
            ..SessionPrefs::default()
        }),
        old_packages: Vec::new(),
    };
    profiles.save("Reinos", &profile).unwrap();
    profiles.save("Aardwolf", &Profile::default()).unwrap();
    assert_eq!(profiles.names(), ["Aardwolf", "Reinos"]);
    assert_eq!(profiles.load("Reinos").unwrap(), profile);
    assert!(profiles.load("Nope").is_err());
    assert_eq!(
        profiles.map_path("Reinos"),
        profiles.dir.join("profiles/Reinos/map.sqlite")
    );
    std::fs::remove_dir_all(&profiles.dir).unwrap();
}

#[test]
fn file_with_missing_fields_takes_defaults() {
    let profiles = scratch("defaults");
    let dir = profiles.dir.join("profiles/Old");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("profile.toml"), "host = \"old.example.org\"\n").unwrap();
    let profile = profiles.load("Old").unwrap();
    assert_eq!(
        (profile.host.as_str(), profile.port),
        ("old.example.org", 23)
    );
    assert!(profile.verify_certificate);
    std::fs::write(dir.join("profile.toml"), "port = \"many\"\n").unwrap();
    assert!(profiles.load("Old").unwrap_err().contains("profile.toml"));
    std::fs::remove_dir_all(&profiles.dir).unwrap();
}

#[test]
fn names_that_are_not_plain_are_refused() {
    let profiles = scratch("names");
    for name in ["", " ", "../evil", "a/b", " padded", "dot.dot"] {
        assert!(!valid_name(name), "{name:?}");
        assert!(profiles.save(name, &Profile::default()).is_err());
    }
    assert!(valid_name("Reinos de Leyenda-2_b"));
    assert!(!profiles.dir.exists());
}

#[test]
fn own_scripts_are_in_order_and_old_packages_become_plugins() {
    let profiles = scratch("scripts");
    profiles.save("Main", &Profile::default()).unwrap();
    let own = profiles.dir.join("profiles/Main/scripts");
    for file in ["20-triggers.luau", "10-setup.luau", "notes.txt"] {
        std::fs::write(own.join(file), "").unwrap();
    }
    let relative: Vec<String> = (profiles.own_scripts("Main").iter())
        .map(|path| {
            // Each part, joined with / on every system.
            let inner = path.strip_prefix(&profiles.dir).unwrap();
            let parts: Vec<_> = inner.iter().map(|part| part.to_string_lossy()).collect();
            parts.join("/")
        })
        .collect();
    assert_eq!(
        relative,
        [
            "profiles/Main/scripts/10-setup.luau",
            "profiles/Main/scripts/20-triggers.luau",
        ]
    );
    // A profile of an earlier version ran plugins downloaded from where
    // they were: they are switched on, to be installed in it, and the
    // folder they were downloaded to is the plugins' now.
    let file = profiles.dir.join("profiles/Main/profile.toml");
    std::fs::write(
        &file,
        "packages = [\"combat\"]\nincluded_off = [\"common\"]\n",
    )
    .unwrap();
    let loaded = profiles.load("Main").unwrap();
    assert_eq!(
        (loaded.plugins_on, loaded.plugins_off),
        (vec!["combat".to_owned()], vec!["standard".to_owned()])
    );
    std::fs::create_dir_all(profiles.dir.join("packages/combat")).unwrap();
    assert!(profiles.plugins_dir().join("combat").is_dir());
    assert_eq!(profiles.init_script(), None);
    std::fs::write(profiles.dir.join("init.luau"), "").unwrap();
    assert!(profiles.init_script().is_some());
    std::fs::remove_dir_all(&profiles.dir).unwrap();
}
