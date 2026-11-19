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
    /// The connection ended; the string is the error, if it was one.
    Disconnected(Option<String>),
}

/// Size of the chunks compressed data is inflated in.
const INFLATE_BUFFER: usize = 16 * 1024;

pub struct Pipeline {
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
        for telnet_event in self.telnet_events.drain(..) {
            match &telnet_event {
                telnet::Event::Data(bytes) => {
                    self.decoder.feed(bytes, &mut self.text);
                    self.assembler.feed(&self.text, &mut self.lines);
                    self.text.clear();
                    events.extend(self.lines.drain(..).map(Event::Line));
                    // Answers to MXP queries go back as lines.
                    for answer in self.assembler.take_replies() {
                        telnet::escape_iac(&self.decoder.encode(&answer), reply);
                        reply.extend_from_slice(b"\r\n");
                    }
                    continue;
                }
                telnet::Event::Subneg(opt::MCCP2, _) => {
                    self.inflater = Some(Decompress::new(true));
                    continue;
                }
                telnet::Event::Subneg(opt::GMCP, payload) => {
                    let message = String::from_utf8_lossy(payload);
                    let (package, data) = message.split_once(' ').unwrap_or((&message, ""));
                    events.push(Event::Gmcp {
                        package: package.to_owned(),
                        data: data.trim().to_owned(),
                    });
                    continue;
                }
                telnet::Event::Subneg(opt::MSDP, payload) => {
                    let variables = msdp::parse(payload).into_iter();
                    events.extend(variables.map(|(variable, data)| Event::Msdp { variable, data }));
                    continue;
                }
                telnet::Event::Subneg(opt::MSSP, payload) => {
                    events.push(Event::Mssp(msdp::parse_mssp(payload)));
                    continue;
                }
                _ => {}
            }
            match self.negotiator.handle(&telnet_event, reply) {
                Some(Signal::Prompt) => {
                    self.decoder.flush(&mut self.text);
                    self.assembler.feed(&self.text, &mut self.lines);
                    self.text.clear();
                    events.extend(self.lines.drain(..).map(Event::Line));
                    if let Some(prompt) = self.assembler.take_partial() {
                        events.push(Event::Prompt(prompt));
                    }
                }
                Some(Signal::ServerEcho(on)) => {
                    self.input_hidden = on;
                    events.push(Event::InputHidden(on));
                }
                Some(Signal::CompressOutgoing) => {
                    self.deflater = Some(Compress::new(Compression::default(), true));
                    self.deflate_from = Some(reply.len());
                }
                Some(Signal::Mxp) => self.assembler.enable_mxp(),
                None => {}
            }
        }
        used
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
mod tests {
    use super::*;
    use crate::telnet::{DO, GA, IAC, WILL, WONT, opt};

    fn receive(pipeline: &mut Pipeline, input: &[u8]) -> (Vec<u8>, Vec<Event>) {
        let (mut reply, mut events) = (Vec::new(), Vec::new());
        pipeline.receive(input, &mut reply, &mut events).unwrap();
        (reply, events)
    }

