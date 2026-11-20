//! The cheapest way from one room to another.

use super::*;

impl Map {
    /// The cheapest known route between two rooms: each move costs the
    /// weight of the room entered plus the exit's own cost, locked exits
    /// are never used, and rooms marked to avoid are only entered as the
    /// destination. Empty if both are the same room; `None`
    /// if there is no route.
    pub fn route(&self, from: RoomId, to: RoomId) -> Option<Vec<Step>> {
        // Dijkstra's algorithm, remembering how each room was best reached.
        let mut cost_to: HashMap<RoomId, u64> = HashMap::from([(from, 0)]);
        let mut reached_by: HashMap<RoomId, (RoomId, &str)> = HashMap::new();
        let mut queue = BinaryHeap::from([Reverse((0_u64, from))]);
        while let Some(Reverse((cost, id))) = queue.pop() {
            if id == to {
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
                if avoid && next != to {
                    continue;
                }
                let next_cost = cost + u64::from(weight) + u64::from(exit.cost);
                if cost_to.get(&next).is_none_or(|known| next_cost < *known) {
                    cost_to.insert(next, next_cost);
                    let command = exit.command.as_deref().unwrap_or(direction);
                    reached_by.insert(next, (id, command));
                    queue.push(Reverse((next_cost, next)));
                }
            }
        }
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
        Some(steps)
    }
}
