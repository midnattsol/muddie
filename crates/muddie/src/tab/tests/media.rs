//! The sounds the MUD asks for.

use super::*;

#[test]
fn sounds_a_mud_asks_for_are_played_from_the_players_own_or_fetched() {
    let dir = std::env::temp_dir().join(format!("muddie-media-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    let sounds = profiles.sound_dirs(None).remove(0);
    std::fs::create_dir_all(&sounds).unwrap();
    std::fs::write(sounds.join("own.wav"), b"RIFF").unwrap();
    let mut harness = window(Some(profiles));
    let app = harness.state_mut().tab_mut();
    // One the player has is found there, whatever address is given.
    let found = app.media_file("own.wav", Some("https://mud.example.org/s"));
    assert_eq!(found, Some((String::new(), Some(sounds.join("own.wav")))));
    // One they do not have is on the web, at the address the message
    // gives, or the one given for all of them.
    assert_eq!(app.media_file("far.wav", None), None);
    app.media(crate::media::Media::Default(
        "https://mud.example.org/sounds/".into(),
    ));
    let (address, here) = app.media_file("far.wav", None).unwrap();
    assert_eq!(address, "https://mud.example.org/sounds/far.wav");
    assert_eq!(here, None);
    // Nothing is looked for outside its folder, nor that is no sound.
    assert_eq!(app.media_file("../own.wav", None), None);
    assert_eq!(app.media_file("/etc/passwd", None), None);
    assert_eq!(app.media_file("script.sh", None), None);
    // With fetching switched off, nothing is asked of the web; nor is
    // what could not be had before.
    app.prefs.media_downloads = false;
    app.media_fetch(address.clone(), None);
    assert!(app.fetching.jobs.is_empty());
    app.prefs.media_downloads = true;
    app.fetching.missing.push(address.clone());
    app.media_fetch(address, None);
    assert!(app.fetching.jobs.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_sound_of_the_sound_protocol_is_found_by_kind_and_by_pattern() {
    let dir = std::env::temp_dir().join(format!("muddie-msp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let profiles = Profiles::at(dir.clone());
    let sounds = profiles.sound_dirs(None).remove(0);
    std::fs::create_dir_all(sounds.join("combat")).unwrap();
    for name in [
        "combat/hit1.wav",
        "combat/hit2.wav",
        "combat/miss.wav",
        "door.wav",
    ] {
        std::fs::write(sounds.join(name), b"RIFF").unwrap();
    }
    let mut harness = window(Some(profiles));
    let app = harness.state_mut().tab_mut();
    // A name with a star is one of the files it stands for.
    let found = app.sound_among("combat/hit*.wav").unwrap();
    assert!(
        ["combat/hit1.wav", "combat/hit2.wav"].contains(&found.as_str()),
        "{found}"
    );
    assert_eq!(app.sound_among("d??r.wav").as_deref(), Some("door.wav"));
    assert_eq!(app.sound_among("combat/parry*.wav"), None);
    // One without is looked for as it is, and none out of the folder.
    assert_eq!(app.sound_among("door.wav"), None);
    assert_eq!(app.sound_among("../*.wav"), None);
    // Stopping may say where files are fetched from when none is given.
    app.msp(Msp::Stop {
        music: true,
        url: Some("http://mud.example/m/".into()),
    });
    assert_eq!(app.fetching.base.as_deref(), Some("http://mud.example/m/"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn sound_requests_from_the_server_are_not_shown() {
    let mut harness = window(None);
    let app = harness.state_mut().tab_mut();
    let (events, _sent) = fake_connection(app);
    for text in [
        "before",
        "!!SOUND(missing.wav V=50)",
        "!!MUSIC(Off)",
        "after",
    ] {
        events
            .send(Event::Line(panels::styled_lines(text).remove(0)))
            .unwrap();
    }
    harness.run_steps(2);
    assert_eq!(output_lines(harness.state().tab()), ["before", "after"]);
}
