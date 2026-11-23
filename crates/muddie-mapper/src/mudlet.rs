//! Maps made with Mudlet, read from its binary `.dat` files: their rooms,
//! where they are, their exits, what they cost and their colours, made
//! rooms of this map. The file is what Qt's `QDataStream` writes
//! (big-endian, at Qt 5.12, reals as doubles), in Mudlet's format versions
//! 14 to 18. Labels are read past, as nothing here shows them.

use crate::{Colour, Door, Exit, Pos, Room, RoomId, STANDARD_COLOURS};
use std::collections::{BTreeMap, HashMap};

/// Which reader of Mudlet's maps this is: raise it when the reader reads
/// more of Mudlet's maps, so that maps read before are taken again. A map
/// with none noted is taken again too.
pub const MUDLET_READER: u32 = 1;

/// The versions of Mudlet's format read here. 19 and later begin with the
/// font of the map's symbols, in Qt's own format for fonts.
const VERSIONS: std::ops::RangeInclusive<i32> = 14..=18;

/// Mudlet's twelve exits, in the order a room is stored with them, by the
/// directions they are here.
const STORED: [&str; 12] = [
    "n", "ne", "e", "se", "s", "sw", "w", "nw", "u", "d", "in", "out",
];

/// The directions Mudlet numbers its exits with, from 1, where it says
/// which are locked, as Mudlet calls the exits routes do not use.
const NUMBERED: [&str; 12] = [
    "n", "ne", "nw", "e", "w", "s", "se", "sw", "u", "d", "in", "out",
];

/// A map read from Mudlet: its rooms, and the one the player was last in.
#[derive(Debug)]
pub struct MudletMap {
    pub version: i32,
    pub rooms: Vec<Room>,
    pub current: Option<RoomId>,
}

/// Reads a map Mudlet saved. Says what is wrong with one that cannot be
/// read: of another version, cut short or damaged.
pub fn read_mudlet_map(bytes: &[u8]) -> Result<MudletMap, String> {
    let mut stream = Stream { bytes, at: 0 };
    let version = stream.i32()?;
    if !(1..=127).contains(&version) {
        return Err("this is not a map of Mudlet's".to_owned());
    }
    if !VERSIONS.contains(&version) {
        return Err(format!(
            "this map is in Mudlet's format {version}, and only {} to {} are read",
            VERSIONS.start(),
            VERSIONS.end()
        ));
    }
    let standard: HashMap<i32, i32> = stream.map(Stream::i32, Stream::i32)?.into_iter().collect();
    let areas: HashMap<i32, String> = (stream.map(Stream::i32, Stream::string)?.into_iter())
        .map(|(id, name)| (id, name.unwrap_or_default()))
        .collect();
    let custom: HashMap<i32, Option<Colour>> = stream
        .map(Stream::i32, Stream::color)?
        .into_iter()
        .collect();
    let colours = Environments { standard, custom };
    stream.map(Stream::string, Stream::i32)?;
    if version >= 17 {
        stream.map(Stream::string, Stream::string)?;
    }
    for _ in 0..stream.count()? {
        skip_area(&mut stream, version)?;
    }
    let current = match version {
        18.. => (stream.map(Stream::string, Stream::i32)?.into_iter())
            .map(|(_, room)| RoomId::from(room))
            .next(),
        _ => Some(RoomId::from(stream.i32()?)),
    };
    skip_labels(&mut stream, version)?;
    let mut rooms = Vec::new();
    while !stream.at_end() {
        rooms.push(read_room(&mut stream, version, &areas, &colours)?);
    }
    let current = current.filter(|id| rooms.iter().any(|room| room.id == *id));
    Ok(MudletMap {
        version,
        rooms,
        current,
    })
}

/// An area's record: what it holds and where its rooms reach, all of which
/// is worked out again from the rooms.
fn skip_area(stream: &mut Stream<'_>, version: i32) -> Result<(), String> {
    stream.i32()?;
    stream.list(Stream::i32)?;
    stream.list(Stream::i32)?;
    stream.map(Stream::i32, |stream| Ok((stream.i32()?, stream.i32()?)))?;
    stream.bool()?;
    for _ in 0..6 {
        stream.i32()?;
    }
    stream.vector()?;
    let extremes = if version >= 17 { 4 } else { 6 };
    for _ in 0..extremes {
        stream.map(Stream::i32, Stream::i32)?;
    }
    stream.vector()?;
    stream.bool()?;
    stream.i32()?;
    if version >= 17 {
        stream.map(Stream::string, Stream::string)?;
    }
    Ok(())
}

