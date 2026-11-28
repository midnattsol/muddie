//! MSDP (Mud Server Data Protocol) and MSSP (Mud Server Status Protocol)
//! payloads. MSDP values are converted to JSON so they travel through the
//! client the same way GMCP data does.

use crate::negotiate::json_string;
use crate::telnet::{self, opt};

const VAR: u8 = 1;
const VAL: u8 = 2;
const TABLE_OPEN: u8 = 3;
const TABLE_CLOSE: u8 = 4;
const ARRAY_OPEN: u8 = 5;
const ARRAY_CLOSE: u8 = 6;
/// The most tables and arrays one is read inside of: what is deeper is
/// not read, so that a server cannot make the reading go on and on.
const DEEPEST: usize = 32;

/// Parses an MSDP subnegotiation into `(variable, JSON value)` pairs.
/// Strings stay strings (MSDP has no number type), tables become objects
/// and arrays arrays; a variable given several values becomes an array.
pub fn parse(payload: &[u8]) -> Vec<(String, String)> {
    let mut reader = Reader {
        bytes: payload,
        at: 0,
    };
    let mut variables = Vec::new();
    while reader.eat(VAR) {
        let name = reader.text();
        let mut values = Vec::new();
        while reader.eat(VAL) {
            values.push(reader.value(0));
        }
        let value = match values.len() {
            1 => values.remove(0),
            _ => format!("[{}]", values.join(",")),
        };
        variables.push((name, value));
    }
    variables
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn eat(&mut self, marker: u8) -> bool {
        let found = self.bytes.get(self.at) == Some(&marker);
        if found {
            self.at += 1;
        }
        found
    }

    /// Reads plain text up to the next marker byte.
    fn text(&mut self) -> String {
        let rest = &self.bytes[self.at..];
        let len = (rest.iter())
            .position(|byte| (VAR..=ARRAY_CLOSE).contains(byte))
            .unwrap_or(rest.len());
        self.at += len;
        String::from_utf8_lossy(&rest[..len]).into_owned()
    }

    /// A value `depth` tables and arrays inside.
    fn value(&mut self, depth: usize) -> String {
        let opens = matches!(self.bytes.get(self.at), Some(&(ARRAY_OPEN | TABLE_OPEN)));
        if opens && depth >= DEEPEST {
            self.at = self.bytes.len();
            return "null".to_owned();
        }
        if self.eat(ARRAY_OPEN) {
            let mut items = Vec::new();
            while self.eat(VAL) {
                items.push(self.value(depth + 1));
            }
            self.eat(ARRAY_CLOSE);
            format!("[{}]", items.join(","))
        } else if self.eat(TABLE_OPEN) {
            let mut fields = Vec::new();
            while self.eat(VAR) {
                let name = json_string(&self.text());
                let value = if self.eat(VAL) {
                    self.value(depth + 1)
                } else {
                    "null".to_owned()
                };
                fields.push(format!("{name}:{value}"));
            }
            self.eat(TABLE_CLOSE);
            format!("{{{}}}", fields.join(","))
        } else {
            json_string(&self.text())
        }
    }
}

/// The names in a list of them as [`parse`] gives it: `["A","B"]`, or one
/// alone as `"A"`. Names of variables have neither quotes nor commas.
pub fn names(json: &str) -> Vec<String> {
    (json.trim_matches(['[', ']']).split(','))
        .map(|name| name.trim().trim_matches('"').to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Appends the MSDP message `variable = values...` to `out`, for example
/// `REPORT` with the names of the variables to report.
pub fn write(variable: &str, values: &[String], out: &mut Vec<u8>) {
    let mut payload = vec![VAR];
    payload.extend_from_slice(variable.as_bytes());
    for value in values {
        payload.push(VAL);
        payload.extend_from_slice(value.as_bytes());
    }
    telnet::write_subneg(opt::MSDP, &payload, out);
}

/// Parses an MSSP subnegotiation into `(name, value)` pairs. A name with
/// several values appears once per value.
pub fn parse_mssp(payload: &[u8]) -> Vec<(String, String)> {
    let mut reader = Reader {
        bytes: payload,
        at: 0,
    };
    let mut pairs = Vec::new();
    while reader.eat(VAR) {
        let name = reader.text();
        while reader.eat(VAL) {
            pairs.push((name.clone(), reader.text()));
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
        (items.iter())
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn simple_variables_and_multiple_values() {
        let payload = b"\x01HEALTH\x02120\x01NAME\x02Ana \"la roja\"\x01LIST\x02a\x02b\x01EMPTY";
        assert_eq!(
            parse(payload),
            pairs(&[
                ("HEALTH", r#""120""#),
                ("NAME", r#""Ana \"la roja\"""#),
                ("LIST", r#"["a","b"]"#),
                ("EMPTY", "[]"),
            ])
        );
    }

    #[test]
    fn nested_tables_and_arrays() {
        let payload = b"\x01ROOM\x02\x03\x01VNUM\x027\x01EXITS\x02\x03\x01n\x028\x01e\x029\x04\x01TAGS\x02\x05\x02safe\x02shop\x06\x04";
        assert_eq!(
            parse(payload),
            pairs(&[(
                "ROOM",
                r#"{"VNUM":"7","EXITS":{"n":"8","e":"9"},"TAGS":["safe","shop"]}"#
            )])
        );
    }

    #[test]
    fn truncated_data_does_not_panic() {
        assert_eq!(
            parse(b"\x01ROOM\x02\x03\x01VNUM"),
            pairs(&[("ROOM", r#"{"VNUM":null}"#)])
        );
        assert_eq!(parse(b"garbage"), []);
    }

    #[test]
    fn values_too_deep_are_not_read() {
        // Read on a thread with little stack, as the session's is.
        let deep = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut payload = b"\x01X\x02".to_vec();
                payload.extend(b"\x05\x02".repeat(200_000));
                parse(&payload)
            })
            .unwrap()
            .join()
            .expect("read without running out of stack");
        let (name, value) = &deep[0];
        assert_eq!(name, "X");
        assert!(value.ends_with(&format!("null{}", "]".repeat(DEEPEST))));
        // As deep as is read, it is all there.
        let mut payload = b"\x01X\x02".to_vec();
        payload.extend(b"\x05\x02".repeat(DEEPEST));
        payload.push(b'v');
        let read = parse(&payload);
        assert!(read[0].1.contains("\"v\""));
    }

    #[test]
    fn writes_report_requests() {
        let mut out = Vec::new();
        write("REPORT", &["HEALTH".into(), "MANA".into()], &mut out);
        let mut expected = Vec::new();
        telnet::write_subneg(opt::MSDP, b"\x01REPORT\x02HEALTH\x02MANA", &mut expected);
        assert_eq!(out, expected);
    }

    #[test]
    fn mssp_pairs() {
        assert_eq!(
            parse_mssp(b"\x01NAME\x02FakeMUD\x01PLAYERS\x0212\x01PORT\x0223\x024000"),
            pairs(&[
                ("NAME", "FakeMUD"),
                ("PLAYERS", "12"),
                ("PORT", "23"),
                ("PORT", "4000")
            ])
        );
    }
}
