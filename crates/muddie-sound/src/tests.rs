//! Tests of the sound requests, file names and the player's arguments.

use super::*;

#[test]
fn msp_triggers_are_read() {
    let sound = |file: &str, volume, times, priority, kind: &str, url: Option<&str>| {
        Some(Msp::Play {
            music: false,
            file: file.into(),
            volume,
            times,
            priority,
            keep_going: false,
            kind: kind.into(),
            url: url.map(String::from),
        })
    };
    assert_eq!(
        parse_msp("!!SOUND(sword.wav)"),
        sound("sword.wav", 1.0, Some(1), 50, "", None)
    );
    // With no ending, the usual one; and all it can say.
    assert_eq!(
        parse_msp("  !!SOUND(hit* V=80 L=-1 P=70 T=combat U=http://mud.example/s/)  "),
        sound(
            "hit*.wav",
            0.8,
            None,
            70,
            "combat",
            Some("http://mud.example/s/")
        )
    );
    assert_eq!(
        parse_msp("!!SOUND(a.wav l=3 v=250)"),
        sound("a.wav", 1.0, Some(3), 50, "", None)
    );
    // Music goes on unless told to start again, and has no priority.
    let music = |keep_going| {
        Some(Msp::Play {
            music: true,
            file: "town.mid".into(),
            volume: 0.5,
            times: Some(1),
            priority: 0,
            keep_going,
            kind: String::new(),
            url: None,
        })
    };
    assert_eq!(parse_msp("!!MUSIC(town V=50)"), music(true));
    assert_eq!(parse_msp("!!MUSIC(town V=50 C=0)"), music(false));
    // Off stops the sounds or the music, and may say where files are.
    assert_eq!(
        parse_msp("!!SOUND(Off)"),
        Some(Msp::Stop {
            music: false,
            url: None
        })
    );
    assert_eq!(
        parse_msp("!!MUSIC(off U=http://mud.example/m/)"),
        Some(Msp::Stop {
            music: true,
            url: Some("http://mud.example/m/".into())
        })
    );
    for other in [
        "You hear a !!SOUND(x.wav)",
        "!!SOUND(x.wav",
        "!!SOUND()",
        "!!SOUND(x.wav V=loud)",
    ] {
        assert_eq!(parse_msp(other), None, "{other}");
    }
}

#[test]
fn a_name_with_stars_stands_for_several_files() {
    assert!(name_matches("hit*.wav", "hit3.wav") && name_matches("hit*.wav", "HIT.WAV"));
    assert!(name_matches("step?.wav", "step1.wav") && !name_matches("step?.wav", "step12.wav"));
    assert!(!name_matches("hit*.wav", "miss.wav") && name_matches("*", "anything.ogg"));
}

#[test]
fn ffplay_is_told_where_to_start_and_how_to_fade() {
    let plain = Options {
        volume: 0.5,
        ..Options::default()
    };
    assert!(!plain.shaped());
    assert_eq!(
        ffplay_arguments(&plain).join(" "),
        "-nodisp -autoexit -loglevel quiet -volume 50"
    );
    let shaped = Options {
        volume: 1.0,
        start: 1500,
        finish: Some(11500),
        fade_in: 2000,
        fade_out: 3000,
        ..Options::default()
    };
    assert!(shaped.shaped());
    assert_eq!(
        ffplay_arguments(&shaped).join(" "),
        "-nodisp -autoexit -loglevel quiet -volume 100 -ss 1.500 -t 10.000 \
         -af afade=t=in:st=0:d=2.000,afade=t=out:st=7.000:d=3.000"
    );
    // It can only die away before an end it knows.
    let open = Options {
        fade_out: 3000,
        ..Options::default()
    };
    assert!(!ffplay_arguments(&open).join(" ").contains("afade"));
}
