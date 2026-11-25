//! The exits of a room in a Mudlet map, and what is kept by name.

use super::*;

/// Numbers kept by name (what exits cost, how their doors are): none
/// before the version that has them.
pub(super) fn read_named_numbers(
    stream: &mut Stream<'_>,
    version: i32,
) -> Result<HashMap<String, i32>, String> {
    Ok(match version {
        16.. => stream
            .map(Stream::string, Stream::i32)?
            .into_iter()
            .map(|(key, number)| (key.unwrap_or_default(), number))
            .collect(),
        _ => HashMap::new(),
    })
}

/// The door an exit has, by the number Mudlet keeps for it.
fn door_of(doors: &HashMap<String, i32>, key: &str) -> Option<Door> {
    match doors.get(key) {
        Some(1) => Some(Door::Open),
        Some(2) => Some(Door::Closed),
        Some(3) => Some(Door::Locked),
        _ => None,
    }
}

/// The exits of the compass: those that lead anywhere, with what they cost,
/// whether they are locked and their doors.
pub(super) fn compass_exits(
    leading: [i32; 12],
    locks: &[i32],
    costs: &HashMap<String, i32>,
    doors: &HashMap<String, i32>,
) -> BTreeMap<String, Exit> {
    let door = |key: &str| door_of(doors, key);
    let locked_number = |direction: &str| {
        let number = NUMBERED.iter().position(|known| *known == direction);
        number.is_some_and(|at| locks.contains(&(at as i32 + 1)))
    };
    let mut exits = BTreeMap::new();
    for (direction, to) in STORED.iter().zip(leading) {
        if to <= 0 {
            continue;
        }
        let key = mudlet_name(direction);
        exits.insert(
            (*direction).to_owned(),
            Exit {
                to: RoomId::from(to),
                cost: costs.get(key).map_or(0, |cost| (*cost).max(0) as u32),
                blocked: locked_number(direction) || door(key) == Some(Door::Locked),
                door: door(key),
                ..Exit::default()
            },
        );
    }
    exits
}

/// Adds the special exits, by their command, to `exits`.
pub(super) fn add_special_exits(
    exits: &mut BTreeMap<String, Exit>,
    special: Vec<(i32, Option<String>)>,
    costs: &HashMap<String, i32>,
    doors: &HashMap<String, i32>,
) {
    let door = |key: &str| door_of(doors, key);
    // A special exit's command was kept with 1 before it if locked, 0 if not.
    for (to, command) in special {
        let Some(command) = command else { continue };
        let (locked, command) = match command.split_at_checked(1) {
            Some(("1", rest)) => (true, rest),
            Some(("0", rest)) => (false, rest),
            _ => (false, command.as_str()),
        };
        let command = command.trim();
        if to <= 0 || command.is_empty() {
            continue;
        }
        exits.insert(
            crate::canonical_direction(command),
            Exit {
                to: RoomId::from(to),
                cost: costs.get(command).map_or(0, |cost| (*cost).max(0) as u32),
                blocked: locked || door(command) == Some(Door::Locked),
                door: door(command),
                ..Exit::default()
            },
        );
    }
}
