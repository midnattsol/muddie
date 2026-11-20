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
pub fn offered(typed: &str, groups: &[(String, bool)], plugins: &[String]) -> Vec<Item> {
    if !typed.starts_with('/') {
        return Vec::new();
    }
    let words: Vec<&str> = typed.split_whitespace().collect();
    // A word is still being typed unless a space follows it.
    let open = !typed.ends_with(char::is_whitespace);
    let item = |insert: String, usage: String, about: &str, more: bool| Item {
        insert,
        usage,
        about: about.to_owned(),
        more,
        act: None,
    };
    // The names of what there is, for the commands that take one.
    let named = |prefix: &[&str], names: Vec<(String, String)>| -> Option<Vec<Item>> {
        let given = words.len() == prefix.len() + usize::from(open);
        if words.len() < prefix.len()
            || words[..prefix.len()] != *prefix
            || !(given || (words.len() == prefix.len() && !open))
        {
            return None;
        }
        let start = if open {
            words.last().copied().unwrap_or_default().to_lowercase()
        } else {
            String::new()
        };
        let head = prefix.join(" ");
        let matching = names
            .into_iter()
            .filter(|(name, _)| name.to_lowercase().starts_with(&start));
        let items: Vec<Item> = matching
            .map(|(name, about)| {
                let whole = format!("{head} {name}");
                item(whole.clone(), whole, &about, false)
            })
            .collect();
        (!items.is_empty()).then_some(items)
    };
    let group_names = || {
        groups
            .iter()
            .map(|(name, on)| (name.clone(), tr(if *on { "on" } else { "off" }).to_owned()))
            .collect()
    };
    let plugin_names = || {
        plugins
            .iter()
            .map(|name| (name.clone(), String::new()))
            .collect()
    };
    let by_name = named(&["/group"], group_names())
        .filter(|_| words.len() > 1 || !open)
        .or_else(|| {
            ["enable", "disable", "update", "remove"]
                .iter()
                .find_map(|what| named(&["/plugin", what], plugin_names()))
        });
    if let Some(items) = by_name {
        return items;
    }
    let mut items = Vec::new();
    for (usage, about) in COMMANDS {
        let command = fixed(usage);
        // What was typed must be the start of the command, word by word;
        // past its fixed words it is being given what it takes.
        let within = words.len().min(command.len());
        let exact = if open && words.len() <= command.len() {
            within.saturating_sub(1)
        } else {
            within
        };
        let fits = words[..exact] == command[..exact]
            && (exact == within || command[exact].starts_with(words[exact]));
        let takes_more = usage.split_whitespace().count() > command.len();
        if !fits || (words.len() > command.len() && !takes_more) {
            continue;
        }
        // Only what it cannot do without keeps it from being run as it is.
        let more = usage.contains('<');
        let insert = format!("{}{}", command.join(" "), if more { " " } else { "" });
        items.push(item(insert, (*usage).to_owned(), tr(about), more));
    }
    items
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
        let groups = [("Combat".to_owned(), true), ("Social".to_owned(), false)];
        let plugins = ["mapper".to_owned()];
        offered(typed, &groups, &plugins)
            .into_iter()
            .map(|item| item.usage)
            .collect()
    }

    #[test]
    fn commands_are_offered_as_they_are_typed() {
        assert!(usages("north").is_empty());
        assert_eq!(usages("/").len(), COMMANDS.len());
        assert!(usages("/ma").iter().all(|usage| usage.starts_with("/map")));
        assert_eq!(usages("/map w"), ["/map where", "/map walk <room>"]);
        assert_eq!(usages("/map walk Temple of"), ["/map walk <room>"]);
        assert_eq!(usages("/re"), ["/reload"]);
        assert!(usages("/dance").is_empty());
        // One that takes nothing is not offered once more follows it.
        assert!(usages("/map where now").is_empty());
        // Those that take the name of something there is offer the names.
        assert_eq!(usages("/group "), ["/group Combat", "/group Social"]);
        assert_eq!(usages("/group so"), ["/group Social"]);
        assert_eq!(usages("/plugin enable "), ["/plugin enable mapper"]);
        // What choosing one puts in the line leaves room for what it
        // cannot do without, and only then is it not run as it is.
        let walk = offered("/map wa", &[], &[]).remove(0);
        assert_eq!((walk.insert.as_str(), walk.more), ("/map walk ", true));
        let stop = offered("/map st", &[], &[]).remove(0);
        assert_eq!((stop.insert.as_str(), stop.more), ("/map stop", false));
        let avoid = offered("/map av", &[], &[]).remove(0);
        assert_eq!((avoid.insert.as_str(), avoid.more), ("/map avoid", false));
        let combat = offered("/group Com", &[("Combat".to_owned(), true)], &[]).remove(0);
        assert_eq!(
            (combat.insert.as_str(), combat.more),
            ("/group Combat", false)
        );
    }
}
