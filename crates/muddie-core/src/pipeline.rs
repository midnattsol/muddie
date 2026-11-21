//! The synchronous path from raw server bytes to session events. It has no
//! I/O, so it can be driven by tests as well as by [`crate::session`].

use std::io;

use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};

use crate::ansi::Assembler;
use crate::decode::Decoder;
use crate::msdp;
use crate::negotiate::{Config, Negotiator, Signal};
use crate::telnet::{self, Parser, opt};
use crate::text::Line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Connected,
    /// A complete line of output.
    Line(Line),
    /// Text the server explicitly marked as a prompt (GA/EOR).
    Prompt(Line),
    /// Unterminated text at the end of what has arrived so far. It may be a
    /// prompt from a server that does not mark them, or the start of a line
    /// still in transit; the next `Line`, `Prompt` or `Partial` replaces it.
    Partial(Line),
    /// Typed input should be hidden (true) or shown again (false).
    InputHidden(bool),
    /// A line of input was sent. Not emitted while input is hidden.
    Sent(String),
    /// A GMCP message such as `Char.Vitals`; `data` is its JSON payload,
    /// empty if the message had none.
    Gmcp {
        package: String,
        data: String,
    },
    /// An MSDP variable was reported; `data` is its value as JSON.
    Msdp {
        variable: String,
        data: String,
    },
    /// The server's MSSP status, as `(name, value)` pairs.
    Mssp(Vec<(String, String)>),
    /// Something of another protocol arrived, told while it is being
    /// traced: an MXP tag, or what an option this client does not know
    /// was sent with.
    Other {
        protocol: String,
        name: String,
        data: String,
    },
    /// An MXP tag asked something of the client besides showing text.
    Mxp(crate::ansi::Mxp),
    /// What went over telnet besides text, told while tracing: options
    /// offered and answered either way, and what the client sent with
    /// them (GMCP, MSDP and the rest), by the protocol it went by.
    Wire {
        outgoing: bool,
        protocol: String,
        text: String,
    },
    /// The server offered a protocol, or withdrew it; `on` says whether
    /// it is now in use.
    Protocol {
        name: String,
        on: bool,
    },
    /// The connection ended; the string is the error, if it was one.
    Disconnected(Option<String>),
}

/// Size of the chunks compressed data is inflated in.
const INFLATE_BUFFER: usize = 16 * 1024;

pub struct Pipeline {
    /// Whether what is otherwise made nothing of is told, for debugging.
    trace: bool,
    /// What went over telnet while tracing, to be told with the rest.
    told: Vec<Event>,
    /// Whether the server has been asked for all the MSDP variables it
    /// said it can report.
    msdp_asked: bool,
    parser: Parser,
    negotiator: Negotiator,
    decoder: Decoder,
    assembler: Assembler,
    telnet_events: Vec<telnet::Event>,
    text: String,
    lines: Vec<Line>,
    input_hidden: bool,
    /// `Some` while the server's stream is compressed (MCCP2).
    inflater: Option<Decompress>,
    inflated: Vec<u8>,
    /// `Some` once what the client sends must be compressed (MCCP3).
    deflater: Option<Compress>,
    /// Where in the buffer being filled compression starts, when it was
    /// turned on part-way through filling it.
    deflate_from: Option<usize>,
}

impl Pipeline {
    pub fn new(config: Config) -> Self {
        Self {
            trace: false,
            told: Vec::new(),
            msdp_asked: false,
            parser: Parser::new(),
            decoder: Decoder::for_label(&config.encoding).unwrap_or_default(),
            negotiator: Negotiator::new(config),
            assembler: Assembler::new(),
            telnet_events: Vec::new(),
            text: String::new(),
            lines: Vec::new(),
            input_hidden: false,
            inflater: None,
            inflated: Vec::new(),
            deflater: None,
            deflate_from: None,
        }
    }

    /// Processes bytes read from the server. Telnet answers are appended to
    /// `reply`, which the caller must send; output is appended to `events`.
    /// Fails only if the server's compressed stream is corrupt.
    pub fn receive(
        &mut self,
        input: &[u8],
        reply: &mut Vec<u8>,
        events: &mut Vec<Event>,
    ) -> io::Result<()> {
        let reply_start = reply.len();
        let mut rest = input;
        while !rest.is_empty() {
            match self.inflater.take() {
                None => {
                    let used = self.receive_plain(rest, reply, events);
                    rest = &rest[used..];
                }
                Some(mut inflater) => {
                    let ended = self.receive_compressed(&mut inflater, &mut rest, reply, events)?;
                    if !ended {
                        self.inflater = Some(inflater);
                    }
                }
            }
        }
        if !self.assembler.partial().is_empty() {
            events.push(Event::Partial(self.assembler.partial().clone()));
        }
        self.finish_outgoing(reply, reply_start);
        Ok(())
    }

