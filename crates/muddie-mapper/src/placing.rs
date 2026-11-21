//! Placing the rooms visited: by the direction walked, beside a neighbour,
//! or as an island lined up once its way in is found.

use super::*;

/// How many cells a link may be stretched to find a free spot for a new
/// room whose natural place is taken.
pub(super) const MAX_STRETCH: i32 = 6;

impl Map {
    /// Records that the player is now in the room described by `info`,
    /// adding or updating it. The map is updated even if saving it fails.
    pub fn visit(&mut self, info: RoomInfo) -> Result<(), Error> {
        let id = info.id;
        let came_from = self.current;
        // Without coordinates from the server, the layout is ours to fix.
        let free_layout = info.pos.is_none();
        let saved = self.upsert(info, came_from);
        self.current = Some(id);
        self.candidates.clear();
        let joined = if free_layout {
            self.join_islands(came_from, id)
        } else {
            Ok(())
        };
        saved.and(joined)
    }

    /// Lines up separately drawn parts of the map that the room just
    /// entered shows to be connected: the part walked into, and any part an
    /// exit of this room leads to. A part is only moved when it has no other
    /// link to the rest, so consistent geometry is never disturbed.
    pub(super) fn join_islands(
        &mut self,
        came_from: Option<RoomId>,
        here: RoomId,
    ) -> Result<(), Error> {
        let mut links = Vec::new();
        if let Some(previous) = came_from.and_then(|id| self.rooms.get(&id)) {
            let exit = previous.exits.iter().find(|(_, exit)| exit.to == here);
            links.extend(exit.map(|(direction, _)| (previous.id, direction.clone(), here)));
        }
        if let Some(room) = self.rooms.get(&here) {
            let exits = room.exits.iter();
            links.extend(exits.map(|(direction, exit)| (here, direction.clone(), exit.to)));
        }
        let mut moved = Vec::new();
        for (anchor, direction, other) in links {
            moved.extend(self.align(anchor, &direction, other));
        }
        self.keep(moved);
        Ok(())
    }

    /// If `other` sits on a separate island from `anchor`, moves that island
    /// so `other` is `direction` of `anchor`. Returns the rooms it moved.
    pub(super) fn align(&mut self, anchor: RoomId, direction: &str, other: RoomId) -> Vec<RoomId> {
        let (Some(from), Some(to)) = (self.rooms.get(&anchor), self.rooms.get(&other)) else {
            return Vec::new();
        };
        let Some(step) = direction_offset(direction) else {
            return Vec::new();
        };
        if anchor == other || from.area != to.area || from.pos.in_line_with(to.pos, step) {
            return Vec::new();
        }
        let Some(island) = self.island(other, anchor) else {
            return Vec::new();
        };
        let (start, origin) = (from.pos, to.pos);
        let elsewhere: HashSet<Pos> = (self.rooms.values())
            .filter(|room| room.area == to.area && !island.contains(&room.id))
            .map(|room| room.pos)
            .collect();
        let shifted = |pos: Pos, target: Pos| Pos {
            x: pos.x + target.x - origin.x,
            y: pos.y + target.y - origin.y,
            z: pos.z + target.z - origin.z,
        };
        // As when placing one room: lengthen the link until nothing overlaps.
        let fits = |target: &Pos| {
            (island.iter()).all(|id| !elsewhere.contains(&shifted(self.rooms[id].pos, *target)))
        };
        let target = (1..=MAX_STRETCH)
            .map(|distance| start.offset(step, distance))
            .find(fits)
            .unwrap_or(start.offset(step, 1));
        for id in &island {
            let room = self.rooms.get_mut(id).expect("island rooms exist");
            room.pos = shifted(room.pos, target);
        }
        island.into_iter().collect()
    }

