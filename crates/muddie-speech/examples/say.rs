//! Says its arguments, to check that speech works on this machine, and
//! tells what it went through: the screen reader if one is running, or else
//! the system's voice.
//! `cargo run -p muddie-speech --example say -- hello there`
//! `cargo run -p muddie-speech --example say -- --language es hola`

use std::time::Duration;

use muddie_speech::{Speech, Through};

fn main() {
    let mut words: Vec<String> = std::env::args().skip(1).collect();
    let language = (words.first().is_some_and(|word| word == "--language") && words.len() >= 2)
        .then(|| words.drain(..2).nth(1))
        .flatten();
    let text = match words.join(" ") {
        text if text.is_empty() => "Speech works.".to_owned(),
        text => text,
    };
    // The screen reader, or else the voice.
    let reader = Speech::start(Through::ScreenReader);
    std::thread::sleep(Duration::from_millis(500));
    let speech = match reader.status().name {
        Some(_) => reader,
        None => Speech::start(Through::Voice),
    };
    if let Some(language) = language {
        speech.set_language(&language);
    }
    speech.say(&text, false);
    // Requests are carried out on a thread of their own: give it time.
    std::thread::sleep(Duration::from_secs(2));
    let status = speech.status();
    println!(
        "spoken through {}",
        status.name.as_deref().unwrap_or("nothing")
    );
}
