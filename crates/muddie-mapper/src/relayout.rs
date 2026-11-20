//! Drawing an area again from scratch, around the player.

use super::*;
use crate::placing::MAX_STRETCH;

impl Map {
    /// Draws an area again from scratch: starting from the player's room
    /// if it is there, each room is placed by the compass direction of the
    /// exits that reach it. For an area whose drawing has become tangled.
    /// Returns how many rooms moved.
    pub fn relayout(&mut self, area: &str) -> Result<usize, Error> {
        let mut members: Vec<RoomId> = (self.rooms.values())
            .filter(|room| room.area == area)
            .map(|room| room.id)
            .collect();
        members.sort_unstable();
        // Every compass exit links two rooms both ways: a neighbour, the
        // grid step, and which way along it the neighbour lies.
        let mut links: HashMap<RoomId, Vec<Link>> = HashMap::new();
        for id in &members {
            for (direction, exit) in &self.rooms[id].exits {
                let inside = (self.rooms.get(&exit.to)).is_some_and(|room| room.area == area);
                if let (Some(step), true) = (direction_offset(direction), inside) {
                    links.entry(*id).or_default().push((exit.to, step, 1));
                    links.entry(exit.to).or_default().push((*id, step, -1));
                }
            }
        }
        let player = self.current.filter(|id| members.contains(id));
        let mut placed: HashMap<RoomId, Pos> = HashMap::new();
        let mut taken: HashSet<Pos> = HashSet::new();
        for seed in player.into_iter().chain(members.iter().copied()) {
            if placed.contains_key(&seed) {
                continue;
            }
            // The first room stays where it is; a part no compass exit
            // reaches starts its own island east of everything placed.
            let old = self.rooms[&seed].pos;
            let east_edge = placed.values().map(|pos| pos.x).max();
            let start = match east_edge {
                Some(x) => Pos {
                    x: x + 2,
                    y: 0,
                    z: old.z,
                },
                None => old,
            };
            placed.insert(seed, start);
            taken.insert(start);
            let mut queue = VecDeque::from([seed]);
            while let Some(id) = queue.pop_front() {
                for &(next, step, sign) in links.get(&id).into_iter().flatten() {
                    if placed.contains_key(&next) {
                        continue;
                    }
                    let from = placed[&id];
                    let pos = (1..=MAX_STRETCH)
                        .map(|distance| from.offset(step, sign * distance))
                        .find(|pos| !taken.contains(pos))
                        .unwrap_or(from.offset(step, sign));
                    placed.insert(next, pos);
                    taken.insert(pos);
                    queue.push_back(next);
                }
            }
        }
        let mut moved = Vec::new();
        for (id, pos) in placed {
            let room = self.rooms.get_mut(&id).expect("members exist");
            if room.pos != pos {
                room.pos = pos;
                moved.push(id);
            }
        }
        let rooms = moved.iter().filter_map(|id| self.rooms.get(id));
        if let Some(store) = &mut self.store {
            store.save(rooms)?;
        }
        Ok(moved.len())
    }
}