    /// Compresses what was appended to `out` from `start` on, if outgoing
    /// compression is on. Everything the client sends goes through here.
    fn finish_outgoing(&mut self, out: &mut Vec<u8>, start: usize) {
        if self.trace {
            self.told.extend(wire(&out[start..], true));
        }
        // Compression turned on while this buffer was being filled only
        // covers what came after the message that announced it.
        let from = self.deflate_from.take().unwrap_or(start);
        let Some(deflater) = &mut self.deflater else {
            return;
        };
        let plain = out.split_off(from);
        let mut rest = plain.as_slice();
        loop {
            out.reserve(rest.len() + 64);
            let before = deflater.total_in();
            // A sync flush makes each write decodable on arrival.
            let _ = deflater.compress_vec(rest, out, FlushCompress::Sync);
            rest = &rest[(deflater.total_in() - before) as usize..];
            if rest.is_empty() && out.len() < out.capacity() {
                return;
            }
        }
    }

    /// Inflates as much of `rest` as belongs to the compressed stream and
    /// processes the result. Returns true once the stream has ended, after
    /// which the remaining bytes of `rest` are plain again.
    fn receive_compressed(
        &mut self,
        inflater: &mut Decompress,
        rest: &mut &[u8],
        reply: &mut Vec<u8>,
        events: &mut Vec<Event>,
    ) -> io::Result<bool> {
        let mut buffer = std::mem::take(&mut self.inflated);
        buffer.resize(INFLATE_BUFFER, 0);
        let ended = loop {
            let (in_before, out_before) = (inflater.total_in(), inflater.total_out());
            let status = inflater
                .decompress(rest, &mut buffer, FlushDecompress::None)
                .map_err(io::Error::other)?;
            let used = (inflater.total_in() - in_before) as usize;
            let made = (inflater.total_out() - out_before) as usize;
            *rest = &rest[used..];
            let mut plain = &buffer[..made];
            while !plain.is_empty() {
                let used = self.receive_plain(plain, reply, events);
                plain = &plain[used..];
            }
            if status == Status::StreamEnd {
                break true;
            }
            // A full buffer may mean more output is waiting; otherwise the
            // inflater needs input that has not arrived yet.
            if made < buffer.len() && (rest.is_empty() || used == 0) {
                break false;
            }
        };
        self.inflated = buffer;
        Ok(ended)
    }

    /// Processes uncompressed bytes and returns how many it consumed: all of
    /// them unless compression starts part-way through.
    fn receive_plain(
        &mut self,
        input: &[u8],
        reply: &mut Vec<u8>,
        events: &mut Vec<Event>,
    ) -> usize {
        let used = self.parser.feed(input, &mut self.telnet_events);
        // The buffer is lent out and given back, to keep what it holds.
        let mut telnet_events = std::mem::take(&mut self.telnet_events);
        for telnet_event in telnet_events.drain(..) {
            if !self.take_known(&telnet_event, reply, events) {
                self.negotiate(telnet_event, reply, events);
            }
        }
        self.telnet_events = telnet_events;
        used
    }

    /// Takes what is the client's own to read: text, and the subnegotiations
    /// of the options it makes something of. Returns whether it was one.
    fn take_known(
        &mut self,
        telnet_event: &telnet::Event,
        reply: &mut Vec<u8>,
        events: &mut Vec<Event>,
    ) -> bool {
        match telnet_event {
            telnet::Event::Data(bytes) => self.take_text(bytes, reply, events),
            telnet::Event::Subneg(opt::MCCP2, _) => self.inflater = Some(Decompress::new(true)),
            telnet::Event::Subneg(opt::GMCP, payload) => {
                let message = String::from_utf8_lossy(payload);
                let (package, data) = message.split_once(' ').unwrap_or((&message, ""));
                events.push(Event::Gmcp {
                    package: package.to_owned(),
                    data: data.trim().to_owned(),
                });
            }
            telnet::Event::Subneg(opt::MSDP, payload) => self.take_msdp(payload, reply, events),
            telnet::Event::Subneg(opt::MSSP, payload) => {
                events.push(Event::Mssp(msdp::parse_mssp(payload)))
            }
            _ => return false,
        }
        true
    }

