//! Incremental bytes-to-text decoding.

/// Decodes the server's bytes as UTF-8, reading any byte that is not valid
/// UTF-8 as Latin-1. Many MUDs still send Latin-1 without announcing it, and
/// accented Latin-1 text is almost never valid UTF-8, so this shows both
/// correctly without configuration.
#[derive(Debug, Default)]
pub struct Decoder {
    /// Start of a multi-byte sequence cut off by the end of the last read.
    pending: Vec<u8>,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, input: &[u8], out: &mut String) {
        let mut buf = std::mem::take(&mut self.pending);
        buf.extend_from_slice(input);
        let mut rest = buf.as_slice();
        loop {
            let error = match std::str::from_utf8(rest) {
                Ok(text) => {
                    out.push_str(text);
                    return;
                }
                Err(error) => error,
            };
            let (valid, invalid) = rest.split_at(error.valid_up_to());
            out.push_str(std::str::from_utf8(valid).expect("validated above"));
            match error.error_len() {
                Some(len) => {
                    push_latin1(&invalid[..len], out);
                    rest = &invalid[len..];
                }
                None => {
                    self.pending = invalid.to_vec();
                    return;
                }
            }
        }
    }

    /// Emits a held-back incomplete sequence as Latin-1. Call when the server
    /// marks a boundary (a prompt), since no continuation bytes will follow.
    pub fn flush(&mut self, out: &mut String) {
        push_latin1(&self.pending, out);
        self.pending.clear();
    }
}

fn push_latin1(bytes: &[u8], out: &mut String) {
    out.extend(bytes.iter().map(|&b| char::from(b)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(chunks: &[&[u8]]) -> String {
        let mut decoder = Decoder::new();
        let mut out = String::new();
        for chunk in chunks {
            decoder.feed(chunk, &mut out);
        }
        out
    }

    #[test]
    fn utf8_split_across_reads() {
        let bytes = "ñandú €".as_bytes();
        for cut in 0..bytes.len() {
            let (a, b) = bytes.split_at(cut);
            assert_eq!(decode(&[a, b]), "ñandú €", "cut at {cut}");
        }
    }

    #[test]
    fn latin1_bytes_fall_back() {
        assert_eq!(decode(&[b"ca\xf1\xf3n y a\xf1o"]), "cañón y año");
    }

    #[test]
    fn flush_releases_trailing_latin1_byte() {
        let mut decoder = Decoder::new();
        let mut out = String::new();
        decoder.feed(b"caf\xe9", &mut out);
        assert_eq!(out, "caf");
        decoder.flush(&mut out);
        assert_eq!(out, "café");
    }
}
