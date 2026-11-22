//! What the client does, as the palette's actions ask it: each by a
//! line of its own, `/map go <place>`, which nothing shows the user.

use super::*;

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
            "/map-show" | "/map-hide" => self.set_map_shown(command == "/map-show"),
            "/tag" => self.tag_command(argument),
            "/where" => self.say_where(),
            "/find" if given => self.find_rooms(argument),
            "/go" if given => self.go_to(argument),
            "/here" => self.here(argument),
            "/route" if given => self.say_route(argument),
            "/exits" => self.say_exits(),
            "/explore" => self.explore_map(),
            "/near" => self.say_near(argument),
            "/info" => self.say_info(argument),
            "/mark" => self.mark_here(argument),
            "/unmark" if given => self.unmark(argument),
            "/note" if given => self.note_here(argument),
            "/unnote" => self.note_here(""),
            "/areas" => self.list_areas(),
            "/import" => self.import_map(argument),
            "/avoid" => self.toggle_avoid(argument),
            "/lock" if given => self.toggle_lock(argument),
            "/tidy" => self.tidy_here(argument),
            "/added" => self.list_added(),
            "/undo" => self.undo_change(),
            "/check" => self.check_map(),
            "/remove-room" => self.remove_room_command(argument),
            "/packages" => self.list_plugins(),
            "/install" if given => self.install_plugin(argument),
            "/update" if given => self.update_plugin(argument),
            "/enable" | "/disable" if given => self.enable_plugin(argument, command == "/enable"),
            "/remove" if given => self.remove_plugin(argument),
            "/reload" => self.restart_scripts(),
            "/log" => self.toggle_log(),
            "/stop" => self.stop_walking(),
            _ => return false,
        }
        true
    }

    /// Switches a tag for this session, or lists them all.
    fn tag_command(&mut self, argument: &str) {
        let (name, wanted) = match argument.rsplit_once(' ') {
            Some((name, "on")) => (name.trim(), Some(true)),
            Some((name, "off")) => (name.trim(), Some(false)),
            _ => (argument, None),
        };
        let known = self.scripts.tags();
        if name.is_empty() {
            return self.list_tags(known);
        }
        let Some(tag) = known
            .iter()
            .find(|tag| **tag == muddie_script::tag_key(name))
        else {
            return self.refuse(&trf("There is no tag called {0}.", &[name]));
        };
        let mut switches = self.scripts.switches();
        let on = wanted.unwrap_or(!switches.tag_on(tag));
        switches.set_tag(tag, on);
        self.scripts.set_switches(switches);
        self.run_actions();
        let said = if on { "Tag {0} on." } else { "Tag {0} off." };
        self.note(&trf(said, &[tag]));
    }

    /// Lists the tags, each with whether it is on, to switch.
    fn list_tags(&mut self, known: Vec<String>) {
        if known.is_empty() {
            self.refuse(tr("There are no tags. Give a rule a tag in the editor."));
        }
        let switches = self.scripts.switches();
        for tag in known {
            let state = if switches.tag_on(&tag) {
                tr("on")
            } else {
                tr("off")
            };
            self.offer(&tag, state, &format!("/tag {tag}"));
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
        // Travel and walk were its names before; kept for those used to them.
        ("/map", "go" | "travel" | "walk") => "/go",
        ("/map", "here") => "/here",
        ("/map", "route") => "/route",
        ("/map", "exits") => "/exits",
        ("/map", "explore") => "/explore",
        ("/map", "near") => "/near",
        ("/map", "info") => "/info",
        ("/map", "mark") => "/mark",
        ("/map", "unmark") => "/unmark",
        ("/map", "note") => "/note",
        ("/map", "unnote") => "/unnote",
        ("/map", "areas") => "/areas",
        ("/map", "import") => "/import",
        ("/map", "stop") => "/stop",
        ("/map", "avoid") => "/avoid",
        ("/map", "lock") => "/lock",
        ("/map", "tidy") => "/tidy",
        ("/map", "added") => "/added",
        ("/map", "undo") => "/undo",
        ("/map", "check") => "/check",
        ("/map", "remove") => "/remove-room",
        ("/map", "show") => "/map-show",
        ("/map", "hide") => "/map-hide",
        ("/plugin", "list") => "/packages",
        ("/plugin", "install") => "/install",
        ("/plugin", "update") => "/update",
        ("/plugin", "enable") => "/enable",
        ("/plugin", "disable") => "/disable",
        ("/plugin", "remove") => "/remove",
        ("/tag", _) => {
            let after = line.trim_start().strip_prefix("/tag").unwrap_or_default();
            return Some(("/tag", after.trim().to_owned()));
        }
        ("/log", _) => "/log",
        ("/reload", _) => "/reload",
        _ => return None,
    };
    Some((command, rest))
}
