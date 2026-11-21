//! Words for places: the directions in the server's language, the names
//! the user gives rooms, rooms by their names, and where the map came from.

use super::*;

/// What the map keeps of where it was taken from, and of which version.
const SOURCE: &str = "source";
const SOURCE_VERSION: &str = "source version";

/// A room's name as compared: without case, the spaces around it, or the
/// exits some maps write after it in brackets (`Plaza [n,s]`).
pub(crate) fn name_key(name: &str) -> String {
    let name = name.trim();
    let bare = (name.strip_suffix(']'))
        .and_then(|rest| Some(rest[..rest.rfind('[')?].trim_end()))
        .filter(|bare| !bare.is_empty());
    bare.unwrap_or(name).to_lowercase()
}

impl Map {
    /// Teaches the map the server's words for directions: for each
    /// direction (`n`, `ne`... `u`, `d`, `in`, `out`), the words that mean
    /// it, the first being what a walk sends. Replaces what was taught.
    pub fn set_directions(&mut self, words: &[(String, Vec<String>)]) {
        self.words.clear();
        self.sent.clear();
        for (direction, said) in words {
            let direction = canonical_direction(direction);
            for word in said {
                (self.words).insert(word.trim().to_lowercase(), direction.clone());
            }
            if let Some(first) = said.first() {
                self.sent.insert(direction, first.trim().to_owned());
            }
        }
    }

    /// Takes what `other` was taught of the server's words for directions:
    /// a map opened in place of another, for the session, knows them too.
    pub fn learn_from(&mut self, other: &Map) {
        self.words.clone_from(&other.words);
        self.sent.clone_from(&other.sent);
    }

    /// A direction as the map keeps it, however the server or the user
    /// said it: in the server's words, in English, long or short.
    pub fn direction(&self, word: &str) -> String {
        let lower = word.trim().to_lowercase();
        match self.words.get(&lower) {
            Some(direction) => direction.clone(),
            None => canonical_direction(&lower),
        }
    }

    /// What a walk sends to go `direction`: the server's word for it, if
    /// the map was taught one.
    pub fn command_for<'a>(&'a self, direction: &'a str) -> &'a str {
        self.sent.get(direction).map_or(direction, String::as_str)
    }

    /// The rooms the user named, by name.
    pub fn marks(&self) -> &BTreeMap<String, RoomId> {
        &self.marks
    }

    /// The room the user named `name`, whatever the case.
    pub fn marked(&self, name: &str) -> Option<RoomId> {
        let name = name.trim().to_lowercase();
        (self.marks.iter()).find_map(|(mark, id)| (mark.to_lowercase() == name).then_some(*id))
    }

    /// Names a room, to go to it by that name. A name given before, in
    /// whatever case, is taken from the room it named.
    pub fn mark(&mut self, name: &str, room: RoomId) -> Result<(), Error> {
        let name = name.trim();
        self.unmark(name)?;
        self.marks.insert(name.to_owned(), room);
        match &mut self.store {
            Some(store) => store.mark(name, Some(room)),
            None => Ok(()),
        }
    }

    /// Forgets the name the user gave a room. Returns false if no room had
    /// that name.
    pub fn unmark(&mut self, name: &str) -> Result<bool, Error> {
        let name = name.trim().to_lowercase();
        let Some(mark) = self
            .marks
            .keys()
            .find(|mark| mark.to_lowercase() == name)
            .cloned()
        else {
            return Ok(false);
        };
        self.marks.remove(&mark);
        match &mut self.store {
            Some(store) => store.mark(&mark, None).map(|()| true),
            None => Ok(true),
        }
    }

    /// Where the map was taken from, and which version of it, if it was
    /// taken whole from elsewhere.
    pub fn source(&self) -> Option<(&str, Option<&str>)> {
        let source = self.about.get(SOURCE)?;
        Some((source, self.about.get(SOURCE_VERSION).map(String::as_str)))
    }

    /// Notes where the map was taken from and which version of it, or that
    /// it was not taken from anywhere.
    pub fn set_source(&mut self, source: Option<(&str, Option<&str>)>) -> Result<(), Error> {
        let (from, version) = source.unzip();
        let version = version.flatten();
        for (key, value) in [(SOURCE, from), (SOURCE_VERSION, version)] {
            match value {
                Some(value) => self.about.insert(key.to_owned(), value.to_owned()),
                None => self.about.remove(key),
            };
            if let Some(store) = &mut self.store {
                store.set_about(key, value)?;
            }
        }
        Ok(())
    }

    /// Calls `use_them` with the rooms by name as compared, made again
    /// when the rooms have changed since.
    pub(crate) fn with_names<T>(&self, use_them: impl FnOnce(&Names) -> T) -> T {
        let mut names = self.names.borrow_mut();
        if names
            .as_ref()
            .is_none_or(|(revision, _)| *revision != self.revision)
        {
            let mut index = Names::new();
            for room in self.rooms.values() {
                index.entry(name_key(&room.name)).or_default().push(room.id);
            }
            *names = Some((self.revision, index));
        }
        let (_, index) = names.as_ref().expect("made above");
        use_them(index)
    }

    /// The rooms called `name`, in order of number.
    pub(crate) fn called(&self, name: &str) -> Vec<RoomId> {
        let key = name_key(name);
        let mut ids = (self.with_names(|names| names.get(&key).cloned())).unwrap_or_default();
        ids.sort_unstable();
        ids
    }

    /// The rooms `query` may mean, as the user names a place to go to: a
    /// room's number, a mark, a name, or an area followed by a name in it.
    /// By a name, the rooms called just that if any are, else those whose
    /// name has it. An area alone means all its rooms.
    pub fn rooms_meant(&self, query: &str) -> Vec<RoomId> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        if let Ok(id) = query.parse::<RoomId>()
            && self.rooms.contains_key(&id)
        {
            return vec![id];
        }
        if let Some(id) = self.marked(query) {
            return vec![id];
        }
        let called = self.called(query);
        if !called.is_empty() {
            return called;
        }
        let in_area = self.area_and_name(query).map(|(area, name)| {
            let in_it = |id: &RoomId| self.rooms.get(id).is_some_and(|room| room.area == area);
            if name.is_empty() {
                let mut ids: Vec<RoomId> = (self.rooms.values())
                    .filter(|room| room.area == area)
                    .map(|room| room.id)
                    .collect();
                ids.sort_unstable();
                return ids;
            }
            let called: Vec<RoomId> = self.called(name).into_iter().filter(in_it).collect();
            if !called.is_empty() {
                return called;
            }
            (self.find(name).into_iter())
                .map(|room| room.id)
                .filter(in_it)
                .collect()
        });
        match in_area {
            Some(ids) if !ids.is_empty() => ids,
            _ => self.find(query).into_iter().map(|room| room.id).collect(),
        }
    }

    /// The area `query` begins with, the longest if several do, and what
    /// follows it.
    fn area_and_name<'a>(&'a self, query: &'a str) -> Option<(&'a str, &'a str)> {
        let lower = query.to_lowercase();
        let area = (self.areas().into_iter())
            .filter(|area| !area.is_empty())
            .filter(|area| {
                let area = area.to_lowercase();
                (lower.strip_prefix(&area))
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
            })
            .max_by_key(|area| area.len())?;
        // The same number of characters, though their case may differ.
        let length = area.chars().count();
        let at = query
            .char_indices()
            .nth(length)
            .map_or(query.len(), |(at, _)| at);
        Some((area, query[at..].trim()))
    }
}
