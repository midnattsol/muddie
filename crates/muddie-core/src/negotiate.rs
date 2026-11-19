//! Option negotiation policy: decides how the client answers the server's
//! telnet negotiations and turns the relevant ones into [`Signal`]s.

use crate::telnet::{self, Event, Verb, opt};

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client_name: "MUDDIE".into(),
            terminal_type: "XTERM-TRUECOLOR".into(),
            window: (80, 24),
            screen_reader: false,
            tls: false,
            gmcp_supports: ["Core 1", "Char 1", "Room 1", "Comm.Channel 1"]
                .map(String::from)
                .to_vec(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// The server took over echoing (true) or gave it back (false). While it
    /// echoes, typed input is normally a password and must be hidden.
    ServerEcho(bool),
    /// End of a prompt (`IAC GA` or `IAC EOR`).
    Prompt,
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

    /// Handles one non-data telnet event, appending any answer to `reply`.
    pub fn handle(&mut self, event: &Event, reply: &mut Vec<u8>) -> Option<Signal> {
        match *event {
            Event::Data(_) => None,
            Event::Command(telnet::GA | telnet::EOR) => Some(Signal::Prompt),
            Event::Command(_) => None,
            Event::Negotiate(verb, option) => self.negotiate(verb, option, reply),
            Event::Subneg(option, ref payload) => {
                self.subneg(option, payload, reply);
                None
            }
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
                    opt::ECHO | opt::SGA | opt::EOR | opt::CHARSET | opt::GMCP | opt::MCCP2
                );
                if !accept {
                    send(reply, Verb::Dont, option);
                } else if !self.remote[o] {
                    self.remote[o] = true;
                    send(reply, Verb::Do, option);
                    if option == opt::GMCP {
                        self.write_gmcp_hello(reply);
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
                let accept = matches!(option, opt::TTYPE | opt::NAWS | opt::CHARSET);
                if !accept {
                    send(reply, Verb::Wont, option);
                } else if !self.local[o] {
                    self.local[o] = true;
                    send(reply, Verb::Will, option);
                    if option == opt::NAWS {
                        self.write_naws(reply);
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

    fn subneg(&mut self, option: u8, payload: &[u8], reply: &mut Vec<u8>) {
        match option {
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
            opt::CHARSET if payload.first() == Some(&CHARSET_REQUEST) => {
                let mut list = &payload[1..];
                if let Some(rest) = list.strip_prefix(b"[TTABLE]") {
                    list = rest.get(1..).unwrap_or_default();
                }
                let offers_utf8 = list.split_first().is_some_and(|(sep, names)| {
                    names
                        .split(|b| b == sep)
                        .any(|name| name.eq_ignore_ascii_case(b"UTF-8"))
                });
                if offers_utf8 {
                    let mut body = vec![CHARSET_ACCEPTED];
                    body.extend_from_slice(b"UTF-8");
                    telnet::write_subneg(opt::CHARSET, &body, reply);
                } else {
                    telnet::write_subneg(opt::CHARSET, &[CHARSET_REJECTED], reply);
                }
            }
            _ => {}
        }
    }

    /// Whether the server agreed to exchange GMCP messages.
    pub fn gmcp_enabled(&self) -> bool {
        self.remote[opt::GMCP as usize]
    }

    /// Introduces the client and says which GMCP modules it wants.
    fn write_gmcp_hello(&self, reply: &mut Vec<u8>) {
        let hello = format!(
            r#"Core.Hello {{"client":{},"version":"{}"}}"#,
            json_string(&self.config.client_name),
            env!("CARGO_PKG_VERSION")
        );
        telnet::write_subneg(opt::GMCP, hello.as_bytes(), reply);
        let modules: Vec<String> = (self.config.gmcp_supports.iter())
            .map(|module| json_string(module))
            .collect();
        let supports = format!("Core.Supports.Set [{}]", modules.join(","));
        telnet::write_subneg(opt::GMCP, supports.as_bytes(), reply);
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
fn json_string(text: &str) -> String {
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

fn send(reply: &mut Vec<u8>, verb: Verb, option: u8) {
    reply.extend_from_slice(&[telnet::IAC, verb.byte(), option]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telnet::{DO, DONT, IAC, SB, SE, WILL, WONT};

    fn negotiator() -> Negotiator {
        Negotiator::new(Config::default())
    }

    fn handle(n: &mut Negotiator, event: Event) -> (Vec<u8>, Option<Signal>) {
        let mut reply = Vec::new();
        let signal = n.handle(&event, &mut reply);
        (reply, signal)
    }

    #[test]
    fn server_echo_toggles_and_is_not_answered_twice() {
        let mut n = negotiator();
        let will = Event::Negotiate(Verb::Will, opt::ECHO);
        assert_eq!(
            handle(&mut n, will.clone()),
            (vec![IAC, DO, opt::ECHO], Some(Signal::ServerEcho(true)))
        );
        assert_eq!(handle(&mut n, will), (vec![], None));
        assert_eq!(
            handle(&mut n, Event::Negotiate(Verb::Wont, opt::ECHO)),
            (vec![IAC, DONT, opt::ECHO], Some(Signal::ServerEcho(false)))
        );
    }

    #[test]
    fn unknown_options_are_refused() {
        let mut n = negotiator();
        assert_eq!(
            handle(&mut n, Event::Negotiate(Verb::Will, 99)).0,
            [IAC, DONT, 99]
        );
        assert_eq!(
            handle(&mut n, Event::Negotiate(Verb::Do, 99)).0,
            [IAC, WONT, 99]
        );
    }

    #[test]
    fn naws_reports_size_on_enable_and_on_resize() {
        let mut n = negotiator();
        let mut reply = Vec::new();
        n.set_window(100, 30, &mut reply);
        assert!(reply.is_empty(), "not reported before the server asks");
        assert_eq!(
            handle(&mut n, Event::Negotiate(Verb::Do, opt::NAWS)).0,
            [
                IAC,
                WILL,
                opt::NAWS,
                IAC,
                SB,
                opt::NAWS,
                0,
                100,
                0,
                30,
                IAC,
                SE
            ]
        );
        n.set_window(120, 40, &mut reply);
        assert_eq!(reply, [IAC, SB, opt::NAWS, 0, 120, 0, 40, IAC, SE]);
    }

    #[test]
    fn ttype_cycles_through_mtts_answers() {
        let mut n = Negotiator::new(Config {
            screen_reader: true,
            ..Config::default()
        });
        let mut answers = Vec::new();
        for _ in 0..4 {
            let (reply, _) = handle(&mut n, Event::Subneg(opt::TTYPE, vec![TTYPE_SEND]));
            assert_eq!(reply[..4], [IAC, SB, opt::TTYPE, TTYPE_IS]);
            assert_eq!(reply[reply.len() - 2..], [IAC, SE]);
            answers.push(String::from_utf8(reply[4..reply.len() - 2].to_vec()).unwrap());
        }
        assert_eq!(
            answers,
            ["MUDDIE", "XTERM-TRUECOLOR", "MTTS 333", "MTTS 333"]
        );
    }

    #[test]
    fn gmcp_is_accepted_with_hello_and_supports() {
        let mut n = Negotiator::new(Config {
            gmcp_supports: vec!["Char 1".into(), "Say \"hi\" 1".into()],
            ..Config::default()
        });
        assert!(!n.gmcp_enabled());
        let (reply, _) = handle(&mut n, Event::Negotiate(Verb::Will, opt::GMCP));
        let mut expected = vec![IAC, DO, opt::GMCP];
        let version = env!("CARGO_PKG_VERSION");
        let hello = format!(r#"Core.Hello {{"client":"MUDDIE","version":"{version}"}}"#);
        telnet::write_subneg(opt::GMCP, hello.as_bytes(), &mut expected);
        let supports = r#"Core.Supports.Set ["Char 1","Say \"hi\" 1"]"#;
        telnet::write_subneg(opt::GMCP, supports.as_bytes(), &mut expected);
        assert_eq!(reply, expected);
        assert!(n.gmcp_enabled());
    }

    #[test]
    fn charset_accepts_utf8_only() {
        let mut n = negotiator();
        let request = |names: &[u8]| {
            let mut payload = vec![CHARSET_REQUEST];
            payload.extend_from_slice(names);
            Event::Subneg(opt::CHARSET, payload)
        };
        let (reply, _) = handle(&mut n, request(b";ISO-8859-1;utf-8"));
        assert_eq!(reply[3], CHARSET_ACCEPTED);
        assert_eq!(&reply[4..9], b"UTF-8");
        let (reply, _) = handle(&mut n, request(b" ISO-8859-1"));
        assert_eq!(reply, [IAC, SB, opt::CHARSET, CHARSET_REJECTED, IAC, SE]);
    }
}
