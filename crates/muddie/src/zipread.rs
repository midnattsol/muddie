//! Reading a zip held in memory, shared by what is taken from the web: its
//! list of files, and each file inflated within a limit of size that is
//! checked while it is inflated, not taken from what the zip says.
//! Nothing here writes anywhere or runs anything.

use std::io::Read;

/// Why a zip could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipError {
    /// Cut short, or not made as a zip is.
    Damaged,
    /// A file in it is larger than it may be.
    TooLarge,
    /// Packed in a way, or with a feature, that is not read.
    Unsupported,
}

impl ZipError {
    /// What went wrong, in a few words.
    pub fn message(self) -> &'static str {
        match self {
            ZipError::Damaged => "the zip is damaged",
            ZipError::TooLarge => "a file in the zip is too large",
            ZipError::Unsupported => "the zip is packed in a way that cannot be read",
        }
    }
}

/// A file listed in a zip.
#[derive(Debug, Clone)]
pub struct Entry {
    /// As it is written in the zip, not looked at otherwise.
    pub name: Vec<u8>,
    method: u16,
    flags: u16,
    packed: usize,
    /// What the zip says it is once inflated: not to be trusted.
    pub size: usize,
    local: usize,
    made_by: u16,
    attributes: u32,
}

/// What kind of thing an entry is, by the system that made the zip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Directory,
    /// A link, a device, or anything else that is not a plain file.
    Other,
}

impl Entry {
    /// Whether it is stored encrypted.
    pub fn encrypted(&self) -> bool {
        self.flags & 1 != 0
    }

    /// A file, a folder, or what is neither: by the mode of a zip made on
    /// Unix, by the attributes of one made elsewhere, else by its name.
    pub fn kind(&self) -> Kind {
        // Windows' mark of a link to somewhere else.
        if self.attributes & 0x400 != 0 {
            return Kind::Other;
        }
        let from_unix = self.made_by >> 8 == 3;
        let mode = self.attributes >> 16;
        match (from_unix, mode & 0o170_000) {
            (true, 0o120_000) => Kind::Other,
            (true, 0o040_000) => Kind::Directory,
            (true, 0 | 0o100_000) | (false, _) if self.name.ends_with(b"/") => Kind::Directory,
            (true, 0 | 0o100_000) | (false, _) => Kind::File,
            (true, _) => Kind::Other,
        }
    }
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// The files listed in the zip, in order. A zip that lists more than
/// `most` is refused before they are read.
pub fn entries(bytes: &[u8], most: usize) -> Result<Vec<Entry>, ZipError> {
    let damaged = ZipError::Damaged;
    // The end of the zip says where its list of files is; it is followed
    // only by a comment, of at most 64 KiB.
    let last = bytes.len().checked_sub(22).ok_or(damaged)?;
    let end = (last.saturating_sub(0xffff)..=last)
        .rev()
        .find(|at| bytes[*at..].starts_with(b"PK\x05\x06"))
        .ok_or(damaged)?;
    let count = usize::from(u16_at(bytes, end + 10).ok_or(damaged)?);
    if count > most {
        return Err(ZipError::TooLarge);
    }
    let mut at = u32_at(bytes, end + 16).ok_or(damaged)? as usize;
    let mut listed = Vec::with_capacity(count);
    for _ in 0..count {
        if !(bytes.get(at..)).is_some_and(|rest| rest.starts_with(b"PK\x01\x02")) {
            return Err(damaged);
        }
        let name_length = usize::from(u16_at(bytes, at + 28).ok_or(damaged)?);
        let extra = usize::from(u16_at(bytes, at + 30).ok_or(damaged)?)
            + usize::from(u16_at(bytes, at + 32).ok_or(damaged)?);
        listed.push(Entry {
            name: bytes
                .get(at + 46..at + 46 + name_length)
                .ok_or(damaged)?
                .to_vec(),
            method: u16_at(bytes, at + 10).ok_or(damaged)?,
            flags: u16_at(bytes, at + 8).ok_or(damaged)?,
            packed: u32_at(bytes, at + 20).ok_or(damaged)? as usize,
            size: u32_at(bytes, at + 24).ok_or(damaged)? as usize,
            local: u32_at(bytes, at + 42).ok_or(damaged)? as usize,
            made_by: u16_at(bytes, at + 4).ok_or(damaged)?,
            attributes: u32_at(bytes, at + 38).ok_or(damaged)?,
        });
        at += 46 + name_length + extra;
    }
    Ok(listed)
}

/// What the entry holds, inflated: no more than `limit` bytes, which is
/// counted as they come out, and exactly as many as the zip says.
pub fn data(bytes: &[u8], entry: &Entry, limit: u64) -> Result<Vec<u8>, ZipError> {
    let damaged = ZipError::Damaged;
    if entry.size as u64 > limit {
        return Err(ZipError::TooLarge);
    }
    if entry.encrypted() {
        return Err(ZipError::Unsupported);
    }
    // The file itself, after its own header.
    let local = entry.local;
    if !(bytes.get(local..)).is_some_and(|rest| rest.starts_with(b"PK\x03\x04")) {
        return Err(damaged);
    }
    let start = local
        + 30
        + usize::from(u16_at(bytes, local + 26).ok_or(damaged)?)
        + usize::from(u16_at(bytes, local + 28).ok_or(damaged)?);
    let packed = bytes.get(start..start + entry.packed).ok_or(damaged)?;
    let data = match entry.method {
        0 => packed.to_vec(),
        8 => {
            let mut out = Vec::with_capacity(entry.size);
            let inflater = flate2::read::DeflateDecoder::new(packed);
            (inflater.take(limit + 1).read_to_end(&mut out)).map_err(|_| damaged)?;
            if out.len() as u64 > limit {
                return Err(ZipError::TooLarge);
            }
            out
        }
        _ => return Err(ZipError::Unsupported),
    };
    if data.len() != entry.size {
        return Err(damaged);
    }
    Ok(data)
}

/// Zips made for the tests, as a program would make them, and as one that
/// means harm would.
#[cfg(test)]
pub mod make {
    use std::io::Write;

