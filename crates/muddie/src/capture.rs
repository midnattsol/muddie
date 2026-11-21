//! A record of a run, to see what happened in it: with
//! `--capture-protocols`, everything that goes over the protocols, either
//! way, each line under its protocol's name (`GMCP<` arrived, `GMCP>`
//! sent); with `--debug`, that and the rest: the text of the game, what was
//! sent, what the client said, the scripts' errors and what they log, and
//! where the map has the player. One file per run, written line by line so
//! that nothing is lost if it stops. Passwords are never in it: what is
//! typed while the server hides it is not told.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// How much is recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// What goes over the protocols.
    Protocols,
    /// That, and all else that helps to see what happened.
    Debug,
}

struct Capture {
    level: Level,
    path: PathBuf,
    file: std::fs::File,
}

static CAPTURE: OnceLock<Mutex<Capture>> = OnceLock::new();

/// Starts recording, in a new file in the folder of the user's data.
/// Returns where it is written.
pub fn start(level: Level) -> Result<PathBuf, String> {
    let folder = crate::profile::user_dir(crate::profile::UserDir::Data)
        .ok_or("no data folder was found")?
        .join("captures");
    std::fs::create_dir_all(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let stamp = jiff::ZonedDateTime::now().strftime("%Y-%m-%d_%H-%M-%S");
    let path = folder.join(format!("{stamp}.log"));
    let file =
        std::fs::File::create(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let capture = Capture {
        level,
        path: path.clone(),
        file,
    };
    CAPTURE
        .set(Mutex::new(capture))
        .map_err(|_| "already recording".to_owned())?;
    Ok(path)
}

/// How much is being recorded, if anything is.
pub fn level() -> Option<Level> {
    Some(CAPTURE.get()?.lock().ok()?.level)
}

/// Where the record is written, if one is.
pub fn path() -> Option<PathBuf> {
    Some(CAPTURE.get()?.lock().ok()?.path.clone())
}

/// Something that went over a protocol, in the session called `session`:
/// `incoming` from the server, or else to it.
pub fn protocol(session: &str, protocol: &str, incoming: bool, text: &str) {
    let way = if incoming { '<' } else { '>' };
    write(Level::Protocols, session, &format!("{protocol}{way}"), text);
}

/// Something else that happened, under `what` (`TEXT`, `SENT`, `NOTE`,
/// `MAP`...), recorded with `--debug` only.
pub fn debug(session: &str, what: &str, text: &str) {
    write(Level::Debug, session, what, text);
}

fn write(needed: Level, session: &str, what: &str, text: &str) {
    let Some(capture) = CAPTURE.get() else {
        return;
    };
    let Ok(mut capture) = capture.lock() else {
        return;
    };
    if capture.level < needed {
        return;
    }
    let time = jiff::ZonedDateTime::now()
        .strftime("%H:%M:%S%.3f")
        .to_string();
    let _ = writeln!(capture.file, "{}", line(&time, session, what, text));
}

/// One line of the record: when, in which session, under what, and the
/// text, on one line however many it had.
fn line(time: &str, session: &str, what: &str, text: &str) -> String {
    let text = text.replace('\r', "").replace('\n', "\\n");
    format!("{time} [{session}] {what} {text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_thing_recorded_is_one_line_under_its_name() {
        assert_eq!(
            line(
                "21:05:03.120",
                "Reinos",
                "GMCP<",
                r#"Room.Actual "Plaza [n, s]""#
            ),
            r#"21:05:03.120 [Reinos] GMCP< Room.Actual "Plaza [n, s]""#
        );
        assert_eq!(
            line("21:05:03.121", "Reinos", "TEXT", "Una plaza\r\namplia"),
            r"21:05:03.121 [Reinos] TEXT Una plaza\namplia"
        );
        // What goes over the protocols is in either record; the rest, in
        // that of --debug only.
        assert!(Level::Debug >= Level::Protocols && Level::Protocols < Level::Debug);
    }
}