    /// Text: decoded, made into lines, with what MXP asked for and, when
    /// tracing, the tags it came in; answers to MXP queries go back as
    /// lines.
    fn take_text(&mut self, bytes: &[u8], reply: &mut Vec<u8>, events: &mut Vec<Event>) {
        self.decoder.feed(bytes, &mut self.text);
        self.assembler.feed(&self.text, &mut self.lines);
        self.text.clear();
        events.extend(self.lines.drain(..).map(Event::Line));
        events.extend(self.assembler.take_asked().into_iter().map(Event::Mxp));
        if self.trace {
            events.extend(self.assembler.take_tags().into_iter().map(|tag| {
                let name = tag.trim_start_matches('/').split_whitespace().next();
                Event::Other {
                    protocol: "MXP".to_owned(),
                    name: name.unwrap_or_default().to_lowercase(),
                    data: format!("<{tag}>"),
                }
            }));
        }
        for answer in self.assembler.take_replies() {
            telnet::escape_iac(&self.decoder.encode(&answer), reply);
            reply.extend_from_slice(b"\r\n");
        }
    }

    /// MSDP variables. Told which variables it can report, the server is
    /// asked for them all, once: nothing has to be set up to have what a
    /// MUD offers.
    fn take_msdp(&mut self, payload: &[u8], reply: &mut Vec<u8>, events: &mut Vec<Event>) {
        let variables = msdp::parse(payload);
        let listed = (variables.iter())
            .find(|(variable, _)| variable == "REPORTABLE_VARIABLES")
            .map(|(_, data)| msdp::names(data));
        if let (Some(names), false) = (listed, self.msdp_asked) {
            self.msdp_asked = true;
            if !names.is_empty() {
                msdp::write("REPORT", &names, reply);
            }
        }
        events.extend(
            variables
                .into_iter()
                .map(|(variable, data)| Event::Msdp { variable, data }),
        );
    }

    /// What the negotiator makes of the rest: options offered and agreed,
    /// and what that sets going. What comes with an option this client
    /// makes nothing of is told when tracing; what the server offers is
    /// told, taken up or not.
    fn negotiate(
        &mut self,
        telnet_event: telnet::Event,
        reply: &mut Vec<u8>,
        events: &mut Vec<Event>,
    ) {
        if self.trace
            && let telnet::Event::Subneg(option, payload) = &telnet_event
            && !matches!(*option, opt::TTYPE | opt::CHARSET | opt::NAWS | opt::MCCP3)
        {
            events.push(Event::Other {
                protocol: opt::name(*option),
                name: opt::name(*option),
                data: String::from_utf8_lossy(payload).into_owned(),
            });
        }
        if self.trace && matches!(telnet_event, telnet::Event::Negotiate(..)) {
            self.told.extend(wire_event(telnet_event.clone(), false));
        }
        let signal = self.negotiator.handle(&telnet_event, reply);
        if let telnet::Event::Negotiate(_, option) = telnet_event {
            events.push(Event::Protocol {
                name: opt::name(option),
                on: self.negotiator.in_use(option),
            });
        }
        if let Some(signal) = signal {
            self.follow(signal, reply.len(), events);
        }
    }

    /// Does what an agreement sets going.
    fn follow(&mut self, signal: Signal, replied: usize, events: &mut Vec<Event>) {
        match signal {
            Signal::Prompt => {
                self.decoder.flush(&mut self.text);
                self.assembler.feed(&self.text, &mut self.lines);
                self.text.clear();
                events.extend(self.lines.drain(..).map(Event::Line));
                if let Some(prompt) = self.assembler.take_partial() {
                    events.push(Event::Prompt(prompt));
                }
            }
            Signal::ServerEcho(on) => {
                self.input_hidden = on;
                events.push(Event::InputHidden(on));
            }
            Signal::CompressOutgoing => {
                self.deflater = Some(Compress::new(Compression::default(), true));
                self.deflate_from = Some(replied);
            }
            Signal::Mxp => self.assembler.enable_mxp(),
            // What arrives from here on is in the encoding agreed.
            Signal::Charset(name) => {
                if let Some(decoder) = Decoder::for_label(&name) {
                    self.decoder.flush(&mut self.text);
                    self.decoder = decoder;
                }
            }
        }
    }

    /// Encodes a GMCP message; `data` is JSON or empty. Nothing is written
    /// if the server did not enable GMCP.
    pub fn send_gmcp(&mut self, package: &str, data: &str, out: &mut Vec<u8>) {
        if !self.negotiator.gmcp_enabled() {
            return;
        }
        let start = out.len();
        let message = if data.is_empty() {
            package.to_owned()
        } else {
            format!("{package} {data}")
        };
        telnet::write_subneg(opt::GMCP, message.as_bytes(), out);
        self.finish_outgoing(out, start);
    }

