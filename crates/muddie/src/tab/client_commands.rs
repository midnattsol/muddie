//! The commands of the client itself, typed after a slash.

use super::*;

/// The client's own commands, as they are typed, and what each is for.
/// They are in English whatever the language of the interface, like the
/// words of a programming language.
pub(super) const COMMANDS: [(&str, &str); 21] = [
    ("/map where", "Says where you are"),
    (
        "/map find <text>",
        "Lists the rooms whose name contains the text",
    ),
    ("/map walk <room>", "Walks to a room, by name or number"),
    ("/map stop", "Stops walking"),
    (
        "/map avoid [number]",
        "Keeps routes out of a room, or lets them in again",
    ),
    (
        "/map lock <direction>",
        "Keeps routes from an exit of this room, or lets them use it again",
    ),
    ("/map tidy", "Draws this area again from scratch"),
    ("/map show", "Shows the map panel"),
    ("/map hide", "Hides the map panel"),
    ("/plugin list", "Lists the installed plugins"),
    (
        "/plugin install <git address> [name]",
        "Installs a plugin and enables it",
    ),
    ("/plugin update <name>", "Brings a plugin up to date"),
    ("/plugin enable <name>", "Enables a plugin in this profile"),
    (
        "/plugin disable <name>",
        "Disables a plugin in this profile",
    ),
    ("/plugin remove <name>", "Deletes a plugin"),
    (
        "/group",
        "Lists the groups of rules and whether they are on",
    ),
    (
        "/group <name> [on|off]",
        "Switches a group on or off for this session",
    ),
    ("/log", "Starts or stops logging the session"),
    ("/reload", "Runs the scripts and rules again"),
    ("/lua <code>", "Runs a line of Luau"),
    ("/help", "Lists these commands"),
];

impl Tab {
    /// Carries out a command of the client's own. Returns false if `line`
    /// is not one of them.
    pub(super) fn do_client_command(&mut self, line: &str) -> bool {
        let Some((command, argument)) = client_command(line) else {
            return false;
        };
        let argument = argument.as_str();
        let given = !argument.is_empty();
        match command {
            "/help" | "/map" | "/plugin" => self.open_command_list(command),
            "/map-show" | "/map-hide" => self.set_map_shown(command == "/map-show"),
            "/group" => self.group_command(argument),
            "/where" => self.say_where(),
            "/find" if given => self.find_rooms(argument),
            "/go" if given => self.go_to(argument),
            "/avoid" => self.toggle_avoid(argument),
            "/lock" if given => self.toggle_lock(argument),
            "/tidy" => self.tidy_here(),
            "/packages" => self.list_plugins(),
            "/install" if given => self.install_plugin(argument),
            "/update" if given => self.update_plugin(argument),
            "/enable" | "/disable" if given => self.enable_plugin(argument, command == "/enable"),
            "/remove" if given => self.remove_plugin(argument),
            "/reload" => {
                self.load_scripts();
                self.note(tr("Scripts reloaded."));
            }
            "/log" => self.toggle_log(),
            "/stop" => self.stop_walking(),
            _ => return false,
        }
        true
    }

    /// The palette is the list of commands: opens it, at the group if one
    /// was given.
    fn open_command_list(&mut self, command: &str) {
        self.palette_open = true;
        self.palette_focus = true;
        self.palette_cursor_end = true;
        self.palette_query = match command {
            "/help" => String::new(),
            group => format!("{} ", &group[1..]),
        };
    }

    /// Switches a group for this session, or lists them all.
    fn group_command(&mut self, argument: &str) {
        let (name, wanted) = match argument.rsplit_once(' ') {
            Some((name, "on")) => (name.trim(), Some(true)),
            Some((name, "off")) => (name.trim(), Some(false)),
            _ => (argument, None),
        };
        let known = self.scripts.groups();
        if name.is_empty() {
            return self.list_groups(known);
        }
        let Some(group) = known.iter().find(|group| group.eq_ignore_ascii_case(name)) else {
            return self.refuse(&trf(
                "There is no group called {0}; /group lists them.",
                &[name],
            ));
        };
        let on = wanted.unwrap_or(!self.scripts.group_enabled(group));
        self.scripts.set_group(group, on);
        let said = if on {
            "Group {0} on."
        } else {
            "Group {0} off."
        };
        self.note(&trf(said, &[group]));
    }

    /// Lists the groups, each with whether it is on, to switch.
    fn list_groups(&mut self, known: Vec<String>) {
        if known.is_empty() {
            self.refuse(tr(
                "There are no groups. Give a rule a group in the editor.",
            ));
        }
        for group in known {
            let state = if self.scripts.group_enabled(&group) {
                tr("on")
            } else {
                tr("off")
            };
            self.offer(&group, state, &format!("/group {group}"));
        }
    }

    /// Starts or stops logging the session.
    fn toggle_log(&mut self) {
        match self.log.take() {
            Some(log) => {
                let path = log.path().display().to_string();
                self.note(&trf("Logging stopped; the log is {0}", &[&path]));
            }
            None => self.start_log(),
        }
    }
}

/// Reads a line as a command of the client: what it does, under the name
/// the code knows it by, and what it was given. Anything else is for the
/// MUD.
pub(super) fn client_command(line: &str) -> Option<(&'static str, String)> {
    let mut words = line.split_whitespace();
    let (group, what) = (words.next()?, words.next().unwrap_or_default());
    let rest = words.collect::<Vec<_>>().join(" ");
    let command = match (group, what) {
        ("/map", "where") => "/where",
        ("/map", "find") => "/find",
        ("/map", "walk") => "/go",
        ("/map", "stop") => "/stop",
        ("/map", "avoid") => "/avoid",
        ("/map", "lock") => "/lock",
        ("/map", "tidy") => "/tidy",
        ("/map", "show") => "/map-show",
        ("/map", "hide") => "/map-hide",
        ("/map", _) => "/map",
        ("/plugin", "list") => "/packages",
        ("/plugin", "install") => "/install",
        ("/plugin", "update") => "/update",
        ("/plugin", "enable") => "/enable",
        ("/plugin", "disable") => "/disable",
        ("/plugin", "remove") => "/remove",
        ("/plugin", _) => "/plugin",
        ("/group", _) => {
            let after = line.trim_start().strip_prefix("/group").unwrap_or_default();
            return Some(("/group", after.trim().to_owned()));
        }
        ("/log", _) => "/log",
        ("/reload", _) => "/reload",
        ("/help", _) => "/help",
        _ => return None,
    };
    Some((command, rest))
}
