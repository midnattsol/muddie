//! Speaks its arguments, to check that speech works on this machine:
//! `cargo run -p muddie-speech --example say -- hello there`

use std::time::Duration;

use muddie_speech::Speech;

fn main() {
    let text = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let speech = Speech::start();
    speech.say(if text.is_empty() {
        "Speech works."
    } else {
        &text
    });
    // Requests are sent from a background thread; give it time to deliver.
    std::thread::sleep(Duration::from_secs(1));
}
