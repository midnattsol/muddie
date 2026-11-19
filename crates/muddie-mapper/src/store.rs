//! SQLite storage for a map.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{Connection, Result, params};

use crate::{Pos, Room};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS rooms (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        area TEXT NOT NULL,
        x INTEGER NOT NULL,
        y INTEGER NOT NULL,
        z INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS exits (
        room_id INTEGER NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
        direction TEXT NOT NULL,
        to_id INTEGER NOT NULL,
        PRIMARY KEY (room_id, direction)
    );
";

pub struct Store {
    connection: Connection,
}

impl Store {
    /// Opens (or creates) the database and returns it with all its rooms.
    pub fn open(path: &Path) -> Result<(Self, Vec<Room>)> {
        let connection = Connection::open(path)?;
        connection.execute_batch(SCHEMA)?;

        let mut exits: BTreeMap<i64, BTreeMap<String, i64>> = BTreeMap::new();
        let mut statement = connection.prepare("SELECT room_id, direction, to_id FROM exits")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        for row in rows {
            let (room_id, direction, to_id): (i64, String, i64) = row?;
            exits.entry(room_id).or_default().insert(direction, to_id);
        }
        drop(statement);

        let mut statement = connection.prepare("SELECT id, name, area, x, y, z FROM rooms")?;
        let rooms = statement
            .query_map([], |row| {
                let id = row.get(0)?;
                Ok(Room {
                    id,
                    name: row.get(1)?,
                    area: row.get(2)?,
                    pos: Pos {
                        x: row.get(3)?,
                        y: row.get(4)?,
                        z: row.get(5)?,
                    },
                    exits: exits.remove(&id).unwrap_or_default(),
                })
            })?
            .collect::<Result<Vec<Room>>>()?;
        drop(statement);
        Ok((Self { connection }, rooms))
    }

    /// Inserts or replaces a room and its exits.
    pub fn save(&mut self, room: &Room) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO rooms (id, name, area, x, y, z) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name, area = excluded.area,
                 x = excluded.x, y = excluded.y, z = excluded.z",
            params![
                room.id, room.name, room.area, room.pos.x, room.pos.y, room.pos.z
            ],
        )?;
        transaction.execute("DELETE FROM exits WHERE room_id = ?1", [room.id])?;
        for (direction, to) in &room.exits {
            transaction.execute(
                "INSERT INTO exits (room_id, direction, to_id) VALUES (?1, ?2, ?3)",
                params![room.id, direction, to],
            )?;
        }
        transaction.commit()
    }
}
