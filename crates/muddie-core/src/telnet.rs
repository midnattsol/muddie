//! Telnet byte-stream parser: splits the raw stream into application data,
//! commands, option negotiations and subnegotiations. It holds no policy;
//! see [`crate::negotiate`] for how the client answers.

pub const IAC: u8 = 255;
pub const DONT: u8 = 254;
pub const DO: u8 = 253;
pub const WONT: u8 = 252;
pub const WILL: u8 = 251;
pub const SB: u8 = 250;
pub const GA: u8 = 249;
pub const NOP: u8 = 241;
pub const SE: u8 = 240;
pub const EOR: u8 = 239;

/// Telnet option codes the client knows about.
pub mod opt {
    pub const ECHO: u8 = 1;
    pub const SGA: u8 = 3;
    pub const TTYPE: u8 = 24;
    pub const EOR: u8 = 25;
    pub const NAWS: u8 = 31;
    pub const CHARSET: u8 = 42;
    pub const MSDP: u8 = 69;
    pub const MSSP: u8 = 70;
    pub const MCCP2: u8 = 86;
    pub const MCCP3: u8 = 87;
    pub const MXP: u8 = 91;
    pub const MSP: u8 = 90;
    pub const GMCP: u8 = 201;

    /// What an option is called, for those a MUD is likely to offer.
    pub fn name(option: u8) -> String {
        let known = match option {
            ECHO => "ECHO",
            SGA => "SGA",
            TTYPE => "TTYPE",
            EOR => "EOR",
            NAWS => "NAWS",
            CHARSET => "CHARSET",
            MSDP => "MSDP",
            MSSP => "MSSP",
            MCCP2 => "MCCP2",
            MCCP3 => "MCCP3",
            MXP => "MXP",
            MSP => "MSP",
            GMCP => "GMCP",
            // Others some MUDs offer, which this client does not take up.
            85 => "MCCP1",
            93 => "ZMP",
            102 => "Aardwolf",
            200 => "ATCP",
            other => return format!("option {other}"),
        };
        known.to_owned()
    }
}

/// Largest subnegotiation payload kept; further bytes are dropped.
const MAX_SUBNEG: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Will,
    Wont,
    Do,
    Dont,
}

impl Verb {
    pub fn byte(self) -> u8 {
        match self {
            Verb::Will => WILL,
            Verb::Wont => WONT,
            Verb::Do => DO,
            Verb::Dont => DONT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Application bytes, with `IAC IAC` already collapsed.
    Data(Vec<u8>),
    /// A two-byte command such as `IAC GA`.
    Command(u8),
    Negotiate(Verb, u8),
    /// `IAC SB <option> <payload> IAC SE`, payload unescaped.
    Subneg(u8, Vec<u8>),
}

#[derive(Debug, Clone, Copy)]
enum State {
    Data,
    Iac,
    Neg(Verb),
    SbOption,
    Sb,
    SbIac,
}

#[derive(Debug)]
pub struct Parser {
    state: State,
    data: Vec<u8>,
    sb_opt: u8,
    sb: Vec<u8>,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            state: State::Data,
            data: Vec::new(),
            sb_opt: 0,
            sb: Vec::new(),
        }
    }
}

impl Parser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses `input`, appending events to `out`, and returns how many bytes
    /// were consumed. That is all of them unless an MCCP2 subnegotiation ends
    /// inside `input`: everything after it is compressed, so the parser stops
    /// there and the caller must inflate the rest before feeding it back.
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<Event>) -> usize {
        let mut i = 0;
        while i < input.len() {
            let b = input[i];
            i += 1;
            match self.state {
                State::Data => {
                    if b == IAC {
                        self.state = State::Iac;
                    } else {
                        self.data.push(b);
                    }
                }
                State::Iac => match b {
                    IAC => {
                        self.data.push(IAC);
                        self.state = State::Data;
                    }
                    WILL => self.state = State::Neg(Verb::Will),
                    WONT => self.state = State::Neg(Verb::Wont),
                    DO => self.state = State::Neg(Verb::Do),
                    DONT => self.state = State::Neg(Verb::Dont),
                    SB => self.state = State::SbOption,
                    _ => {
                        self.flush_data(out);
                        out.push(Event::Command(b));
                        self.state = State::Data;
                    }
                },
                State::Neg(verb) => {
                    self.flush_data(out);
                    out.push(Event::Negotiate(verb, b));
                    self.state = State::Data;
                }
                State::SbOption => {
                    self.sb_opt = b;
                    self.sb.clear();
                    self.state = State::Sb;
                }
                State::Sb => {
                    if b == IAC {
                        self.state = State::SbIac;
                    } else {
                        self.push_sb(b);
                    }
                }
                State::SbIac => {
                    if b == IAC {
                        self.push_sb(IAC);
                        self.state = State::Sb;
                        continue;
                    }
                    self.flush_data(out);
                    out.push(Event::Subneg(self.sb_opt, std::mem::take(&mut self.sb)));
                    if b == SE {
                        self.state = State::Data;
                        if self.sb_opt == opt::MCCP2 {
                            return i;
                        }
                    } else {
                        // Unterminated subnegotiation: the server started a new
                        // command, so close this one and reparse the byte.
                        self.state = State::Iac;
                        i -= 1;
                    }
                }
            }
        }
        self.flush_data(out);
        i
    }

    fn push_sb(&mut self, b: u8) {
        if self.sb.len() < MAX_SUBNEG {
            self.sb.push(b);
        }
    }

    fn flush_data(&mut self, out: &mut Vec<Event>) {
        if !self.data.is_empty() {
            out.push(Event::Data(std::mem::take(&mut self.data)));
        }
    }
}

