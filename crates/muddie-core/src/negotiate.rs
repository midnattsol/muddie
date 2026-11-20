//! Option negotiation policy: decides how the client answers the server's
//! telnet negotiations and turns the relevant ones into [`Signal`]s.

use crate::msdp;
use crate::telnet::{self, Event, Verb, opt};

// NEW-ENVIRON (RFC 1572): the commands, and what marks a name and a value.
const ENVIRON_IS: u8 = 0;
const ENVIRON_SEND: u8 = 1;
const ENVIRON_INFO: u8 = 2;
const ENVIRON_VAR: u8 = 0;
const ENVIRON_VALUE: u8 = 1;
const ENVIRON_USERVAR: u8 = 3;

// MTTS capability bits (https://tintin.mudhalla.net/protocols/mtts/).
const MTTS_ANSI: u32 = 1;
const MTTS_UTF8: u32 = 4;
const MTTS_256_COLORS: u32 = 8;
const MTTS_SCREEN_READER: u32 = 64;
const MTTS_TRUECOLOR: u32 = 256;
const MTTS_TLS: u32 = 2048;

const TTYPE_IS: u8 = 0;
const TTYPE_SEND: u8 = 1;
const CHARSET_REQUEST: u8 = 1;
const CHARSET_ACCEPTED: u8 = 2;
const CHARSET_REJECTED: u8 = 3;
const CHARSET_TTABLE_IS: u8 = 4;
const CHARSET_TTABLE_REJECTED: u8 = 5;

#[derive(Debug, Clone)]
pub struct Config {
    pub client_name: String,
    pub terminal_type: String,
    /// Window size in character cells, reported through NAWS.
    pub window: (u16, u16),
    /// Advertised to the server so it can adapt its output.
    pub screen_reader: bool,
    pub tls: bool,
    /// GMCP modules announced to the server, as `"Name version"`.
    pub gmcp_supports: Vec<String>,
    /// MSDP variables the server is asked to report when it enables MSDP.
    pub msdp_report: Vec<String>,
    /// Label of the text encoding to use, such as `big5`; `auto` reads
    /// UTF-8 with Latin-1 as a fallback.
    pub encoding: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client_name: "MUDDIE".into(),
            terminal_type: "XTERM-TRUECOLOR".into(),
            window: (80, 24),
            screen_reader: false,
            tls: false,
            gmcp_supports: [
                "Core 1",
                "Char 1",
                "Room 1",
                "Comm.Channel 1",
                "Client.Media 1",
            ]
            .map(String::from)
            .to_vec(),
            msdp_report: [
                "HEALTH",
                "HEALTH_MAX",
                "MANA",
                "MANA_MAX",
                "MOVEMENT",
                "MOVEMENT_MAX",
                "ROOM",
            ]
            .map(String::from)
            .to_vec(),
            encoding: "auto".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// The server took over echoing (true) or gave it back (false). While it
    /// echoes, typed input is normally a password and must be hidden.
    ServerEcho(bool),
    /// End of a prompt (`IAC GA` or `IAC EOR`).
    Prompt,
    /// Everything sent from now on must be compressed (MCCP3).
    CompressOutgoing,
    /// The server will send MXP markup.
    Mxp,
    /// The encoding agreed with the server, which what arrives is in.
    Charset(String),
}

pub struct Negotiator {
    config: Config,
    /// Options enabled on our side (we sent WILL).
    local: [bool; 256],
    /// Options enabled on the server side (we sent DO).
    remote: [bool; 256],
    ttype_step: u8,
}

