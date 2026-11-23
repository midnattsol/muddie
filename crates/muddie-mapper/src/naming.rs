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

/// A place found by what the user looked for (see [`Map::search`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub room: RoomId,
    pub by: FoundBy,
    /// How many moves away it is from where it was looked for from, if
    /// the map knows a way.
    pub moves: Option<usize>,
    /// Found by its name: how many rooms have it, of which this is the
    /// nearest.
    pub alike: usize,
}

/// What a place was found by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoundBy {
    /// The name the user gave it.
    Mark(String),
    /// The note the user wrote on it.
    Note,
    /// Its name, as the MUD names it.
    Name,
}

impl Map {
    /// The places `query` is in, ignoring case, as seen from `from`: the
    /// rooms the user gave a name that has it, then those whose note or
    /// name has it, each name once, at its nearest room; the nearest
    /// first. Rooms left over are not found.
    pub fn search(&self, query: &str, from: Option<RoomId>) -> Vec<Found> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let moves = from.map(|from| self.moves_from(from)).unwrap_or_default();
        let moves: &HashMap<RoomId, usize> = &moves;
        let far = |id: &RoomId| moves.get(id).copied();
        let nearest_first =
            |found: &Found, name: &str| (found.moves.is_none(), found.moves, name.to_lowercase());
        let mut marks: Vec<(Found, &str)> = (self.marks.iter())
            .filter(|(mark, id)| {
                mark.to_lowercase().contains(&query) && self.rooms.contains_key(id)
            })
            .map(|(mark, id)| {
                let found = Found {
                    room: *id,
                    by: FoundBy::Mark(mark.clone()),
                    moves: far(id),
                    alike: 1,
                };
                (found, mark.as_str())
            })
            .collect();
        marks.sort_by_cached_key(|(found, mark)| nearest_first(found, mark));
        let mut rest: Vec<(Found, &str)> = (self.rooms.values())
            .filter(|room| !room.note.is_empty() && room.note.to_lowercase().contains(&query))
            .filter(|room| !self.is_ghost(room.id))
            .map(|room| {
                let found = Found {
                    room: room.id,
                    by: FoundBy::Note,
                    moves: far(&room.id),
                    alike: 1,
                };
                (found, room.note.as_str())
            })
            .collect();
        // Each name once, at the nearest of the rooms that have it.
        let mut names: BTreeMap<&str, Found> = BTreeMap::new();
        for room in self.find(&query) {
            let moves = far(&room.id);
            let named = names.entry(self.bare_name(&room.name)).or_insert(Found {
                room: room.id,
                by: FoundBy::Name,
                moves,
                alike: 0,
            });
            named.alike += 1;
            if moves.is_some_and(|moves| named.moves.is_none_or(|known| moves < known)) {
                (named.room, named.moves) = (room.id, moves);
            }
        }
        rest.extend(names.into_iter().map(|(name, found)| (found, name)));
        rest.sort_by_cached_key(|(found, name)| nearest_first(found, name));
        // A room once, by the first it was found by.
        let mut seen = HashSet::new();
        (marks.into_iter().chain(rest))
            .map(|(found, _)| found)
            .filter(|found| seen.insert(found.room))
            .collect()
    }

    /// The names the user gave a room.
    pub fn marks_of(&self, room: RoomId) -> Vec<&str> {
        (self.marks.iter())
            .filter(|(_, id)| **id == room)
            .map(|(mark, _)| mark.as_str())
            .collect()
    }

    /// Takes how `other` was set up for the MUD: its words for directions,
    /// how it names rooms, and whether to learn as the player goes. The
    /// session's map, opened on connecting in place of the one scripts set
    /// up before, is set up as that one was.
    pub fn take_settings_from(&mut self, other: &Map) {
        self.settings = other.settings.clone();
        // The rooms by name are found again by its rule.
        self.names.borrow_mut().take();
        self.respell_exits();
    }

    /// Teaches the map how the MUD names its rooms, or, with the default,
    /// that it names them in no way of its own.
    pub fn set_naming(&mut self, naming: Naming) {
        self.settings.naming = naming;
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
        let Some((open, close)) = &self.settings.naming.exits_between else {
            return name;
        };
        (name.strip_suffix(close.as_str()))
            .and_then(|rest| Some(rest[..rest.rfind(open.as_str())?].trim_end()))
            .filter(|bare| !bare.is_empty())
            .unwrap_or(name)
    }

    /// A room's name without its region, if the MUD begins names with it,
    /// nor the exits written after it: "Plaza Mayor" of "Anduar: Plaza
    /// Mayor [n,s]".
    pub fn short_name<'a>(&self, room: &'a Room) -> &'a str {
        let bare = self.bare_name(&room.name);
        let Some(before) = &self.settings.naming.region_before else {
            return bare;
        };
        match bare.split_once(before.as_str()) {
            Some((region, rest)) if !region.trim().is_empty() && !rest.trim().is_empty() => {
                rest.trim()
            }
            _ => bare,
        }
    }

    /// Which way `to` lies from `from`, on the map as drawn, of the eight of
    /// the compass: none if they are not in one area and on one level, or
    /// are in one place.
    pub fn bearing(&self, from: RoomId, to: RoomId) -> Option<&'static str> {
        let (from, to) = (self.rooms.get(&from)?, self.rooms.get(&to)?);
        if from.area != to.area || from.pos.z != to.pos.z {
            return None;
        }
        let (dx, dy) = (to.pos.x - from.pos.x, to.pos.y - from.pos.y);
        if dx == 0 && dy == 0 {
            return None;
        }
        // Eight slices of the circle, each centred on its direction: the
        // compass ones, clockwise from the north at a quarter turn.
        let angle = f64::from(dy).atan2(f64::from(dx)).to_degrees();
        let slice = ((90.0 - angle + 720.0 + 22.5) / 45.0) as usize % 8;
        Some(DIRECTIONS[slice].key)
    }

    /// Where a room is, in words: what its name begins with, if the MUD
    /// names its rooms so, or its name if it begins with nothing; else its
    /// area, when the map has more than one. None when there is nothing
    /// to tell one place from another.
    pub fn region<'a>(&self, room: &'a Room) -> Option<&'a str> {
        if let Some(before) = &self.settings.naming.region_before {
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
    pub(crate) fn set_about(&mut self, key: &str, value: Option<&str>) -> Result<(), Error> {
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
    /// when the rooms have changed since; not those left over, which are
    /// not found by name.
    pub(crate) fn with_names<T>(&self, use_them: impl FnOnce(&Names) -> T) -> T {
        let mut names = self.names.borrow_mut();
        if names
            .as_ref()
            .is_none_or(|(revision, _)| *revision != self.revision)
        {
            let mut index = Names::new();
            self.with_ghosts(|ghosts| {
                for room in (self.rooms.values()).filter(|room| !ghosts.contains_key(&room.id)) {
                    index
                        .entry(self.name_key(&room.name))
                        .or_default()
                        .push(room.id);
                }
            });
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

    /// The room `query` means, as [`Map::rooms_meant`] finds them: of
    /// several, the nearest to the player that a route reaches, else the
    /// first.
    pub fn room_meant(&self, query: &str) -> Option<RoomId> {
        let rooms = self.rooms_meant(query);
        let from = self.current().map(|room| room.id);
        let nearest = from.filter(|_| rooms.len() > 1).and_then(|from| {
            let targets = rooms.iter().copied().collect();
            self.route_to_nearest(from, &targets).map(|(to, _)| to)
        });
        nearest.or_else(|| rooms.first().copied())
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
