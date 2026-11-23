//! The cheapest way from one room to another, or to the nearest of several.

use super::*;

impl Map {
    /// The cheapest known route between two rooms: each move costs the
    /// move, plus the cost of the room entered and of the exit, exits blocked to routes
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
                if exit.blocked {
                    continue;
                }
                let next = exit.to;
                // An unexplored room can be walked into, but not through.
                let (room_cost, avoid) = match self.rooms.get(&next) {
                    Some(room) => (room.cost, room.avoid),
                    None => (0, false),
                };
                if avoid && !targets.contains(&next) {
                    continue;
                }
                let next_cost = cost + 1 + u64::from(room_cost) + u64::from(exit.cost);
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
    /// by the cheapest routes, as [`Map::route`] finds them: the moves a
    /// walk there would take. Worked out once for a room and a revision
    /// of the map, as what is said of every place counts on it.
    pub fn moves_from(&self, from: RoomId) -> Distances {
        let mut known = self.distances.borrow_mut();
        if let Some((of, revision, distances)) = known.as_ref()
            && *of == from
            && *revision == self.revision
        {
            return std::sync::Arc::clone(distances);
        }
        // Dijkstra's algorithm, as for a route, over every room reached;
        // a room to avoid is reached, but not gone through.
        let mut cost_to: HashMap<RoomId, u64> = HashMap::from([(from, 0)]);
        let mut moves: HashMap<RoomId, usize> = HashMap::from([(from, 0)]);
        let mut queue = BinaryHeap::from([Reverse((0_u64, from))]);
        while let Some(Reverse((cost, id))) = queue.pop() {
            if cost > cost_to[&id] {
                continue;
            }
            let Some(room) = self.rooms.get(&id) else {
                continue;
            };
            if room.avoid && id != from {
                continue;
            }
            let far = moves[&id] + 1;
            for exit in room.exits.values().filter(|exit| !exit.blocked) {
                let next = exit.to;
                let room_cost = self.rooms.get(&next).map_or(0, |room| room.cost);
                let next_cost = cost + 1 + u64::from(room_cost) + u64::from(exit.cost);
                if cost_to.get(&next).is_none_or(|known| next_cost < *known) {
                    cost_to.insert(next, next_cost);
                    moves.insert(next, far);
                    queue.push(Reverse((next_cost, next)));
                }
            }
        }
        let distances = std::sync::Arc::new(moves);
        *known = Some((from, self.revision, std::sync::Arc::clone(&distances)));
        distances
    }

    /// How many moves a walk from `from` to `to` takes, if there is a way.
    pub fn moves_between(&self, from: RoomId, to: RoomId) -> Option<usize> {
        self.moves_from(from).get(&to).copied()
    }

    /// The rooms up to `moves` moves away, the cheapest to reach first,
    /// each with the route to it: what is around, by the routes a walk
    /// there takes, as [`Map::route`] finds them.
    pub fn around(&self, from: RoomId, moves: usize) -> Vec<(RoomId, Vec<Step>)> {
        // Dijkstra's algorithm, as for a route, no further than `moves`.
        let mut cost_to: HashMap<RoomId, u64> = HashMap::from([(from, 0)]);
        let mut reached_by: HashMap<RoomId, (RoomId, &str, usize)> = HashMap::new();
        let mut queue = BinaryHeap::from([Reverse((0_u64, from))]);
        let mut order = Vec::new();
        while let Some(Reverse((cost, id))) = queue.pop() {
            if cost > cost_to[&id] {
                continue;
            }
            if id != from {
                order.push(id);
            }
            let far = reached_by.get(&id).map_or(0, |(_, _, far)| *far);
            let Some(room) = self.rooms.get(&id) else {
                continue;
            };
            if (room.avoid && id != from) || far == moves {
                continue;
            }
            for (direction, exit) in room.exits.iter().filter(|(_, exit)| !exit.blocked) {
                let Some(next_room) = self.rooms.get(&exit.to) else {
                    continue;
                };
                let next_cost = cost + 1 + u64::from(next_room.cost) + u64::from(exit.cost);
                if cost_to.get(&exit.to).is_none_or(|known| next_cost < *known) {
                    cost_to.insert(exit.to, next_cost);
                    let command =
                        (exit.command.as_deref()).unwrap_or_else(|| self.command_for(direction));
                    reached_by.insert(exit.to, (id, command, far + 1));
                    queue.push(Reverse((next_cost, exit.to)));
                }
            }
        }
        order.dedup();
        (order.into_iter())
            .map(|to| {
                let mut steps = Vec::new();
                let mut at = to;
                while let Some(&(previous, command, _)) = reached_by.get(&at) {
                    steps.push(Step {
                        command: command.to_owned(),
                        to: at,
                    });
                    at = previous;
                }
                steps.reverse();
                (to, steps)
            })
            .collect()
    }
}
