//! What the MUD sends besides text, as it comes: kept while it is looked
//! at, with its values laid out by path.

use super::*;

/// What was received under one name: a GMCP package or a MSDP variable.
#[derive(Clone)]
pub struct Seen {
    pub name: String,
    /// The protocol it came by: GMCP, MSDP, MXP...
    pub protocol: String,
    pub msdp: bool,
    /// The latest, as JSON; empty if it came with nothing.
    pub json: String,
    /// When the latest came, and how many have.
    pub at: String,
    pub count: u32,
    /// Those before the latest, newest first: when, and what.
    pub earlier: VecDeque<(String, String)>,
}

/// The values in a message, each with the path that leads to it.
/// `gauge` says whether a gauge could follow it: a number, reached
/// through names only.
pub struct Leaf {
    pub path: String,
    pub shown: String,
    pub gauge: bool,
}

/// Lays a message out as its values. Fields of tables are reached with a
/// dot, as gauges and scripts reach them; items of lists by their place,
/// counted from one as Luau counts.
pub fn leaves(path: &str, value: &Value, named: bool, into: &mut Vec<Leaf>) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                leaves(&format!("{path}.{key}"), value, named, into);
            }
        }
        Value::Array(items) => {
            for (place, value) in items.iter().enumerate() {
                leaves(&format!("{path}[{}]", place + 1), value, false, into);
            }
        }
        leaf => {
            let number = match leaf {
                Value::Number(_) => true,
                Value::String(text) => text.trim().parse::<f64>().is_ok(),
                _ => false,
            };
            let shown = match leaf {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            into.push(Leaf {
                path: path.to_owned(),
                shown,
                gauge: number && named,
            });
        }
    }
}

/// Where the maximum of a value is likely to be, among the values beside
/// it: `max` for a `current`, `maxhp` for an `hp`, and their like.
pub fn likely_max(path: &str, all: &[Leaf]) -> Option<String> {
    let (parent, key) = path.rsplit_once('.')?;
    let key = key.to_lowercase();
    let wanted: Vec<String> = if matches!(key.as_str(), "current" | "cur" | "value" | "now") {
        vec!["max".into(), "maximum".into()]
    } else {
        vec![
            format!("max{key}"),
            format!("{key}max"),
            format!("max_{key}"),
            format!("{key}_max"),
        ]
    };
    let beside = all.iter().filter(|leaf| {
        leaf.gauge
            && leaf
                .path
                .rsplit_once('.')
                .is_some_and(|(of, _)| of == parent)
    });
    let mut beside = beside.filter(|leaf| leaf.path != path);
    beside
        .find(|leaf| {
            wanted.contains(
                &leaf
                    .path
                    .rsplit('.')
                    .next()
                    .unwrap_or_default()
                    .to_lowercase(),
            )
        })
        .map(|leaf| leaf.path.clone())
}

impl Tab {
    /// Notes something the MUD sent, for the editor to show.
    pub(in crate::tab) fn saw_data(&mut self, name: &str, msdp: bool, json: &str) {
        self.saw(if msdp { "MSDP" } else { "GMCP" }, name, json);
    }

    /// Whether what arrives is being kept to be looked at: while it is
    /// shown, in the editor, or if asked to go on outside it. It is not
    /// otherwise, so that it costs nothing.
    pub(in crate::tab) fn recording(&self) -> bool {
        self.data_open() || self.data_view.keep
    }

    /// Whether what the MUD sends is on the screen.
    pub(in crate::tab) fn data_open(&self) -> bool {
        self.editor.open && self.editor.section == automation::DATA
    }

    /// Tells the connection whether to say what it otherwise passes over,
    /// when that has changed.
    pub(in crate::tab) fn sync_trace(&mut self) {
        // With --debug, always: what goes over the protocols is shown.
        let wanted = self.recording() || crate::debug::on();
        if self.data_view.trace_sent != Some(wanted)
            && let Some(connection) = &self.connection
        {
            let _ = connection.commands.send(Command::Trace(wanted));
            self.data_view.trace_sent = Some(wanted);
        }
    }

    /// Keeps something that arrived by `protocol` under `name`: the
    /// latest, and a few of those before it.
    pub(in crate::tab) fn saw(&mut self, protocol: &str, name: &str, json: &str) {
        let msdp = protocol == "MSDP";
        let at = crate::output::clock();
        match self
            .data_view
            .seen
            .iter_mut()
            .find(|seen| seen.name == name && seen.protocol == protocol)
        {
            Some(seen) => {
                let before = (
                    std::mem::replace(&mut seen.at, at),
                    std::mem::replace(&mut seen.json, json.to_owned()),
                );
                seen.earlier.push_front(before);
                seen.earlier.truncate(KEPT);
                seen.count += 1;
            }
            None => {
                // No more names than can be looked through.
                if self.data_view.seen.len() >= MOST {
                    return;
                }
                self.data_view.seen.push(Seen {
                    name: name.to_owned(),
                    protocol: protocol.to_owned(),
                    msdp,
                    json: json.to_owned(),
                    at,
                    count: 1,
                    earlier: VecDeque::new(),
                });
                self.data_view
                    .seen
                    .sort_by_key(|seen| seen.name.to_lowercase());
            }
        }
    }
}

impl Seen {
    /// How many times and when, for lists.
    pub fn summary(&self) -> String {
        format!("{} · {} × · {}", self.protocol, self.count, self.at)
    }
}