/// The labels of the areas, written over the map: read past.
fn skip_labels(stream: &mut Stream<'_>, version: i32) -> Result<(), String> {
    for _ in 0..stream.count()? {
        let labels = stream.count()?;
        stream.i32()?;
        for _ in 0..labels {
            stream.i32()?;
            stream.vector()?;
            stream.point()?;
            stream.point()?;
            stream.string()?;
            stream.color()?;
            stream.color()?;
            stream.pixmap()?;
            if version >= 15 {
                stream.bool()?;
                stream.bool()?;
            }
        }
    }
    Ok(())
}

/// The colours of a map's environments, the kinds of place a room may be:
/// one of the standard colours by its number, or one of the map's own.
struct Environments {
    standard: HashMap<i32, i32>,
    custom: HashMap<i32, Option<Colour>>,
}

impl Environments {
    /// The colour of rooms of an environment: the map's own for it if it
    /// has one, else the standard colour it is given, else none.
    fn colour(&self, environment: i32) -> Option<Colour> {
        if let Some(colour) = self.custom.get(&environment) {
            return *colour;
        }
        // Mudlet numbers the standard colours from 1: red to white, then
        // black, then the light ones, and light black last.
        let ansi = match *self.standard.get(&environment)? {
            8 => 0,
            16 => 8,
            number @ 1..=15 => number,
            _ => return None,
        };
        Some(STANDARD_COLOURS[ansi as usize])
    }
}

/// One room, made a room of this map: its place, its colour, its exits,
/// the special ones by their command, which are locked and what they cost.
fn read_room(
    stream: &mut Stream<'_>,
    version: i32,
    areas: &HashMap<i32, String>,
    colours: &Environments,
) -> Result<Room, String> {
    let id = RoomId::from(stream.i32()?);
    let area = stream.i32()?;
    let pos = Pos {
        x: stream.i32()?,
        y: stream.i32()?,
        z: stream.i32()?,
    };
    let mut leading = [0; 12];
    for to in &mut leading {
        *to = stream.i32()?;
    }
    let environment = stream.i32()?;
    let weight = stream.i32()?.max(1);
    let name = stream.string()?.unwrap_or_default();
    let avoid = stream.bool()?;
    let special = stream.map(Stream::i32, Stream::string)?;
    // The room's symbol: a character of old, kept also, as Mudlet does
    // now, with what it knows of the room.
    let character = stream.i8()?;
    let known = stream.map(Stream::string, Stream::string)?;
    let symbol = (known.into_iter())
        .find(|(key, _)| key.as_deref() == Some("system.fallback_symbol"))
        .and_then(|(_, symbol)| symbol)
        .or_else(|| (character > b' ' as i8).then(|| char::from(character as u8).to_string()))
        .map(|symbol| symbol.trim().to_owned())
        .unwrap_or_default();
    let points = |stream: &mut Stream<'_>| stream.list(Stream::point);
    stream.map(Stream::string, points)?;
    stream.map(Stream::string, Stream::bool)?;
    stream.map(Stream::string, |stream| stream.list(Stream::i32))?;
    stream.map(Stream::string, Stream::string)?;
    let locks = stream.list(Stream::i32)?;
    stream.list(Stream::i32)?;
    let costs: HashMap<String, i32> = match version {
        16.. => stream
            .map(Stream::string, Stream::i32)?
            .into_iter()
            .map(|(key, cost)| (key.unwrap_or_default(), cost))
            .collect(),
        _ => HashMap::new(),
    };
    let doors: HashMap<String, i32> = match version {
        16.. => stream
            .map(Stream::string, Stream::i32)?
            .into_iter()
            .map(|(key, door)| (key.unwrap_or_default(), door))
            .collect(),
        _ => HashMap::new(),
    };
    let door = |key: &str| match doors.get(key) {
        Some(1) => Some(Door::Open),
        Some(2) => Some(Door::Closed),
        Some(3) => Some(Door::Locked),
        _ => None,
    };
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
    Ok(Room {
        id,
        name,
        area: areas.get(&area).cloned().unwrap_or_default(),
        pos,
        exits,
        weight: weight as u32,
        avoid,
        colour: colours.colour(environment),
        note: String::new(),
        renamed: false,
        added: false,
        shape: None,
        symbol,
        styled: false,
    })
}

