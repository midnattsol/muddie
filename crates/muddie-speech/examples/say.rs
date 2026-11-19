//! Speaks its arguments, to check that speech works on this machine:
//! `cargo run -p muddie-speech --example say -- hello there`
//! `cargo run -p muddie-speech --example say -- --language es hola`

use std::time::Duration;

use muddie_speech::Speech;

fn main() {
    let mut words: Vec<String> = std::env::args().skip(1).collect();
    let speech = Speech::start();
    if words.first().is_some_and(|word| word == "--language") && words.len() >= 2 {
        speech.set_language(&words[1]);
        words.drain(..2);
    }
    let text = words.join(" ");
    speech.say(if text.is_empty() {
        "Speech works."
    } else {
        &text
    });
    // Requests are sent from a background thread; give it time to deliver.
    std::thread::sleep(Duration::from_secs(1));
}
