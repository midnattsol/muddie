//! Conversion between the server's bytes and text.

use encoding_rs::{CoderResult, Encoding};

/// Decodes the server's bytes and encodes what is sent back.
///
/// With no encoding chosen, bytes are read as UTF-8, and any byte that is
/// not valid UTF-8 as Latin-1. Many MUDs still send Latin-1 without
/// announcing it, and accented Latin-1 text is almost never valid UTF-8,
/// so this shows both correctly without configuration. Text is sent as
/// UTF-8.
pub struct Decoder {
    /// Start of a multi-byte sequence cut off by the end of the last read.
    pending: Vec<u8>,
    /// The encoding chosen by the user, with its decoding state.
    named: Option<(&'static Encoding, encoding_rs::Decoder)>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            named: None,
        }
    }

    /// A decoder for the encoding with this label, such as `big5`,
    /// `koi8-r` or `windows-1252`. An empty label or `auto` gives the
    /// default behaviour; an unknown label gives `None`.
    pub fn for_label(label: &str) -> Option<Self> {
        let label = label.trim();
        if label.is_empty() || label.eq_ignore_ascii_case("auto") {
            return Some(Self::new());
        }
        let encoding = Encoding::for_label(label.as_bytes())?;
        Some(Self {
            pending: Vec::new(),
            named: Some((encoding, encoding.new_decoder_without_bom_handling())),
        })
    }

    /// The chosen encoding's name, or `None` for the default behaviour.
    pub fn encoding_name(&self) -> Option<&'static str> {
        self.named.as_ref().map(|(encoding, _)| encoding.name())
    }

    pub fn feed(&mut self, input: &[u8], out: &mut String) {
        if let Some((_, decoder)) = &mut self.named {
            let mut rest = input;
            loop {
                out.reserve(
                    decoder
                        .max_utf8_buffer_length(rest.len())
                        .unwrap_or(rest.len() * 3 + 16),
                );
                let (result, read, _) = decoder.decode_to_string(rest, out, false);
                rest = &rest[read..];
                if result == CoderResult::InputEmpty {
                    return;
                }
            }
        }
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
    /// A chosen encoding keeps its own state and needs no flushing.
    pub fn flush(&mut self, out: &mut String) {
        push_latin1(&self.pending, out);
        self.pending.clear();
    }

    /// Encodes text to send. Characters the chosen encoding lacks are sent
    /// as `?` rather than as the HTML references `encoding_rs` would use.
    pub fn encode(&self, text: &str) -> Vec<u8> {
        let Some((encoding, _)) = &self.named else {
            return text.as_bytes().to_vec();
        };
        let mut bytes = Vec::with_capacity(text.len());
        for ch in text.chars() {
            let mut buffer = [0; 4];
            let (encoded, _, unmappable) = encoding.encode(ch.encode_utf8(&mut buffer));
            if unmappable {
                bytes.push(b'?');
            } else {
                bytes.extend_from_slice(&encoded);
            }
        }
        bytes
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

    #[test]
    fn chosen_encoding_decodes_and_encodes() {
        let mut decoder = Decoder::for_label("Big5").unwrap();
        assert_eq!(decoder.encoding_name(), Some("Big5"));
        let bytes = decoder.encode("你好 ok");
        assert_eq!(bytes, b"\xa7\x41\xa6\x6e ok");
        // A character split between two reads is put back together.
        let mut out = String::new();
        decoder.feed(&bytes[..1], &mut out);
        decoder.feed(&bytes[1..], &mut out);
        assert_eq!(out, "你好 ok");

        let cyrillic = Decoder::for_label("koi8-r").unwrap();
        assert_eq!(cyrillic.encode("да ñ"), b"\xc4\xc1 ?");
    }

    #[test]
    fn labels() {
        assert!(
            Decoder::for_label("auto")
                .unwrap()
                .encoding_name()
                .is_none()
        );
        assert!(Decoder::for_label("").unwrap().encoding_name().is_none());
        assert_eq!(
            Decoder::for_label(" latin1 ").unwrap().encoding_name(),
            Some("windows-1252")
        );
        assert!(Decoder::for_label("klingon").is_none());
        assert_eq!(Decoder::new().encode("ñ"), "ñ".as_bytes());
    }
}
