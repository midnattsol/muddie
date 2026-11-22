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
        // A key bound several times does what the first that is on does.
        let action = {
            let state = self.state.borrow();
            let bound = state.bindings.get(&key_name(key));
            let live = (bound.into_iter().flatten())
                .find(|bound| state.switches.active(Kind::Macro, &bound.belongs));
            match live.map(|bound| (&bound.owner, &bound.binding)) {
                Some((_, Binding::Command(command))) => Ok(state.expand(command)),
                // Whose it is, for what it makes when it runs.
                Some((owner, Binding::Function(function))) => {
                    Err((owner.clone(), function.clone()))
                }
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
        for hit in self.hits(text, Kind::Trigger, usize::MAX) {
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
        let separator = match &self.state.borrow().separator {
            Some(chosen) => chosen.clone(),
            None => Some(";".to_owned()),
        };
        for command in split_commands(input, separator.as_deref()) {
            match self.hits(&command, Kind::Alias, 1).pop() {
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
            // A package that sends only what changed is merged into what
            // was kept of it; its handlers still get what came.
            let kept = match (state.data.get(&key), &data) {
                (Some(Value::Table(old)), Value::Table(new)) if state.merged.contains(&key) => {
                    match merged(&self.lua, old, new) {
                        Ok(both) => Value::Table(both),
                        Err(error) => {
                            drop(state);
                            return self.error(format!("bad data for {name}: {error}"));
                        }
                    }
                }
                _ => data.clone(),
            };
            state.data.insert(key.clone(), kept);
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
            let off = |timer: &Timer| !state.switches.active(Kind::Timer, &timer.belongs);
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
            // One switched off lets its turn pass.
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

    pub(super) fn hits(&self, text: &str, kind: Kind, limit: usize) -> Vec<Hit> {
        let candidates = match kind {
            Kind::Trigger => self.matching_triggers(text),
            _ => None,
        };
        let state = self.state.borrow();
        let enabled = |matcher: &&Matcher| state.switches.active(kind, &matcher.belongs);
        let list = match kind {
            Kind::Alias => &state.aliases,
            _ => &state.triggers,
        };
        let candidate = |index: usize| (candidates.as_ref()).is_none_or(|may| may[index]);
        let listed = list
            .iter()
            .enumerate()
            .filter(|(index, _)| candidate(*index));
        let matches = listed
            .map(|(_, matcher)| matcher)
            .filter(enabled)
            .filter_map(|matcher| {
                let regex = matcher.pattern.regex()?;
                let captures = regex.captures(text)?;
                let group = |m: regex::Match| m.as_str().to_owned();
                Some(Hit {
                    id: matcher.id,
                    owner: matcher.owner.clone(),
                    once: matcher.once,
                    range: captures.get(0).map_or((0, 0), |m| (m.start(), m.end())),
                    callback: matcher.callback.clone(),
                    groups: captures.iter().map(|m| m.map(group)).collect(),
                    named: (regex.capture_names().flatten())
                        .filter_map(|name| Some((name.to_owned(), group(captures.name(name)?))))
                        .collect(),
                })
            });
        matches.take(limit).collect()
    }

    /// Which triggers may match a line, learned in one pass over it with
    /// all their patterns in one set: with many triggers, much quicker
    /// than trying each. None with few. While the set of patterns changed
    /// is made, the set before says for the triggers whose patterns it
    /// had, and the others are tried each: as one is being written, only it.
    fn matching_triggers(&self, text: &str) -> Option<Vec<bool>> {
        /// Below this many, trying each is as quick.
        const SET_FROM: usize = 16;
        let mut state = self.state.borrow_mut();
        if state.triggers.len() < SET_FROM {
            return None;
        }
        let state = &mut *state;
        let set = &mut state.trigger_set;
        let same = set.ids.len() == state.triggers.len()
            && (set.ids.iter().zip(&state.triggers)).all(|(id, matcher)| *id == matcher.id);
        let mut remap = !same;
        if !same {
            let patterns: Vec<String> = (state.triggers.iter())
                .map(|matcher| matcher.pattern.as_str().to_owned())
                .collect();
            set.ids = state.triggers.iter().map(|matcher| matcher.id).collect();
            set.making = Some(make_set(patterns));
        }
        // With no set before, the first is waited for, once: trying the
        // lines on each trigger meanwhile would compile every pattern.
        let first = set.set.is_none() && set.patterns.is_empty();
        let made = set.making.as_ref().and_then(|making| match first {
            true => making.recv().ok(),
            false => making.try_recv().ok(),
        });
        if let Some((patterns, made)) = made {
            (set.patterns, set.set, set.making) = (patterns, made, None);
            remap = true;
        }
        if remap {
            // Where each trigger's pattern is in the set there is.
            let place: HashMap<&str, usize> = (set.patterns.iter().enumerate())
                .map(|(at, pattern)| (pattern.as_str(), at))
                .collect();
            set.places = (state.triggers.iter())
                .map(|matcher| place.get(matcher.pattern.as_str()).copied())
                .collect();
        }
        let matches = set.set.as_ref()?.matches(text);
        Some(
            set.places
                .iter()
                .map(|place| place.is_none_or(|at| matches.matched(at)))
                .collect(),
        )
    }

    /// Waits for the set of the triggers' patterns being made, if it is:
    /// for tests that measure, or count on, the set.
    #[cfg(test)]
    pub(crate) fn trigger_set_made(&self) -> bool {
        let mut state = self.state.borrow_mut();
        let set = &mut state.trigger_set;
        if let Some(making) = set.making.take()
            && let Ok((patterns, made)) = making.recv()
        {
            (set.patterns, set.set) = (patterns, made);
            set.places = (0..set.patterns.len()).map(Some).collect();
        }
        set.set.is_some()
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

/// Puts patterns in one set, on a thread of its own, as many take a
/// moment: what it sends is the patterns and the set, None if they cannot
/// be one.
fn make_set(
    patterns: Vec<String>,
) -> std::sync::mpsc::Receiver<(Vec<String>, Option<regex::RegexSet>)> {
    /// How big the set may grow: patterns with `\w`, which is every letter
    /// of every language, are big, and three hundred of them do not fit in
    /// what the regex crate allows by default, which left every line tried
    /// on every trigger.
    const SET_SIZE: usize = 128 << 20;
    let (made, making) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let set = regex::RegexSetBuilder::new(&patterns)
            .size_limit(SET_SIZE)
            .dfa_size_limit(SET_SIZE)
            .build()
            .ok();
        let _ = made.send((patterns, set));
    });
    making
}

/// A copy of `old` with what is in `new` put over it, tables inside both
/// merged the same way.
fn merged(lua: &Lua, old: &mlua::Table, new: &mlua::Table) -> mlua::Result<mlua::Table> {
    let both = lua.create_table()?;
    for pair in old.pairs::<Value, Value>() {
        let (key, value) = pair?;
        both.raw_set(key, value)?;
    }
    for pair in new.pairs::<Value, Value>() {
        let (key, value) = pair?;
        let value = match (both.raw_get::<Value>(key.clone())?, value) {
            (Value::Table(before), Value::Table(now)) => Value::Table(merged(lua, &before, &now)?),
            (_, now) => now,
        };
        both.raw_set(key, value)?;
    }
    Ok(both)
}
