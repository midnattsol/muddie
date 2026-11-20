//! MXP links: what they send or open, with their hints and menus.

use super::*;

/// A link being read.
#[derive(Debug, Clone, Default)]
pub(crate) struct Link {
    /// Where its text starts in the line.
    pub(crate) start: usize,
    /// Where it leads, if the tag said.
    pub(crate) href: Option<String>,
    pub(crate) hint: String,
    pub(crate) name: String,
    /// It puts its command in the command line instead of sending it.
    pub(crate) prompt: bool,
    /// It leads to the web.
    pub(crate) web: bool,
}

impl Assembler {
    pub(crate) fn open_link(&mut self, link: Link) {
        self.close_link();
        self.link = Some(Link {
            start: self.line.text().len(),
            ..link
        });
    }

    pub(crate) fn close_link(&mut self) {
        let Some(link) = self.link.take() else {
            return;
        };
        let text = self.line.text();
        if link.start >= text.len() {
            return;
        }
        let shown = &text[link.start..];
        // A link that does not say where it leads sends its own text;
        // `&text;` in where it leads stands for that text too.
        let href = link.href.unwrap_or_else(|| shown.to_owned());
        let href = replace_entity(&href, "text", shown);
        // The hint may be several: what the link is for, then a name for
        // each of the commands it offers.
        let mut hints = link.hint.split('|').map(str::trim);
        let hint = hints.next().unwrap_or_default().to_owned();
        let commands: Vec<&str> = href.split('|').map(str::trim).collect();
        let target = match commands.as_slice() {
            _ if link.web => LinkTarget::Url(href.clone()),
            [command] if link.prompt => LinkTarget::Prompt((*command).to_owned()),
            [command] => LinkTarget::Send((*command).to_owned()),
            several => LinkTarget::Menu(
                (several.iter())
                    .map(|command| {
                        let called = hints.next().filter(|name| !name.is_empty());
                        (called.unwrap_or(command).to_owned(), (*command).to_owned())
                    })
                    .collect(),
            ),
        };
        self.line.links.push(LineLink {
            range: link.start..text.len(),
            target,
            hint,
            name: link.name,
        });
    }
}
