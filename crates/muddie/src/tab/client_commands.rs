//! What the client does, as the palette's actions, the map and the
//! settings ask it: each by a line of its own, `/map go <place>`, which
//! nothing shows the user.

use super::*;

impl Tab {
    /// Carries out a command of the client's own. Returns false if `line`
    /// is not one of them.
    pub(super) fn do_client_command(&mut self, line: &str) -> bool {
        let line = line.trim();
        let (group, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        match group {
            "/lua" => return self.run_lua(rest),
            "/reload" => return self.restart_scripts_said(),
            "/tag" => return self.tag_command(rest),
            _ => {}
        }
        let (what, argument) = rest.split_once(' ').unwrap_or((rest, ""));
        let argument = argument.trim();
        let given = !argument.is_empty();
        match (group, what) {
            ("/map", "where") => self.say_where(),
            ("/map", "find") if given => self.find_rooms(argument),
            ("/map", "go") if given => self.go_to(argument),
            ("/map", "here") => self.here(argument),
            ("/map", "way") if given => self.say_route(argument),
            ("/map", "exits") => self.say_exits(),
            ("/map", "explore") => self.explore_map(),
            ("/map", "near") => self.say_near(argument),
            ("/map", "info") => self.say_info(argument),
            ("/map", "mark") if given => self.mark_here(argument),
            ("/map", "unmark") if given => self.unmark(argument),
            ("/map", "note") if given => self.note_here(argument),
            ("/map", "unnote") => self.note_here(""),
            ("/map", "areas") => self.list_areas(),
            ("/map", "show-area") if given => self.show_area(argument),
            ("/map", "import") => self.import_map(argument),
            ("/map", "avoid") => self.toggle_avoid(argument),
            ("/map", "block") if given => self.toggle_blocked(argument),
            ("/map", "added") => self.list_added(),
            ("/map", "check") => self.check_map(),
            ("/map", "remove") if given => self.remove_room_command(argument),
            ("/map", "stop") => self.stop_walking(),
            ("/plugin", "install") if given => self.install_plugin(argument),
            ("/plugin", "update") if given => self.update_plugin(argument),
            ("/plugin", "enable" | "disable") if given => {
                self.enable_plugin(argument, what == "enable");
            }
            // Taking a plugin away, files and all, is asked once more.
            ("/plugin", "remove") if given => {
                let name = argument.to_owned();
                self.app_request = Some(commands::AppRequest::RemovePlugin(name));
            }
            _ => return false,
        }
        true
    }

    /// Runs a line of Lua: what it prints is its answer, and if it prints
    /// nothing, that it was run is.
    fn run_lua(&mut self, code: &str) -> bool {
        self.scripts.exec("lua", code);
        self.run_actions();
        if self.palette.answer.is_empty() {
            self.note(tr("Done."));
        }
        true
    }

    /// Restarts the scripts, as asked.
    fn restart_scripts_said(&mut self) -> bool {
        self.restart_scripts();
        true
    }

    /// Switches a tag on or off for this session, as `name on` or
    /// `name off` say.
    fn tag_command(&mut self, argument: &str) -> bool {
        let wanted = match argument.rsplit_once(' ') {
            Some((name, "on")) => Some((name.trim(), true)),
            Some((name, "off")) => Some((name.trim(), false)),
            _ => None,
        };
        let known = self.scripts.tags();
        let tag = wanted.and_then(|(name, on)| {
            let key = muddie_script::tag_key(name);
            known
                .iter()
                .find(|tag| **tag == key)
                .map(|tag| (tag.clone(), on))
        });
        let Some((tag, on)) = tag else {
            self.refuse(&trf("There is no tag called {0}.", &[argument]));
            return true;
        };
        let mut switches = self.scripts.switches();
        switches.set_tag(&tag, on);
        self.scripts.set_switches(switches);
        self.run_actions();
        let said = if on { "Tag {0} on." } else { "Tag {0} off." };
        self.note(&trf(said, &[&tag]));
        true
    }

    /// Shows an area on the map, to look at it, without going.
    fn show_area(&mut self, area: &str) {
        let map = self.map.borrow();
        let Some(area) = (map.areas().into_iter())
            .find(|name| name.to_lowercase() == area.trim().to_lowercase())
        else {
            drop(map);
            return self.refuse(tr("The map has no area of that name."));
        };
        let area = area.to_owned();
        self.map_view.show_area(&map, &area);
        drop(map);
        self.set_map_shown(true);
    }

    /// Starts or stops logging the session.
    pub(super) fn toggle_log(&mut self) {
        match self.log.take() {
            Some(log) => {
                let path = log.path().display().to_string();
                self.note(&trf("Logging stopped; the log is {0}", &[&path]));
            }
            None => self.start_log(),
        }
    }
}
