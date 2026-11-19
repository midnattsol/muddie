//! Reads room descriptions out of GMCP `Room.Info` and MSDP `ROOM` data.

use std::collections::BTreeMap;

use serde_json::{Map as Fields, Value};

use crate::{Pos, RoomId, canonical_direction, room_id};

/// What the server says about the room the player is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomInfo {
    pub id: RoomId,
    pub name: String,
    pub area: String,
    pub exits: BTreeMap<String, RoomId>,
    /// Map coordinates, for servers that provide them.
    pub pos: Option<Pos>,
}

impl RoomInfo {
    /// Parses a GMCP `Room.Info` or MSDP `ROOM` payload (the latter as
    /// JSON). Field names differ between servers, so the common spellings
    /// are all accepted, in any letter case. Returns `None` if there is no
    /// room number, without which a room cannot be mapped.
    pub fn from_json(json: &str) -> Option<Self> {
        let Value::Object(fields) = serde_json::from_str(json).ok()? else {
            return None;
        };
        let fields = lower_keys(fields);
        let first = |names: &[&str]| names.iter().find_map(|name| fields.get(*name));
        let text = |names: &[&str]| match first(names) {
            Some(Value::String(text)) => text.clone(),
            Some(Value::Number(number)) => number.to_string(),
            _ => String::new(),
        };
        let exits = match first(&["exits"]) {
            Some(Value::Object(exits)) => (exits.iter())
                .filter_map(|(direction, to)| {
                    Some((canonical_direction(direction), identifier(to)?))
                })
                .collect(),
            _ => BTreeMap::new(),
        };
        Some(Self {
            id: identifier(first(&["num", "vnum", "id", "identifier"])?)?,
            name: text(&["name"]),
            area: text(&["area", "zone"]),
            exits,
            // Not Aardwolf's `coord`: that locates the whole area on a
            // continent, so every room in it would land on the same spot.
            pos: first(&["coords"]).and_then(position),
        })
    }
}

fn lower_keys(fields: Fields<String, Value>) -> Fields<String, Value> {
    (fields.into_iter())
        .map(|(name, value)| (name.to_lowercase(), value))
        .collect()
}

/// A room identifier: a number, or text (numeric or not, see [`room_id`]).
fn identifier(value: &Value) -> Option<RoomId> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) if !text.trim().is_empty() => Some(room_id(text)),
        _ => None,
    }
}

/// Servers send numbers either as JSON numbers or as strings.
fn integer(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// Coordinates as `{x, y, z}` or as the string `"area,x,y,z"` (or
/// `"x,y,z"`) that IRE-style servers send.
fn position(value: &Value) -> Option<Pos> {
    let coordinate = |value: &Value| i32::try_from(integer(value)?).ok();
    match value {
        Value::Object(fields) => {
            let fields = lower_keys(fields.clone());
            Some(Pos {
                x: coordinate(fields.get("x")?)?,
                y: coordinate(fields.get("y")?)?,
                z: fields.get("z").and_then(coordinate).unwrap_or(0),
            })
        }
        Value::String(text) => {
            let parts: Vec<i32> = (text.split(','))
                .map(|part| part.trim().parse())
                .collect::<Result<_, _>>()
                .ok()?;
            match parts[..] {
                [x, y, z] | [_, x, y, z, ..] => Some(Pos { x, y, z }),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exits(items: &[(&str, RoomId)]) -> BTreeMap<String, RoomId> {
        (items.iter())
            .map(|(direction, to)| ((*direction).to_owned(), *to))
            .collect()
    }

    #[test]
    fn ire_style_gmcp() {
        let info = RoomInfo::from_json(
            r#"{"num": 12, "name": "On a hill", "area": "Barony", "environment": "Hills",
                "coords": "45,5,4,3", "exits": {"n": 13, "southeast": 15, "in": 20}}"#,
        )
        .unwrap();
        assert_eq!(info.id, 12);
        assert_eq!(
            (info.name.as_str(), info.area.as_str()),
            ("On a hill", "Barony")
        );
        assert_eq!(info.exits, exits(&[("n", 13), ("se", 15), ("in", 20)]));
        assert_eq!(info.pos, Some(Pos { x: 5, y: 4, z: 3 }));
    }

    #[test]
    fn aardwolf_style_gmcp_without_room_coordinates() {
        let info = RoomInfo::from_json(
            r#"{"num": 32418, "name": "The Grand City", "zone": "aylor",
                "exits": {"e": 32419, "w": -1}, "coord": {"id": 0, "x": 30, "y": 20, "cont": 0}}"#,
        )
        .unwrap();
        assert_eq!(info.area, "aylor");
        assert_eq!(info.exits, exits(&[("e", 32419), ("w", -1)]));
        assert_eq!(info.pos, None);
    }

    #[test]
    fn msdp_room_with_string_numbers() {
        let info = RoomInfo::from_json(
            r#"{"VNUM": "7", "NAME": "Plaza", "AREA": "Centro",
                "COORDS": {"X": "1", "Y": "-2", "Z": "0"}, "EXITS": {"North": "8", "up": "9"}}"#,
        )
        .unwrap();
        assert_eq!(info.id, 7);
        assert_eq!(info.exits, exits(&[("n", 8), ("u", 9)]));
        assert_eq!(info.pos, Some(Pos { x: 1, y: -2, z: 0 }));
    }

    #[test]
    fn text_identifiers_are_accepted() {
        let info = RoomInfo::from_json(
            r#"{"identifier": "a7f3c9", "name": "Dock", "exits": {"north": "b01d", "south": ""}}"#,
        )
        .unwrap();
        assert_eq!(info.id, room_id("a7f3c9"));
        assert_eq!(info.exits, exits(&[("n", room_id("b01d"))]));
    }

    #[test]
    fn room_without_a_number_is_rejected() {
        assert_eq!(RoomInfo::from_json(r#"{"name": "Limbo"}"#), None);
        assert_eq!(RoomInfo::from_json("[1]"), None);
        assert_eq!(RoomInfo::from_json("not json"), None);
    }
}
