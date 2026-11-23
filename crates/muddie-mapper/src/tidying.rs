//! Drawing an area again from scratch, around the player, if that draws
//! it better.

use super::*;
use crate::placing::MAX_STRETCH;

/// How an area is drawn: how many of its compass exits point another way
/// than they say, and how many of its rooms lie on another's cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Drawing {
    pub wrong_exits: usize,
    pub stacked: usize,
}

impl Drawing {
    /// Whether this is no worse than `other` in anything, and better in
    /// something.
    fn better_than(self, other: Drawing) -> bool {
        self.wrong_exits <= other.wrong_exits && self.stacked <= other.stacked && self != other
    }
}

/// What tidying an area came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tidied {
    /// Drawn again: how many rooms moved. None, if it was as tidy already.
    Moved(usize),
    /// Left as it was: drawn again, it would look worse. How it is drawn,
    /// and how it would be.
    Kept { now: Drawing, then: Drawing },
}

impl Map {
    /// Draws an area again from scratch, if that draws it better: starting
    /// from the player's room if it is there, each room is placed by the
    /// compass direction of the exits that reach it, on its own level. A
    /// drawing that would have more exits pointing wrong, or more rooms on
    /// one cell, is not kept: a map drawn by hand stays as it is.
    pub fn tidy(&mut self, area: &str) -> Result<Tidied, Error> {
        let mut members: Vec<RoomId> = (self.rooms.values())
            .filter(|room| room.area == area)
            .map(|room| room.id)
            .collect();
        members.sort_unstable();
        let now: HashMap<RoomId, Pos> =
            members.iter().map(|id| (*id, self.rooms[id].pos)).collect();
        let placed = self.drawn_again(area, &members);
        let (before, after) = (self.drawing(area, &now), self.drawing(area, &placed));
        if before == after {
            return Ok(Tidied::Moved(0));
        }
        if !after.better_than(before) {
            return Ok(Tidied::Kept {
                now: before,
                then: after,
            });
        }
        let mut moved = Vec::new();
        for (id, pos) in placed {
            let room = self.rooms.get_mut(&id).expect("members exist");
            if room.pos != pos {
                room.pos = pos;
                moved.push(id);
            }
        }
        let count = moved.len();
        self.keep(moved);
        Ok(Tidied::Moved(count))
    }

    /// Where each room of an area would be, drawn again from scratch: the
    /// player's room, or the first, stays; each room reached by a compass
    /// exit on its level is put the way the exit says, past rooms already
    /// there; a part no such exit reaches starts its own island east of
    /// everything placed. No room changes level.
    fn drawn_again(&self, area: &str, members: &[RoomId]) -> HashMap<RoomId, Pos> {
        // Every compass exit on one level links two rooms both ways: a
        // neighbour, the grid step, and which way along it it lies.
        let mut links: HashMap<RoomId, Vec<Link>> = HashMap::new();
        for id in members {
            let from = &self.rooms[id];
            for (direction, exit) in &from.exits {
                let level = (self.rooms.get(&exit.to))
                    .is_some_and(|room| room.area == area && room.pos.z == from.pos.z);
                let step = direction_offset(direction).filter(|step| step.2 == 0);
                if let (Some(step), true) = (step, level) {
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
        placed
    }

    /// How an area would be drawn with its rooms at `at`.
    fn drawing(&self, area: &str, at: &HashMap<RoomId, Pos>) -> Drawing {
        let mut drawing = Drawing::default();
        let mut cells: HashSet<Pos> = HashSet::new();
        for (id, pos) in at {
            if !cells.insert(*pos) {
                drawing.stacked += 1;
            }
            for (direction, exit) in &self.rooms[id].exits {
                let (Some(step), Some(to)) = (direction_offset(direction), at.get(&exit.to)) else {
                    continue;
                };
                let inside = self
                    .rooms
                    .get(&exit.to)
                    .is_some_and(|room| room.area == area);
                if step.2 != 0 || !inside || to.z != pos.z || exit.to == *id {
                    continue;
                }
                let (dx, dy) = (to.x - pos.x, to.y - pos.y);
                let its_way = (dx.signum(), dy.signum()) == (step.0, step.1)
                    && (dx == 0 || dy == 0 || dx.abs() == dy.abs());
                if !its_way {
                    drawing.wrong_exits += 1;
                }
            }
        }
        drawing
    }
}
