//! The cheapest way from one room to another, or to the nearest of several.

use super::*;

impl Map {
    /// The cheapest known route between two rooms: each move costs the
    /// weight of the room entered plus the exit's own cost, locked exits
    /// are never used, and rooms marked to avoid are only entered as the
    /// destination. Empty if both are the same room; `None` if there is no
    /// route.
    pub fn route(&self, from: RoomId, to: RoomId) -> Option<Vec<Step>> {
        self.route_to_nearest(from, &HashSet::from([to]))
            .map(|(_, steps)| steps)
    }

    /// The cheapest known route to whichever of `targets` is cheapest to
    /// reach, as [`Map::route`] finds one, with the room it ends in. The
    /// search stops at the first it reaches, so the nearest of many is
    /// found as quickly as one.
    pub fn route_to_nearest(
        &self,
        from: RoomId,
        targets: &HashSet<RoomId>,
    ) -> Option<(RoomId, Vec<Step>)> {
        // Dijkstra's algorithm, remembering how each room was best reached.
        let mut cost_to: HashMap<RoomId, u64> = HashMap::from([(from, 0)]);
        let mut reached_by: HashMap<RoomId, (RoomId, &str)> = HashMap::new();
        let mut queue = BinaryHeap::from([Reverse((0_u64, from))]);
        let mut reached = None;
        while let Some(Reverse((cost, id))) = queue.pop() {
            if targets.contains(&id) {
                reached = Some(id);
                break;
            }
            if cost > cost_to[&id] {
                continue;
            }
            let Some(room) = self.rooms.get(&id) else {
                continue;
            };
            for (direction, exit) in &room.exits {
                if exit.locked {
                    continue;
                }
                let next = exit.to;
                // An unexplored room can be walked into, but not through.
                let (weight, avoid) = match self.rooms.get(&next) {
                    Some(room) => (room.weight, room.avoid),
                    None => (1, false),
                };
                if avoid && !targets.contains(&next) {
                    continue;
                }
                let next_cost = cost + u64::from(weight) + u64::from(exit.cost);
                if cost_to.get(&next).is_none_or(|known| next_cost < *known) {
                    cost_to.insert(next, next_cost);
                    let command =
                        (exit.command.as_deref()).unwrap_or_else(|| self.command_for(direction));
                    reached_by.insert(next, (id, command));
                    queue.push(Reverse((next_cost, next)));
                }
            }
        }
        let to = reached?;
        let mut steps = Vec::new();
        let mut at = to;
        while at != from {
            let &(previous, command) = reached_by.get(&at)?;
            steps.push(Step {
                command: command.to_owned(),
                to: at,
            });
            at = previous;
        }
        steps.reverse();
        Some((to, steps))
    }

    /// How many moves away each room is that can be reached from `from`,
    /// by exits that are not locked, through rooms routes may go through.
    pub fn moves_from(&self, from: RoomId) -> HashMap<RoomId, usize> {
        let mut moves = HashMap::from([(from, 0)]);
        let mut queue = VecDeque::from([from]);
        while let Some(id) = queue.pop_front() {
            let Some(room) = self.rooms.get(&id) else {
                continue;
            };
            let far = moves[&id] + 1;
            for exit in room.exits.values().filter(|exit| !exit.locked) {
                let through = self.rooms.get(&exit.to).is_some_and(|room| !room.avoid);
                if let std::collections::hash_map::Entry::Vacant(entry) = moves.entry(exit.to) {
                    entry.insert(far);
                    if through {
                        queue.push_back(exit.to);
                    }
                }
            }
        }
        moves
    }

    /// The rooms up to `moves` moves away, nearest first, each with the
    /// route to it: what is around, as routes would go there.
    pub fn around(&self, from: RoomId, moves: usize) -> Vec<(RoomId, Vec<Step>)> {
        let mut found: Vec<(RoomId, Vec<Step>)> = Vec::new();
        let mut seen = HashSet::from([from]);
        let mut frontier = vec![(from, Vec::<Step>::new())];
        for _ in 0..moves {
            let mut next_frontier = Vec::new();
            for (id, steps) in &frontier {
                let Some(room) = self.rooms.get(id) else {
                    continue;
                };
                for (direction, exit) in &room.exits {
                    if exit.locked || !self.rooms.contains_key(&exit.to) || !seen.insert(exit.to) {
                        continue;
                    }
                    let command =
                        (exit.command.as_deref()).unwrap_or_else(|| self.command_for(direction));
                    let mut route = steps.clone();
                    route.push(Step {
                        command: command.to_owned(),
                        to: exit.to,
                    });
                    next_frontier.push((exit.to, route));
                }
            }
            found.extend(next_frontier.iter().cloned());
            frontier = next_frontier;
        }
        found
    }
}