    fn text(event: &Event) -> String {
        match event {
            Event::Line(line) => format!("line:{}", line.text()),
            Event::Prompt(line) => format!("prompt:{}", line.text()),
            Event::Partial(line) => format!("partial:{}", line.text()),
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn lines_prompt_and_negotiation_in_stream_order() {
        let mut pipeline = Pipeline::new(Config::default());
        let mut input = b"Welcome\r\nPassword: ".to_vec();
        input.extend_from_slice(&[IAC, WILL, opt::ECHO, IAC, GA]);
        input.extend_from_slice(b"next");
        let (reply, events) = receive(&mut pipeline, &input);
        assert_eq!(reply, [IAC, DO, opt::ECHO]);
        let seen: Vec<String> = events.iter().map(text).collect();
        assert_eq!(
            seen,
            [
                "line:Welcome",
                "InputHidden(true)",
                "prompt:Password: ",
                "partial:next"
            ]
        );
    }

    #[test]
    fn partial_grows_until_the_line_completes() {
        let mut pipeline = Pipeline::new(Config::default());
        let mut seen = |input: &[u8]| -> Vec<String> {
            receive(&mut pipeline, input).1.iter().map(text).collect()
        };
        assert_eq!(seen(b"The orc"), ["partial:The orc"]);
        assert_eq!(seen(b" hits"), ["partial:The orc hits"]);
        assert_eq!(seen(b" you.\r\n"), ["line:The orc hits you."]);
    }

    #[test]
    fn gmcp_messages_become_events_and_can_be_sent() {
        let mut pipeline = Pipeline::new(Config::default());
        let mut out = Vec::new();
        pipeline.send_gmcp("Core.Ping", "", &mut out);
        assert_eq!(out, [], "nothing is sent before GMCP is enabled");

        let mut input = vec![IAC, WILL, opt::GMCP];
        telnet::write_subneg(opt::GMCP, br#"Char.Vitals {"hp": 10}"#, &mut input);
        telnet::write_subneg(opt::GMCP, b"Core.Goodbye", &mut input);
        let (_, events) = receive(&mut pipeline, &input);
        let gmcp = |package: &str, data: &str| Event::Gmcp {
            package: package.into(),
            data: data.into(),
        };
        assert_eq!(
            events,
            [
                gmcp("Char.Vitals", r#"{"hp": 10}"#),
                gmcp("Core.Goodbye", "")
            ]
        );

        pipeline.send_gmcp("Core.Supports.Add", r#"["Room 1"]"#, &mut out);
        let mut expected = Vec::new();
        telnet::write_subneg(opt::GMCP, br#"Core.Supports.Add ["Room 1"]"#, &mut expected);
        assert_eq!(out, expected);
    }

    /// A server stream: plain text, the MCCP2 start marker, then `body`
    /// compressed, then plain text again after the compressed stream ends.
    fn mccp2_stream(body: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut stream = b"plain\r\n".to_vec();
        stream.extend_from_slice(&[IAC, crate::telnet::SB, opt::MCCP2, IAC, crate::telnet::SE]);
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(body).unwrap();
        stream.extend_from_slice(&encoder.finish().unwrap());
        stream.extend_from_slice(b"after\r\n");
        stream
    }

    #[test]
    fn mccp2_stream_is_inflated_wherever_reads_split_it() {
        let mut body = b"zipped \x1b[31mred\x1b[0m\r\n".to_vec();
        body.extend_from_slice(&[IAC, GA]);
        let stream = mccp2_stream(&body);
        let expected = ["line:plain", "line:zipped red", "line:after"];
        for cut in 0..=stream.len() {
            let mut pipeline = Pipeline::new(Config::default());
            let mut seen = Vec::new();
            for chunk in [&stream[..cut], &stream[cut..]] {
                seen.extend(receive(&mut pipeline, chunk).1.iter().map(text));
            }
            seen.retain(|event| !event.starts_with("partial:"));
            assert_eq!(seen, expected, "cut at {cut}");
        }
    }

    #[test]
    fn mccp2_output_larger_than_the_buffer_is_not_lost() {
        let body: Vec<u8> = (0..5000).flat_map(|_| *b"0123456789\r\n").collect();
        let mut pipeline = Pipeline::new(Config::default());
        let (_, events) = receive(&mut pipeline, &mccp2_stream(&body));
        assert_eq!(events.len(), 5002);
        assert_eq!(text(&events[5000]), "line:0123456789");
        assert_eq!(text(&events[5001]), "line:after");
    }

    #[test]
    fn corrupt_compressed_data_is_an_error() {
        let mut pipeline = Pipeline::new(Config::default());
        let mut stream = vec![IAC, crate::telnet::SB, opt::MCCP2, IAC, crate::telnet::SE];
        stream.extend_from_slice(b"this is not zlib data");
        let (mut reply, mut events) = (Vec::new(), Vec::new());
        assert!(pipeline.receive(&stream, &mut reply, &mut events).is_err());
    }

    #[test]
    fn mccp3_compresses_everything_after_its_announcement() {
        let mut pipeline = Pipeline::new(Config::default());
        // Accepting MCCP3 and NAWS in one read: the answers up to and
        // including the MCCP3 start marker are plain, the rest is not.
        let (reply, _) = receive(&mut pipeline, &[IAC, WILL, opt::MCCP3, IAC, DO, opt::NAWS]);
        let plain = [
            IAC,
            DO,
            opt::MCCP3,
            IAC,
            crate::telnet::SB,
            opt::MCCP3,
            IAC,
            crate::telnet::SE,
        ];
        assert_eq!(reply[..plain.len()], plain);
        let mut compressed = reply[plain.len()..].to_vec();
        let (mut events, mut out) = (Vec::new(), Vec::new());
        pipeline.send_line("look", &mut out, &mut events);
        compressed.extend_from_slice(&out);
        out.clear();
        pipeline.keep_alive(&mut out);
        compressed.extend_from_slice(&out);

        let mut inflater = Decompress::new(true);
        let mut sent = vec![0; 256];
        inflater
            .decompress(&compressed, &mut sent, FlushDecompress::Sync)
            .unwrap();
        sent.truncate(inflater.total_out() as usize);
        let mut expected = vec![IAC, WILL, opt::NAWS];
        telnet::write_subneg(opt::NAWS, &[0, 80, 0, 24], &mut expected);
        expected.extend_from_slice(b"look\r\n");
        expected.extend_from_slice(&[IAC, crate::telnet::NOP]);
        assert_eq!(sent, expected);
    }

    #[test]
    fn chosen_encoding_is_used_both_ways() {
        let mut pipeline = Pipeline::new(Config {
            encoding: "koi8-r".into(),
            ..Config::default()
        });
        let (_, events) = receive(&mut pipeline, b"\xf0\xd2\xc9\xd7\xc5\xd4\r\n");
        assert_eq!(events.iter().map(text).collect::<Vec<_>>(), ["line:Привет"]);
        let (mut events, mut out) = (Vec::new(), Vec::new());
        pipeline.send_line("да", &mut out, &mut events);
        assert_eq!(out, b"\xc4\xc1\r\n");
    }

    #[test]
    fn mxp_is_interpreted_once_negotiated_and_queries_answered() {
        let mut pipeline = Pipeline::new(Config::default());
        let (_, events) = receive(&mut pipeline, b"<b>raw</b>\r\n");
        assert_eq!(
            events.iter().map(text).collect::<Vec<_>>(),
            ["line:<b>raw</b>"]
        );
        let mut input = vec![IAC, WILL, opt::MXP];
        input.extend_from_slice(b"\x1b[1z<version><send>look</send>\r\n");
        let (reply, events) = receive(&mut pipeline, &input);
        assert_eq!(reply[..3], [IAC, DO, opt::MXP]);
        assert!(reply[3..].starts_with(b"\x1b[1z<VERSION MXP="));
        assert!(reply.ends_with(b"\r\n"));
        let Event::Line(line) = &events[0] else {
            panic!("expected a line");
        };
        assert_eq!((line.text().as_str(), line.links.len()), ("look", 1));
    }

    #[test]
    fn unsupported_options_are_refused() {
        let mut pipeline = Pipeline::new(Config::default());
        const NEW_ENVIRON: u8 = 39;
        let (reply, _) = receive(&mut pipeline, &[IAC, WILL, NEW_ENVIRON, IAC, DO, opt::GMCP]);
        assert_eq!(
            reply,
            [IAC, crate::telnet::DONT, NEW_ENVIRON, IAC, WONT, opt::GMCP]
        );
    }

    #[test]
    fn msdp_and_mssp_become_events() {
        let mut pipeline = Pipeline::new(Config::default());
        let mut input = vec![IAC, WILL, opt::MSDP];
        telnet::write_subneg(
            opt::MSDP,
            b"\x01HEALTH\x0280\x01HEALTH_MAX\x02100",
            &mut input,
        );
        telnet::write_subneg(opt::MSSP, b"\x01NAME\x02FakeMUD", &mut input);
        let (_, events) = receive(&mut pipeline, &input);
        let msdp = |variable: &str, data: &str| Event::Msdp {
            variable: variable.into(),
            data: data.into(),
        };
        assert_eq!(
            events,
            [
                msdp("HEALTH", r#""80""#),
                msdp("HEALTH_MAX", r#""100""#),
                Event::Mssp(vec![("NAME".into(), "FakeMUD".into())]),
            ]
        );
        let mut out = Vec::new();
        pipeline.send_msdp("LIST", &["COMMANDS".into()], &mut out);
        let mut expected = Vec::new();
        telnet::write_subneg(opt::MSDP, b"\x01LIST\x02COMMANDS", &mut expected);
        assert_eq!(out, expected);
    }

    #[test]
    fn sending_closes_the_partial_and_echoes_unless_hidden() {
        let mut pipeline = Pipeline::new(Config::default());
        receive(&mut pipeline, b"Name: ");
        let (mut out, mut events) = (Vec::new(), Vec::new());
        pipeline.send_line("say \u{ff}", &mut out, &mut events);
        assert_eq!(out, b"say \xc3\xbf\r\n");
        let seen: Vec<String> = events.iter().map(text).collect();
        assert_eq!(seen, ["prompt:Name: ", "Sent(\"say \u{ff}\")"]);

        receive(&mut pipeline, &[IAC, WILL, opt::ECHO]);
        events.clear();
        pipeline.send_line("secret", &mut out, &mut events);
        assert_eq!(events, []);
    }
}
