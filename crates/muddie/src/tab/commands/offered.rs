//! The client's commands offered as they are typed.

use super::*;

/// The words of a command that are typed as they are, not the `<this>` or
/// `[that]` that stand for something to give it.
pub(in crate::tab) fn fixed(usage: &str) -> Vec<&str> {
    (usage.split_whitespace())
        .take_while(|word| !word.starts_with(['<', '[']))
        .collect()
}

/// What the palette offers for what has been typed: the commands that
/// begin so, and for those that take the name of something there is, the
/// names.
pub fn offered(typed: &str, tags: &[(String, bool)], plugins: &[String]) -> Vec<Item> {
    if !typed.starts_with('/') {
        return Vec::new();
    }
    let typing = Typing::of(typed);
    let tag_names = (tags.iter())
        .map(|(name, on)| (name.clone(), tr(if *on { "on" } else { "off" }).to_owned()))
        .collect();
    let by_name = typing
        .names(&["/tag"], tag_names)
        .filter(|_| typing.words.len() > 1 || !typing.open)
        .or_else(|| {
            let plugin_names = || {
                plugins
                    .iter()
                    .map(|name| (name.clone(), String::new()))
                    .collect()
            };
            ["enable", "disable", "update", "remove"]
                .iter()
                .find_map(|what| typing.names(&["/plugin", what], plugin_names()))
        });
    by_name.unwrap_or_else(|| {
        COMMANDS
            .iter()
            .filter_map(|command| typing.command(command))
            .collect()
    })
}

/// What has been typed, word by word, and whether the last word is still
/// being typed, which it is unless a space follows it.
struct Typing<'a> {
    words: Vec<&'a str>,
    open: bool,
}

impl<'a> Typing<'a> {
    fn of(typed: &'a str) -> Self {
        Self {
            words: typed.split_whitespace().collect(),
            open: !typed.ends_with(char::is_whitespace),
        }
    }

    /// The names of what there is, for a command that takes one: those
    /// that begin as the word being typed.
    fn names(&self, prefix: &[&str], names: Vec<(String, String)>) -> Option<Vec<Item>> {
        let words = &self.words;
        let given = words.len() == prefix.len() + usize::from(self.open);
        if words.len() < prefix.len()
            || words[..prefix.len()] != *prefix
            || !(given || (words.len() == prefix.len() && !self.open))
        {
            return None;
        }
        let start = if self.open {
            words.last().copied().unwrap_or_default().to_lowercase()
        } else {
            String::new()
        };
        let head = prefix.join(" ");
        let items: Vec<Item> = (names.into_iter())
            .filter(|(name, _)| name.to_lowercase().starts_with(&start))
            .map(|(name, about)| {
                let whole = format!("{head} {name}");
                item(whole.clone(), whole, &about, false)
            })
            .collect();
        (!items.is_empty()).then_some(items)
    }

    /// The command, if what was typed is its start, word by word; past its
    /// fixed words it is being given what it takes. Only what it cannot do
    /// without keeps it from being run as it is.
    fn command(&self, (usage, about): &(&'static str, &'static str)) -> Option<Item> {
        let words = &self.words;
        let command = fixed(usage);
        let within = words.len().min(command.len());
        let exact = if self.open && words.len() <= command.len() {
            within.saturating_sub(1)
        } else {
            within
        };
        let fits = words[..exact] == command[..exact]
            && (exact == within || command[exact].starts_with(words[exact]));
        let takes_more = usage.split_whitespace().count() > command.len();
        if !fits || (words.len() > command.len() && !takes_more) {
            return None;
        }
        let more = usage.contains('<');
        let insert = format!("{}{}", command.join(" "), if more { " " } else { "" });
        Some(item(insert, (*usage).to_owned(), tr(about), more))
    }
}

/// A command offered, to put in the palette's field or run.
fn item(insert: String, usage: String, about: &str, more: bool) -> Item {
    Item {
        insert,
        usage,
        group: tr("Commands").to_owned(),
        about: about.to_owned(),
        more,
        act: None,
    }
}

/// A command as the palette shows it: without the slash it is typed
/// after in the command line.
pub(in crate::tab) fn shown_usage(usage: &str) -> &str {
    usage.strip_prefix('/').unwrap_or(usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usages(typed: &str) -> Vec<String> {
        crate::i18n::set_language("en");
        let tags = [("combat".to_owned(), true), ("social".to_owned(), false)];
        let plugins = ["mapper".to_owned()];
        offered(typed, &tags, &plugins)
            .into_iter()
            .map(|item| item.usage)
            .collect()
    }

    #[test]
    fn commands_are_offered_as_they_are_typed() {
        assert!(usages("north").is_empty());
        assert_eq!(usages("/").len(), COMMANDS.len());
        assert!(usages("/ma").iter().all(|usage| usage.starts_with("/map")));
        assert_eq!(usages("/map w"), ["/map where"]);
        assert_eq!(usages("/map tr"), ["/map travel <place>"]);
        assert_eq!(usages("/map travel Temple of"), ["/map travel <place>"]);
        assert_eq!(usages("/re"), ["/reload"]);
        assert!(usages("/dance").is_empty());
        // One that takes nothing is not offered once more follows it.
        assert!(usages("/map where now").is_empty());
        // Those that take the name of something there is offer the names.
        assert_eq!(usages("/tag "), ["/tag combat", "/tag social"]);
        assert_eq!(usages("/tag so"), ["/tag social"]);
        assert_eq!(usages("/plugin enable "), ["/plugin enable mapper"]);
        // What choosing one puts in the line leaves room for what it
        // cannot do without, and only then is it not run as it is.
        let travel = offered("/map tra", &[], &[]).remove(0);
        assert_eq!(
            (travel.insert.as_str(), travel.more),
            ("/map travel ", true)
        );
        let stop = offered("/map st", &[], &[]).remove(0);
        assert_eq!((stop.insert.as_str(), stop.more), ("/map stop", false));
        let avoid = offered("/map av", &[], &[]).remove(0);
        assert_eq!((avoid.insert.as_str(), avoid.more), ("/map avoid", false));
        let combat = offered("/tag com", &[("combat".to_owned(), true)], &[]).remove(0);
        assert_eq!(
            (combat.insert.as_str(), combat.more),
            ("/tag combat", false)
        );
    }
}
