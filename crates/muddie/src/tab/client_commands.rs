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
    /// Handles the client's own map commands. Returns false if `line` is
    /// not one of them.
    pub(super) fn map_command(&mut self, line: &str) -> bool {
        let Some((command, argument)) = client_command(line) else {
            return false;
        };
        let argument = argument.as_str();
        match command {
            // The palette is the list of commands: open it, at the group
            // if one was given.
            "/help" | "/map" | "/plugin" => {
                self.palette_open = true;
                self.palette_focus = true;
                self.palette_cursor_end = true;
                self.palette_query = match command {
                    "/help" => String::new(),
                    group => format!("{} ", &group[1..]),
                };
            }
            "/map-show" | "/map-hide" => self.set_map_shown(command == "/map-show"),
            // Switches a group for this session, or lists them all.
            "/group" => {
                let (name, wanted) = match argument.rsplit_once(' ') {
                    Some((name, "on")) => (name.trim(), Some(true)),
                    Some((name, "off")) => (name.trim(), Some(false)),
                    _ => (argument, None),
                };
                let known = self.scripts.groups();
                if name.is_empty() {
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
                } else if let Some(group) =
                    known.iter().find(|group| group.eq_ignore_ascii_case(name))
                {
                    let on = wanted.unwrap_or(!self.scripts.group_enabled(group));
                    self.scripts.set_group(group, on);
                    let said = if on {
                        "Group {0} on."
                    } else {
                        "Group {0} off."
                    };
                    self.note(&trf(said, &[group]));
                } else {
                    self.refuse(&trf(
                        "There is no group called {0}; /group lists them.",
                        &[name],
                    ));
                }
            }
            "/remove" if !argument.is_empty() => self.remove_plugin(argument),
            "/where" => {
                let place = self.map.borrow().current().map(|room| {
                    let exits: Vec<&str> = room.exits.keys().map(String::as_str).collect();
                    trf(
                        "{0} (#{1}) in {2}. Exits: {3}.",
                        &[
                            &room.name,
                            &room.id.to_string(),
                            &room.area,
                            &exits.join(", "),
                        ],
                    )
                });
                match place {
                    Some(place) => self.note(&place),
                    None => self.refuse(tr("The map does not know where you are yet.")),
                }
            }
            "/find" if !argument.is_empty() => {
                if !self.offer_rooms(argument) {
                    self.refuse(tr("No room matches."));
                }
            }
            "/go" if !argument.is_empty() => {
                if let Ok(room) = argument.parse::<RoomId>() {
                    self.walk_to(room);
                } else {
                    let found: Vec<RoomId> = (self.map.borrow().find(argument).iter())
                        .map(|room| room.id)
                        .collect();
                    match found[..] {
                        [] => self.refuse(tr("No room matches.")),
                        [room] => self.walk_to(room),
                        // Several: listed, to choose the one meant.
                        _ => {
                            self.offer_rooms(argument);
                        }
                    }
                }
            }
            // Toggles whether routes may pass through a room: the given
            // one, or the current one.
            "/avoid" => {
                let toggled = {
                    let mut map = self.map.borrow_mut();
                    let room = match argument.parse::<RoomId>() {
                        Ok(id) => map.room(id),
                        Err(_) if argument.is_empty() => map.current(),
                        Err(_) => None,
                    };
                    let target = room.map(|room| (room.id, !room.avoid, room.name.clone()));
                    target.map(|(id, avoid, name)| (map.set_avoid(id, avoid), avoid, name))
                };
                match toggled {
                    Some((Ok(_), true, name)) => {
                        self.note(&trf("Routes will avoid {0}.", &[&name.to_string()]))
                    }
                    Some((Ok(_), false, name)) => {
                        self.note(&trf(
                            "Routes may pass through {0} again.",
                            &[&name.to_string()],
                        ));
                    }
                    Some((Err(error), ..)) => {
                        self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()]))
                    }
                    None => self.refuse(tr("No such room; use /map avoid with a room number.")),
                }
            }
            // Toggles whether routes may use an exit of the current room.
            "/lock" if !argument.is_empty() => {
                let toggled = {
                    let mut map = self.map.borrow_mut();
                    let here = map.current().map(|room| room.id);
                    let direction = muddie_mapper::canonical_direction(argument);
                    let exit = here.and_then(|id| map.room(id)?.exits.get(&direction).cloned());
                    here.zip(exit).map(|(id, exit)| {
                        let locked = !exit.locked;
                        (
                            map.edit_exit(id, &direction, |exit| exit.locked = locked),
                            locked,
                        )
                    })
                };
                match toggled {
                    Some((Ok(_), true)) => self.note(tr("Routes will not use that exit.")),
                    Some((Ok(_), false)) => self.note(tr("Routes may use that exit again.")),
                    Some((Err(error), _)) => {
                        self.refuse(&trf("Could not save the map: {0}", &[&error.to_string()]))
                    }
                    None => self.refuse(tr("This room has no such exit.")),
                }
            }
            // Draws the current area again from scratch.
            "/tidy" => {
                let area = self.map.borrow().current().map(|room| room.area.clone());
                match area {
                    Some(area) => self.tidy(&area),
                    None => self.refuse(tr("The map does not know where you are yet.")),
                }
            }
            "/packages" => {
                let installed = self
                    .profiles
                    .as_ref()
                    .map(Profiles::packages)
                    .unwrap_or_default();
                if installed.is_empty() {
                    self.refuse(tr(
                        "No plugins are installed. Use /plugin install with a git address.",
                    ));
                }
                for package in installed {
                    let on = self.packages.contains(&package);
                    let (state, switch) = if on {
                        (tr("enabled in this profile"), "disable")
                    } else {
                        (tr("not enabled; use /plugin enable"), "enable")
                    };
                    self.offer(&package, state, &format!("/plugin {switch} {package}"));
                }
            }
            // Downloads a package from a git repository and enables it.
            "/install" if !argument.is_empty() => {
                let (address, given) = argument.split_once(' ').unwrap_or((argument, ""));
                let name = match given.trim() {
                    "" => packages::name_from_address(address),
                    given => crate::profile::valid_name(given).then(|| given.to_owned()),
                };
                let folder = self.profiles.as_ref().map(Profiles::packages_dir);
                match (folder, name) {
                    (Some(folder), Some(name)) => {
                        self.note(&trf("Installing {0}...", &[&name]));
                        let job = packages::install(&folder, &name, address);
                        self.package_jobs.push((job, true));
                    }
                    (None, _) => self.note(tr(
                        "Plugins need a configuration folder, and none was found.",
                    )),
                    (_, None) => self.note(tr(
                        "Give the plugin a name after the address: letters, digits, - and _.",
                    )),
                }
            }
            "/update" if !argument.is_empty() => {
                match self.profiles.as_ref().map(Profiles::packages_dir) {
                    Some(folder) => {
                        self.note(&trf("Updating {0}...", &[argument]));
                        let job = packages::update(&folder, argument);
                        self.package_jobs.push((job, false));
                    }
                    None => self.note(tr(
                        "Plugins need a configuration folder, and none was found.",
                    )),
                }
            }
            "/enable" | "/disable" if !argument.is_empty() => {
                let enable = command == "/enable";
                let installed = self
                    .profiles
                    .as_ref()
                    .map(Profiles::packages)
                    .unwrap_or_default();
                if enable && !installed.iter().any(|package| package == argument) {
                    self.note(tr("No such plugin is installed; see /plugin list."));
                } else {
                    self.set_package(argument, enable);
                }
            }
            "/reload" => {
                self.load_scripts();
                self.note(tr("Scripts reloaded."));
            }
            // Starts or stops logging the session.
            "/log" => match self.log.take() {
                Some(log) => {
                    let path = log.path().display().to_string();
                    self.note(&trf("Logging stopped; the log is {0}", &[&path]));
                }
                None => self.start_log(),
            },
            "/stop" => {
                let stopped = self.walk.take().is_some();
                self.note(if stopped {
                    tr("Walk stopped.")
                } else {
                    tr("Not walking.")
                });
            }
            _ => return false,
        }
        true
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
