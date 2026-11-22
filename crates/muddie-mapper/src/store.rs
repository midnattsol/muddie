//! SQLite storage for a map.

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{Connection, Result, Transaction, params};

use crate::{Colour, Exit, Pos, Room};

/// Version of the schema below, kept in the database's `user_version`.
/// Version 1 lacked the rooms' `weight` and `avoid` columns, versions 1
/// and 2 the exits' `cost`, `locked`, `command` and `custom`, and versions
/// up to 3 the marks and what is known about the map, versions up to 4
/// the rooms' `colour`, and versions up to 5 their `note`.
const VERSION: i64 = 6;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS rooms (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        area TEXT NOT NULL,
        x INTEGER NOT NULL,
        y INTEGER NOT NULL,
        z INTEGER NOT NULL,
        weight INTEGER NOT NULL DEFAULT 1,
        avoid INTEGER NOT NULL DEFAULT 0,
        colour INTEGER,
        note TEXT NOT NULL DEFAULT ''
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
    CREATE TABLE IF NOT EXISTS marks (
        name TEXT PRIMARY KEY,
        room_id INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS about (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
";

/// The columns added since the first format, each with how it is made: a
/// map of an older format is given those it lacks, whatever its version
/// says.
const ADDED: [(&str, &str, &str); 8] = [
    ("rooms", "weight", "INTEGER NOT NULL DEFAULT 1"),
    ("rooms", "avoid", "INTEGER NOT NULL DEFAULT 0"),
    ("exits", "cost", "INTEGER NOT NULL DEFAULT 0"),
    ("exits", "locked", "INTEGER NOT NULL DEFAULT 0"),
    ("exits", "command", "TEXT"),
    ("exits", "custom", "INTEGER NOT NULL DEFAULT 0"),
    ("rooms", "colour", "INTEGER"),
    ("rooms", "note", "TEXT NOT NULL DEFAULT ''"),
];

pub struct Store {
    connection: Connection,
}

/// What a stored map holds: its rooms, its marks by name, and what is known
/// about it by key.
#[derive(Default)]
pub struct Stored {
    pub rooms: Vec<Room>,
    pub marks: BTreeMap<String, i64>,
    pub about: BTreeMap<String, String>,
}

impl Store {
    /// Opens (or creates) the database and returns it with all it holds.
    pub fn open(path: &Path) -> Result<(Self, Stored)> {
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
        if version < VERSION {
            for (table, column, made) in ADDED {
                let has: bool = connection.query_row(
                    &format!(
                        "SELECT count(*) > 0 FROM pragma_table_info('{table}') WHERE name = ?1"
                    ),
                    [column],
                    |row| row.get(0),
                )?;
                if !has {
                    connection.execute_batch(&format!(
                        "ALTER TABLE {table} ADD COLUMN {column} {made};"
                    ))?;
                }
            }
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

        let mut statement = connection
            .prepare("SELECT id, name, area, x, y, z, weight, avoid, colour, note FROM rooms")?;
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
                    colour: row.get::<_, Option<u32>>(8)?.map(unpacked),
                    note: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<Room>>>()?;
        drop(statement);
        let pairs = |sql: &str| -> Result<Vec<(String, rusqlite::types::Value)>> {
            let mut statement = connection.prepare(sql)?;
            let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect()
        };
        let marks = (pairs("SELECT name, room_id FROM marks")?.into_iter())
            .filter_map(|(name, id)| match id {
                rusqlite::types::Value::Integer(id) => Some((name, id)),
                _ => None,
            })
            .collect();
        let about = (pairs("SELECT key, value FROM about")?.into_iter())
            .filter_map(|(key, value)| match value {
                rusqlite::types::Value::Text(value) => Some((key, value)),
                _ => None,
            })
            .collect();
        let stored = Stored {
            rooms,
            marks,
            about,
        };
        Ok((Self { connection }, stored))
    }

    /// Puts `rooms` in place of every room there was, all or none, with
    /// their marks kept: a map taken from elsewhere.
    pub fn replace<'a>(&mut self, rooms: impl IntoIterator<Item = &'a Room>) -> Result<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute_batch("DELETE FROM exits; DELETE FROM rooms;")?;
        for room in rooms {
            save_room(&transaction, room, false)?;
        }
        transaction.commit()
    }

    /// Names a room, or forgets the name if `room` is `None`.
    pub fn mark(&mut self, name: &str, room: Option<i64>) -> Result<()> {
        match room {
            Some(id) => self.connection.execute(
                "INSERT INTO marks (name, room_id) VALUES (?1, ?2)
                 ON CONFLICT(name) DO UPDATE SET room_id = excluded.room_id",
                params![name, id],
            ),
            None => (self.connection).execute("DELETE FROM marks WHERE name = ?1", [name]),
        }
        .map(drop)
    }

    /// Keeps something known about the map, or forgets it if `value` is
    /// `None`.
    pub fn set_about(&mut self, key: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(value) => self.connection.execute(
                "INSERT INTO about (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            ),
            None => (self.connection).execute("DELETE FROM about WHERE key = ?1", [key]),
        }
        .map(drop)
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
            save_room(&transaction, room, true)?;
        }
        transaction.commit()
    }
}

/// Writes a room and its exits; `known` if it may be there already, to be
/// replaced, rather than certainly new.
fn save_room(transaction: &Transaction, room: &Room, known: bool) -> Result<()> {
    transaction
        .prepare_cached(
            "INSERT INTO rooms (id, name, area, x, y, z, weight, avoid, colour, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
             name = excluded.name, area = excluded.area,
             x = excluded.x, y = excluded.y, z = excluded.z,
             weight = excluded.weight, avoid = excluded.avoid,
             colour = excluded.colour, note = excluded.note",
        )?
        .execute(params![
            room.id,
            room.name,
            room.area,
            room.pos.x,
            room.pos.y,
            room.pos.z,
            room.weight,
            room.avoid,
            room.colour.map(packed),
            room.note
        ])?;
    if known {
        (transaction.prepare_cached("DELETE FROM exits WHERE room_id = ?1")?).execute([room.id])?;
    }
    let mut insert = transaction.prepare_cached(
        "INSERT INTO exits (room_id, direction, to_id, cost, locked, command, custom)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for (direction, exit) in &room.exits {
        insert.execute(params![
            room.id,
            direction,
            exit.to,
            exit.cost,
            exit.locked,
            exit.command,
            exit.custom
        ])?;
    }
    Ok(())
}

/// A colour as the file keeps it: one number, `0xRRGGBB`.
fn packed([red, green, blue]: Colour) -> u32 {
    u32::from_be_bytes([0, red, green, blue])
}

fn unpacked(number: u32) -> Colour {
    let [_, red, green, blue] = number.to_be_bytes();
    [red, green, blue]
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
        let (mut store, Stored { mut rooms, .. }) = Store::open(&path).unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!((rooms[0].weight, rooms[0].avoid), (1, false));
        assert_eq!(rooms[0].colour, None);
        assert_eq!(
            rooms[0].exits["n"],
            Exit {
                to: 2,
                ..Exit::default()
            }
        );
        rooms[0].weight = 5;
        rooms[0].avoid = true;
        rooms[0].colour = Some([200, 16, 0]);
        rooms[0].note = "A guard".to_owned();
        store.save(&rooms).unwrap();
        drop(store);
        let (store, Stored { rooms, .. }) = Store::open(&path).unwrap();
        assert_eq!((rooms[0].weight, rooms[0].avoid), (5, true));
        assert_eq!(rooms[0].colour, Some([200, 16, 0]));
        assert_eq!(rooms[0].note, "A guard");
        // Said to be older than it is, with a column it is said to lack: it
        // opens, and keeps what it has.
        store
            .connection
            .pragma_update(None, "user_version", 4)
            .unwrap();
        drop(store);
        let (store, Stored { rooms, .. }) = Store::open(&path).unwrap();
        assert_eq!(rooms[0].colour, Some([200, 16, 0]));

        store
            .connection
            .pragma_update(None, "user_version", VERSION + 1)
            .unwrap();
        drop(store);
        assert!(Store::open(&path).is_err());
        std::fs::remove_file(&path).unwrap();
    }
}
