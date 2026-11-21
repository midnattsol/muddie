//! Writes what a session shows to a log file, as plain text or as HTML
//! that keeps the colours.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use muddie_core::{Color, Line, Style};

use crate::settings::LogFormat;
use crate::theme;

pub struct SessionLog {
    file: BufWriter<File>,
    path: PathBuf,
    format: LogFormat,
}

impl SessionLog {
    /// Starts a log in `dir`, named after `title` and the current time.
    pub fn start(dir: &Path, title: &str, format: LogFormat) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let safe: String = (title.chars())
            .map(|ch| {
                if ch.is_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let stamp = jiff::ZonedDateTime::now().strftime("%Y-%m-%d_%H-%M-%S");
        let extension = match format {
            LogFormat::Text => "log",
            LogFormat::Html => "html",
        };
        let path = dir.join(format!("{safe}_{stamp}.{extension}"));
        let mut file = BufWriter::new(File::create(&path)?);
        if format == LogFormat::Html {
            let theme = theme::current();
            writeln!(
                file,
                "<!doctype html>\n<meta charset=\"utf-8\">\n<title>{}</title>\n\
                 <body style=\"background:{};color:{}\">\n<pre>",
                escape(title),
                css(theme.background),
                css(theme.text)
            )?;
        }
        Ok(Self { file, path, format })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes a line from the server.
    pub fn line(&mut self, time: &str, line: &Line) -> io::Result<()> {
        match self.format {
            LogFormat::Text => writeln!(self.file, "{time} {}", line.text())?,
            LogFormat::Html => {
                write!(self.file, "{time} ")?;
                for span in &line.spans {
                    match span_css(span.style) {
                        Some(style) => write!(
                            self.file,
                            "<span style=\"{style}\">{}</span>",
                            escape(&span.text)
                        )?,
                        None => write!(self.file, "{}", escape(&span.text))?,
                    }
                }
                writeln!(self.file)?;
            }
        }
        Ok(())
    }

    /// Writes a line the client added itself.
    pub fn local(&mut self, time: &str, text: &str) -> io::Result<()> {
        match self.format {
            LogFormat::Text => writeln!(self.file, "{time} {text}")?,
            LogFormat::Html => {
                let dim = css(theme::current().text_dim);
                writeln!(
                    self.file,
                    "{time} <span style=\"color:{dim}\">{}</span>",
                    escape(text)
                )?;
            }
        }
        Ok(())
    }

    /// Writes out what is waiting to be written: once a frame, not once a
    /// line.
    pub fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn css(color: eframe::egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

/// The CSS for a span's style, or `None` if it is plain.
fn span_css(style: Style) -> Option<String> {
    if style == Style::default() {
        return None;
    }
    let format = crate::palette::format(style);
    let mut css_text = format!("color:{}", css(format.color));
    if style.bg != Color::Default || style.inverse {
        css_text.push_str(&format!(";background:{}", css(format.background)));
    }
    if style.bold {
        css_text.push_str(";font-weight:bold");
    }
    if style.italic {
        css_text.push_str(";font-style:italic");
    }
    if style.underline {
        css_text.push_str(";text-decoration:underline");
    }
    Some(css_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::styled_lines;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("muddie-log-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn text_log_has_plain_lines() {
        let dir = scratch("text");
        let mut log = SessionLog::start(&dir, "mud.example.org:4000", LogFormat::Text).unwrap();
        log.line(
            "12:00:01",
            &styled_lines("\x1b[31mAn orc\x1b[0m attacks")[0],
        )
        .unwrap();
        log.local("12:00:02", "> kill orc").unwrap();
        let path = log.path().to_owned();
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("mud.example.org_4000_")
        );
        drop(log);
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "12:00:01 An orc attacks\n12:00:02 > kill orc\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn html_log_keeps_colours_and_escapes_markup() {
        let dir = scratch("html");
        let mut log = SessionLog::start(&dir, "A <b> & c", LogFormat::Html).unwrap();
        log.line("12:00:01", &styled_lines("\x1b[1;31m<orc>\x1b[0m & co")[0])
            .unwrap();
        let path = log.path().to_owned();
        drop(log);
        let html = std::fs::read_to_string(path).unwrap();
        assert!(html.contains("<title>A &lt;b&gt; &amp; c</title>"));
        assert!(html.contains("font-weight:bold\">&lt;orc&gt;</span> &amp; co"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