impl Negotiator {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            local: [false; 256],
            remote: [false; 256],
            ttype_step: 0,
        }
    }

    /// Whether an option the server offered, or asked this client for,
    /// is in use.
    pub fn in_use(&self, option: u8) -> bool {
        self.remote[option as usize] || self.local[option as usize]
    }

    /// Handles one non-data telnet event, appending any answer to `reply`.
    pub fn handle(&mut self, event: &Event, reply: &mut Vec<u8>) -> Option<Signal> {
        match *event {
            Event::Data(_) => None,
            Event::Command(telnet::GA | telnet::EOR) => Some(Signal::Prompt),
            Event::Command(_) => None,
            Event::Negotiate(verb, option) => self.negotiate(verb, option, reply),
            Event::Subneg(option, ref payload) => self.subneg(option, payload, reply),
        }
    }

    /// Updates the window size and reports it if the server asked for NAWS.
    pub fn set_window(&mut self, width: u16, height: u16, reply: &mut Vec<u8>) {
        self.config.window = (width, height);
        if self.local[opt::NAWS as usize] {
            self.write_naws(reply);
        }
    }

    fn negotiate(&mut self, verb: Verb, option: u8, reply: &mut Vec<u8>) -> Option<Signal> {
        let o = option as usize;
        match verb {
            Verb::Will => {
                let accept = matches!(
                    option,
                    opt::ECHO
                        | opt::SGA
                        | opt::EOR
                        | opt::CHARSET
                        | opt::GMCP
                        | opt::MCCP2
                        | opt::MSDP
                        | opt::MSSP
                        | opt::MSP
                        | opt::MCCP3
                        | opt::MXP
                );
                if !accept {
                    send(reply, Verb::Dont, option);
                } else if !self.remote[o] {
                    self.remote[o] = true;
                    send(reply, Verb::Do, option);
                    if option == opt::GMCP {
                        self.write_gmcp_hello(reply);
                    }
                    if option == opt::MSDP {
                        // The usual ones at once, and a list of all the
                        // server can report, to ask for the rest.
                        if !self.config.msdp_report.is_empty() {
                            msdp::write("REPORT", &self.config.msdp_report, reply);
                        }
                        msdp::write("LIST", &["REPORTABLE_VARIABLES".to_owned()], reply);
                    }
                    if option == opt::MCCP3 {
                        telnet::write_subneg(opt::MCCP3, &[], reply);
                        return Some(Signal::CompressOutgoing);
                    }
                    if option == opt::MXP {
                        return Some(Signal::Mxp);
                    }
                    if option == opt::ECHO {
                        return Some(Signal::ServerEcho(true));
                    }
                }
            }
            Verb::Wont => {
                if self.remote[o] {
                    self.remote[o] = false;
                    send(reply, Verb::Dont, option);
                    if option == opt::ECHO {
                        return Some(Signal::ServerEcho(false));
                    }
                }
            }
            Verb::Do => {
                // MXP is asked for this way by some servers, and offered
                // with WILL by others: it is taken up either way.
                let accept = matches!(
                    option,
                    opt::TTYPE | opt::NAWS | opt::CHARSET | opt::MXP | opt::NEW_ENVIRON
                );
                if !accept {
                    send(reply, Verb::Wont, option);
                } else if !self.local[o] {
                    self.local[o] = true;
                    send(reply, Verb::Will, option);
                    if option == opt::NAWS {
                        self.write_naws(reply);
                    }
                    // Asked to say which encoding to use: the one the
                    // user chose, or UTF-8.
                    if option == opt::CHARSET {
                        let chosen = self.config.encoding.trim();
                        let wanted = match chosen.is_empty() || chosen.eq_ignore_ascii_case("auto")
                        {
                            true => "UTF-8",
                            false => chosen,
                        };
                        let mut body = vec![CHARSET_REQUEST, b';'];
                        body.extend_from_slice(wanted.as_bytes());
                        telnet::write_subneg(opt::CHARSET, &body, reply);
                    }
                    if option == opt::MXP {
                        return Some(Signal::Mxp);
                    }
                }
            }
            Verb::Dont => {
                if self.local[o] {
                    self.local[o] = false;
                    send(reply, Verb::Wont, option);
                }
            }
        }
        None
    }

    /// What this client says of itself when asked through NEW-ENVIRON,
    /// by the names of the MUD New-Environ Standard. Nothing of the
    /// computer or its user is among them, whatever is asked for.
    fn environment(&self) -> [(&'static str, String); 5] {
        [
            ("CHARSET", "UTF-8".to_owned()),
            ("CLIENT_NAME", self.config.client_name.clone()),
            ("CLIENT_VERSION", env!("CARGO_PKG_VERSION").to_owned()),
            ("MTTS", self.mtts().to_string()),
            ("TERMINAL_TYPE", self.config.terminal_type.clone()),
        ]
    }

    fn subneg(&mut self, option: u8, payload: &[u8], reply: &mut Vec<u8>) -> Option<Signal> {
        match option {
            // `SEND` with the names wanted, each after `VAR` or `USERVAR`;
            // with none, all there are. Answered with `IS`, each name
            // after `VAR` and its value after `VALUE`.
            opt::NEW_ENVIRON if payload.first() == Some(&ENVIRON_SEND) => {
                let asked: Vec<&[u8]> = (payload[1..])
                    .split(|byte| matches!(*byte, ENVIRON_VAR | ENVIRON_USERVAR))
                    .filter(|name| !name.is_empty())
                    .collect();
                let mut body = vec![ENVIRON_IS];
                for (name, value) in self.environment() {
                    if asked.is_empty() || asked.contains(&name.as_bytes()) {
                        body.push(ENVIRON_VAR);
                        body.extend_from_slice(name.as_bytes());
                        body.push(ENVIRON_VALUE);
                        body.extend_from_slice(value.as_bytes());
                    }
                }
                telnet::write_subneg(opt::NEW_ENVIRON, &body, reply);
            }
            opt::TTYPE if payload == [TTYPE_SEND] => {
                // MTTS: client name, then terminal type, then the capability
                // bitvector, which is repeated for any further request.
                let name = match self.ttype_step {
                    0 => self.config.client_name.clone(),
                    1 => self.config.terminal_type.clone(),
                    _ => format!("MTTS {}", self.mtts()),
                };
                self.ttype_step = self.ttype_step.saturating_add(1);
                let mut body = vec![TTYPE_IS];
                body.extend_from_slice(name.as_bytes());
                telnet::write_subneg(opt::TTYPE, &body, reply);
            }
            // The server names the encodings it can use. The one the
            // user chose is taken if it is among them, and none other;
            // with none chosen, UTF-8, or failing that the first of them
            // this client knows, which what arrives is then read as.
            opt::CHARSET if payload.first() == Some(&CHARSET_REQUEST) => {
                let mut list = &payload[1..];
                if let Some(rest) = list.strip_prefix(b"[TTABLE]") {
                    list = rest.get(1..).unwrap_or_default();
                }
                let names: Vec<&[u8]> = match list.split_first() {
                    Some((sep, names)) => names.split(|byte| byte == sep).collect(),
                    None => Vec::new(),
                };
                let named = |wanted: &str| {
                    (names.iter().copied())
                        .find(|name| name.eq_ignore_ascii_case(wanted.as_bytes()))
                };
                let chosen = self.config.encoding.trim();
                let taken = if chosen.is_empty() || chosen.eq_ignore_ascii_case("auto") {
                    named("UTF-8").or_else(|| {
                        (names.iter().copied()).find(|name| {
                            let label = String::from_utf8_lossy(name);
                            !name.is_empty() && crate::decode::Decoder::for_label(&label).is_some()
                        })
                    })
                } else {
                    named(chosen)
                };
                if let Some(name) = taken {
                    let mut body = vec![CHARSET_ACCEPTED];
                    body.extend_from_slice(name);
                    telnet::write_subneg(opt::CHARSET, &body, reply);
                    return Some(Signal::Charset(String::from_utf8_lossy(name).into_owned()));
                }
                telnet::write_subneg(opt::CHARSET, &[CHARSET_REJECTED], reply);
            }
            // The server took the encoding this client asked for.
            opt::CHARSET if payload.first() == Some(&CHARSET_ACCEPTED) => {
                let name = String::from_utf8_lossy(&payload[1..]).trim().to_owned();
                return Some(Signal::Charset(name));
            }
            // A table to translate by is not something this client takes.
            opt::CHARSET if payload.first() == Some(&CHARSET_TTABLE_IS) => {
                telnet::write_subneg(opt::CHARSET, &[CHARSET_TTABLE_REJECTED], reply);
            }
            _ => {}
        }
        None
    }

    /// Whether the server agreed to exchange GMCP messages.
    pub fn gmcp_enabled(&self) -> bool {
        self.remote[opt::GMCP as usize]
    }

    /// Whether the server agreed to exchange MSDP messages.
    pub fn msdp_enabled(&self) -> bool {
        self.remote[opt::MSDP as usize]
    }

    /// Introduces the client and says which GMCP modules it wants.
    fn write_gmcp_hello(&self, reply: &mut Vec<u8>) {
        let hello = format!(
            r#"Core.Hello {{"client":{},"version":"{}"}}"#,
            json_string(&self.config.client_name),
            env!("CARGO_PKG_VERSION")
        );
        telnet::write_subneg(opt::GMCP, hello.as_bytes(), reply);
        let modules: Vec<&str> = (self.config.gmcp_supports.iter())
            .map(String::as_str)
            .collect();
        write_gmcp_list("Core.Supports.Set", &modules, reply);
    }

    /// Changes the GMCP modules wanted, as `"Name version"`. Once GMCP is
    /// on, the server is told only of the change: what is new is added,
    /// and what is no longer wanted is removed, by its name alone. Before
    /// that, they are what the greeting will ask for.
    pub fn set_gmcp_supports(&mut self, modules: Vec<String>, reply: &mut Vec<u8>) {
        if self.gmcp_enabled() {
            let told = &self.config.gmcp_supports;
            let removed: Vec<&str> = (told.iter())
                .map(|module| module_name(module))
                .filter(|name| !modules.iter().any(|module| module_name(module) == *name))
                .collect();
            let added: Vec<&str> = (modules.iter())
                .filter(|module| !told.contains(module))
                .map(String::as_str)
                .collect();
            if !removed.is_empty() {
                write_gmcp_list("Core.Supports.Remove", &removed, reply);
            }
            if !added.is_empty() {
                write_gmcp_list("Core.Supports.Add", &added, reply);
            }
        }
        self.config.gmcp_supports = modules;
    }

    /// A screen reader started or stopped speaking. A server that asked
    /// through NEW-ENVIRON is told the new MTTS at once, as MNES says;
    /// any other learns it when it next asks.
    pub fn set_screen_reader(&mut self, on: bool, reply: &mut Vec<u8>) {
        if on == self.config.screen_reader {
            return;
        }
        self.config.screen_reader = on;
        if self.in_use(opt::NEW_ENVIRON) {
            let mut body = vec![ENVIRON_INFO, ENVIRON_VAR];
            body.extend_from_slice(b"MTTS");
            body.push(ENVIRON_VALUE);
            body.extend_from_slice(self.mtts().to_string().as_bytes());
            telnet::write_subneg(opt::NEW_ENVIRON, &body, reply);
        }
    }

    fn mtts(&self) -> u32 {
        let mut bits = MTTS_ANSI | MTTS_UTF8 | MTTS_256_COLORS | MTTS_TRUECOLOR;
        if self.config.screen_reader {
            bits |= MTTS_SCREEN_READER;
        }
        if self.config.tls {
            bits |= MTTS_TLS;
        }
        bits
    }

    fn write_naws(&self, reply: &mut Vec<u8>) {
        let (w, h) = self.config.window;
        let [w1, w0] = w.to_be_bytes();
        let [h1, h0] = h.to_be_bytes();
        telnet::write_subneg(opt::NAWS, &[w1, w0, h1, h0], reply);
    }
}

/// Quotes `text` as a JSON string.
pub(crate) fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// The name of a GMCP module given as `"Name version"`.
fn module_name(module: &str) -> &str {
    module.split_whitespace().next().unwrap_or_default()
}

/// Writes a GMCP message whose data is a list of strings.
fn write_gmcp_list(package: &str, items: &[&str], reply: &mut Vec<u8>) {
    let items: Vec<String> = items.iter().map(|item| json_string(item)).collect();
    let message = format!("{package} [{}]", items.join(","));
    telnet::write_subneg(opt::GMCP, message.as_bytes(), reply);
}

fn send(reply: &mut Vec<u8>, verb: Verb, option: u8) {
    reply.extend_from_slice(&[telnet::IAC, verb.byte(), option]);
}

#[cfg(test)]
mod tests;
