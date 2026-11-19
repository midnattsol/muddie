//! Sounds and music a MUD asks for with GMCP's `Client.Media`: it names a
//! file and, unless the player already has it, where to fetch it from.
//!
//! What a server names is not trusted: only sound files are fetched, only
//! over HTTP or HTTPS, of no more than a size, into a folder of their own.

use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use serde_json::Value;

/// The kinds of file fetched and played, by how their names end.
const KINDS: [&str; 10] = [
    "wav", "ogg", "oga", "mp3", "flac", "opus", "m4a", "aac", "mid", "midi",
];

/// The largest file fetched.
const LARGEST: u64 = 25 * 1024 * 1024;

/// Something to play.
#[derive(Debug, Clone, PartialEq)]
pub struct Play {
    pub name: String,
    /// The folder on the web the file is in, if this message gave one.
    pub url: Option<String>,
    /// `sound` or `music`.
    pub kind: String,
    pub tag: String,
    pub key: String,
    /// From 0 to 1.
    pub volume: f32,
    /// How many times; `None` until stopped.
    pub loops: Option<u32>,
    /// Music that is already playing goes on rather than starting again.
    pub keep_going: bool,
    /// From 1 to 100: it stops what has less, and is not played over
    /// what has more. 0 for none, which neither stops nor is kept out.
    pub priority: u8,
    /// Where in it to start, and to end if before its own end; and how
    /// long it takes to come up and to die away. In milliseconds.
    pub start: u32,
    pub finish: Option<u32>,
    pub fade_in: u32,
    pub fade_out: u32,
}

/// What to stop: what matches all that is given; all, if nothing is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stop {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub key: Option<String>,
}

impl Stop {
    /// Whether something playing under these labels is to be stopped.
    pub fn matches(&self, name: &str, kind: &str, tag: &str, key: &str) -> bool {
        let same = |wanted: &Option<String>, has: &str| wanted.as_deref().is_none_or(|w| w == has);
        same(&self.name, name)
            && same(&self.kind, kind)
            && same(&self.tag, tag)
            && same(&self.key, key)
    }
}

/// A message of `Client.Media`.
#[derive(Debug, Clone, PartialEq)]
pub enum Media {
    /// Where files are fetched from when a message does not say.
    Default(String),
    /// Fetch a file now, to have it when it is played.
    Load {
        name: String,
        url: Option<String>,
    },
    Play(Play),
    Stop(Stop),
}

/// Reads a GMCP message if it is one of `Client.Media`. Video is not
/// played, so a message about one is none.
pub fn parse(package: &str, data: &str) -> Option<Media> {
    let what = package.to_ascii_lowercase();
    let what = what.strip_prefix("client.media.")?;
    let value: Value = serde_json::from_str(data).unwrap_or(Value::Null);
    let text = |field: &str| {
        (value.get(field).and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let number = |field: &str| value.get(field).and_then(Value::as_f64);
    let kind = text("type")
        .unwrap_or_else(|| "sound".to_owned())
        .to_ascii_lowercase();
    match what {
        "default" => Some(Media::Default(text("url")?)),
        "load" => Some(Media::Load {
            name: text("name")?,
            url: text("url"),
        }),
        "play" if kind != "video" => Some(Media::Play(Play {
            name: text("name")?,
            url: text("url"),
            tag: text("tag").unwrap_or_default(),
            key: text("key").unwrap_or_default(),
            // From 1 to 100, and 50 if not said.
            volume: (number("volume").unwrap_or(50.0).clamp(0.0, 100.0) / 100.0) as f32,
            // A number of times, 1 if not said; -1 is until stopped.
            loops: match number("loops").unwrap_or(1.0) {
                times if times < 0.0 => None,
                times => Some((times as u32).max(1)),
            },
            // Music goes on unless told to start again.
            keep_going: kind == "music"
                && value.get("continue").and_then(Value::as_bool) != Some(false),
            priority: number("priority").map_or(0, |priority| priority.clamp(0.0, 100.0) as u8),
            start: number("start").map_or(0, |at| at.max(0.0) as u32),
            finish: number("finish").map(|at| at.max(0.0) as u32),
            fade_in: number("fadein").map_or(0, |lasting| lasting.max(0.0) as u32),
            fade_out: number("fadeout").map_or(0, |lasting| lasting.max(0.0) as u32),
            kind,
        })),
        "stop" => Some(Media::Stop(Stop {
            name: text("name"),
            kind: text("type").map(|kind| kind.to_ascii_lowercase()),
            tag: text("tag"),
            key: text("key"),
        })),
        _ => None,
    }
}

/// Whether `name` is a sound file inside the folder it is looked for in:
/// a path that does not climb out of it, ending as a sound file does.
pub fn safe_name(name: &str) -> bool {
    let path = Path::new(name);
    let inside = !name.contains('\\')
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)));
    let kind = path
        .extension()
        .and_then(|kind| kind.to_str())
        .unwrap_or_default();
    inside && KINDS.contains(&kind.to_ascii_lowercase().as_str())
}

/// The address of `name` in the folder `url` of the web, if it is one
/// that may be fetched: HTTP or HTTPS, and a file that is safe to name.
pub fn address(url: &str, name: &str) -> Option<String> {
    let url = url.trim();
    let web = url.starts_with("https://") || url.starts_with("http://");
    (web && safe_name(name)).then(|| format!("{}/{name}", url.trim_end_matches('/')))
}