/// Appends `data` to `out`, doubling any IAC byte.
pub fn escape_iac(data: &[u8], out: &mut Vec<u8>) {
    for &b in data {
        if b == IAC {
            out.push(IAC);
        }
        out.push(b);
    }
}

/// Appends a complete subnegotiation frame to `out`.
pub fn write_subneg(option: u8, payload: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&[IAC, SB, option]);
    escape_iac(payload, out);
    out.extend_from_slice(&[IAC, SE]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(chunks: &[&[u8]]) -> Vec<Event> {
        let mut parser = Parser::new();
        let mut out = Vec::new();
        for chunk in chunks {
            assert_eq!(parser.feed(chunk, &mut out), chunk.len());
        }
        out
    }

    #[test]
    fn plain_data() {
        assert_eq!(parse(&[b"hello"]), [Event::Data(b"hello".to_vec())]);
    }

    #[test]
    fn escaped_iac_is_data() {
        assert_eq!(
            parse(&[&[b'a', IAC, IAC, b'b']]),
            [Event::Data(vec![b'a', IAC, b'b'])]
        );
    }

    #[test]
    fn negotiation_splits_data_in_order() {
        assert_eq!(
            parse(&[&[b'a', IAC, WILL, opt::ECHO, b'b', IAC, GA]]),
            [
                Event::Data(b"a".to_vec()),
                Event::Negotiate(Verb::Will, opt::ECHO),
                Event::Data(b"b".to_vec()),
                Event::Command(GA),
            ]
        );
    }

    #[test]
    fn sequences_split_across_reads() {
        let whole: &[u8] = &[
            IAC,
            SB,
            opt::GMCP,
            b'x',
            IAC,
            IAC,
            b'y',
            IAC,
            SE,
            IAC,
            DO,
            31,
        ];
        let expected = [
            Event::Subneg(opt::GMCP, vec![b'x', IAC, b'y']),
            Event::Negotiate(Verb::Do, 31),
        ];
        assert_eq!(parse(&[whole]), expected);
        for cut in 1..whole.len() {
            let (a, b) = whole.split_at(cut);
            assert_eq!(parse(&[a, b]), expected, "cut at {cut}");
        }
    }

    #[test]
    fn unterminated_subneg_is_closed_by_next_command() {
        assert_eq!(
            parse(&[&[IAC, SB, opt::TTYPE, 1, IAC, WILL, opt::ECHO]]),
            [
                Event::Subneg(opt::TTYPE, vec![1]),
                Event::Negotiate(Verb::Will, opt::ECHO),
            ]
        );
    }

    #[test]
    fn stops_after_mccp2_start() {
        let input = [b'a', IAC, SB, opt::MCCP2, IAC, SE, 0x78, 0x9c];
        let mut parser = Parser::new();
        let mut out = Vec::new();
        assert_eq!(parser.feed(&input, &mut out), 6);
        assert_eq!(
            out,
            [
                Event::Data(b"a".to_vec()),
                Event::Subneg(opt::MCCP2, vec![])
            ]
        );
    }

    #[test]
    fn subneg_writer_escapes_iac() {
        let mut out = Vec::new();
        write_subneg(opt::NAWS, &[0, 255, 0, 24], &mut out);
        assert_eq!(out, [IAC, SB, opt::NAWS, 0, IAC, IAC, 0, 24, IAC, SE]);
    }
}
