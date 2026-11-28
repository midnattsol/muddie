//! Tests of the sound requests, the file names, and what is heard of the
//! sounds played, read back from the mix.

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
    assert!(name_matches("*a*b?.wav", "xxaYYbbZ.wav") && !name_matches("*a*b", "ab.wav"));
    assert!(name_matches("", "") && !name_matches("", "a") && name_matches("**", ""));
    assert!(name_matches("ñ*.WAV", "Ñu.wav") && !name_matches("a?", "a"));
    // However many stars there are, it is soon known.
    let started = std::time::Instant::now();
    let stars = format!("{}z", "*".repeat(40));
    assert!(!name_matches(&stars, &"a".repeat(200)));
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

/// A sound of one channel at 48 kHz, as loud as `level` all through,
/// lasting `seconds`, written to a file of its own.
fn steady(name: &str, level: f32, seconds: f32) -> std::path::PathBuf {
    let rate: u32 = 48_000;
    let frames = (rate as f32 * seconds) as u32;
    let mut wav = Vec::new();
    let data = frames * 2;
    for (bytes, _) in [
        (b"RIFF".to_vec(), 0),
        ((36 + data).to_le_bytes().to_vec(), 0),
        (b"WAVEfmt ".to_vec(), 0),
        (16u32.to_le_bytes().to_vec(), 0),
        (1u16.to_le_bytes().to_vec(), 0),
        (1u16.to_le_bytes().to_vec(), 0),
        (rate.to_le_bytes().to_vec(), 0),
        ((rate * 2).to_le_bytes().to_vec(), 0),
        (2u16.to_le_bytes().to_vec(), 0),
        (16u16.to_le_bytes().to_vec(), 0),
        (b"data".to_vec(), 0),
        (data.to_le_bytes().to_vec(), 0),
    ] {
        wav.extend(bytes);
    }
    let sample = (level * f32::from(i16::MAX)) as i16;
    for _ in 0..frames {
        wav.extend(sample.to_le_bytes());
    }
    let dir = std::env::temp_dir().join(format!("muddie-sound-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, wav).unwrap();
    path
}

/// The next `seconds` of the mix, as frames of the left and the right.
fn heard(mix: &mut rodio::mixer::MixerSource, seconds: f32) -> Vec<[f32; 2]> {
    let frames = (48_000.0 * seconds) as usize;
    (0..frames)
        .map(|_| [mix.next().unwrap_or(0.0), mix.next().unwrap_or(0.0)])
        .collect()
}

/// How loud each side is at the end of what was heard.
fn level(frames: &[[f32; 2]]) -> [f32; 2] {
    frames.last().copied().unwrap_or_default()
}

fn near(heard: [f32; 2], wanted: [f32; 2]) -> bool {
    (heard[0] - wanted[0]).abs() < 0.01 && (heard[1] - wanted[1]).abs() < 0.01
}

#[test]
fn a_sound_is_as_loud_as_it_is_asked_and_changes_as_it_plays() {
    let (mut player, mut mix) = Player::in_memory();
    let file = steady("steady.wav", 0.5, 2.0);
    let label = Label {
        tag: "one".into(),
        group: "ambient".into(),
        ..Label::default()
    };
    player.play_as(&file, 0.5, None, label).unwrap();
    let start = level(&heard(&mut mix, 0.1));
    assert!(near(start, [0.25, 0.25]), "{start:?}");
    // Quieter as it plays.
    player.set_volume_where(|label| label.tag == "one", 0.2);
    let quieter = level(&heard(&mut mix, 0.1));
    assert!(near(quieter, [0.1, 0.1]), "{quieter:?}");
    // Its group too, and a sound of the group played after.
    player.set_group_volume("ambient", 0.5);
    let grouped = level(&heard(&mut mix, 0.1));
    assert!(near(grouped, [0.05, 0.05]), "{grouped:?}");
    player.stop_all();
    let label = Label {
        group: "ambient".into(),
        ..Label::default()
    };
    player.play_as(&file, 1.0, Some(1), label).unwrap();
    let after = level(&heard(&mut mix, 0.1));
    assert!(near(after, [0.25, 0.25]), "{after:?}");
    // Stopped, it is heard no more.
    player.stop_all();
    assert!(!player.is_playing(|_| true));
    let silent = level(&heard(&mut mix, 0.05));
    assert!(near(silent, [0.0, 0.0]), "{silent:?}");
}

#[test]
fn a_sound_is_put_to_one_side() {
    let (mut player, mut mix) = Player::in_memory();
    let file = steady("side.wav", 0.5, 1.0);
    let left = Options {
        pan: -1.0,
        ..Options::default()
    };
    player.play_with(&file, left, Label::default()).unwrap();
    let heard_left = level(&heard(&mut mix, 0.1));
    assert!(near(heard_left, [0.5, 0.0]), "{heard_left:?}");
    player.stop_all();
    let half_right = Options {
        pan: 0.5,
        ..Options::default()
    };
    player
        .play_with(&file, half_right, Label::default())
        .unwrap();
    let [left, right] = level(&heard(&mut mix, 0.1));
    assert!(right > left && left > 0.1, "{left} {right}");
}

#[test]
fn a_sound_starts_part_way_comes_up_and_dies_away_before_its_end() {
    let (mut player, mut mix) = Player::in_memory();
    let file = steady("shaped.wav", 0.5, 2.0);
    let shaped = Options {
        start: 500,
        finish: Some(1000),
        fade_in: 200,
        fade_out: 200,
        ..Options::default()
    };
    player.play_with(&file, shaped, Label::default()).unwrap();
    let frames = heard(&mut mix, 0.6);
    let at = |seconds: f32| frames[(48_000.0 * seconds) as usize][0];
    // Coming up, all there, and dying away: 0.5 s of it, then nothing.
    assert!(at(0.01) < 0.05, "{}", at(0.01));
    assert!((at(0.1) - 0.25).abs() < 0.02, "{}", at(0.1));
    assert!((at(0.25) - 0.5).abs() < 0.01, "{}", at(0.25));
    assert!((at(0.4) - 0.25).abs() < 0.02, "{}", at(0.4));
    assert!(at(0.55).abs() < 0.001, "{}", at(0.55));
    player.poll();
    assert!(!player.is_playing(|_| true));
}

/// Plays a silent sound through the system's audio, to see the mix reach
/// it: `cargo test -p muddie-sound -- --ignored heard_by_the_system`.
#[test]
#[ignore = "plays through the system's audio"]
fn heard_by_the_system() {
    let file = steady("silent.wav", 0.0, 2.0);
    let mut player = Player::default();
    player.play(&file, 1.0, false).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(800));
    let streams = std::process::Command::new("pw-dump").output().unwrap();
    let streams = String::from_utf8_lossy(&streams.stdout);
    assert!(streams.contains("\"Muddie\""), "no stream named Muddie");
    assert!(player.is_playing(|_| true));
    drop(player);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let after = std::process::Command::new("pw-dump").output().unwrap();
    assert!(!String::from_utf8_lossy(&after.stdout).contains("\"Muddie\""));
}

#[test]
fn a_volume_that_is_no_number_is_not_taken() {
    assert_eq!(parse_msp("!!SOUND(hit.wav V=NaN)"), None);
    assert_eq!(parse_msp("!!SOUND(hit.wav V=inf)"), None);
}

#[test]
fn a_sound_dies_away_without_being_told_where_it_ends() {
    let (mut player, mut mix) = Player::in_memory();
    let file = steady("fading.wav", 0.5, 1.0);
    let options = Options {
        fade_out: 500,
        ..Options::default()
    };
    player.play_with(&file, options, Label::default()).unwrap();
    // A tenth of a second before its end, it is nearly gone.
    let near_the_end = level(&heard(&mut mix, 0.9));
    assert!(near_the_end[0] < 0.2, "{near_the_end:?}");
}

#[test]
fn a_volume_that_is_no_number_leaves_the_rest_heard() {
    let (mut player, mut mix) = Player::in_memory();
    let file = steady("others.wav", 0.5, 1.0);
    player.play_as(&file, 1.0, None, Label::default()).unwrap();
    player
        .play_as(&file, f32::NAN, None, Label::default())
        .unwrap();
    player
        .play_with(
            &file,
            Options {
                pan: f32::NAN,
                volume: 0.0,
                ..Options::default()
            },
            Label::default(),
        )
        .unwrap();
    let both = level(&heard(&mut mix, 0.1));
    assert!(near(both, [0.5, 0.5]), "the mix is {both:?}");
}
