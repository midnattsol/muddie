//! SQLite storage for a map.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{Connection, Result, Transaction, params};

use crate::{Exit, Pos, Room};

/// Version of the schema below, kept in the database's `user_version`.
/// Version 1 lacked the rooms' `weight` and `avoid` columns, and versions
/// 1 and 2 the exits' `cost`, `locked`, `command` and `custom`.
const VERSION: i64 = 3;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS rooms (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        area TEXT NOT NULL,
        x INTEGER NOT NULL,
        y INTEGER NOT NULL,
        z INTEGER NOT NULL,
        weight INTEGER NOT NULL DEFAULT 1,
        avoid INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE IF NOT EXISTS exits (
        room_id INTEGER NOT NULL,
        direction TEXT NOT NULL,
        to_id INTEGER NOT NULL,
        cost INTEGER NOT NULL DEFAULT 0,
        locked INTEGER NOT NULL DEFAULT 0,
        command TEXT,
        custom INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (room_id, direction)
    );
";

const UPGRADE_ROOMS_FROM_1: &str = "
    ALTER TABLE rooms ADD COLUMN weight INTEGER NOT NULL DEFAULT 1;
    ALTER TABLE rooms ADD COLUMN avoid INTEGER NOT NULL DEFAULT 0;
";

const UPGRADE_EXITS_FROM_2: &str = "
    ALTER TABLE exits ADD COLUMN cost INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE exits ADD COLUMN locked INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE exits ADD COLUMN command TEXT;
    ALTER TABLE exits ADD COLUMN custom INTEGER NOT NULL DEFAULT 0;
";

pub struct Store {
    connection: Connection,
}

impl Store {
    /// Opens (or creates) the database and returns it with all its rooms.
    pub fn open(path: &Path) -> Result<(Self, Vec<Room>)> {
        let connection = Connection::open(path)?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > VERSION {
            // Written by a newer muddie: refuse rather than risk damaging it.
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "the map file has format {version}, newer than this version understands"
            )));
        }
        // A room is saved on every move: write-ahead logging without a sync
        // per write keeps that from stalling the caller on slow disks.
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.execute_batch(SCHEMA)?;
        if version == 1 {
            connection.execute_batch(UPGRADE_ROOMS_FROM_1)?;
        }
        if version == 1 || version == 2 {
            connection.execute_batch(UPGRADE_EXITS_FROM_2)?;
        }
        connection.pragma_update(None, "user_version", VERSION)?;

        let mut exits: BTreeMap<i64, BTreeMap<String, Exit>> = BTreeMap::new();
        let mut statement = connection.prepare(
            "SELECT room_id, direction, to_id, cost, locked, command, custom FROM exits",
        )?;
        let rows = statement.query_map([], |row| {
            let exit = Exit {
                to: row.get(2)?,
                cost: row.get(3)?,
                locked: row.get(4)?,
                command: row.get(5)?,
                custom: row.get(6)?,
            };
            Ok((row.get(0)?, row.get(1)?, exit))
        })?;
        for row in rows {
            let (room_id, direction, exit): (i64, String, Exit) = row?;
            exits.entry(room_id).or_default().insert(direction, exit);
        }
        drop(statement);

        let mut statement =
            connection.prepare("SELECT id, name, area, x, y, z, weight, avoid FROM rooms")?;
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
                    weight: row.get(6)?,
                    avoid: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<Room>>>()?;
        drop(statement);
        Ok((Self { connection }, rooms))
    }

    /// Removes every room.
    pub fn clear(&mut self) -> Result<()> {
        (self.connection).execute_batch("BEGIN; DELETE FROM exits; DELETE FROM rooms; COMMIT;")
    }

    /// Removes a room and its exits.
    pub fn delete(&mut self, id: i64) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM exits WHERE room_id = ?1", [id])?;
        transaction.execute("DELETE FROM rooms WHERE id = ?1", [id])?;
        transaction.commit()
    }

    /// Inserts or replaces rooms and their exits, all or none.
    pub fn save<'a>(&mut self, rooms: impl IntoIterator<Item = &'a Room>) -> Result<()> {
        let transaction = self.connection.transaction()?;
        for room in rooms {
            save_room(&transaction, room)?;
        }
        transaction.commit()
    }
}

fn save_room(transaction: &Transaction, room: &Room) -> Result<()> {
    transaction.execute(
        "INSERT INTO rooms (id, name, area, x, y, z, weight, avoid)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
             name = excluded.name, area = excluded.area,
             x = excluded.x, y = excluded.y, z = excluded.z,
             weight = excluded.weight, avoid = excluded.avoid",
        params![
            room.id,
            room.name,
            room.area,
            room.pos.x,
            room.pos.y,
            room.pos.z,
            room.weight,
            room.avoid
        ],
    )?;
    transaction.execute("DELETE FROM exits WHERE room_id = ?1", [room.id])?;
    for (direction, exit) in &room.exits {
        transaction.execute(
            "INSERT INTO exits (room_id, direction, to_id, cost, locked, command, custom)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                room.id,
                direction,
                exit.to,
                exit.cost,
                exit.locked,
                exit.command,
                exit.custom
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_1_file_is_upgraded_and_a_newer_one_refused() {
        let path =
            std::env::temp_dir().join(format!("muddie-store-test-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let old = Connection::open(&path).unwrap();
            old.execute_batch(
                "CREATE TABLE rooms (id INTEGER PRIMARY KEY, name TEXT NOT NULL, area TEXT NOT NULL,
                     x INTEGER NOT NULL, y INTEGER NOT NULL, z INTEGER NOT NULL);
                 CREATE TABLE exits (room_id INTEGER NOT NULL, direction TEXT NOT NULL,
                     to_id INTEGER NOT NULL, PRIMARY KEY (room_id, direction));
                 INSERT INTO rooms VALUES (1, 'Gate', 'town', 0, 0, 0);
                 INSERT INTO exits VALUES (1, 'n', 2);
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        let (mut store, mut rooms) = Store::open(&path).unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!((rooms[0].weight, rooms[0].avoid), (1, false));
        assert_eq!(
            rooms[0].exits["n"],
            Exit {
                to: 2,
                ..Exit::default()
            }
        );
        rooms[0].weight = 5;
        rooms[0].avoid = true;
        store.save(&rooms).unwrap();
        drop(store);
        let (store, rooms) = Store::open(&path).unwrap();
        assert_eq!((rooms[0].weight, rooms[0].avoid), (5, true));

        store
            .connection
            .pragma_update(None, "user_version", VERSION + 1)
            .unwrap();
        drop(store);
        assert!(Store::open(&path).is_err());
        std::fs::remove_file(&path).unwrap();
    }
}
