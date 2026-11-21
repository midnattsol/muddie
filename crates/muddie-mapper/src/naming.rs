//! Words for places: the directions in the server's language, the names
//! the user gives rooms, rooms by their names, and where the map came from.

use super::*;

/// What the map keeps of where it was taken from, and of which version.
const SOURCE: &str = "source";
const SOURCE_VERSION: &str = "source version";
/// Which reader read the map from there (see [`crate::MUDLET_READER`]).
const SOURCE_READER: &str = "source reader";

/// How a MUD names its rooms, where it has a way of its own that the map
/// is to understand: none, unless taught (see [`Map::set_naming`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Naming {
    /// A room's name begins with its region, up to this ("Anduar" in
    /// "Anduar: Plaza Mayor" with ":"); else the region is its area.
    pub region_before: Option<String>,
    /// A room's name may end with its exits between these two
    /// ("Plaza [n,s]" with "[" and "]"), which are not part of the name.
    pub exits_between: Option<(String, String)>,
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

    /// Takes what `other` was taught of the MUD's ways, its words for
    /// directions and how it names rooms: a map opened in place of
    /// another, for the session, knows them too.
    pub fn learn_from(&mut self, other: &Map) {
        self.words.clone_from(&other.words);
        self.sent.clone_from(&other.sent);
        self.set_naming(other.naming.clone());
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

    /// Teaches the map how the MUD names its rooms, or, with the default,
    /// that it names them in no way of its own.
    pub fn set_naming(&mut self, naming: Naming) {
        self.naming = naming;
        // The rooms by name are found again by the new rule.
        self.names.borrow_mut().take();
    }

    /// A room's name as compared: without case, the spaces around it, or
    /// the exits written after it, if the MUD writes them so.
    pub(crate) fn name_key(&self, name: &str) -> String {
        self.bare_name(name).to_lowercase()
    }

    /// A room's name without the exits written after it, if the MUD
    /// writes them so.
    pub fn bare_name<'a>(&self, name: &'a str) -> &'a str {
        let name = name.trim();
        let Some((open, close)) = &self.naming.exits_between else {
            return name;
        };
        (name.strip_suffix(close.as_str()))
            .and_then(|rest| Some(rest[..rest.rfind(open.as_str())?].trim_end()))
            .filter(|bare| !bare.is_empty())
            .unwrap_or(name)
    }

    /// Where a room is, in words: what its name begins with, if the MUD
    /// names its rooms so, or its name if it begins with nothing; else its
    /// area, when the map has more than one. None when there is nothing
    /// to tell one place from another.
    pub fn region<'a>(&self, room: &'a Room) -> Option<&'a str> {
        if let Some(before) = &self.naming.region_before {
            let bare = self.bare_name(&room.name);
            return match bare.split_once(before.as_str()) {
                Some((region, _)) if !region.trim().is_empty() => Some(region.trim()),
                _ => Some(bare).filter(|bare| !bare.is_empty()),
            };
        }
        (self.several_areas() && !room.area.is_empty()).then_some(room.area.as_str())
    }

    /// The regions a route goes through, in order, each once, but the one
    /// it starts in and the one it ends in.
    pub fn regions_along(&self, from: RoomId, steps: &[Step]) -> Vec<String> {
        let region = |id: RoomId| {
            (self.rooms.get(&id)).and_then(|room| self.region(room).map(str::to_owned))
        };
        let (start, end) = (region(from), steps.last().and_then(|step| region(step.to)));
        let mut regions: Vec<String> = Vec::new();
        for region in steps.iter().filter_map(|step| region(step.to)) {
            let known = regions.contains(&region);
            if !known && Some(&region) != start.as_ref() && Some(&region) != end.as_ref() {
                regions.push(region);
            }
        }
        regions
    }

    /// Whether the rooms are in more than one area: worked out again only
    /// when they change.
    fn several_areas(&self) -> bool {
        match self.several_areas.get() {
            Some((revision, several)) if revision == self.revision => several,
            _ => {
                let mut areas = self.rooms.values().map(|room| room.area.as_str());
                let first = areas.next();
                let several = areas.any(|area| Some(area) != first);
                self.several_areas.set(Some((self.revision, several)));
                several
            }
        }
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
        self.set_about(SOURCE, from)?;
        self.set_about(SOURCE_VERSION, version.flatten())
    }

    /// Which reader read the map taken from its source, if that was noted:
    /// maps taken before it was are not.
    pub fn source_reader(&self) -> Option<u32> {
        self.about.get(SOURCE_READER)?.parse().ok()
    }

    /// Notes which reader read the map taken from its source, or forgets it.
    pub fn set_source_reader(&mut self, reader: Option<u32>) -> Result<(), Error> {
        self.set_about(
            SOURCE_READER,
            reader.map(|reader| reader.to_string()).as_deref(),
        )
    }

    /// Keeps something known about the map under `key`, or forgets it.
    fn set_about(&mut self, key: &str, value: Option<&str>) -> Result<(), Error> {
        match value {
            Some(value) => self.about.insert(key.to_owned(), value.to_owned()),
            None => self.about.remove(key),
        };
        match &mut self.store {
            Some(store) => store.set_about(key, value),
            None => Ok(()),
        }
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
                index
                    .entry(self.name_key(&room.name))
                    .or_default()
                    .push(room.id);
            }
            *names = Some((self.revision, index));
        }
        let (_, index) = names.as_ref().expect("made above");
        use_them(index)
    }

    /// The rooms called `name`, as the MUD names them, in order of number.
    pub fn called(&self, name: &str) -> Vec<RoomId> {
        let key = self.name_key(name);
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
