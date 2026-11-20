//! What is handed to the scripts as it happens: lines, keys, input, data
//! from the MUD, events, and the passing of time.

use super::*;

impl Engine {
    /// Offers a completed output line to the triggers. Returns false if a
    /// trigger gagged it, meaning it must not be shown or spoken.
    pub fn on_line(&mut self, line: &str) -> bool {
        self.dispatch_line(line, None).is_some()
    }

    /// Like [`Self::on_line`], with the line's colours available to the
    /// triggers through `line_spans()`.
    /// Returns the line to show, which triggers may have recoloured, or
    /// `None` if one gagged it.
    pub fn on_styled_line(&mut self, line: &Line) -> Option<Line> {
        self.dispatch_line(&line.text(), Some(line.clone()))
    }

    /// Handles a key press, named as in `bind` (see [`key_name`]). Returns
    /// whether the key was bound, in which case nothing else should use it.
    pub fn on_key(&mut self, key: &str) -> bool {
        // A key can be bound once per group, so that it does one thing in
        // one mode and another in another: the first whose group is on.
        let action = {
            let state = self.state.borrow();
            let bound = state.bindings.get(&key_name(key));
            let live = bound.into_iter().flatten().find(|(group, _)| {
                group
                    .as_ref()
                    .is_none_or(|group| !state.disabled_groups.contains(group))
            });
            // Whose it is, for what it makes when it runs.
            let owner = |group: &Option<String>| {
                let name = key_name(key);
                let by =
                    (state.bound_by.iter()).find(|(_, known, of)| *known == name && of == group);
                by.map(|(owner, ..)| owner.clone()).unwrap_or_default()
            };
            match live {
                Some((_, Binding::Command(command))) => Ok(state.expand(command)),
                Some((group, Binding::Function(function))) => Err((owner(group), function.clone())),
                None => return false,
            }
        };
        match action {
            Ok(command) => self.on_input(&command),
            Err((owner, function)) => {
                self.as_owner(&owner, || function.call::<()>(()));
            }
        }
        true
    }

    pub(super) fn dispatch_line(&mut self, text: &str, styled: Option<Line>) -> Option<Line> {
        {
            let mut state = self.state.borrow_mut();
            state.gag = false;
            state.current_line = styled;
        }
        for hit in self.hits(text, |state| &state.triggers, usize::MAX) {
            let still_there = (self.state.borrow().triggers.iter()).any(|t| t.id == hit.id);
            if still_there {
                self.fire(&hit, text);
            }
        }
        let mut state = self.state.borrow_mut();
        let line = state.current_line.take();
        if std::mem::take(&mut state.gag) {
            return None;
        }
        Some(line.unwrap_or_default())
    }

    /// Handles a line typed by the user: a client command, or one or more
    /// commands, each of them an alias or sent as it is.
    pub fn on_input(&mut self, input: &str) {
        if let Some(code) = input.strip_prefix("/lua ") {
            return self.exec("input", code);
        }
        if input == "/reload" {
            return self.reload();
        }
        let separator = match &self.state.borrow().separator {
            Some(chosen) => chosen.clone(),
            None => Some(";".to_owned()),
        };
        for command in split_commands(input, separator.as_deref()) {
            match self.hits(&command, |state| &state.aliases, 1).pop() {
                Some(hit) => self.fire(&hit, &command),
                None => self.push(Action::Send(command)),
            }
        }
    }

    /// Records a GMCP message and runs the handlers for its package.
    /// `json` is the message's payload, empty if it had none.
    pub fn on_gmcp(&mut self, package: &str, json: &str) {
        self.on_data(package.to_lowercase(), package, json);
    }

    /// Records an MSDP variable and runs the handlers for it. `json` is the
    /// value converted to JSON.
    pub fn on_msdp(&mut self, variable: &str, json: &str) {
        self.on_data(msdp_key(variable), variable, json);
    }

    /// Tells scripts of something that happened in the client itself,
    /// such as `connect` or the `panel` changing size. `json` is what
    /// their handlers are given, empty for nothing.
    pub fn on_event(&mut self, name: &str, json: &str) {
        self.on_data(event_key(name), name, json);
    }

    /// Remembers the server's MSSP status for `mssp()`.
    pub fn on_mssp(&mut self, status: &[(String, String)]) {
        self.state.borrow_mut().mssp = status.to_vec();
    }

