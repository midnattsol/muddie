//! Minimal client for speech-dispatcher's SSIP protocol over its Unix
//! socket, which avoids linking the C client library.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

const IO_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Connection {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Connection {
    /// Connects to the user's speech-dispatcher, starting it if needed.
    pub fn open() -> io::Result<Self> {
        let path = socket_path()?;
        let stream = match UnixStream::connect(&path) {
            Ok(stream) => stream,
            Err(_) => {
                // Same as the C library: ask the daemon to start itself.
                Command::new("speech-dispatcher")
                    .arg("--spawn")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()?;
                UnixStream::connect(&path)?
            }
        };
        stream.set_read_timeout(Some(IO_TIMEOUT))?;
        stream.set_write_timeout(Some(IO_TIMEOUT))?;
        let mut connection = Self {
            reader: BufReader::new(stream.try_clone()?),
            writer: stream,
        };
        connection.command("SET self CLIENT_NAME user:muddie:main")?;
        // "message" queues utterances in order instead of replacing them.
        connection.command("SET self PRIORITY message")?;
        Ok(connection)
    }

    pub fn speak(&mut self, text: &str) -> io::Result<()> {
        self.command("SPEAK")?;
        self.writer.write_all(encode_text(text).as_bytes())?;
        self.reply()
    }

    /// Cancels this client's current and queued messages.
    pub fn cancel(&mut self) -> io::Result<()> {
        self.command("CANCEL self")
    }

    pub fn set_rate(&mut self, rate: i32) -> io::Result<()> {
        self.command(&format!("SET self RATE {rate}"))
    }

    fn command(&mut self, command: &str) -> io::Result<()> {
        self.writer.write_all(format!("{command}\r\n").as_bytes())?;
        self.reply()
    }

    /// Reads one reply. Each line is `NNN-text`, the last one `NNN text`;
    /// codes other than 2xx are errors.
    fn reply(&mut self) -> io::Result<()> {
        let mut line = String::new();
        loop {
            line.clear();
            if self.reader.read_line(&mut line)? == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            if line.as_bytes().get(3) == Some(&b' ') {
                return if line.starts_with('2') {
                    Ok(())
                } else {
                    Err(io::Error::other(line.trim().to_owned()))
                };
            }
        }
    }
}

fn socket_path() -> io::Result<PathBuf> {
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| io::Error::other("XDG_RUNTIME_DIR is not set"))?;
    Ok(PathBuf::from(runtime_dir).join("speech-dispatcher/speechd.sock"))
}

/// Frames message text for `SPEAK`: CRLF lines ended by a lone dot, with
/// leading dots doubled so they are not taken for the terminator.
fn encode_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for line in text.lines() {
        if line.starts_with('.') {
            out.push('.');
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str(".\r\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_framed_and_dots_escaped() {
        assert_eq!(encode_text("hello"), "hello\r\n.\r\n");
        assert_eq!(encode_text("a\n.\n..b"), "a\r\n..\r\n...b\r\n.\r\n");
    }
}
