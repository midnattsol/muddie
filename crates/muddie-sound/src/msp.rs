//! Reads the sound requests of the MUD Sound Protocol.

use std::path::Path;

/// A request in a line of the MUD Sound Protocol, such as
/// `!!SOUND(sword.wav V=80 L=2 P=60 T=combat U=http://mud.example/s/)`.
#[derive(Debug, Clone, PartialEq)]
pub enum Msp {
    Play {
        /// `!!MUSIC` rather than `!!SOUND`.
        music: bool,
        /// The file, which may have `*` and `?` in it to stand for any
        /// of several, and no ending, which is then `.wav` or `.mid`.
        file: String,
        /// From 0 to 1.
        volume: f32,
        /// How many times; `None` for until stopped.
        times: Option<u32>,
        /// From 0 to 100: a sound stops those of less, and is not played
        /// over one of more. Music has none.
        priority: u8,
        /// Music already playing goes on rather than starting again.
        keep_going: bool,
        /// The kind of sound, which is also the folder it may be in.
        kind: String,
        /// Where on the web to fetch it from.
        url: Option<String>,
    },
    /// `Off` as the file name stops the sounds, or the music, and with an
    /// address says where files are fetched from when none is given.
    Stop { music: bool, url: Option<String> },
}

/// Reads an MSP trigger if `line` is one.
pub fn parse_msp(line: &str) -> Option<Msp> {
    let line = line.trim();
    let (music, body) = match (line.strip_prefix("!!SOUND("), line.strip_prefix("!!MUSIC(")) {
        (Some(body), _) => (false, body),
        (_, Some(body)) => (true, body),
        _ => return None,
    };
    let mut parts = body.strip_suffix(')')?.split_whitespace();
    let file = parts.next()?;
    let (mut volume, mut times, mut priority) = (1.0, Some(1), 50);
    let (mut keep_going, mut kind, mut url) = (true, String::new(), None);
    for part in parts {
        let (name, value) = part.split_once('=')?;
        match name.to_ascii_uppercase().as_str() {
            "V" => volume = value.parse::<f32>().ok()?.clamp(0.0, 100.0) / 100.0,
            "L" => {
                times = match value.parse::<i32>().ok()? {
                    times if times < 0 => None,
                    times => Some((times as u32).max(1)),
                }
            }
            "P" => priority = value.parse::<u32>().ok()?.min(100) as u8,
            "C" => keep_going = value.parse::<i32>().ok()? != 0,
            "T" => value.clone_into(&mut kind),
            "U" => url = Some(value.to_owned()),
            _ => {}
        }
    }
    if file.eq_ignore_ascii_case("off") {
        return Some(Msp::Stop { music, url });
    }
    // With no ending it is the usual one for its kind.
    let named = Path::new(file).extension().is_some();
    let ending = if music { "mid" } else { "wav" };
    Some(Msp::Play {
        music,
        file: if named {
            file.to_owned()
        } else {
            format!("{file}.{ending}")
        },
        volume,
        times,
        priority: if music { 0 } else { priority },
        keep_going: music && keep_going,
        kind,
        url,
    })
}

/// Whether the file name `name` is one `pattern` stands for: `*` for any
/// run of characters and `?` for any one, whatever the case of letters.
pub fn name_matches(pattern: &str, name: &str) -> bool {
    let lower = |text: &str| -> Vec<char> { text.to_lowercase().chars().collect() };
    let (pattern, name) = (lower(pattern), lower(name));
    // Each character of the name once, going back only to just after the
    // last `*`, so that many of them do not make it go on and on.
    let (mut at, mut from) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while from < name.len() {
        match pattern.get(at) {
            Some('*') => {
                star = Some((at, from));
                at += 1;
            }
            Some(&wanted) if wanted == '?' || wanted == name[from] => {
                at += 1;
                from += 1;
            }
            _ => match star {
                // The last `*` stands for one more character.
                Some((after, matched)) => {
                    star = Some((after, matched + 1));
                    at = after + 1;
                    from = matched + 1;
                }
                None => return false,
            },
        }
    }
    pattern[at..].iter().all(|&left| left == '*')
}