    /// Changes the GMCP modules wanted; the server is told what changed
    /// if GMCP is on.
    pub fn set_gmcp_supports(&mut self, modules: Vec<String>, out: &mut Vec<u8>) {
        let start = out.len();
        self.negotiator.set_gmcp_supports(modules, out);
        if out.len() > start {
            self.finish_outgoing(out, start);
        }
    }

    /// A screen reader started or stopped speaking; the server is told if
    /// it can be.
    pub fn set_screen_reader(&mut self, on: bool, out: &mut Vec<u8>) {
        let start = out.len();
        self.negotiator.set_screen_reader(on, out);
        if out.len() > start {
            self.finish_outgoing(out, start);
        }
    }

    /// Starts or stops telling what arrives of the protocols this client
    /// only passes over: it costs nothing while nobody is looking.
    /// Moves what went over telnet since last asked, while tracing, to
    /// `events`.
    pub fn take_told(&mut self, events: &mut Vec<Event>) {
        events.append(&mut self.told);
    }

    pub fn set_trace(&mut self, trace: bool) {
        self.trace = trace;
        self.assembler.keep_tags(trace);
    }

    /// Writes a telnet no-operation, to keep an idle connection alive.
    pub fn keep_alive(&mut self, out: &mut Vec<u8>) {
        let start = out.len();
        out.extend_from_slice(&[telnet::IAC, telnet::NOP]);
        self.finish_outgoing(out, start);
    }

    /// Encodes an MSDP message such as `REPORT` with variable names.
    /// Nothing is written if the server did not enable MSDP.
    pub fn send_msdp(&mut self, variable: &str, values: &[String], out: &mut Vec<u8>) {
        if self.negotiator.msdp_enabled() {
            let start = out.len();
            msdp::write(variable, values, out);
            self.finish_outgoing(out, start);
        }
    }

    /// Sends a line of user input: its wire bytes are appended to `out`.
    /// Whatever the server left unterminated is what the user is answering,
    /// so it is closed as a prompt first and the echo follows it in order.
    pub fn send_line(&mut self, line: &str, out: &mut Vec<u8>, events: &mut Vec<Event>) {
        self.decoder.flush(&mut self.text);
        self.assembler.feed(&self.text, &mut self.lines);
        self.text.clear();
        events.extend(self.lines.drain(..).map(Event::Line));
        if let Some(prompt) = self.assembler.take_partial() {
            events.push(Event::Prompt(prompt));
        }
        if !self.input_hidden {
            events.push(Event::Sent(line.to_owned()));
        }
        let start = out.len();
        telnet::escape_iac(&self.decoder.encode(line), out);
        out.extend_from_slice(b"\r\n");
        self.finish_outgoing(out, start);
    }

    /// Reports a new window size; any telnet message goes to `reply`.
    pub fn set_window(&mut self, width: u16, height: u16, reply: &mut Vec<u8>) {
        let start = reply.len();
        self.negotiator.set_window(width, height, reply);
        self.finish_outgoing(reply, start);
    }
}

#[cfg(test)]
mod tests;

/// What `bytes` the client wrote carry besides text, as it is told.
fn wire(bytes: &[u8], outgoing: bool) -> Vec<Event> {
    let mut parsed = Vec::new();
    telnet::Parser::new().feed(bytes, &mut parsed);
    (parsed.into_iter())
        .filter_map(|event| wire_event(event, outgoing))
        .collect()
}

/// One thing of telnet as it is told: an option offered or answered, or
/// what was sent with one, in the words of its protocol.
fn wire_event(event: telnet::Event, outgoing: bool) -> Option<Event> {
    let (protocol, text) = match event {
        telnet::Event::Negotiate(verb, option) => {
            let verb = format!("{verb:?}").to_uppercase();
            ("TELNET".to_owned(), format!("{verb} {}", opt::name(option)))
        }
        telnet::Event::Subneg(option, payload) => {
            let text = match option {
                opt::MSDP => (msdp::parse(&payload).into_iter())
                    .map(|(variable, data)| format!("{variable} {data}"))
                    .collect::<Vec<_>>()
                    .join("; "),
                _ => String::from_utf8_lossy(&payload).into_owned(),
            };
            (opt::name(option), text)
        }
        telnet::Event::Data(_) | telnet::Event::Command(_) => return None,
    };
    Some(Event::Wire {
        outgoing,
        protocol,
        text,
    })
}