    /// The rooms of `start`'s area joined to it by exits in either
    /// direction, ignoring any direct link to `apart`. `None` if `apart` is
    /// among them, meaning the two are not on separate islands.
    pub(super) fn island(&self, start: RoomId, apart: RoomId) -> Option<HashSet<RoomId>> {
        let area = &self.rooms.get(&start)?.area;
        let mut neighbours: HashMap<RoomId, Vec<RoomId>> = HashMap::new();
        for room in self.rooms.values().filter(|room| &room.area == area) {
            for to in room.exits.values().map(|exit| exit.to) {
                let direct = (room.id, to) == (start, apart) || (room.id, to) == (apart, start);
                if !direct && self.rooms.get(&to).is_some_and(|other| &other.area == area) {
                    neighbours.entry(room.id).or_default().push(to);
                    neighbours.entry(to).or_default().push(room.id);
                }
            }
        }
        let mut island = HashSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(id) = queue.pop_front() {
            for next in neighbours.get(&id).into_iter().flatten() {
                if *next == apart {
                    return None;
                }
                if island.insert(*next) {
                    queue.push_back(*next);
                }
            }
        }
        Some(island)
    }

    /// Adds or updates a room without moving the player there, for servers
    /// or scripts that describe rooms other than the current one.
    pub fn set_room(&mut self, info: RoomInfo) -> Result<(), Error> {
        self.upsert(info, None)
    }

    pub(super) fn upsert(
        &mut self,
        info: RoomInfo,
        came_from: Option<RoomId>,
    ) -> Result<(), Error> {
        let pos = self.place(&info, came_from);
        let existing = self.rooms.get(&info.id);
        let old_exits = existing.map(|room| &room.exits);
        // The server decides where its exits lead; what the user added to
        // them, and the exits the user added, carry over.
        let mut exits: BTreeMap<String, Exit> = (info.exits.into_iter())
            .map(|(direction, to)| {
                let direction = self.direction(&direction);
                let old = old_exits.and_then(|exits| exits.get(&direction));
                let exit = Exit {
                    to,
                    custom: false,
                    ..old.cloned().unwrap_or_default()
                };
                (direction, exit)
            })
            .collect();
        for (direction, exit) in old_exits.into_iter().flatten() {
            if exit.custom && !exits.contains_key(direction) {
                exits.insert(direction.clone(), exit.clone());
            }
        }
        let room = Room {
            id: info.id,
            name: info.name,
            area: info.area,
            pos,
            exits,
            weight: existing.map_or(1, |room| room.weight),
            avoid: existing.is_some_and(|room| room.avoid),
        };
        if self.rooms.get(&room.id) == Some(&room) {
            return Ok(());
        }
        self.keep([room.id]);
        self.rooms.insert(room.id, room);
        Ok(())
    }

    /// Chooses where a room goes: where the server says, else where it
    /// already is, else next to a room it connects to, else clear of the
    /// rest of its area.
    pub(super) fn place(&self, info: &RoomInfo, came_from: Option<RoomId>) -> Pos {
        if let Some(pos) = info.pos {
            return pos;
        }
        if let Some(room) = self.rooms.get(&info.id) {
            return room.pos;
        }
        // An anchor room, the grid step of the exit joining it to the new
        // room, and which way along that step the new room lies.
        let after_previous = || {
            let previous = self.rooms.get(&came_from?)?;
            let (direction, _) = (previous.exits.iter()).find(|(_, exit)| exit.to == info.id)?;
            Some((previous.pos, direction_offset(direction)?, 1))
        };
        let beside_neighbour = || {
            info.exits.iter().find_map(|(direction, to)| {
                let neighbour = self.rooms.get(to)?;
                Some((neighbour.pos, direction_offset(direction)?, -1))
            })
        };
        let taken =
            |pos: Pos| (self.rooms.values()).any(|room| room.area == info.area && room.pos == pos);
        match after_previous().or_else(beside_neighbour) {
            // MUD geography is rarely a true grid: when the natural cell is
            // taken, lengthen the link rather than stack two rooms.
            Some((anchor, step, sign)) => (1..=MAX_STRETCH)
                .map(|distance| anchor.offset(step, sign * distance))
                .find(|pos| !taken(*pos))
                .unwrap_or(anchor.offset(step, sign)),
            // Nothing to hang it from (the first room, or after a teleport):
            // start a new island to the east of what is already drawn.
            None => {
                let east_edge = (self.rooms.values())
                    .filter(|room| room.area == info.area)
                    .map(|room| room.pos.x)
                    .max();
                Pos {
                    x: east_edge.map_or(0, |x| x + 2),
                    y: 0,
                    z: 0,
                }
            }
        }
    }
}
