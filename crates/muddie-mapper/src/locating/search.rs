//! The search for the room the player has come to: the rooms that fit
//! what the server showed, by the exit taken, around, or by shape.

use super::*;

/// What the server showed of a room, and where the player may have been,
/// to look for the room by.
pub(super) struct Search<'a> {
    pub(super) map: &'a Map,
    pub(super) seen: Sighting<'a>,
    /// The exit taken, as the server says or as the move sent.
    pub(super) came: Option<&'a str>,
    pub(super) key: &'a Option<String>,
    pub(super) exits: &'a Option<BTreeSet<String>>,
    pub(super) pending: &'a [(RoomId, String)],
    pub(super) before: &'a [RoomId],
}

/// The rooms found, and how they were.
pub(super) struct Found {
    pub(super) rooms: Vec<RoomId>,
    /// The room the player was followed to though it is named otherwise.
    pub(super) followed: Option<RoomId>,
    /// Whether the room is one next to where the player was, the exit
    /// being wrong in the map.
    pub(super) beside: bool,
}

impl Search<'_> {
    /// Whether a room has the name shown, or is waiting to be put right
    /// as named so.
    fn named(&self, id: RoomId) -> bool {
        (self.key.as_ref()).is_none_or(|key| {
            let called = |name: &str| self.map.name_key(name) == *key;
            (self.map.rooms.get(&id)).is_some_and(|room| called(&room.name))
                || (self.pending.iter()).any(|(room, name)| *room == id && called(name))
        })
    }

    /// Whether a room has the name and the exits shown.
    fn like(&self, id: RoomId) -> bool {
        self.named(id)
            && self
                .exits
                .as_ref()
                .is_none_or(|exits| self.map.shows_exits(id, exits))
    }

    /// The rooms that the exit `way` leads to from where the player was.
    fn led_by(&self, way: &str) -> Vec<RoomId> {
        let mut led: Vec<RoomId> = (self.before.iter())
            .filter_map(|id| Some(self.map.rooms.get(id)?.exits.get(way)?.to))
            .filter(|id| self.map.rooms.contains_key(id))
            .collect();
        led.sort_unstable();
        led.dedup();
        led
    }

    /// A room of that name the exit leads to, though its exits are not
    /// those shown, is taken only if no room of that name has them: else
    /// the map is not trusted there, and the player is looked for.
    fn fits_elsewhere(&self) -> bool {
        self.exits.is_some()
            && (self.seen.name)
                .is_some_and(|name| self.map.called(name).into_iter().any(|id| self.like(id)))
    }

    /// The rooms the exit taken leads to that fit what was shown.
    fn moved(&self) -> Vec<RoomId> {
        match self.came {
            Some(way) => {
                let led_to: Vec<RoomId> = (self.led_by(way).into_iter())
                    .filter(|id| self.named(*id))
                    .collect();
                let fitting: Vec<RoomId> = (led_to.iter().copied())
                    .filter(|id| self.like(*id))
                    .collect();
                if !fitting.is_empty() {
                    fitting
                } else if self.fits_elsewhere() {
                    Vec::new()
                } else {
                    led_to
                }
            }
            None => Vec::new(),
        }
    }

    /// The rooms up to two moves from where the player was that are as
    /// the server shows: where a map that is wrong in a place, or a move
    /// it did not follow, most likely left the player.
    fn around_within(&self, moves: usize) -> Vec<RoomId> {
        let mut near: Vec<RoomId> = Vec::new();
        let mut seen: HashSet<RoomId> = self.before.iter().copied().collect();
        let mut ring = self.before.to_vec();
        for _ in 0..moves {
            let next: Vec<RoomId> = (ring.iter())
                .filter_map(|id| self.map.rooms.get(id))
                .flat_map(|room| room.exits.values().map(|exit| exit.to))
                .filter(|id| seen.insert(*id))
                .collect();
            near.extend(next.iter().copied().filter(|id| self.like(*id)));
            if !near.is_empty() {
                break;
            }
            ring = next;
        }
        near
    }

    /// By the way the server says was taken and the exits shown, though
    /// not the name: the one room it leads to with just those exits.
    fn shaped(&self) -> Option<RoomId> {
        let (Some(way), Some(exits), Some(_)) =
            (self.seen.came.and(self.came), self.exits, self.key)
        else {
            return None;
        };
        match self.led_by(way).as_slice() {
            [one] if self.map.shows_exits(*one, exits) && !exits.is_empty() => Some(*one),
            _ => None,
        }
    }

    /// The rooms the player may be in, by each way of looking in turn;
    /// `None` if the server showed neither name nor exits, and the move
    /// found none.
    pub(super) fn find(&self) -> Option<Found> {
        let moved = self.moved();
        let mut followed = None;
        let mut beside = false;
        let found = if !moved.is_empty() {
            moved
        } else if self.key.is_none() && self.exits.is_none() {
            return None;
        } else if self.seen.came.is_some() {
            // A move the server says was made, which the map's exits do not
            // lead to such a room by: one next to where the player was, as
            // the map has the exit wrong; else the one the way and the
            // exits lead to, by whatever name; else one further.
            let next_to = self.around_within(1);
            match self.shaped() {
                Some(room) if next_to.is_empty() => {
                    followed = Some(room);
                    vec![room]
                }
                _ if !next_to.is_empty() => {
                    beside = true;
                    next_to
                }
                _ => self.around_within(2),
            }
        } else {
            // No move, or only one sent, which may not have been made:
            // still there, as after looking; else one around.
            let staying: Vec<RoomId> = (self.before.iter().copied())
                .filter(|id| self.like(*id))
                .collect();
            if staying.is_empty() {
                self.around_within(2)
            } else {
                staying
            }
        };
        Some(Found {
            rooms: found,
            followed,
            beside,
        })
    }
}