    /// A file to put in a zip, and how it is to look.
    pub struct Item<'a> {
        pub name: &'a [u8],
        pub data: &'a [u8],
        /// The external attributes: a Unix mode in the high half.
        pub attributes: u32,
        pub made_by: u16,
        /// Packed with Deflate, or stored as it is.
        pub deflate: bool,
        /// The size the zip says, if not the true one.
        pub said_size: Option<u32>,
        pub flags: u16,
    }

    impl<'a> Item<'a> {
        /// A plain file, as a Unix system makes it.
        pub fn file(name: &'a str, data: &'a [u8]) -> Self {
            Self {
                name: name.as_bytes(),
                data,
                attributes: 0o100_644 << 16,
                made_by: 3 << 8 | 20,
                deflate: true,
                said_size: None,
                flags: 0,
            }
        }

        /// A folder.
        pub fn folder(name: &'a str) -> Self {
            Self {
                attributes: 0o040_755 << 16,
                ..Self::file(name, b"")
            }
        }

        /// A link to `target`.
        pub fn link(name: &'a str, target: &'a [u8]) -> Self {
            Self {
                attributes: 0o120_777 << 16,
                ..Self::file(name, target)
            }
        }
    }

    /// A zip of `items`.
    pub fn items(items: &[Item]) -> Vec<u8> {
        let (mut bytes, mut list) = (Vec::new(), Vec::new());
        for item in items {
            let packed = if item.deflate {
                let mut packer = flate2::write::DeflateEncoder::new(Vec::new(), Default::default());
                packer.write_all(item.data).unwrap();
                packer.finish().unwrap()
            } else {
                item.data.to_vec()
            };
            let local = bytes.len() as u32;
            let size = item.said_size.unwrap_or(item.data.len() as u32);
            let sizes = |out: &mut Vec<u8>| {
                out.extend(if item.deflate { 8u16 } else { 0 }.to_le_bytes());
                out.extend([0; 8]);
                out.extend((packed.len() as u32).to_le_bytes());
                out.extend(size.to_le_bytes());
                out.extend((item.name.len() as u16).to_le_bytes());
                out.extend(0u16.to_le_bytes());
            };
            bytes.extend(b"PK\x03\x04");
            bytes.extend([20, 0]);
            bytes.extend(item.flags.to_le_bytes());
            sizes(&mut bytes);
            bytes.extend(item.name);
            bytes.extend(&packed);
            list.extend(b"PK\x01\x02");
            list.extend(item.made_by.to_le_bytes());
            list.extend([20, 0]);
            list.extend(item.flags.to_le_bytes());
            sizes(&mut list);
            list.extend([0; 6]);
            list.extend(item.attributes.to_le_bytes());
            list.extend(local.to_le_bytes());
            list.extend(item.name);
        }
        let start = bytes.len() as u32;
        bytes.extend(&list);
        bytes.extend(b"PK\x05\x06");
        bytes.extend([0; 4]);
        bytes.extend((items.len() as u16).to_le_bytes());
        bytes.extend((items.len() as u16).to_le_bytes());
        bytes.extend((list.len() as u32).to_le_bytes());
        bytes.extend(start.to_le_bytes());
        bytes.extend([0; 2]);
        bytes
    }

    /// A zip of plain files.
    pub fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let all: Vec<Item> = (files.iter())
            .map(|(name, data)| Item::file(name, data))
            .collect();
        items(&all)
    }
}
