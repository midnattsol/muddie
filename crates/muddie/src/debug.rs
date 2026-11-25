//! What `--debug` shows: everything that happens in a run, one line each on
//! the standard output, as it happens, to read in the terminal or to keep
//! with `tee`. What goes over the protocols is under its name and way
//! (`GMCP<` arrived, `GMCP>` sent, and so `MSDP`, `MSSP`, `MXP`, `TELNET`);
//! the rest under what it is (`TEXT`, a line; `PROMPT`, one the MUD marks
//! as such; `PARTIAL`, what has no end yet; `SENT`, `NOTE`, `SCRIPT`, `LOG`,
//! `MAP`). Passwords are never shown: what is sent as one is not told, and
//! a password in what is sent by GMCP is hidden.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static ON: AtomicBool = AtomicBool::new(false);

/// Starts showing what happens.
pub fn start() {
    ON.store(true, Ordering::Relaxed);
}

/// Whether what happens is shown.
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Shows something that went over a protocol in the session called
/// `session`: `incoming` from the server, or else to it.
pub fn protocol(session: &str, protocol: &str, incoming: bool, text: &str) {
    if !on() {
        return;
    }
    let way = if incoming { '<' } else { '>' };
    let text = match (protocol, incoming) {
        ("GMCP", false) => hide_passwords(text),
        _ => text.to_owned(),
    };
    show(session, &format!("{protocol}{way}"), &text);
}

/// Shows something else that happened, under `what`.
pub fn show(session: &str, what: &str, text: &str) {
    if !on() {
        return;
    }
    let time = jiff::ZonedDateTime::now()
        .strftime("%H:%M:%S%.3f")
        .to_string();
    let mut out = std::io::stdout().lock();
    // At once, line by line, though the output goes to a pipe.
    let _ = writeln!(out, "{}", line(&time, session, what, text));
    let _ = out.flush();
}

/// One line: when, in which session, under what, and the text, on one
/// line however many it had.
fn line(time: &str, session: &str, what: &str, text: &str) -> String {
    let text = text.replace('\r', "").replace('\n', "\\n");
    format!("{time} [{session}] {what} {text}")
}

/// A GMCP message, `Package {json}`, with the value of each field whose
/// name says it is a password put out of sight.
fn hide_passwords(message: &str) -> String {
    let (package, data) = message.split_once(' ').unwrap_or((message, ""));
    let Ok(mut json) = serde_json::from_str::<serde_json::Value>(data) else {
        return message.to_owned();
    };
    fn hide(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                for (name, field) in fields.iter_mut() {
                    let name = name.to_lowercase();
                    let secret = ["pass", "clave", "contraseña", "secret", "token"]
                        .iter()
                        .any(|word| name.contains(word));
                    match secret {
                        true => *field = serde_json::Value::String("********".to_owned()),
                        false => hide(field),
                    }
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(hide),
            _ => {}
        }
    }
    hide(&mut json);
    format!("{package} {json}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_thing_shown_is_one_line_under_its_name() {
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
    }

    #[test]
    fn a_password_sent_by_gmcp_is_not_shown() {
        assert_eq!(
            hide_passwords(r#"Char.Login {"name": "Naylhena", "password": "hunter2"}"#),
            r#"Char.Login {"name":"Naylhena","password":"********"}"#
        );
        assert_eq!(hide_passwords("Core.Ping"), "Core.Ping");
    }
}