/// Reads what `QDataStream` wrote, in order.
struct Stream<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Stream<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], String> {
        let end = (self.at.checked_add(count)).filter(|end| *end <= self.bytes.len());
        let Some(end) = end else {
            return Err(format!("the map is cut short, at byte {}", self.at));
        };
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn at_end(&self) -> bool {
        self.at >= self.bytes.len()
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().unwrap_or([0; N]))
    }

    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_be_bytes(self.array()?))
    }

    fn i8(&mut self) -> Result<i8, String> {
        Ok(i8::from_be_bytes(self.array()?))
    }

    fn bool(&mut self) -> Result<bool, String> {
        Ok(self.array::<1>()?[0] != 0)
    }

    fn f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_be_bytes(self.array()?))
    }

    /// How many of something follow, never more than the rest of the file
    /// could hold, so that a damaged one asks for nothing it cannot have.
    fn count(&mut self) -> Result<usize, String> {
        let count = u32::from_be_bytes(self.array()?) as usize;
        if count > self.bytes.len() - self.at {
            return Err(format!("the map is damaged, at byte {}", self.at));
        }
        Ok(count)
    }

    /// A string, as UTF-16; None for a null one.
    fn string(&mut self) -> Result<Option<String>, String> {
        let length = u32::from_be_bytes(self.array()?);
        if length == u32::MAX {
            return Ok(None);
        }
        let bytes = self.take(length as usize)?;
        let (pairs, _) = bytes.as_chunks::<2>();
        let units: Vec<u16> = pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
        Ok(Some(String::from_utf16_lossy(&units)))
    }

    fn list<T>(&mut self, item: impl Fn(&mut Self) -> Result<T, String>) -> Result<Vec<T>, String> {
        (0..self.count()?).map(|_| item(self)).collect()
    }

    fn map<K, V>(
        &mut self,
        key: impl Fn(&mut Self) -> Result<K, String>,
        value: impl Fn(&mut Self) -> Result<V, String>,
    ) -> Result<Vec<(K, V)>, String> {
        (0..self.count()?)
            .map(|_| Ok((key(self)?, value(self)?)))
            .collect()
    }

    /// A colour: its kind, then alpha, red, green and blue, each in 16
    /// bits, and 16 more unused. `None` for an invalid colour or one that
    /// is not kept as red, green and blue, which Mudlet does not write.
    fn color(&mut self) -> Result<Option<Colour>, String> {
        let bytes = self.array::<11>()?;
        const RGB: u8 = 1;
        Ok((bytes[0] == RGB).then(|| [bytes[3], bytes[5], bytes[7]]))
    }

    fn point(&mut self) -> Result<(f64, f64), String> {
        Ok((self.f64()?, self.f64()?))
    }

    /// Three reals, as a `QVector3D` keeps them.
    fn vector(&mut self) -> Result<(), String> {
        for _ in 0..3 {
            self.f64()?;
        }
        Ok(())
    }

    /// A picture: whether there is one, then a PNG of no stated length,
    /// read chunk by chunk to its end.
    fn pixmap(&mut self) -> Result<(), String> {
        if self.i32()? == 0 {
            return Ok(());
        }
        if self.take(8)? != b"\x89PNG\r\n\x1a\n" {
            return Err(format!("a label's picture is damaged, at byte {}", self.at));
        }
        loop {
            let length = u32::from_be_bytes(self.array()?) as usize;
            let kind = self.array::<4>()?;
            self.take(length.saturating_add(4))?;
            if &kind == b"IEND" {
                return Ok(());
            }
        }
    }
}

