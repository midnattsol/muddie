//! Following a route one room at a time.

use std::collections::VecDeque;

use crate::{RoomId, Step};

/// A route being walked. Each move is sent only after the server confirms
/// arrival in the room the previous one led to, so a blocked exit or a
/// closed door stops the walk instead of sending the rest blindly.
#[derive(Debug)]
pub struct Walk {
    /// The room the player was last known to be in.
    at: RoomId,
    steps: VecDeque<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Send this next: one or more commands separated by `;`.
    Go(String),
    /// Nothing to do: the pending move has not taken effect yet.
    Waiting,
    Arrived,
    /// The player ended up somewhere the route did not expect.
    Lost,
}

impl Walk {
    /// A walk along `route`, starting in the room `from`.
    pub fn new(from: RoomId, route: Vec<Step>) -> Self {
        Self {
            at: from,
            steps: route.into(),
        }
    }

    /// What to do first: the opening move, or nothing for an empty route.
    pub fn start(&self) -> Progress {
        match self.steps.front() {
            Some(step) => Progress::Go(step.command.clone()),
            None => Progress::Arrived,
        }
    }

    /// How many moves are left, and the room they end in.
    pub fn left(&self) -> (usize, Option<RoomId>) {
        (self.steps.len(), self.steps.back().map(|step| step.to))
    }

    /// The server reported the player in `room`; what to do next.
    pub fn entered(&mut self, room: RoomId) -> Progress {
        match self.steps.front() {
            Some(step) if step.to == room => {
                self.at = room;
                self.steps.pop_front();
                self.start()
            }
            // Servers repeat the current room, for instance after `look`.
            Some(_) if room == self.at => Progress::Waiting,
            _ => Progress::Lost,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(steps: &[(&str, RoomId)]) -> Vec<Step> {
        (steps.iter())
            .map(|(command, to)| Step {
                command: (*command).to_owned(),
                to: *to,
            })
            .collect()
    }

    #[test]
    fn walks_step_by_step_to_the_end() {
        let mut walk = Walk::new(1, route(&[("n", 2), ("e", 3)]));
        assert_eq!(walk.start(), Progress::Go("n".into()));
        assert_eq!(walk.left(), (2, Some(3)));
        assert_eq!(walk.entered(1), Progress::Waiting);
        assert_eq!(walk.entered(2), Progress::Go("e".into()));
        assert_eq!(walk.entered(2), Progress::Waiting);
        assert_eq!(walk.entered(3), Progress::Arrived);
        assert_eq!(walk.entered(3), Progress::Lost);
    }

    #[test]
    fn unexpected_room_stops_the_walk() {
        let mut walk = Walk::new(1, route(&[("n", 2), ("e", 3)]));
        assert_eq!(walk.entered(7), Progress::Lost);
        assert_eq!(Walk::new(1, Vec::new()).start(), Progress::Arrived);
    }
}