    /// Stores out-of-band data under `key` and calls the handlers covering
    /// it with the data and `name`.
    pub(super) fn on_data(&mut self, key: String, name: &str, json: &str) {
        let data = if json.is_empty() {
            Value::Nil
        } else {
            // JSON null becomes nil and arrays plain tables, as scripts expect.
            let options = SerializeOptions::new()
                .serialize_none_to_null(false)
                .serialize_unit_to_null(false)
                .set_array_metatable(false);
            let parsed = serde_json::from_str::<serde_json::Value>(json)
                .map_err(|error| error.to_string())
                .and_then(|value| {
                    (self.lua.to_value_with(&value, options)).map_err(|error| error.to_string())
                });
            match parsed {
                Ok(data) => data,
                Err(error) => return self.error(format!("bad data for {name}: {error}")),
            }
        };
        let callbacks: Vec<(u64, String, Function)> = {
            let mut state = self.state.borrow_mut();
            state.data.insert(key.clone(), data.clone());
            (state.data_handlers.iter())
                .filter(|handler| handler.covers(&key))
                .map(|handler| (handler.id, handler.owner.clone(), handler.callback.clone()))
                .collect()
        };
        for (id, owner, callback) in callbacks {
            let still_there = (self.state.borrow().data_handlers.iter()).any(|h| h.id == id);
            if still_there {
                self.as_owner(&owner, || callback.call::<()>((data.clone(), name)));
            }
        }
    }

    /// Runs the timers that are due at `now`.
    pub fn tick(&mut self, now: Instant) {
        let due: Vec<(u64, String, Function, bool)> = {
            let state = self.state.borrow();
            let off = |timer: &Timer| {
                timer
                    .group
                    .as_ref()
                    .is_some_and(|group| state.disabled_groups.contains(group))
            };
            (state.timers.iter())
                .filter(|timer| timer.due <= now)
                .map(|timer| {
                    (
                        timer.id,
                        timer.owner.clone(),
                        timer.callback.clone(),
                        off(timer),
                    )
                })
                .collect()
        };
        for (id, owner, callback, off) in due {
            // One whose group is off lets its turn pass.
            let ok = off || self.as_owner(&owner, || callback.call::<()>(()));
            let mut state = self.state.borrow_mut();
            // The callback may have cancelled its own timer.
            let Some(index) = state.timers.iter().position(|timer| timer.id == id) else {
                continue;
            };
            match state.timers[index].period {
                // A repeating timer that fails would fail forever: drop it.
                Some(period) if ok => state.timers[index].due = now + period,
                _ => {
                    state.timers.remove(index);
                }
            }
        }
    }

    pub(super) fn hits(
        &self,
        text: &str,
        list: impl Fn(&State) -> &Vec<Matcher>,
        limit: usize,
    ) -> Vec<Hit> {
        let state = self.state.borrow();
        let enabled = |matcher: &&Matcher| {
            (matcher.group.as_ref()).is_none_or(|group| !state.disabled_groups.contains(group))
        };
        let matches = list(&state).iter().filter(enabled).filter_map(|matcher| {
            let captures = matcher.regex.captures(text)?;
            let group = |m: regex::Match| m.as_str().to_owned();
            Some(Hit {
                id: matcher.id,
                owner: matcher.owner.clone(),
                once: matcher.once,
                range: captures.get(0).map_or((0, 0), |m| (m.start(), m.end())),
                callback: matcher.callback.clone(),
                groups: captures.iter().map(|m| m.map(group)).collect(),
                named: (matcher.regex.capture_names().flatten())
                    .filter_map(|name| Some((name.to_owned(), group(captures.name(name)?))))
                    .collect(),
            })
        });
        matches.take(limit).collect()
    }

    pub(super) fn fire(&mut self, hit: &Hit, text: &str) {
        self.state.borrow_mut().current_match = Some(hit.range);
        if hit.once {
            let mut state = self.state.borrow_mut();
            state.triggers.retain(|trigger| trigger.id != hit.id);
            state.aliases.retain(|alias| alias.id != hit.id);
        }
        self.as_owner(&hit.owner, || {
            let m = self.lua.create_table()?;
            for (index, group) in hit.groups.iter().enumerate() {
                m.set(index, group.as_deref())?;
            }
            for (name, value) in &hit.named {
                m.set(name.as_str(), value.as_str())?;
            }
            hit.callback.call::<()>((m, text))
        });
    }

    /// When [`Self::tick`] next has something to do.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.state.borrow().timers.iter().map(|t| t.due).min()
    }
}