/// A direction as Mudlet names it where it keys exits by name: up and down
/// in full.
fn mudlet_name(direction: &str) -> &str {
    match direction {
        "u" => "up",
        "d" => "down",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes what `QDataStream` would, for maps to read.
    #[derive(Default)]
    struct Writer(Vec<u8>);

    impl Writer {
        fn i32(&mut self, value: i32) -> &mut Self {
            self.0.extend(value.to_be_bytes());
            self
        }
        fn count(&mut self, count: usize) -> &mut Self {
            self.i32(count as i32)
        }
        fn bool(&mut self, value: bool) -> &mut Self {
            self.0.push(u8::from(value));
            self
        }
        fn f64(&mut self, value: f64) -> &mut Self {
            self.0.extend(value.to_be_bytes());
            self
        }
        fn string(&mut self, text: &str) -> &mut Self {
            let units: Vec<u16> = text.encode_utf16().collect();
            self.count(units.len() * 2);
            for unit in units {
                self.0.extend(unit.to_be_bytes());
            }
            self
        }
        fn color(&mut self, [red, green, blue]: Colour) -> &mut Self {
            let kind = 1;
            (self.0).extend([kind, 255, 255, red, red, green, green, blue, blue, 0, 0]);
            self
        }
        fn vector(&mut self) -> &mut Self {
            self.f64(1.0).f64(2.0).f64(3.0)
        }
    }

    /// A map of version 16: an area, a label with a picture, and three
    /// rooms, the first with a locked exit north, a costly one east and a
    /// special exit, locked, into the second. The first is of an
    /// environment with a colour of its own, the second of one with a
    /// standard colour, light red, and the third of one with none.
    fn little_map() -> Vec<u8> {
        let mut map = Writer::default();
        map.i32(16).count(1).i32(7).i32(9);
        map.count(1).i32(5).string("Ciudad");
        map.count(1).i32(1).color([10, 120, 30]);
        map.count(0);
        // The area: its rooms, levels, exits, bounds, and where it is.
        map.count(1)
            .i32(5)
            .count(2)
            .i32(1)
            .i32(2)
            .count(1)
            .i32(0)
            .count(0)
            .bool(false);
        for bound in [2, 0, 0, 0, 0, 0] {
            map.i32(bound);
        }
        map.vector();
        for _ in 0..6 {
            map.count(0);
        }
        map.vector().bool(false).i32(0);
        // The room the player was in.
        map.i32(2);
        // One area with one label, with a tiny picture.
        map.count(1)
            .count(1)
            .i32(5)
            .i32(1)
            .vector()
            .f64(0.0)
            .f64(0.0)
            .f64(4.0)
            .f64(1.0);
        map.string("Plaza").color([0, 0, 0]).color([9, 9, 9]).i32(1);
        map.0.extend(b"\x89PNG\r\n\x1a\n");
        map.i32(2).0.extend(b"IHDRab");
        map.i32(0).i32(0).0.extend(b"IEND");
        map.i32(0);
        map.bool(false).bool(true);
        // The rooms.
        for (id, x, north, east, environment) in
            [(1, 0, 2, 2, 1), (2, 2, -1, -1, 7), (3, 4, -1, -1, 99)]
        {
            map.i32(id).i32(5).i32(x).i32(0).i32(0);
            map.i32(north).i32(-1).i32(east);
            for _ in 3..12 {
                map.i32(-1);
            }
            map.i32(environment).i32(if id == 1 { 3 } else { 0 });
            map.string(&format!("Sala {id}")).bool(id == 2);
            if id == 1 {
                map.count(1).i32(2).string("1entrar portal");
            } else {
                map.count(0);
            }
            // Its symbol: a character of old, or kept with what is known
            // of the room, as Mudlet keeps it now.
            map.0.push(if id == 1 { b'B' } else { 0 });
            if id == 2 {
                map.count(1).string("system.fallback_symbol").string("⌂");
            } else {
                map.count(0);
            }
            map.count(0).count(0).count(0).count(0);
            // Locked by number: 1 is north.
            if id == 1 {
                map.count(1).i32(1);
            } else {
                map.count(0);
            }
            map.count(0);
            if id == 1 {
                map.count(1).string("e").i32(4);
            } else {
                map.count(0);
            }
            // A door, shut, to the east.
            if id == 1 {
                map.count(1).string("e").i32(2);
            } else {
                map.count(0);
            }
        }
        map.0
    }

    #[test]
    fn a_map_of_mudlet_is_read_as_rooms_of_this_map() {
        let read = read_mudlet_map(&little_map()).unwrap();
        assert_eq!(read.version, 16);
        assert_eq!(read.current, Some(2));
        let first = &read.rooms[0];
        assert_eq!(
            (first.id, first.name.as_str(), first.area.as_str()),
            (1, "Sala 1", "Ciudad")
        );
        assert_eq!(first.weight, 3);
        let north = &first.exits["n"];
        assert!(north.blocked && north.to == 2);
        assert_eq!(first.exits["e"].cost, 4);
        assert_eq!(first.exits["e"].door, Some(Door::Closed));
        assert_eq!(north.door, None);
        let symbols: Vec<&str> = read.rooms.iter().map(|room| room.symbol.as_str()).collect();
        assert_eq!(symbols, ["B", "⌂", ""]);
        let portal = &first.exits["entrar portal"];
        assert!(portal.blocked && portal.to == 2);
        assert!(read.rooms[1].avoid);
        assert_eq!(read.rooms[1].pos, Pos { x: 2, y: 0, z: 0 });
        let colours: Vec<Option<Colour>> = read.rooms.iter().map(|room| room.colour).collect();
        assert_eq!(colours, [Some([10, 120, 30]), Some([255, 0, 0]), None]);
    }

    #[test]
    fn a_map_that_cannot_be_read_says_why() {
        let map = little_map();
        let short = read_mudlet_map(&map[..map.len() - 3]).unwrap_err();
        assert!(short.contains("cut short"), "{short}");
        let mut newer = map.clone();
        newer[3] = 21;
        let newer = read_mudlet_map(&newer).unwrap_err();
        assert!(newer.contains("format 21"), "{newer}");
        // A count no file could hold asks for nothing.
        let mut damaged = map;
        damaged[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        let damaged = read_mudlet_map(&damaged).unwrap_err();
        assert!(damaged.contains("damaged"), "{damaged}");
    }
}
