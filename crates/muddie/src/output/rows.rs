//! The rows of the output: what each holds, how it is laid out, and its
//! links.

use super::*;

/// What a row shows. Rows are laid out when drawn rather than when added,
/// so a change of theme or text size reaches the whole scrollback.
pub(super) enum Content {
    /// A line from the server, with its colours.
    Server(Line),
    /// A line the client added itself.
    Local(String),
}

/// A part of a row that can be clicked: where, what it does, what it is
/// for, and the name the server can take it away by.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RowLink {
    pub(super) range: Range<usize>,
    pub(super) target: LinkTarget,
    pub(super) hint: String,
    pub(super) name: String,
    /// A web address found in the text, rather than a link the server
    /// marked: the server chose no colour for it.
    pub(super) found: bool,
}

pub(super) struct Row {
    pub(super) content: Content,
    /// The row without styling, for search, review and logs.
    pub(super) text: String,
    /// When it arrived, as `HH:MM:SS`.
    pub(super) time: String,
    /// Parts of `text` that can be clicked: web addresses found in it, and
    /// links the server marked.
    pub(super) links: Vec<RowLink>,
    /// Height when wrapped to `Output::width`; `None` until measured.
    pub(super) height: Option<f32>,
}

/// The time of day as it is shown, `HH:MM:SS`. Worked out once a second
/// and not for every line: a burst of text is thousands of lines in one.
pub fn clock() -> String {
    use std::cell::RefCell;
    thread_local! {
        static SHOWN: RefCell<(i64, String)> = const { RefCell::new((i64::MIN, String::new())) };
    }
    let now = jiff::ZonedDateTime::now();
    let second = now.timestamp().as_second();
    SHOWN.with(|shown| {
        let mut shown = shown.borrow_mut();
        if shown.0 != second {
            *shown = (second, now.strftime("%H:%M:%S").to_string());
        }
        shown.1.clone()
    })
}

/// The byte ranges of `text` that are `http://` or `https://` links.
pub fn links(text: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find("http") {
        let start = from + at;
        let rest = &text[start..];
        let is_link = rest.starts_with("http://") || rest.starts_with("https://");
        let len = rest
            .find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '<' | '>' | '\''))
            .unwrap_or(rest.len());
        // Punctuation that ends the sentence is not part of the link.
        let link = rest[..len].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']']);
        if is_link && link.len() > "https://".len() {
            found.push(start..start + link.len());
        }
        from = start + len.max(4);
    }
    found
}

pub(super) fn append_line(job: &mut LayoutJob, line: &Line) {
    for span in &line.spans {
        job.append(&span.text, 0.0, palette::format(span.style));
    }
}

/// A line of server text laid out with its colours.
pub fn server_job(line: &Line) -> LayoutJob {
    let mut job = LayoutJob::default();
    append_line(&mut job, line);
    if job.text.is_empty() {
        job.append(" ", 0.0, palette::format(Default::default()));
    }
    job
}

impl Row {
    pub(super) fn job(&self, timestamps: bool) -> LayoutJob {
        let mut job = LayoutJob::default();
        if timestamps {
            job.append(&format!("{} ", self.time), 0.0, palette::local_format());
        }
        match &self.content {
            Content::Server(line) if self.links.is_empty() => append_line(&mut job, line),
            Content::Server(line) => {
                // A link is underlined; one the server marked keeps the
                // colours it gave it, an address found in the text takes
                // the accent.
                let accent = theme::current().accent;
                let mut line = line.clone();
                for link in &self.links {
                    line.restyle(link.range.clone(), |style| {
                        style.underline = true;
                        if link.found {
                            style.fg = Color::Rgb(accent.r(), accent.g(), accent.b());
                        }
                    });
                }
                append_line(&mut job, &line);
            }
            Content::Local(text) => job.append(text, 0.0, palette::local_format()),
        }
        if job.text.is_empty() {
            // An empty job has no height; a blank line must still take a row.
            job.append(" ", 0.0, palette::local_format());
        }
        job
    }

    /// The link at a character offset into the row as drawn, if any.
    /// The link at a character of the row as it is shown.
    pub(super) fn link(&self, char_index: usize, timestamps: bool) -> Option<&RowLink> {
        let prefix = if timestamps {
            self.time.chars().count() + 1
        } else {
            0
        };
        let char_index = char_index.checked_sub(prefix)?;
        let (byte, _) = self.text.char_indices().nth(char_index)?;
        self.links.iter().find(|link| link.range.contains(&byte))
    }

    #[cfg(test)]
    pub(super) fn link_at(&self, char_index: usize, timestamps: bool) -> Option<&LinkTarget> {
        self.link(char_index, timestamps).map(|link| &link.target)
    }
}