/// Where fetched files are kept: the cache of the user, not the folder
/// of their settings.
pub fn cache_dir() -> Option<PathBuf> {
    let cache = (std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".cache")))?;
    Some(cache.join("muddie").join("media"))
}

/// Where the file at `address` is kept once fetched: a folder for each
/// folder of the web, so that two servers' files of one name do not meet.
pub fn kept_at(cache: &Path, address: &str) -> PathBuf {
    let (folder, file) = address.rsplit_once('/').unwrap_or(("", address));
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    folder.hash(&mut hasher);
    cache.join(format!("{:016x}", hasher.finish())).join(file)
}

/// Fetches the file at `address` into `cache`, away from the thread that
/// asked. What comes back is where it is, or why it could not be had.
pub fn fetch(cache: &Path, address: &str) -> Receiver<Result<PathBuf, String>> {
    let (done, result) = channel();
    let (path, address) = (kept_at(cache, address), address.to_owned());
    std::thread::spawn(move || {
        let _ = done.send(download(&address, &path));
    });
    result
}

fn download(address: &str, path: &Path) -> Result<PathBuf, String> {
    let said = |error: &dyn std::fmt::Display| format!("{address}: {error}");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .max_redirects(3)
        .user_agent("muddie")
        .build()
        .into();
    let mut response = agent.get(address).call().map_err(|error| said(&error))?;
    let bytes = (response
        .body_mut()
        .with_config()
        .limit(LARGEST)
        .read_to_vec())
    .map_err(|error| said(&error))?;
    let folder = path
        .parent()
        .ok_or_else(|| said(&"no folder to keep it in"))?;
    std::fs::create_dir_all(folder).map_err(|error| said(&error))?;
    // Written whole under another name first, so that a file cut short
    // is never taken for the sound.
    let partial = path.with_extension("part");
    std::fs::write(&partial, bytes).map_err(|error| said(&error))?;
    std::fs::rename(&partial, path).map_err(|error| said(&error))?;
    Ok(path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_of_client_media_are_read() {
        assert_eq!(
            parse(
                "Client.Media.Default",
                r#"{"url": "https://mud.example.org/sounds/"}"#
            ),
            Some(Media::Default("https://mud.example.org/sounds/".into()))
        );
        let Some(Media::Play(play)) = parse("client.media.play", r#"{"name": "sword.wav"}"#) else {
            panic!("a sound to play");
        };
        assert_eq!(
            (play.kind.as_str(), play.volume, play.loops),
            ("sound", 0.5, Some(1))
        );
        assert!(!play.keep_going);
        let music = r#"{"name": "town.mp3", "type": "music", "volume": 80, "loops": -1, "tag": "area", "key": "bg"}"#;
        let Some(Media::Play(play)) = parse("Client.Media.Play", music) else {
            panic!("music to play");
        };
        assert_eq!(
            (play.volume, play.loops, play.keep_going),
            (0.8, None, true)
        );
        assert_eq!((play.tag.as_str(), play.key.as_str()), ("area", "bg"));
        assert_eq!(
            (play.priority, play.start, play.finish, play.fade_in),
            (0, 0, None, 0)
        );
        let shaped = r#"{"name": "a.ogg", "priority": 80, "start": 1000, "finish": 9000, "fadein": 500, "fadeout": 700}"#;
        let Some(Media::Play(play)) = parse("Client.Media.Play", shaped) else {
            panic!("a sound to play");
        };
        assert_eq!(
            (
                play.priority,
                play.start,
                play.finish,
                play.fade_in,
                play.fade_out
            ),
            (80, 1000, Some(9000), 500, 700)
        );
        // Video is not played, and what is not of Client.Media is not read.
        assert_eq!(
            parse("Client.Media.Play", r#"{"name": "a.mp4", "type": "video"}"#),
            None
        );
        assert_eq!(parse("Char.Vitals", "{}"), None);
        // What matches all that is given is stopped; all, if nothing is.
        let Some(Media::Stop(stop)) =
            parse("Client.Media.Stop", r#"{"type": "music", "tag": "area"}"#)
        else {
            panic!("something to stop");
        };
        assert!(stop.matches("town.mp3", "music", "area", "bg"));
        assert!(!stop.matches("sword.wav", "sound", "area", ""));
        let Some(Media::Stop(all)) = parse("Client.Media.Stop", "{}") else {
            panic!("something to stop");
        };
        assert!(all.matches("sword.wav", "sound", "", ""));
    }

    #[test]
    fn only_sound_files_from_the_web_are_fetched() {
        let base = "https://mud.example.org/sounds/";
        assert_eq!(
            address(base, "combat/sword.wav").as_deref(),
            Some("https://mud.example.org/sounds/combat/sword.wav")
        );
        assert_eq!(
            address("http://mud.example.org", "a.OGG").as_deref(),
            Some("http://mud.example.org/a.OGG")
        );
        // Not out of the folder, not what is not a sound, not from a disk.
        for name in [
            "../secret.wav",
            "/etc/passwd",
            "a\\..\\b.wav",
            "run.sh",
            "sword",
            "a/../../b.mp3",
        ] {
            assert_eq!(address(base, name), None, "{name}");
        }
        assert_eq!(address("file:///home/me/", "a.wav"), None);
        assert_eq!(address("ftp://mud.example.org/", "a.wav"), None);
        // Two servers' files of one name are kept apart.
        let cache = Path::new("/cache");
        let one = kept_at(cache, "https://a.example/s/hit.wav");
        let other = kept_at(cache, "https://b.example/s/hit.wav");
        assert!(one != other && one.ends_with("hit.wav") && one.starts_with(cache));
    }
}
