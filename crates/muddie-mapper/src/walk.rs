//! Following a route one room at a time.

use std::collections::VecDeque;

use crate::{RoomId, Step};

/// A route being walked. Each move is sent only after the server confirms
/// arrival in the room the previous one led to, so a blocked exit or a
/// closed door stops the walk instead of sending the rest blindly.
#[derive(Debug)]
pub struct Walk {
    steps: VecDeque<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Send this direction next.
    Go(String),
    Arrived,
    /// The player ended up somewhere the route did not expect.
    Lost,
}

impl Walk {
    pub fn new(route: Vec<Step>) -> Self {
        Self {
            steps: route.into(),
        }
    }

    /// What to do first: the opening move, or nothing for an empty route.
    pub fn start(&self) -> Progress {
        match self.steps.front() {
            Some(step) => Progress::Go(step.direction.clone()),
            None => Progress::Arrived,
        }
    }

    /// The server reported the player in `room`; what to do next.
    pub fn entered(&mut self, room: RoomId) -> Progress {
        if self.steps.front().is_none_or(|step| step.to != room) {
            return Progress::Lost;
        }
        self.steps.pop_front();
        self.start()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(steps: &[(&str, RoomId)]) -> Vec<Step> {
        (steps.iter())
            .map(|(direction, to)| Step {
                direction: (*direction).to_owned(),
                to: *to,
            })
            .collect()
    }

    #[test]
    fn walks_step_by_step_to_the_end() {
        let mut walk = Walk::new(route(&[("n", 2), ("e", 3)]));
        assert_eq!(walk.start(), Progress::Go("n".into()));
        assert_eq!(walk.entered(2), Progress::Go("e".into()));
        assert_eq!(walk.entered(3), Progress::Arrived);
        assert_eq!(walk.entered(3), Progress::Lost);
    }

    #[test]
    fn unexpected_room_stops_the_walk() {
        let mut walk = Walk::new(route(&[("n", 2), ("e", 3)]));
        assert_eq!(walk.entered(7), Progress::Lost);
        assert_eq!(Walk::new(Vec::new()).start(), Progress::Arrived);
    }
}
